use dotenvy_macro::dotenv;
use hmac::{Hmac, Mac};
use ruuvi_common::RuuviRawV2;
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type HmacSha256 = Hmac<Sha256>;

const AUTH_KEY: &str = dotenv!("AUTH_KEY");

// Handshake layout (all big endian where numeric):
// MAGIC(4)="RGW1" | VER(1)=0x01 | FLAGS(1) | DEVICE_ID(6) | NONCE(8) | HMAC(32)
// HMAC = HMAC-SHA256 over first 20 bytes (MAGIC..NONCE)
async fn handle_conn(
    mut sock: tokio::net::TcpStream,
    auth_key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let peer = sock.peer_addr()?;
    let mut hs = [0u8; 52];
    if let Err(e) = sock.read_exact(&mut hs).await {
        tracing::warn!("{} early close ({e})", peer);
        return Ok(());
    }
    if &hs[0..4] != b"RGW1" {
        sock.write_all(&[0xFF]).await?;
        return Ok(());
    }
    if hs[4] != 1 {
        sock.write_all(&[0xFE]).await?;
        return Ok(());
    }
    let flags = hs[5];
    let device_id = &hs[6..12];
    let nonce = &hs[12..20];
    let sent_mac = &hs[20..52];

    let mut mac = HmacSha256::new_from_slice(auth_key.as_bytes())?;
    mac.update(&hs[0..20]);
    if mac.verify_slice(sent_mac).is_err() {
        sock.write_all(&[0xFD]).await?;
        tracing::warn!("{} bad HMAC", peer);
        return Ok(());
    }

    // Accept
    sock.write_all(&[0x01]).await?;
    tracing::info!(
        "Device {:02X?} connected flags=0x{:02X} nonce={:02X?} from {}",
        device_id,
        flags,
        nonce,
        peer
    );

    // Frame loop: [LEN(4)][TYPE(1)][PAYLOAD..]
    let mut len_buf = [0u8; 4];
    loop {
        // EOF
        if (sock.read_exact(&mut len_buf).await).is_err() {
            tracing::info!("{} EOF while reading length", peer);
            break;
        }
        let frame_len = u32::from_be_bytes(len_buf);
        tracing::debug!(
            "Device {:02X?} raw_len_bytes={:02X?} parsed_len={}",
            device_id,
            len_buf,
            frame_len
        );
        if frame_len == 0 || frame_len > 64 * 1024 {
            tracing::warn!(
                "{} invalid frame_len={} raw={:02X?} (closing)",
                peer,
                frame_len,
                len_buf
            );
            break;
        }
        let mut frame = vec![0u8; frame_len as usize];
        if let Err(e) = sock.read_exact(&mut frame).await {
            tracing::warn!("{} read frame err {e}", peer);
            break;
        }
        let ftype = frame[0];
        match ftype {
            0x01 => {
                let payload = &frame[1..];
                // Expect postcard-serialized Option<RuuviRawV2>
                match postcard::from_bytes::<RuuviRawV2>(payload) {
                    Ok(ruuvi_data) => {
                        tracing::info!("Device {device_id:02X?} data: {ruuvi_data:?}");
                        // ACK (0x03,0x01)
                        sock.write_all(&[0x03, 0x01]).await?;
                    }
                    Err(e) => {
                        tracing::warn!("Decode error: {e}");
                        sock.write_all(&[0x10, 0x00]).await?;
                    }
                }
            }
            0x02 => {
                // Ping
                sock.write_all(&[0x03, 0x02]).await?;
            }
            _ => {
                sock.write_all(&[0x10, 0x01]).await?; // unknown type
            }
        }
    }

    tracing::info!("Device {:02X?} disconnected {}", device_id, peer);
    Ok(())
}

async fn tcp_server() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9090").await?;
    tracing::info!("TCP ingestion listening on :9090");
    let auth = String::from(AUTH_KEY);
    loop {
        let (sock, addr) = listener.accept().await?;
        let key = auth.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(sock, &key).await {
                tracing::error!("Conn {addr} error: {e}");
            }
        });
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    tcp_server().await
}

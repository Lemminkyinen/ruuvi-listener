use crate::config::AUTH_KEY;
use crate::schema::RuuviRawV2;
use core::net::Ipv4Addr;
use core::sync::atomic::{AtomicU32, Ordering};
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Receiver;
use embassy_time::{Duration, Timer};
use embedded_io_async::{Read, Write};
use hmac::{Hmac, Mac};
use sha2::Sha256;

// --- Protocol constants ---
const MAGIC: &[u8; 4] = b"RGW1";
const VERSION: u8 = 1;
// Flags you may later use (bit0 could mean compression, etc.)
const FLAGS: u8 = 0x00;
// Backoff and timeout settings
const CONNECT_TIMEOUT_SECS: u64 = 10;
const IO_TIMEOUT_SECS: u64 = 10;
const MAX_BACKOFF_SECS: u64 = 30;
const BASE_BACKOFF_MS: u64 = 500;
// Server address (TODO: make configurable)
const SERVER_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 100);
const SERVER_PORT: u16 = 9090;

// Device identity (6 bytes). For now static; replace with real MAC if available.
const DEVICE_ID: [u8; 6] = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];

// We don't have 64-bit atomics on this target. Compose an 8-byte nonce from two 32-bit counters.
static NONCE_LOW: AtomicU32 = AtomicU32::new(1);
const NONCE_HIGH: u32 = 0xA5A5_0001; // fixed high word (could randomize at boot)

// Serialize Option<RuuviRawV2> with postcard into a temporary vec.
fn serialize_packet(pkt: RuuviRawV2, buf: &mut alloc::vec::Vec<u8>) -> Result<(), ()> {
    buf.clear();
    // Serialize Option<RuuviRawV2>
    postcard::to_allocvec(&Some(pkt)).map_err(|_| ()).map(|v| {
        buf.extend_from_slice(&v);
    })
}

// Build handshake (52 bytes) in provided buffer.
fn build_handshake(buf: &mut [u8; 52]) {
    buf.fill(0);
    // Layout: MAGIC(0..4) | VER(4) | FLAGS(5) | DEV(6..12) | NONCE(12..20) | HMAC(20..52)
    buf[0..4].copy_from_slice(MAGIC);
    buf[4] = VERSION;
    buf[5] = FLAGS;
    buf[6..12].copy_from_slice(&DEVICE_ID);
    let low = NONCE_LOW.fetch_add(1, Ordering::Relaxed);
    let nonce_u64: u64 = ((NONCE_HIGH as u64) << 32) | (low as u64);
    buf[12..20].copy_from_slice(&nonce_u64.to_be_bytes());
    // HMAC over first 20 bytes
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(AUTH_KEY.as_bytes()).unwrap();
    mac.update(&buf[0..20]);
    let tag = mac.finalize().into_bytes();
    buf[20..52].copy_from_slice(&tag);
}

async fn send_handshake<S: Write + Read + Unpin>(sock: &mut S) -> Result<(), &'static str> {
    let mut hs = [0u8; 52];
    build_handshake(&mut hs);
    sock.write_all(&hs).await.map_err(|_| "hs_write")?;
    // Expect single byte accept (0x01) or error codes (0xFF/0xFE/0xFD)
    let mut resp = [0u8; 1];
    sock.read_exact(&mut resp).await.map_err(|_| "hs_read")?;
    match resp[0] {
        0x01 => Ok(()),
        0xFF => Err("bad_magic"),
        0xFE => Err("bad_ver"),
        0xFD => Err("bad_hmac"),
        _ => Err("unknown_hs_code"),
    }
}

async fn send_frame<S: Write + Read + Unpin>(
    sock: &mut S,
    ftype: u8,
    payload: &[u8],
) -> Result<(), &'static str> {
    let total_len = 1usize + payload.len();
    if total_len > 64 * 1024 {
        return Err("oversize");
    }
    let hdr = (total_len as u32).to_be_bytes();
    sock.write_all(&hdr).await.map_err(|_| "len_write")?;
    sock.write_all(&[ftype]).await.map_err(|_| "type_write")?;
    if !payload.is_empty() {
        sock.write_all(payload).await.map_err(|_| "pl_write")?;
    }
    // Expect exactly 2-byte response: [0x03,code] (ack) or [0x10,code] (error)
    let mut ack = [0u8; 2];
    if sock.read_exact(&mut ack).await.is_err() {
        return Err("ack_read");
    }
    match ack[0] {
        0x03 => {
            // success kinds: 0x01 data ack, 0x02 ping ack
            // we don't differentiate further here
            Ok(())
        }
        0x10 => {
            // error marker -> log code and treat as failure (forces reconnect)
            log::warn!("Server error code=0x{:02X}", ack[1]);
            Err("server_error")
        }
        _ => {
            log::warn!("Unexpected ack header byte=0x{:02X}", ack[0]);
            Err("bad_ack")
        }
    }
}

#[embassy_executor::task]
pub async fn run(stack: Stack<'static>, receiver: Receiver<'static, NoopRawMutex, RuuviRawV2, 16>) {
    use alloc::vec;
    use alloc::vec::Vec as AVec;

    let mut rx_buffer = [0; 2048];
    let mut tx_buffer = [0; 2048];
    let mut backoff_ms = BASE_BACKOFF_MS;
    let server = (SERVER_IP, SERVER_PORT);

    // Reusable buffers
    let mut ser_buf: AVec<u8> = vec![0u8; 0];
    let mut last_ping = embassy_time::Instant::now();

    loop {
        log::info!("Connecting (proto) to {SERVER_IP:?}:{SERVER_PORT}...");
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(CONNECT_TIMEOUT_SECS)));
        match socket.connect(server).await {
            Ok(_) => {
                log::info!("TCP connected");
            }
            Err(e) => {
                log::warn!("Connect error: {e:?}; backoff {backoff_ms}ms");
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_SECS * 1000);
                continue;
            }
        }

        socket.set_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS)));
        match send_handshake(&mut socket).await {
            Ok(_) => {
                log::info!("Handshake OK");
                backoff_ms = BASE_BACKOFF_MS; // reset after success
            }
            Err(err) => {
                log::warn!("Handshake failed: {err}; reconnecting");
                // short delay then restart outer loop
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_SECS * 1000);
                continue;
            }
        }

        // Frame sending loop
        loop {
            receiver.ready_to_receive().await;
            let pkt = receiver.receive().await;
            if serialize_packet(pkt, &mut ser_buf).is_err() {
                log::warn!("Serialize failed");
                continue;
            }
            // Periodic ping (every ~60s) to keep connection fresh (optional)
            if last_ping.elapsed().as_secs() >= 60 {
                if let Err(e) = send_frame(&mut socket, 0x02, &[]).await {
                    log::warn!("Ping failed: {e}");
                    break;
                }
                last_ping = embassy_time::Instant::now();
            }

            if let Err(e) = send_frame(&mut socket, 0x01, &ser_buf).await {
                log::warn!("Frame send error: {e}");
                break; // break frame loop -> reconnect
            }
        }

        log::info!("Reconnecting after backoff {backoff_ms}ms");
        Timer::after(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_SECS * 1000);
    }
}

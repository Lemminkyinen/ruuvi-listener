use dotenvy_macro::dotenv;
use serde::Deserialize;
use snow::Builder;
use snow::params::NoiseParams;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const AUTH_KEY: &str = dotenv!("AUTH_KEY");
static PARAMS: LazyLock<NoiseParams> =
    LazyLock::new(|| "Noise_XXpsk3_25519_ChaChaPoly_SHA256".parse().unwrap());

// Validate auth key length is 32 bytes
const PSK_KEY: [u8; 32] = {
    if AUTH_KEY.len() != 32 {
        panic!("AUTH_KEY must be exactly 32 bytes");
    }
    const_str::to_byte_array!(AUTH_KEY)
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RuuviRawV2 {
    pub format: u8,           // 0
    pub temp: i16,            // 1-2
    pub humidity: u16,        // 3-4
    pub pressure: u16,        // 5-6
    pub acc_x: i16,           // 7-8
    pub acc_y: i16,           // 9-10
    pub acc_z: i16,           // 11-12
    pub power_info: u16,      // 13-14
    pub movement_counter: u8, // 15
    pub measurement_seq: u16, // 16-17
    pub mac: [u8; 6],         // 18-23
}

// async fn recv(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
//     stream.readable().await?;
//     let mut msg_len_buf = [0_u8; 2];
//     // stream.read_exact(&mut msg_len_buf).await?;

//     let read_len = stream.try_read(&mut msg_len_buf)?;
//     tracing::info!("try_read read {read_len} bytes");

//     let msg_len = usize::from(u16::from_be_bytes(msg_len_buf));
//     tracing::info!("Reading 2 + {msg_len} bytes: {}", 2 + msg_len);
//     let mut msg = vec![0_u8; msg_len];
//     let read_len = stream.try_read(&mut msg[..])?;
//     tracing::info!("try_read read {read_len} bytes");
//     Ok(msg)
// }

async fn recv(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    // Wait until data is readable
    stream.readable().await?;

    let mut msg_len_buf = [0_u8; 2];

    // Read length - should be available now after readable()
    let mut read_len = 0;
    while read_len < 2 {
        match stream.try_read(&mut msg_len_buf) {
            Ok(n) => {
                read_len += n;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Connection closed",
                    ));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // This shouldn't happen after readable(), but handle it
                tokio::time::sleep(Duration::from_millis(500)).await;
                tracing::error!("Reading packet len: {e}");
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    let msg_len = usize::from(u16::from_be_bytes(msg_len_buf));
    tracing::info!("Reading 2 + {msg_len} bytes: {}", 2 + msg_len);

    let mut msg = vec![0_u8; msg_len];
    let mut total_read = 0;

    while total_read < msg_len {
        // Wait for more data if needed
        stream.readable().await?;

        match stream.try_read(&mut msg[total_read..]) {
            Ok(n) => {
                total_read += n;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Connection closed",
                    ));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // This shouldn't happen after readable(), but handle it
                tokio::time::sleep(Duration::from_millis(500)).await;
                tracing::error!("Reading packet: {e}");
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    tracing::info!("Successfully received complete message");
    Ok(msg)
}

async fn send(stream: &mut TcpStream, buf: &[u8]) {
    stream.writable().await.unwrap();
    let len = u16::try_from(buf.len()).expect("Too large message");
    stream.write_all(&len.to_be_bytes()).await.unwrap();
    stream.write_all(buf).await.unwrap();
    tracing::info!("Send exact {len} bytes");
    stream.flush().await.unwrap();
}

static READ_RUUVI_PACKETS: AtomicU64 = AtomicU64::new(0);

async fn handle_conn(mut stream: tokio::net::TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_ttl(10)?;

    let mut rx_buf = [0u8; 4096];
    let mut noise_buf = [0u8; 4096];

    // Initialize our responder using a builder.
    let builder = Builder::new(PARAMS.clone());
    let static_key = builder.generate_keypair().unwrap().private;
    let mut noise = builder
        .local_private_key(&static_key)
        .unwrap()
        .psk(3, &PSK_KEY)
        .unwrap()
        .build_responder()
        .unwrap();
    tracing::info!("Noise handshake started");

    // <- e
    let payload = &recv(&mut stream).await?;
    let _msg_len = noise.read_message(payload, &mut noise_buf).unwrap();

    // -> e, ee, s, es
    let len = noise.write_message(&[], &mut noise_buf).unwrap();
    send(&mut stream, &noise_buf[..len]).await;

    // <- s, se
    let payload = &recv(&mut stream).await?;
    let _len = noise.read_message(payload, &mut noise_buf).unwrap();

    // Transition the state machine into transport mode now that the handshake is complete.
    let mut transport = noise.into_transport_mode().unwrap();

    tracing::info!("In transport mode");

    loop {
        // stream.readable().await?;

        // // Try to read data, this may still fail with `WouldBlock`
        // // if the readiness event is a false positive.
        // match stream.try_read(&mut buf) {
        //     Ok(0) => break,
        //     Ok(n) => {
        //         println!("read {} bytes", n);
        //     }
        //     Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
        //         continue;
        //     }
        //     Err(e) => {
        //         return Err(e.into());
        //     }
        // }

        match recv(&mut stream).await {
            Ok(msg) => {
                // Decrypt message
                let len = transport.read_message(&msg, &mut noise_buf).unwrap();

                // Postcard deserialize
                let data = postcard::from_bytes::<RuuviRawV2>(&noise_buf[..len]);

                match data {
                    Ok(ruuvidata) => {
                        READ_RUUVI_PACKETS.fetch_add(1, Ordering::Relaxed);
                        // tracing::info!("Data: {ruuvidata:?}");
                        tracing::info!(
                            "Successfully received {} times!",
                            READ_RUUVI_PACKETS.load(Ordering::Relaxed)
                        );
                    }
                    Err(err) => tracing::error!("Failed to parse ruuvidata: {err}"),
                }
            }
            Err(e) => {
                // Handle different error types appropriately
                match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => {
                        tracing::warn!("Client disconnected gracefully (EOF)");
                        break;
                    }
                    std::io::ErrorKind::ConnectionReset => {
                        tracing::warn!("Client connection reset");
                        break;
                    }
                    std::io::ErrorKind::BrokenPipe => {
                        tracing::warn!("Client broken pipe");
                        break;
                    }
                    e => {
                        tracing::error!("Network error: {e}, closing connection");
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn tcp_server() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9090").await?;
    tracing::info!("TCP ingestion listening on :9090");
    loop {
        let (sock, addr) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_conn(sock).await {
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

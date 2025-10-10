use dotenvy_macro::dotenv;
use serde::Deserialize;
use snow::Builder;
use snow::params::NoiseParams;
use std::sync::LazyLock;
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
#[derive(Debug, Deserialize)]
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

#[derive(Debug)]
pub struct RuuviV2 {
    pub mac: [u8; 6],
    pub temp: f32,
    pub rel_humidity: f32,
    pub abs_pressure: u32,
    pub acc_x: i16,
    pub acc_y: i16,
    pub acc_z: i16,
    pub battery_voltage: f32,
    pub tx_power: i8,
    pub movement_counter: u8,
    pub measurement_seq: u16,
}

impl From<RuuviRawV2> for RuuviV2 {
    fn from(raw: RuuviRawV2) -> Self {
        // https://docs.ruuvi.com/communication/bluetooth-advertisements/data-format-5-rawv2
        // Temperature in 0.005 degrees
        let temp = raw.temp as f32 * 0.005;
        // Humidity in 0.0025%. 0-163.83% range, though realistically 0-100%
        let rel_humidity = f32::min(raw.humidity as f32 * 0.0025, 100f32);
        // Pressure offset -50 000 Pa
        let abs_pressure = raw.pressure as u32 + 50_000;
        // First 11 bits are for battery voltage. From 1.6V to 3.646V
        let battery_voltage = (1600 + (raw.power_info >> 5)) as f32 / 1000f32;
        // Last 5 bits are for TX power. -40dBm - +20dBm
        let tx_power = (raw.power_info & 0b11111) as i8 * 2 - 40;

        Self {
            mac: raw.mac,
            temp,
            rel_humidity,
            abs_pressure,
            acc_x: raw.acc_x,
            acc_y: raw.acc_y,
            acc_z: raw.acc_z,
            battery_voltage,
            tx_power,
            movement_counter: raw.movement_counter,
            measurement_seq: raw.measurement_seq,
        }
    }
}

async fn recv(stream: &mut TcpStream, rx_buffer: &mut [u8]) -> io::Result<usize> {
    let mut msg_len_buf = [0_u8; 2];
    stream.read_exact(&mut msg_len_buf).await?;
    let msg_len = usize::from(u16::from_be_bytes(msg_len_buf));
    stream.read_exact(&mut rx_buffer[..msg_len]).await
}

async fn send(stream: &mut TcpStream, buf: &[u8]) -> io::Result<()> {
    let len = u16::try_from(buf.len()).expect("Too large message");
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(buf).await?;
    stream.flush().await
}

async fn handle_conn(mut stream: tokio::net::TcpStream) -> Result<(), anyhow::Error> {
    stream.set_ttl(30)?;

    let mut rx_buffer = [0u8; 4096];
    let mut noise_buf = [0u8; 4096];

    // Initialize our responder using a builder.
    let builder = Builder::new(PARAMS.clone());
    let static_key = builder.generate_keypair()?.private;
    let mut noise = builder
        .local_private_key(&static_key)?
        .psk(3, &PSK_KEY)?
        .build_responder()?;

    tracing::info!("Noise handshake started with {:?}", stream.peer_addr());

    // <- e
    let read_len = recv(&mut stream, &mut rx_buffer).await?;
    noise.read_message(&rx_buffer[..read_len], &mut noise_buf)?;

    // -> e, ee, s, es
    let len = noise.write_message(&[], &mut noise_buf)?;
    send(&mut stream, &noise_buf[..len]).await?;

    // <- s, se
    let read_len = recv(&mut stream, &mut rx_buffer).await?;
    noise.read_message(&rx_buffer[..read_len], &mut noise_buf)?;

    // Transition the state machine into transport mode now that the handshake is complete.
    let mut transport = noise.into_transport_mode()?;
    tracing::info!("In transport mode");

    loop {
        match recv(&mut stream, &mut rx_buffer).await {
            Ok(len) => {
                // Decrypt message
                let len = transport.read_message(&rx_buffer[..len], &mut noise_buf)?;

                // Postcard deserialize
                let data = postcard::from_bytes::<RuuviRawV2>(&noise_buf[..len]);

                match data {
                    Ok(raw) => {
                        let ruuvi_data = RuuviV2::from(raw);
                        tracing::info!("Data: {ruuvi_data:?}");
                    }
                    Err(err) => tracing::error!("Failed to parse ruuvidata: {err}"),
                }
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
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

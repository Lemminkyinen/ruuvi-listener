mod database;

use crate::database::insert_data;
use chrono::{DateTime, Utc};
use dotenvy_macro::dotenv;
use serde::Deserialize;
use snow::Builder;
use snow::params::NoiseParams;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::sync::LazyLock;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const AUTH_KEY: &str = dotenv!("AUTH_KEY");
const DATABASE_URI: &str = dotenv!("DATABASE_URI");

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
    pub format: u8,             // 0
    pub temp: i16,              // 1-2
    pub humidity: u16,          // 3-4
    pub pressure: u16,          // 5-6
    pub acc_x: i16,             // 7-8
    pub acc_y: i16,             // 9-10
    pub acc_z: i16,             // 11-12
    pub power_info: u16,        // 13-14
    pub movement_counter: u8,   // 15
    pub measurement_seq: u16,   // 16-17
    pub mac: [u8; 6],           // 18-23
    pub timestamp: Option<u64>, // Added field
}

#[derive(Debug)]
pub struct RuuviV2 {
    pub mac: [u8; 6],
    pub temp: f32,
    pub dew_point_temp: f64,
    pub rel_humidity: f32,
    pub abs_humidity: f64,
    pub abs_pressure: u32,
    pub acc_x: i16,
    pub acc_y: i16,
    pub acc_z: i16,
    pub battery_voltage: f32,
    pub tx_power: i8,
    pub movement_counter: u8,
    pub measurement_seq: u16,
    pub timestamp: DateTime<Utc>,
}

impl RuuviV2 {
    fn calculate_abs_humidity(temp: f32, rel_humidity: f32) -> f64 {
        // https://en.wikipedia.org/wiki/Arden_Buck_equation
        // TODO use enhancement factor

        // Saturation vapor pressure in hPa
        let ps_hpa = 6.1121f64
            * ((18.678f64 - (temp as f64 / 234.5)) * (temp as f64 / (257.14 + temp as f64))).exp();
        // In Pa
        let ps = ps_hpa * 100.0;
        // Actual vapor pressure
        let pa = ps * (rel_humidity as f64 / 100.0);
        // Absolute humidity in g/m^3
        2.167 * pa / (temp as f64 + 273.15)
    }

    fn calculate_dew_pont(temp: f32, rel_humidity: f32) -> f64 {
        // https://en.wikipedia.org/wiki/Tetens_equation
        // https://en.wikipedia.org/wiki/Clausius%E2%80%93Clapeyron_relation#August%E2%80%93Roche%E2%80%93Magnus_approximation
        let a = 17.625f64;
        let b = 243.04f64;
        let gamma = (rel_humidity as f64 / 100.0).ln() + (a * temp as f64) / (b + temp as f64);
        (b * gamma) / (a - gamma)
    }

    fn from_raw(raw: RuuviRawV2, fallback_dt: DateTime<Utc>) -> Self {
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
        // Abs humidity
        let abs_humidity = Self::calculate_abs_humidity(temp, rel_humidity);
        // Dew point temp
        let dew_point_temp = Self::calculate_dew_pont(temp, rel_humidity);

        let timestamp = DateTime::from_timestamp_millis(raw.timestamp.unwrap_or(0) as i64)
            .unwrap_or_else(|| {
                tracing::warn!("Failed to parse timestamp");
                fallback_dt
            });

        Self {
            mac: raw.mac,
            temp,
            dew_point_temp,
            rel_humidity,
            abs_humidity,
            abs_pressure,
            acc_x: raw.acc_x,
            acc_y: raw.acc_y,
            acc_z: raw.acc_z,
            battery_voltage,
            tx_power,
            movement_counter: raw.movement_counter,
            measurement_seq: raw.measurement_seq,
            timestamp,
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

async fn handle_conn(
    mut stream: tokio::net::TcpStream,
    pool: Pool<Postgres>,
) -> Result<(), anyhow::Error> {
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

    // Measure network latency
    let _ = recv(&mut stream, &mut rx_buffer).await?;
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let len = transport.write_message(&time.to_be_bytes(), &mut noise_buf)?;
    send(&mut stream, &noise_buf[..len]).await?;

    loop {
        match recv(&mut stream, &mut rx_buffer).await {
            Ok(len) => {
                let fallback_dt = Utc::now();
                // Decrypt message
                let len = transport.read_message(&rx_buffer[..len], &mut noise_buf)?;

                tracing::info!("Format: {:X?}", &noise_buf[0]);

                continue;

                // Postcard deserialize
                let data = postcard::from_bytes::<RuuviRawV2>(&noise_buf[..len]);

                match data {
                    Ok(raw) => {
                        let ruuvi_data = RuuviV2::from_raw(raw, fallback_dt);
                        tracing::debug!("Data: {ruuvi_data:?}");
                        if let Err(e) = insert_data(&pool, ruuvi_data).await {
                            tracing::error!("Failed insert data: {e}");
                        }
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

async fn tcp_server(pool: sqlx::Pool<sqlx::Postgres>) -> Result<(), anyhow::Error> {
    let listener: TcpListener = TcpListener::bind("0.0.0.0:9090").await?;
    tracing::info!("TCP ingestion listening on :9090");
    loop {
        let (sock, addr) = listener.accept().await?;
        let pool = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(sock, pool).await {
                tracing::error!("Conn {addr} error: {e}");
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    tracing::info!("Connecting to the database...");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(DATABASE_URI)
        .await?;
    tracing::info!("Database connection created!");

    tcp_server(pool).await
}

#[cfg(test)]

mod tests {
    use super::RuuviV2;

    #[test]
    fn test_abs_humidity() {
        let res = RuuviV2::calculate_abs_humidity(22.2f32, 52.4125f32);
        assert_eq!(res, 10.29308183848681);
    }

    fn test_dew_point() {
        let res = RuuviV2::calculate_dew_pont(22.22f32, 52.234f32);
        assert_eq!(res, 12.0);
    }
}

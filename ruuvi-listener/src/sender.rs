use crate::config::GatewayConfig;
use crate::schema::RuuviRawV2;
use alloc::boxed::Box;
use anyhow::anyhow;
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Receiver;
use embassy_time::{Duration, Timer};
use embedded_io_async::{Read, Write};
use esp_hal::rng::Rng;
use snow::params::{CipherChoice, DHChoice, HashChoice};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::types::Dh;
use snow::{
    Builder,
    types::{Cipher, Hash, Random},
};
use snow::{HandshakeState, TransportState};

const PARAMS: &str = "Noise_XXpsk3_25519_ChaChaPoly_SHA256";
const BASE_BACKOFF_MS: u64 = 500;
const TIMEOUT_SECS: u64 = 20;
const MAX_BACKOFF_SECS: u64 = 30;

macro_rules! try_continue {
    ($expr:expr, $error_msg:literal) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                log::error!("{}: {}", $error_msg, e);
                continue;
            }
        }
    };
    ($expr:expr, $error_msg:literal, $op:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                log::error!("{}: {}", $error_msg, e);
                $op;
            }
        }
    };
}

async fn recv(
    socket: &mut TcpSocket<'_>,
    rx_buffer: &mut [u8; 1024],
) -> Result<usize, anyhow::Error> {
    let mut msg_len_buf = [0u8; 2];
    socket
        .read_exact(&mut msg_len_buf)
        .await
        .map_err(|e| anyhow!("Failed to read msg_len from the socket: {e:?}"))?;
    let msg_len = usize::from(u16::from_be_bytes(msg_len_buf));
    socket
        .read_exact(&mut rx_buffer[..msg_len])
        .await
        .map_err(|e| anyhow!("Failed to read exact {msg_len} bytes: {e:?}"))?;
    Ok(msg_len)
}

async fn send(socket: &mut TcpSocket<'_>, tx_buffer: &[u8]) -> Result<(), anyhow::Error> {
    let msg_len = u16::try_from(tx_buffer.len())?;
    log::info!("Sending 2 + {msg_len} bytes: {}", msg_len + 2);
    socket
        .write_all(&msg_len.to_be_bytes())
        .await
        .map_err(|e| anyhow!("Failed to write msg_len to the socket: {e:?}"))?;
    socket
        .write_all(tx_buffer)
        .await
        .map_err(|e| anyhow!("Failed to write buffer to the socket: {e:?}"))?;
    socket
        .flush()
        .await
        .map_err(|e| anyhow!("Failed to flush the socket: {e:?}"))
}

struct SnowHwRng {
    rng: Rng,
}

impl SnowHwRng {
    fn new(rng: Rng) -> Self {
        Self { rng }
    }
}

// Have to implement Random since no_std doesn't
// support use-getrandom snow feature
impl Random for SnowHwRng {
    fn try_fill_bytes(&mut self, out: &mut [u8]) -> Result<(), snow::Error> {
        for chunk in out.chunks_mut(4) {
            let v = self.rng.random().to_le_bytes();
            let n = chunk.len();
            chunk.copy_from_slice(&v[..n]);
        }
        Ok(())
    }
}

pub struct MyResolver<R: CryptoResolver> {
    inner: R,
    rng: Rng,
}

impl<R: CryptoResolver> MyResolver<R> {
    pub fn new(inner: R, rng: Rng) -> Self {
        Self { inner, rng }
    }
}

// Extend DefaultResolver with esp_hal RNG
impl<R: CryptoResolver> CryptoResolver for MyResolver<R> {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        Some(Box::new(SnowHwRng::new(self.rng)))
    }

    // Forward everything else to the inner default resolver
    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<dyn Dh>> {
        self.inner.resolve_dh(choice)
    }
    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<dyn Hash>> {
        self.inner.resolve_hash(choice)
    }
    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<dyn Cipher>> {
        self.inner.resolve_cipher(choice)
    }
}

async fn noise_handshake(
    socket: &mut TcpSocket<'_>,
    mut noise: HandshakeState,
    tx_buffer: &mut [u8; 1024],
    rx_buffer: &mut [u8; 1024],
    noise_buffer: &mut [u8; 1024],
) -> Result<TransportState, anyhow::Error> {
    // https://noiseprotocol.org/noise.html
    // -> e
    let len = noise
        .write_message(&[], tx_buffer)
        .map_err(|e| anyhow!("Failed to write e message: {e}"))?;

    send(socket, &tx_buffer[..len]).await?;

    // <- e, ee, s, es
    let len = recv(socket, noise_buffer).await?;
    noise
        .read_message(&noise_buffer[..len], rx_buffer)
        .map_err(|e| anyhow!("Failed to read e, ee, s, es messages: {e}"))?;

    // -> s, se
    let len = noise
        .write_message(&[], tx_buffer)
        .map_err(|e| anyhow!("Failed to write s, se messages: {e}"))?;
    send(socket, &tx_buffer[..len]).await?;

    // Into transport state
    noise
        .into_transport_mode()
        .map_err(|e| anyhow!("Failed to convert into transport mode: {e:?}"))
}

#[embassy_executor::task]
pub async fn run(
    stack: Stack<'static>,
    receiver: Receiver<'static, NoopRawMutex, RuuviRawV2, 16>,
    gateway_config: GatewayConfig,
    rng: Rng,
) {
    // Buffers
    let mut socket_rx_buffer = [0u8; 2048];
    let mut socket_tx_buffer = [0u8; 2048];
    let mut rx_buffer = [0u8; 1024];
    let mut tx_buffer = [0u8; 1024];
    let mut noise_buf = [0u8; 1024];
    let mut postcard_buf = [0u8; 512];

    let mut backoff_ms = BASE_BACKOFF_MS;
    let server = (gateway_config.ip, gateway_config.port);

    loop {
        // Parse noise params
        let params = try_continue!(PARAMS.parse(), "Failed to parse noise params");

        // Initialize default resolver with esp_hal RNG
        let default_resolver = DefaultResolver;
        let custom_resolver = MyResolver::new(default_resolver, rng);

        // Create builder with custom resolver
        let builder = Builder::with_resolver(params, Box::new(custom_resolver));

        // Generate local static key
        let static_key =
            try_continue!(builder.generate_keypair(), "Failed to generate keypair").private;

        // Build noise handshaker
        let builder = try_continue!(
            builder.local_private_key(&static_key),
            "Failed to add private key"
        );
        let builder = try_continue!(
            builder.psk(3, &gateway_config.auth),
            "Failed to specify PSK"
        );
        let noise = try_continue!(builder.build_initiator(), "Failed to build initiator");

        // Create TCP socket
        let mut socket = TcpSocket::new(stack, &mut socket_rx_buffer, &mut socket_tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(TIMEOUT_SECS)));

        // Connect
        match socket.connect(server).await {
            Ok(_) => log::info!("TCP connected"),
            Err(e) => {
                log::warn!("Connect error: {e:?}; backoff {backoff_ms}ms");
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_SECS * 1000);
                continue;
            }
        }

        // Noise handshake
        let mut tp = match noise_handshake(
            &mut socket,
            noise,
            &mut tx_buffer,
            &mut rx_buffer,
            &mut noise_buf,
        )
        .await
        {
            Ok(transport) => {
                log::info!("Session established with the server");
                transport
            }
            Err(e) => {
                log::warn!("Noise handshake error: {e}");
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_SECS * 1000);
                continue;
            }
        };

        'sending: loop {
            // Receive RuuviRawV2 from the channel
            receiver.ready_to_receive().await;
            let pkt = receiver.receive().await;

            // Serialize it with postcard
            let payload = try_continue!(
                postcard::to_slice(&pkt, &mut postcard_buf),
                "Failed to postcard serialize RuuviRawV2"
            );

            // Encrypt serialized data
            let len = try_continue!(
                tp.write_message(payload, &mut tx_buffer),
                "Failed to noise encrypt the message"
            );

            // Send the encrypted data
            try_continue!(
                send(&mut socket, &tx_buffer[..len]).await,
                "Failed to send the encrypted message",
                break 'sending
            );

            log::info!("Successfully send packet!");
            log::info!(
                "Channel item count: {}",
                receiver.capacity() - receiver.free_capacity()
            );

            // After successful send, reset
            backoff_ms = BASE_BACKOFF_MS;
        }

        log::info!("Reconnecting after backoff {backoff_ms}ms");
        Timer::after(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_SECS * 1000);
    }
}

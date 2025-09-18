use crate::AUTH_KEY;
use crate::schema::RuuviRawV2;
use core::net::Ipv4Addr;
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Receiver;
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;
use heapless::Vec;
use serde_json_core::ser::to_slice;

// Configuration constants
const CONNECT_TIMEOUT_SECS: u64 = 10;
const IO_TIMEOUT_SECS: u64 = 10;
const MAX_BACKOFF_SECS: u64 = 30;
const BASE_BACKOFF_MS: u64 = 500; // initial backoff after failure

// Buffer sizing assumptions:
// JSON: RuuviRawV2 ~ small (< 200 bytes) so 256 is enough.
// HTTP headers + JSON body: enlarged header buffer to handle long AUTH_KEY values.
type JsonBuf = Vec<u8, 256>;
type HttpBuf = Vec<u8, 768>;

fn build_request(
    packet: &RuuviRawV2,
    json: &mut JsonBuf,
    http: &mut HttpBuf,
) -> Result<(), &'static str> {
    json.clear();
    http.clear();

    // Serialize JSON into temporary fixed buffer because serde_json_core::to_slice does not
    // update a heapless::Vec's length when passed as &mut [u8].
    let mut tmp = [0u8; 256];
    let json_len = to_slice(packet, &mut tmp).map_err(|_| "json")?; // returns length written
    if json_len > tmp.len() {
        return Err("json_len");
    }
    if json.extend_from_slice(&tmp[..json_len]).is_err() {
        return Err("json_overflow");
    }

    // Estimate required capacity to fail early (rough)
    let estimated = 128 + AUTH_KEY.len() + json.len();
    if estimated > http.capacity() {
        return Err("http_capacity");
    }

    // Minimal persistent HTTP/1.1 request
    http.extend_from_slice(b"POST /api/ruuvi HTTP/1.1\r\n")
        .map_err(|_| "hdr")?;
    http.extend_from_slice(b"Host: 192.168.1.100:8080\r\n") // match actual server IP
        .map_err(|_| "hdr")?;
    http.extend_from_slice(b"Connection: keep-alive\r\n")
        .map_err(|_| "hdr")?;
    http.extend_from_slice(b"Content-Type: application/json\r\n")
        .map_err(|_| "hdr")?;
    http.extend_from_slice(b"Authorization: ")
        .map_err(|_| "hdr")?;
    http.extend_from_slice(AUTH_KEY.as_bytes())
        .map_err(|_| "auth")?;
    http.extend_from_slice(b"\r\n").map_err(|_| "hdr")?;

    let mut itoa_buf = itoa::Buffer::new();
    let len_str = itoa_buf.format(json.len());
    http.extend_from_slice(b"Content-Length: ")
        .map_err(|_| "hdr")?;
    http.extend_from_slice(len_str.as_bytes())
        .map_err(|_| "hdr")?;
    http.extend_from_slice(b"\r\n\r\n").map_err(|_| "hdr")?;
    http.extend_from_slice(json.as_slice())
        .map_err(|_| "body")?;
    Ok(())
}

// Parse just the status line (e.g. HTTP/1.1 200 OK) from the beginning of the response buffer.
fn parse_status_line(buf: &[u8]) -> Option<u16> {
    // Find end of first line
    let mut end = None;
    for i in 0..buf.len().min(64) {
        // limit search
        if i + 1 < buf.len() && buf[i] == b'\r' && buf[i + 1] == b'\n' {
            end = Some(i);
            break;
        }
    }
    let end = end?;
    let line = &buf[..end];
    // Expect format: HTTP/1.x <code>
    // Split by space
    let mut parts_iter = line.split(|b| *b == b' ');
    let _http = parts_iter.next()?;
    let code = parts_iter.next()?;
    if code.len() == 3 {
        let d0 = (code[0] as char).to_digit(10)? as u16;
        let d1 = (code[1] as char).to_digit(10)? as u16;
        let d2 = (code[2] as char).to_digit(10)? as u16;
        return Some(d0 * 100 + d1 * 10 + d2);
    }
    None
}

#[embassy_executor::task]
pub async fn run(stack: Stack<'static>, receiver: Receiver<'static, NoopRawMutex, RuuviRawV2, 16>) {
    let mut rx_buffer = [0; 2048];
    let mut tx_buffer = [0; 2048];

    let server_ip = Ipv4Addr::new(192, 168, 1, 100); // TODO: make configurable
    let server_port = 8080;
    let socket_address = (server_ip, server_port);

    let mut backoff_ms = BASE_BACKOFF_MS;

    // Reusable buffers
    let mut json_buf: JsonBuf = Vec::new();
    let mut http_buf: HttpBuf = Vec::new();
    let mut resp_buf = [0u8; 256];

    loop {
        // OUTER LOOP: establish (or re-establish) a TCP connection.
        log::info!("Connecting to server...");
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(CONNECT_TIMEOUT_SECS)));
        match socket.connect(socket_address).await {
            Ok(_) => {
                log::info!("Connected");
                backoff_ms = BASE_BACKOFF_MS; // reset backoff after successful connect
            }
            Err(e) => {
                log::warn!("Connect failed: {e:?}, backoff {backoff_ms}ms");
                Timer::after(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_SECS * 1000);
                continue; // retry connect
            }
        }

        // INNER LOOP: reuse the same socket for multiple packets until an IO error occurs.
        loop {
            // Wait for next packet from channel (blocking)
            receiver.ready_to_receive().await;
            let packet = receiver.receive().await;

            if let Err(reason) = build_request(&packet, &mut json_buf, &mut http_buf) {
                log::warn!(
                    "Failed to build HTTP request: {reason} (json_len={}, auth_len={})",
                    json_buf.len(),
                    AUTH_KEY.len()
                );
                continue; // skip this packet but keep connection
            }

            socket.set_timeout(Some(Duration::from_secs(IO_TIMEOUT_SECS)));
            if let Err(e) = socket.write_all(http_buf.as_slice()).await {
                log::warn!("Write failed: {e:?}");
                break; // break inner loop -> drop socket -> reconnect
            }

            match socket.read(&mut resp_buf).await {
                Ok(0) => {
                    log::warn!("Server closed (EOF)");
                    break;
                }
                Ok(n) => {
                    if let Some(code) = parse_status_line(&resp_buf[..n]) {
                        log::info!("HTTP status: {code}");
                    } else {
                        log::info!("Resp {n} bytes");
                    }
                }
                Err(e) => {
                    log::warn!("Read error: {e:?}");
                    break;
                }
            }
        }

        // Connection ended; wait with backoff then reconnect.
        log::info!("Reconnecting after backoff {backoff_ms}ms");
        Timer::after(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_SECS * 1000);
    }
}

use bt_hci::controller::ExternalController;
use core::net::Ipv4Addr;
use dotenvy_macro::dotenv;
use esp_hal::peripherals;
use esp_hal::rng::Rng;
use esp_radio::ble::controller::BleConnector;
use esp_radio::wifi::{Interfaces, WifiController};

pub const SSID: &str = dotenv!("SSID");
pub const PASSWORD: &str = dotenv!("PASSWORD");
pub const GATEWAY_IP: &str = dotenv!("GATEWAY_IP");
pub const GATEWAY_PORT: &str = dotenv!("GATEWAY_PORT");
pub const AUTH_KEY: &str = dotenv!("AUTH_KEY");

// Validate auth key length is 32 bytes
const _: () = {
    if AUTH_KEY.len() != 32 {
        panic!("AUTH_KEY must be exactly 32 bytes");
    }
};

pub struct WifiConfig {
    pub ssid: &'static str,
    pub password: &'static str,
}

impl WifiConfig {
    pub const fn new() -> Self {
        Self {
            ssid: SSID,
            password: PASSWORD,
        }
    }
}

pub struct GatewayConfig {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub auth: [u8; 32],
}

impl GatewayConfig {
    pub const fn new() -> Self {
        let ip = const_str::ip_addr!(v4, GATEWAY_IP);
        let port = const_str::parse!(GATEWAY_PORT, u16);
        let auth_key = const_str::to_byte_array!(AUTH_KEY);
        Self {
            ip,
            port,
            auth: auth_key,
        }
    }
}

pub struct BoardConfig {
    pub rng: Rng,
    pub wifi_controller: Option<WifiController<'static>>,
    pub interfaces: Option<Interfaces<'static>>,
    pub ble_controller: Option<ExternalController<BleConnector<'static>, 20>>,
    pub cpu_ctrl: Option<peripherals::CPU_CTRL<'static>>,
    pub sw_interrupt: Option<peripherals::SW_INTERRUPT<'static>>,
    pub rmt: Option<peripherals::RMT<'static>>,
    pub gpio48: Option<peripherals::GPIO48<'static>>,
}

impl BoardConfig {
    pub fn new(
        rng: Rng,
        wifi_controller: WifiController<'static>,
        interfaces: Interfaces<'static>,
        ble_controller: ExternalController<BleConnector<'static>, 20>,
        cpu_ctrl: peripherals::CPU_CTRL<'static>,
        sw_interrupt: peripherals::SW_INTERRUPT<'static>,
        rmt: peripherals::RMT<'static>,
        gpio48: peripherals::GPIO48<'static>,
    ) -> Self {
        Self {
            rng,
            wifi_controller: Some(wifi_controller),
            interfaces: Some(interfaces),
            ble_controller: Some(ble_controller),
            cpu_ctrl: Some(cpu_ctrl),
            sw_interrupt: Some(sw_interrupt),
            rmt: Some(rmt),
            gpio48: Some(gpio48),
        }
    }
}

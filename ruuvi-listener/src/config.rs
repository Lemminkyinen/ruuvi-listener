use core::{
    net::{AddrParseError, Ipv4Addr},
    num::ParseIntError,
    str::FromStr,
};

use bt_hci::controller::ExternalController;
use dotenvy_macro::dotenv;
use esp_hal::rng::Rng;
use esp_wifi::ble::controller::BleConnector;
use esp_wifi::wifi::{Interfaces, WifiController};

pub const SSID: &str = dotenv!("SSID");
pub const PASSWORD: &str = dotenv!("PASSWORD");
pub const GATEWAY_IP: &str = dotenv!("GATEWAY_IP");
pub const GATEWAY_PORT: &str = dotenv!("GATEWAY_PORT");
pub const AUTH_KEY: &str = dotenv!("AUTH_KEY");

pub struct WifiConfig {
    pub ssid: &'static str,
    pub password: &'static str,
}

impl WifiConfig {
    pub fn new() -> Self {
        Self {
            ssid: SSID,
            password: PASSWORD,
        }
    }
}

pub struct GatewayConfig {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub auth: &'static str,
}

impl GatewayConfig {
    pub fn new() -> Result<Self, ConfigParseError> {
        let ip = Ipv4Addr::from_str(GATEWAY_IP)?;
        let port = u16::from_str(GATEWAY_PORT)?;
        Ok(Self {
            ip,
            port,
            auth: AUTH_KEY,
        })
    }
}

#[derive(Debug)]
pub enum ConfigParseError {
    Addr(AddrParseError),
    Port(ParseIntError),
}

impl From<AddrParseError> for ConfigParseError {
    fn from(e: AddrParseError) -> Self {
        Self::Addr(e)
    }
}

impl From<ParseIntError> for ConfigParseError {
    fn from(e: ParseIntError) -> Self {
        Self::Port(e)
    }
}

pub struct BoardConfig {
    pub rng: Rng,
    pub wifi_controller: WifiController<'static>,
    pub interfaces: Option<Interfaces<'static>>,
    pub ble_controller: ExternalController<BleConnector<'static>, 20>,
}

impl BoardConfig {
    pub fn new(
        rng: Rng,
        wifi_controller: WifiController<'static>,
        interfaces: Interfaces<'static>,
        ble_controller: ExternalController<BleConnector<'static>, 20>,
    ) -> Self {
        Self {
            rng,
            wifi_controller,
            interfaces: Some(interfaces),
            ble_controller,
        }
    }
}

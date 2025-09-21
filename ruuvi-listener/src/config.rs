use core::{
    net::{AddrParseError, Ipv4Addr},
    num::ParseIntError,
    str::FromStr,
};

use dotenvy_macro::dotenv;

pub const SSID: &str = dotenv!("SSID");
pub const PASSWORD: &str = dotenv!("PASSWORD");
pub const GATEWAY_IP: &str = dotenv!("GATEWAY_IP");
pub const GATEWAY_PORT: &str = dotenv!("GATEWAY_PORT");
pub const AUTH_KEY: &str = dotenv!("AUTH_KEY");

pub struct WifiConfig {
    ssid: &'static str,
    password: &'static str,
}

impl WifiConfig {
    pub fn new(ssid: &'static str, password: &'static str) -> Self {
        Self { ssid, password }
    }
}

pub struct GatewayConfig {
    ip: Ipv4Addr,
    port: u16,
    auth: &'static str,
}

impl GatewayConfig {
    fn new(ip: &str, port: &str, auth: &'static str) -> Result<Self, ConfigParseError> {
        let ip = Ipv4Addr::from_str(ip)?;
        let port = u16::from_str(port)?;
        Ok(Self { ip, port, auth })
    }
}

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

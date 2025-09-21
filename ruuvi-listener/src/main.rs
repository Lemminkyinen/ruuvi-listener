#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

mod board;
mod config;
mod net;
mod scanner;
mod schema;
mod sender;

extern crate alloc;
use crate::config::{GatewayConfig, WifiConfig};
use crate::schema::RuuviRawV2;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Receiver};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use static_cell::StaticCell;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[macro_export]
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

static CHANNEL: StaticCell<Channel<NoopRawMutex, RuuviRawV2, 16>> = StaticCell::new();

// Constant configs
const WIFI_CONFIG: WifiConfig = WifiConfig::new();
const BLE_CONFIG: GatewayConfig = GatewayConfig::new();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let mut board_config = board::init();
    let (stack, runner) = net::init_network_stack(&mut board_config);

    spawner
        .spawn(net::connection(board_config.wifi_controller, WIFI_CONFIG))
        .expect("Network connection failed!");
    spawner
        .spawn(net::run_stack(runner))
        .expect("Net runner failed!");

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    log::info!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            log::info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    // Initialize a bounded channel of Ruuvi packets
    let channel = &*CHANNEL.init(Channel::new());
    let sender = channel.sender();
    let receiver = channel.receiver();

    // Run BLE ad scanner
    spawner
        .spawn(scanner::run(board_config.ble_controller, sender))
        .expect("Failed to spawn BLE scanner!");

    // Run HTTP packet sender
    spawner
        .spawn(sender::run(stack, receiver))
        .expect("Failed to HTTP sender logger!");

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-rc.0/examples/src/bin
}

#[embassy_executor::task]
async fn channel_logger(receiver: Receiver<'static, NoopRawMutex, RuuviRawV2, 16>) {
    loop {
        receiver.ready_to_receive().await;
        let value = receiver.receive().await;
        log::info!("{value:?}");
    }
}

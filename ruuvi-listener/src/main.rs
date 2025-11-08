#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

mod board;
mod config;
mod led;
mod net;
mod scanner;
mod schema;
mod sender;

extern crate alloc;
use crate::config::{BoardConfig, GatewayConfig, WifiConfig};
use crate::led::LedEvent;
use crate::net::acquire_address;
use crate::schema::RuuviRaw;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Instant;
use esp_backtrace as _;
use static_cell::StaticCell;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static CHANNEL: StaticCell<Channel<NoopRawMutex, (RuuviRaw, Instant), 16>> = StaticCell::new();
static LED_CHANNEL: StaticCell<Channel<NoopRawMutex, LedEvent, 16>> = StaticCell::new();
static BOARD_CONFIG: StaticCell<BoardConfig> = StaticCell::new();

// Constant configs
const WIFI_CONFIG: WifiConfig = WifiConfig::new();
const GATEWAY_CONFIG: GatewayConfig = GatewayConfig::new();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let board_config = BOARD_CONFIG.init(board::init());
    let (stack, runner) = net::init_network_stack(board_config);

    spawner
        .spawn(net::connection(
            board_config
                .wifi_controller
                .take()
                .expect("Wifi controller taken already"),
            WIFI_CONFIG,
        ))
        .expect("Failed to spawn network connection task!");
    spawner
        .spawn(net::run_stack(runner))
        .expect("Failed to spawn network runner task!");

    acquire_address(stack).await;

    // Run LED task for user feedback
    let led = board_config.led.take().unwrap();
    // Initialize a bounded channel of LED events
    let led_channel = &*LED_CHANNEL.init(Channel::new());
    let led_sender = led_channel.sender();
    let led_sender2 = led_sender;
    let receiver = led_channel.receiver();
    spawner
        .spawn(led::task(led, receiver))
        .expect("Failed to spawn led task!");

    // Initialize a bounded channel of Ruuvi packets
    let channel = &*CHANNEL.init(Channel::new());
    let sender = channel.sender();
    let receiver = channel.receiver();

    // Run BLE ad scanner
    spawner
        .spawn(scanner::run(
            board_config
                .ble_controller
                .take()
                .expect("BLE controller taken already"),
            sender,
            led_sender,
        ))
        .expect("Failed to spawn BLE scanner!");

    // Run TCP packet sender
    spawner
        .spawn(sender::run(
            stack,
            receiver,
            GATEWAY_CONFIG,
            board_config.rng,
            led_sender2,
        ))
        .expect("Failed to HTTP sender logger!");
}

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
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::channel::Channel;
use embassy_time::Instant;
use esp_backtrace as _;
use esp_hal::interrupt::software::{SoftwareInterrupt, SoftwareInterruptControl};
use esp_hal::system::Stack;
use esp_rtos::embassy::Executor;
use ruuvi_schema::RuuviRaw;
use static_cell::StaticCell;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

const CORE1_STACK_SIZE: usize = 8192 * 4;

static CHANNEL: StaticCell<Channel<NoopRawMutex, (RuuviRaw, Instant), 16>> = StaticCell::new();
static BOARD_CONFIG: StaticCell<BoardConfig> = StaticCell::new();
static SECOND_CORE_STACK: StaticCell<Stack<CORE1_STACK_SIZE>> = StaticCell::new();
static LED_CHANNEL: StaticCell<Channel<CriticalSectionRawMutex, LedEvent, 16>> = StaticCell::new();

// Constant configs
const WIFI_CONFIG: WifiConfig = WifiConfig::new();
const GATEWAY_CONFIG: GatewayConfig = GatewayConfig::new();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let peripherals = board::init_peripherals();
    let board_config = BOARD_CONFIG.init(board::init(peripherals));

    let (net_stack, runner) = net::init_network_stack(board_config);
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

    acquire_address(net_stack).await;

    // Initialize a bounded channel of LED events
    let led_channel = &*LED_CHANNEL.init(Channel::new());
    let led_sender = led_channel.sender();
    let led_sender2 = led_sender;
    let led_receiver = led_channel.receiver();

    // Initialize a bounded channel of Ruuvi packets
    let channel = &*CHANNEL.init(Channel::new());
    let sender = channel.sender();
    let receiver = channel.receiver();

    // Start the other core. For now only the LED task, since network stack cannot be shared?
    let app_core_stack = SECOND_CORE_STACK.init(Stack::new());
    let cpu_ctrl = board_config.cpu_ctrl.take().unwrap();
    let sw_int = SoftwareInterruptControl::new(board_config.sw_interrupt.take().unwrap());
    let int0: SoftwareInterrupt<'static, 0> = sw_int.software_interrupt0;
    let int1: SoftwareInterrupt<'static, 1> = sw_int.software_interrupt1;
    let rmt = board_config.rmt.take().unwrap();
    let gpio48 = board_config.gpio48.take().unwrap();
    esp_rtos::start_second_core(cpu_ctrl, int0, int1, app_core_stack, move || {
        static EXECUTOR: StaticCell<Executor> = StaticCell::new();
        let executor = EXECUTOR.init(Executor::new());
        let led = board::init_led(rmt, gpio48);

        // Run LED task for user feedback
        executor.run(|spawner| {
            spawner
                .spawn(led::task(led, led_receiver))
                .expect("Failed to spawn led task!");
        });
    });

    // Run BLE ad scanner task
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

    // Run TCP packet sender task
    spawner
        .spawn(sender::run(
            net_stack,
            receiver,
            GATEWAY_CONFIG,
            board_config.rng,
            led_sender2,
        ))
        .expect("Failed to HTTP sender logger!");
}

use crate::config::BoardConfig;
use bt_hci::controller::ExternalController;
use esp_hal::clock::CpuClock;
use esp_hal::peripherals;
use esp_hal::peripherals::Peripherals;
use esp_hal::rmt::{PulseCode, Rmt};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal_smartled::{SmartLedsAdapterAsync, buffer_size_async};
use esp_radio::ble::controller::BleConnector;
use static_cell::StaticCell;

static RMT_BUF: StaticCell<[PulseCode; buffer_size_async(1)]> = StaticCell::new();
static RADIO: StaticCell<esp_radio::Controller<'static>> = StaticCell::new();

pub fn init_peripherals() -> Peripherals {
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    log::info!("Starting to initialize board.");
    // Allocate memory - 2 x 64KiB
    esp_alloc::heap_allocator!(size: 64 * 1024);
    // Wifi & BLE COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 64 * 1024);
    log::info!("2 x 64KiB heap allocated!");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::default());
    let peripherals = esp_hal::init(config);
    log::info!("Esp-hal peripherals initialized!");
    peripherals
}

pub fn init(peripherals: Peripherals) -> BoardConfig {
    let timer0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timer0.timer0);
    log::info!("RTOS initialized!");

    let rng = esp_hal::rng::Rng::new();
    log::info!("RNG initialized!");

    let radio = RADIO.init(esp_radio::init().expect("Failed to initialize radio"));
    log::info!("Radio controller initialized!");

    let (wifi_controller, interfaces) =
        esp_radio::wifi::new(radio, peripherals.WIFI, Default::default())
            .expect("Failed to initialize WIFI controller");
    log::info!("Wifi controller initialized!");

    let transport = BleConnector::new(radio, peripherals.BT, Default::default())
        .expect("Failed to initialize BLE connector");
    let ble_controller: ExternalController<BleConnector<'static>, 20> =
        ExternalController::<_, 20>::new(transport);
    log::info!("BLE controller initialized!");

    BoardConfig::new(
        rng,
        wifi_controller,
        interfaces,
        ble_controller,
        peripherals.RMT,
        peripherals.GPIO48,
    )
}

pub fn init_led(
    rmt: peripherals::RMT<'static>,
    gpio48: peripherals::GPIO48<'static>,
) -> SmartLedsAdapterAsync<'static, 25> {
    // Configure RMT (Remote Control Transceiver) peripheral globally
    // <https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/peripherals/rmt.html>
    let frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(rmt, frequency)
        .expect("Failed to initialize RMT")
        .into_async();
    log::info!("RMT driver initialized!");

    // Use an RMT channel to instantiate a SmartLedsAdapterAsync
    let rmt_channel = rmt.channel0;
    let rmt_buffer = RMT_BUF.init([PulseCode::end_marker(); buffer_size_async(1)]);
    let led: SmartLedsAdapterAsync<'static, 25> =
        SmartLedsAdapterAsync::new(rmt_channel, gpio48, rmt_buffer);
    log::info!("Smart LED adapter initialized!");
    led
}

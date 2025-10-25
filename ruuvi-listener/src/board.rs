use crate::config::BoardConfig;
use bt_hci::controller::ExternalController;
use esp_hal::clock::CpuClock;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal_smartled::{SmartLedsAdapterAsync, buffer_size_async};
use esp_wifi::EspWifiController;
use esp_wifi::ble::controller::BleConnector;
use static_cell::StaticCell;

static ESP_WIFI_CONTROLLER: StaticCell<EspWifiController<'static>> = StaticCell::new();

pub fn init() -> BoardConfig {
    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    log::info!("Starting to initialize board.");
    // Allocate memory - 2 x 64KiB
    esp_alloc::heap_allocator!(size: 64 * 1024);
    // Wifi & BLE COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(#[unsafe(link_section = ".dram2_uninit")] size: 64 * 1024);
    log::info!("2 x 64KiB heap allocated!");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    log::info!("Esp-hal peripherals initialized!");

    // Configure RMT (Remote Control Transceiver) peripheral globally
    // <https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/peripherals/rmt.html>
    let frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, frequency)
        .expect("Failed to initialize RMT")
        .into_async();
    log::info!("RMT driver initialized!");

    // Use an RMT channel to instantiate a SmartLedsAdapterAsync
    let rmt_channel = rmt.channel0;
    let rmt_buffer = [0_u32; buffer_size_async(1)];
    let led: SmartLedsAdapterAsync<_, 25> =
        SmartLedsAdapterAsync::new(rmt_channel, peripherals.GPIO48, rmt_buffer);
    log::info!("Smart LED adapter initialized!");

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);
    log::info!("Embassy initialized!");

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    log::info!("Timer initialized!");

    let rng = esp_hal::rng::Rng::new(peripherals.RNG);
    log::info!("RNG initialized!");

    let esp_wifi_ctrl = ESP_WIFI_CONTROLLER.init(
        esp_wifi::init(timer1.timer0, rng).expect("Failed to initialize WIFI/BLE controller"),
    );
    let (wifi_controller, interfaces) = esp_wifi::wifi::new(esp_wifi_ctrl, peripherals.WIFI)
        .expect("Failed to initialize WIFI controller");
    log::info!("Wifi controller initialized!");

    let transport = BleConnector::new(esp_wifi_ctrl, peripherals.BT);
    let ble_controller: ExternalController<BleConnector<'static>, 20> =
        ExternalController::<_, 20>::new(transport);

    let config = BoardConfig::new(rng, wifi_controller, interfaces, ble_controller, Some(led));
    log::info!("BLE controller initialized!");
    config
}

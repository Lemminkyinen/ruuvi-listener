use crate::config::BoardConfig;
use bt_hci::controller::ExternalController;
use esp_hal::clock::CpuClock;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_wifi::EspWifiController;
use esp_wifi::ble::controller::BleConnector;

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

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);
    log::info!("Embassy initialized!");

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    log::info!("Timer initialized!");

    let rng = esp_hal::rng::Rng::new(peripherals.RNG);
    log::info!("RNG initialized!");

    let esp_wifi_ctrl = &*crate::mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer1.timer0, rng).expect("Failed to initialize WIFI/BLE controller")
    );
    let (wifi_controller, interfaces) = esp_wifi::wifi::new(esp_wifi_ctrl, peripherals.WIFI)
        .expect("Failed to initialize WIFI controller");
    log::info!("Wifi controller initialized!");

    let transport = BleConnector::new(esp_wifi_ctrl, peripherals.BT);
    let ble_controller: ExternalController<BleConnector<'static>, 20> =
        ExternalController::<_, 20>::new(transport);

    let config = BoardConfig::new(rng, wifi_controller, interfaces, ble_controller);
    log::info!("BLE controller initialized!");
    config
}

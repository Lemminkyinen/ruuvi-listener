use crate::config::{BoardConfig, WifiConfig};
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_wifi::wifi::{
    ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiState,
};

pub fn init_network_stack(
    board_config: &mut BoardConfig,
) -> (Stack<'static>, Runner<'static, WifiDevice<'static>>) {
    log::info!("Starting to initialize network stack.");
    let wifi_interface = board_config.interfaces.take().expect("No interface!").sta;
    let config = embassy_net::Config::dhcpv4(Default::default());
    let seed = (board_config.rng.random() as u64) << 32 | board_config.rng.random() as u64;
    let stack_resources = crate::mk_static!(StackResources<3>, StackResources::<3>::new());
    let stack_n_runner = embassy_net::new(wifi_interface, config, stack_resources, seed);
    log::info!("Network stack initialized!");
    stack_n_runner
}

#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>, config: WifiConfig) {
    log::info!("Start connection task");
    log::info!("Device capabilities: {:?}", controller.capabilities());
    loop {
        if esp_wifi::wifi::wifi_state() == WifiState::StaConnected {
            // Wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: config.ssid.into(),
                password: config.password.into(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            log::info!("Starting wifi");
            controller.start_async().await.unwrap();
            log::info!("Wifi started!");

            log::info!("Scan");
            let result = controller.scan_n_async(10).await.unwrap();
            for ap in result {
                log::info!("{ap:?}");
            }
        }
        log::info!("About to connect...");
        match controller.connect_async().await {
            Ok(_) => log::info!("Wifi connected!"),
            Err(e) => {
                log::info!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
pub async fn run_stack(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

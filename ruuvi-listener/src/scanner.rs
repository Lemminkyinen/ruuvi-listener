use crate::led::LedEvent;
use crate::schema::{RuuviRaw, parse_ruuvi_raw};
use bt_hci::param::LeExtAdvReport;
use core::cell::RefCell;
use embassy_futures::join::join;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::Instant;
use embassy_time::{Duration, Timer};
use esp_wifi::ble::controller::BleConnector;
use heapless::index_map::FnvIndexMap;
use trouble_host::prelude::*;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;
const RUUVI_MAN_ID: [u8; 2] = [0x99, 0x04];

type DataFormat = u8;
type DataIndex = usize;

#[embassy_executor::task]
pub async fn run(
    controller: ExternalController<BleConnector<'static>, 20>,
    sender: Sender<'static, NoopRawMutex, (RuuviRaw, Instant), 16>,
    led_sender: Sender<'static, NoopRawMutex, LedEvent, 16>,
) {
    let address: Address = Address::random([0xB0, 0x0B, 0xCA, 0xFE, 0xB0, 0x0B]);
    log::info!("MAC address: {address:?}");

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        central,
        mut runner,
        ..
    } = stack.build();
    log::info!("BLE stack initialized!");

    let handler = Handler::new(sender, led_sender);
    let mut scanner = Scanner::new(central);
    log::info!("Start scanning BLE ruuvi packets");
    let _ = join(runner.run_with_handler(&handler), async {
        let config = ScanConfig {
            active: false, // No need for scan responses, data is all in advertisement payload
            phys: PhySet::M1,
            interval: Duration::from_millis(1000),
            window: Duration::from_millis(1000),
            ..Default::default()
        };

        // Scan forever
        loop {
            let scan_session = scanner.scan_ext(&config).await;
            if let Err(e) = scan_session {
                log::error!("Error during scanning: {e:?}");
            }
            Timer::after(Duration::from_secs(1)).await;
        }
    })
    .await;
}

struct Handler {
    sender: Sender<'static, NoopRawMutex, (RuuviRaw, Instant), 16>,
    led_sender: Sender<'static, NoopRawMutex, LedEvent, 16>,
    // Use interior mutability since, handler cannot access its mutable self
    sequence_numbers: RefCell<FnvIndexMap<[u8; 6], u32, 16>>,
}

impl Handler {
    fn new(
        sender: Sender<'static, NoopRawMutex, (RuuviRaw, Instant), 16>,
        led_sender: Sender<'static, NoopRawMutex, LedEvent, 16>,
    ) -> Self {
        Handler {
            sender,
            led_sender,
            sequence_numbers: RefCell::new(FnvIndexMap::new()),
        }
    }

    fn is_new_seq(&self, mac: [u8; 6], seq: u32) -> bool {
        let map = self.sequence_numbers.borrow();
        map.get(&mac).is_none_or(|prev_seq| *prev_seq != seq)
    }

    fn upsert_seq(&self, mac: [u8; 6], seq: u32) {
        let mut map = self.sequence_numbers.borrow_mut();
        _ = map.insert(mac, seq).map_err(|(mac, seq_key)| {
            log::error!("Failed to insert key {mac:?}, value: {seq_key}")
        });
    }

    fn extract_ruuvi_format(report: LeExtAdvReport<'_>) -> Option<(DataFormat, DataIndex)> {
        // Ruuvi tag & air address kinds are random
        // Ruuvi manufacturer's ID:
        // Tag - format 5 - 5..7
        // Air - format E1 - 2..4
        // Air - format 6 - 9..11, skipping format 6, since we are using E1
        if report.addr_kind == AddrKind::RANDOM && report.data.len() >= 7 {
            if report.data[5..7] == RUUVI_MAN_ID {
                return Some((report.data[7], 7));
            }

            if report.data[2..4] == RUUVI_MAN_ID {
                return Some((report.data[4], 4));
            }
        }
        None
    }
}

impl EventHandler for Handler {
    fn on_ext_adv_reports(&self, mut reports: LeExtAdvReportsIter) {
        while let Some(Ok(report)) = reports.next() {
            if let Some((data_format, index)) = Self::extract_ruuvi_format(report) {
                // TODO: Add rssi and tx_power to the payload
                let _rssi = report.rssi;
                let _tx_power = report.tx_power;

                log::info!("Data format: {data_format:X?}",);
                log::info!("Data start at: {index}");
                log::info!("Data len: {}", report.data[index..].len());

                let t = Instant::now();
                match parse_ruuvi_raw(data_format, &report.data[index..]) {
                    Ok(parsed) => {
                        // If channel is full, empty it
                        if self.sender.is_full() {
                            self.sender.clear();
                            log::warn!("Channel full. Clearing channel for new data!");
                        }

                        let mac = parsed.mac();
                        let measurement_seq = parsed.measurement_seq();

                        // Verify the sequence number of the packet
                        let is_new = self.is_new_seq(mac, measurement_seq);
                        self.upsert_seq(mac, measurement_seq);

                        // If it's not new, skip the loop
                        if !is_new {
                            if let Err(err) = self.led_sender.try_send(LedEvent::BleDuplicate) {
                                log::error!("Failed to send LedEvent to the channel! {err:?}");
                            }
                            log::info!(
                                "Old data received, skipping! mac: {mac:?}, seq: {measurement_seq}"
                            );
                            continue;
                        }

                        // Send data to the channel
                        if let Err(err) = self.sender.try_send((parsed, t)) {
                            log::error!("Failed to send RuuviRawV2 to the channel! {err:?}");
                        }
                        if let Err(err) = self.led_sender.try_send(LedEvent::BleOk) {
                            log::error!("Failed to send LedEvent to the channel! {err:?}");
                        }
                    }
                    Err(e) => log::error!("Payload error! {e:?}!"),
                }
            }
        }
    }
}

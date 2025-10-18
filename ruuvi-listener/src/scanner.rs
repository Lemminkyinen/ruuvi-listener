use crate::schema::RuuviRawV2;
use bt_hci::param::LeAdvEventKind;
use bt_hci::param::LeAdvReport;
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

#[embassy_executor::task]
pub async fn run(
    controller: ExternalController<BleConnector<'static>, 20>,
    sender: Sender<'static, NoopRawMutex, (RuuviRawV2, Instant), 16>,
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

    let handler = Handler::new(sender);
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
            let scan_session = scanner.scan(&config).await;
            if let Err(e) = scan_session {
                log::error!("Error during scanning: {e:?}");
            }
            Timer::after(Duration::from_secs(1)).await;
        }
    })
    .await;
}

struct Handler {
    sender: Sender<'static, NoopRawMutex, (RuuviRawV2, Instant), 16>,
    // Use interior mutability since, handler cannot access its mutable self
    sequence_numbers: RefCell<FnvIndexMap<[u8; 6], u16, 16>>,
}

impl Handler {
    fn new(sender: Sender<'static, NoopRawMutex, (RuuviRawV2, Instant), 16>) -> Self {
        Handler {
            sender,
            sequence_numbers: RefCell::new(FnvIndexMap::new()),
        }
    }

    fn is_new_seq(&self, mac: [u8; 6], seq: u16) -> bool {
        let map = self.sequence_numbers.borrow();
        map.get(&mac).is_none_or(|prev_seq| *prev_seq != seq)
    }

    fn upsert_seq(&self, mac: [u8; 6], seq: u16) {
        let mut map = self.sequence_numbers.borrow_mut();
        _ = map.insert(mac, seq).map_err(|(mac, seq_key)| {
            log::error!("Failed to insert key {mac:?}, value: {seq_key}")
        });
    }

    fn is_ruuvi_report(&self, report: LeAdvReport<'_>) -> bool {
        // Ruuvi raw v2 data
        report.addr_kind == AddrKind::RANDOM
            && report.event_kind == LeAdvEventKind::AdvInd
            && report.data.len() >= 7
            && report.data[5..7] == RUUVI_MAN_ID
    }
}

impl EventHandler for Handler {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = it.next() {
            if self.is_ruuvi_report(report) {
                let t = Instant::now();
                // Ruuvitag v2 raw data starts at index 7
                match RuuviRawV2::from_bytes(&report.data[7..]) {
                    Ok(parsed) => {
                        // If channel is full, empty it
                        if self.sender.is_full() {
                            self.sender.clear();
                            log::warn!("Channel full. Clearing channel for new data!");
                        }

                        // Verify the sequence number of the packet
                        let is_new = self.is_new_seq(parsed.mac, parsed.measurement_seq);
                        self.upsert_seq(parsed.mac, parsed.measurement_seq);

                        // If it's not new, skip the loop
                        if !is_new {
                            log::info!(
                                "Old data received, skipping! mac: {:?}, seq: {}",
                                parsed.mac,
                                parsed.measurement_seq
                            );
                            continue;
                        }

                        // Send data to the channel
                        if let Err(err) = self.sender.try_send((parsed, t)) {
                            log::error!("Failed to send RuuviRawV2 to the channel! {err:?}");
                        }
                    }
                    Err(e) => log::error!("Payload error! {e:?}!"),
                }
            }
        }
    }
}

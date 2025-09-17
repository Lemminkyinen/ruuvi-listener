use crate::schema::RuuviRawV2;
use bt_hci::controller::ControllerCmdSync;
use bt_hci::param::LeAdvEventKind;
use bt_hci::{cmd::le::LeSetScanParams, param::LeAdvReport};
use core::cell::RefCell;
use core::fmt::Write;
use embassy_futures::join::join;
use embassy_time::{Duration, Timer};
use heapless::Deque;
use trouble_host::prelude::*;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;
const RUUVI_MAN_ID: [u8; 2] = [0x99, 0x04];

pub async fn run<C>(controller: C)
where
    C: Controller + ControllerCmdSync<LeSetScanParams>,
{
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xCA, 0xFE, 0xB0, 0x0B, 0xB0, 0x0B]);

    log::info!("Our address = {address:?}");

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);

    let Host {
        central,
        mut runner,
        ..
    } = stack.build();

    let printer = Printer::new();
    let mut scanner = Scanner::new(central);
    let _ = join(runner.run_with_handler(&printer), async {
        let config = ScanConfig {
            active: false, // No need for scan responses, data is all in advertisement payload
            phys: PhySet::M1,
            interval: Duration::from_millis(1285),
            window: Duration::from_millis(300),
            ..Default::default()
        };
        // Instead of holding the session forever, run scans in bursts
        loop {
            if let Ok(session) = scanner.scan(&config).await {
                // scan for ~1s
                Timer::after(Duration::from_secs(1)).await;
                drop(session); // stop scanning
            }
            // wait before scanning again (tune this)
            Timer::after(Duration::from_secs(4)).await;
        }
    })
    .await;
}

struct Printer {
    seen: RefCell<Deque<BdAddr, 128>>,
}

impl Printer {
    fn new() -> Self {
        Printer {
            seen: RefCell::new(Deque::new()),
        }
    }
}

impl EventHandler for Printer {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        let mut seen = self.seen.borrow_mut();
        while let Some(Ok(report)) = it.next() {
            if !seen.iter().any(|b| b.raw() == report.addr.raw()) {
                let be_mac_address = to_be_mac(report.addr.raw());
                if is_ruuvi_report(report) {
                    log::info!("Ruuvitag found: {}", addr_to_hex(&be_mac_address));
                    log::info!("Signal strength: {:?}", report.rssi);
                } else {
                    log::info!("discovered: {}", addr_to_hex(&be_mac_address));
                }
            }

            if seen.is_full() {
                seen.pop_front();
            }
            seen.push_back(report.addr).unwrap();

            if is_ruuvi_report(report) {
                // Ruuvitag v2 raw data starts at index 7
                match RuuviRawV2::from_bytes(&report.data[7..]) {
                    Ok(parsed) => log::info!("Payload: {parsed:?}"),
                    Err(e) => log::error!("Payload error! {e:?}!"),
                }
            }
        }
    }
}

fn is_ruuvi_report(report: LeAdvReport<'_>) -> bool {
    report.addr_kind == AddrKind::RANDOM
        && report.event_kind == LeAdvEventKind::AdvInd
        && report.data.len() >= 7
        && report.data[5..7] == RUUVI_MAN_ID
}

fn to_be_mac(data: &[u8]) -> [u8; 6] {
    let mut be_mac_address = [0x0u8; 6];
    be_mac_address.copy_from_slice(data);
    be_mac_address.reverse();
    be_mac_address
}

fn addr_to_hex(addr: &[u8]) -> heapless::String<18> {
    let mut s = heapless::String::<18>::new(); // 17 chars + null terminator
    for (i, byte) in addr.iter().enumerate() {
        write!(s, "{byte:02X}").unwrap();
        if i != addr.len() - 1 {
            s.push(':').unwrap();
        }
    }
    s
}

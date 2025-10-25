use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Receiver;
use embassy_time::Duration;
use embassy_time::WithTimeout;
use esp_hal::rmt::{ConstChannelAccess, Tx};
use esp_hal_smartled::SmartLedsAdapterAsync;
use smart_leds::colors::{BLACK, BLUE, GREEN, RED};
use smart_leds::{SmartLedsWriteAsync, brightness};

#[derive(Debug)]
pub enum LedEvent {
    BleOk,
    BleDuplicate,
    TcpOk,
}

#[embassy_executor::task]
pub async fn task(
    mut led: SmartLedsAdapterAsync<ConstChannelAccess<Tx, 0>, 25>,
    receiver: Receiver<'static, NoopRawMutex, LedEvent, 16>,
) {
    let level = 1;
    let mut event = None;
    loop {
        // Wait for at least one event, no spinning b(lock)
        if event.is_none() {
            event = Some(receiver.receive().await);
        }

        // Drain any queued events, keep only the latest one
        while let Ok(v) = receiver.try_receive() {
            event = Some(v);
        }
        log::debug!("Received event: {event:?}");

        // Match event variant to a correct color
        let data: smart_leds::RGB<u8> = match event {
            Some(LedEvent::BleOk) => GREEN,
            Some(LedEvent::TcpOk) => BLUE,
            Some(LedEvent::BleDuplicate) => RED,
            // Should be impossible??
            None => unreachable!(),
        };

        // Write the color in the led
        let brightness_adjusted = brightness([data].into_iter(), level);
        led.write(brightness_adjusted).await.unwrap();

        // Try to read another event for 80 ms and then turn off led.
        // If a new event comes, set it to a variable and continue loop
        event = if let Ok(e) = receiver
            .receive()
            .with_timeout(Duration::from_millis(80))
            .await
        {
            Some(e)
        } else {
            led.write([BLACK].into_iter()).await.unwrap();
            None
        }
    }
}

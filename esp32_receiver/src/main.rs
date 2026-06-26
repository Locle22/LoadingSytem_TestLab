use esp32_receiver::WifiReceiver;
use esp_idf_svc::hal::peripherals::Peripherals;
use log::info;
use std::thread;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // It is necessary to call this once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly.
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Starting ESP32 UDP Receiver...");

    // Replace with your actual Wi-Fi credentials
    let ssid = "Loc";
    let password = "66666666";
    let port = 8080;

    let peripherals = Peripherals::take()?;
    let sys_loop = esp_idf_svc::eventloop::EspSystemEventLoop::take()?;
    let nvs = esp_idf_svc::nvs::EspDefaultNvsPartition::take()?;

    let wifi = esp_idf_svc::wifi::EspWifi::new(
        peripherals.modem,
        sys_loop.clone(),
        Some(nvs),
    )?;

    let receiver = WifiReceiver::new(wifi, ssid, password, port)?;

    info!("Listening for UDP packets on port {}...", port);

    loop {
        if let Some((msg, src)) = receiver.receive_packet() {
            match protocol::Packet::deserialize(&msg) {
                Ok(packet) => {
                    info!("Received from {}: [Seq {}] {}", src, packet.seq_id, packet.command);
                }
                Err(e) => {
                    log::warn!("Failed to parse packet from {}: {}", src, e);
                }
            }
        }

        // Small delay to prevent watchdog starvation if no packets
        thread::sleep(Duration::from_millis(10));
    }
}

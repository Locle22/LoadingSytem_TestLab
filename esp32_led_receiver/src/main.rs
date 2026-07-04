use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration, EspWifi};
use log::{info, warn};
use smart_leds::{SmartLedsWrite, RGB8};
use std::net::UdpSocket;
use std::time::Duration;
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

fn main() -> anyhow::Result<()> {
    // Required to link patches for ESP-IDF runtime
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Starting ESP32 UDP RGB Receiver...");

    // Khởi tạo phần cứng
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // ── 1. Kết nối WiFi ──
    let mut wifi = EspWifi::new(peripherals.modem, sys_loop, Some(nvs))?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: "Loc".try_into().unwrap(),
        password: "66666666".try_into().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;

    wifi.start()?;
    wifi.connect()?;

    info!("Waiting for WIFI connection...");
    while !wifi.is_connected()? {
        std::thread::sleep(Duration::from_millis(500));
    }
    info!("WiFi Connected successfully!");

    // ── 2. Cấu hình Đèn LED WS2812 (Chân 48) ──
    // ESP32 S3 sử dụng chân GPIO 48 cho đèn NeoPixel (RGB) tích hợp trên bo mạch
    let led_pin = peripherals.pins.gpio48;
    let channel = peripherals.rmt.channel0; // Sử dụng bộ RMT (Remote Control) để phát xung siêu nhanh
    let mut ws2812 = Ws2812Esp32Rmt::new(channel, led_pin)?;

    // Chớp đèn Xanh lá 1 lần để báo hiệu đã sẵn sàng
    ws2812.write(std::iter::once(RGB8::new(0, 255, 0)))?;
    std::thread::sleep(Duration::from_millis(500));
    ws2812.write(std::iter::once(RGB8::new(0, 0, 0)))?;

    // ── 3. Khởi tạo UDP Socket ──
    let port = 8888;
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;
    
    info!("Listening for UDP packets on port {}...", port);

    let mut buf = [0u8; 3]; // Chúng ta chỉ nhận đúng 3 byte: [R, G, B]

    loop {
        // Hàm recv_from sẽ block luồng cho đến khi có mạng gửi tới
        match socket.recv_from(&mut buf) {
            Ok((size, src)) => {
                if size == 3 {
                    let r = buf[0];
                    let g = buf[1];
                    let b = buf[2];

                    info!("Nhận màu từ AI ({}): RGB({}, {}, {})", src, r, g, b);
                    
                    // Phát màu ra bóng LED
                    let _ = ws2812.write(std::iter::once(RGB8::new(r, g, b)));
                } else {
                    warn!("Gói tin lỗi! Kích thước: {} bytes", size);
                }
            }
            Err(e) => {
                warn!("Lỗi đọc UDP Socket: {}", e);
            }
        }
    }
}

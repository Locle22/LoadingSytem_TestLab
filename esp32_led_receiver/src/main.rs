use esp_idf_hal::ledc::{LedcDriver, LedcTimerDriver, config::TimerConfig};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::Hertz;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration, EspWifi};
use log::{info, warn};
use smart_leds::{SmartLedsWrite, RGB8};
use std::net::UdpSocket;
use std::time::Duration;
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

// ──────────────────────────────────────────────
//  Hằng số cấu hình Servo
// ──────────────────────────────────────────────

/// Tần số xung PWM chuẩn cho Servo (50 Hz = chu kỳ 20ms)
const SERVO_FREQ_HZ: u32 = 50;

/// Độ phân giải PWM: 14-bit → 16384 bước (Rất mịn, servo xoay cực mượt)
const SERVO_RESOLUTION: u32 = 14;
const SERVO_MAX_DUTY: u32 = (1 << SERVO_RESOLUTION) - 1; // = 16383

/// Xung nhỏ nhất (0°): 0.5ms / 20ms * 16384 ≈ 410
const SERVO_MIN_PULSEWIDTH: u32 = 410;

/// Xung lớn nhất (360°): 2.5ms / 20ms * 16384 ≈ 2048
const SERVO_MAX_PULSEWIDTH: u32 = 2048;

/// Số loài muỗi trong dataset
const NUM_SPECIES: u32 = 36;

/// Góc cách nhau giữa mỗi loài (36 loài × 10° = 360° tròn đĩa)
const DEGREES_PER_SPECIES: u32 = 10;

/// Mã class_id đặc biệt: Không có muỗi → giữ nguyên servo
const CLASS_ID_NONE: u8 = 0xFF;

/// Chân GPIO kết nối tín hiệu điều khiển Servo.
/// ┌─────────────────────────────────────────────┐
/// │  Sơ đồ đấu nối Servo:                      │
/// │  - Dây Cam (Signal) → GPIO 4 (ESP32 S3)    │
/// │  - Dây Đỏ (VCC)    → 5V (Nguồn ngoài)     │
/// │  - Dây Nâu (GND)   → GND chung             │
/// └─────────────────────────────────────────────┘
/// LƯU Ý: Servo tiêu thụ dòng lớn (>500mA), KHÔNG cấp nguồn từ ESP32!
///         Phải dùng nguồn ngoài 5V riêng cho Servo, NỐI CHUNG GND.

// ──────────────────────────────────────────────
//  Hàm tính toán xung PWM cho Servo
// ──────────────────────────────────────────────

/// Chuyển đổi góc (0..360) thành giá trị duty cycle cho LEDC PWM.
///
/// Công thức: duty = MIN_PULSE + (angle / 360) × (MAX_PULSE - MIN_PULSE)
fn angle_to_duty(angle_deg: u32) -> u32 {
    let angle_clamped = angle_deg.min(360);
    let pulse_range = SERVO_MAX_PULSEWIDTH - SERVO_MIN_PULSEWIDTH;
    SERVO_MIN_PULSEWIDTH + (angle_clamped * pulse_range) / 360
}

// ──────────────────────────────────────────────
//  Main
// ──────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    // Required to link patches for ESP-IDF runtime
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Starting ESP32 UDP RGB + Servo Receiver...");

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

    // ── 3. Cấu hình Servo Motor (Chân GPIO 4) ──
    // Sử dụng bộ LEDC (LED Control) của ESP32 để phát xung PWM 50Hz điều khiển Servo
    let timer_config = TimerConfig::default().frequency(Hertz(SERVO_FREQ_HZ));
    let timer = LedcTimerDriver::new(peripherals.ledc.timer0, &timer_config)?;
    let mut servo = LedcDriver::new(peripherals.ledc.channel0, &timer, peripherals.pins.gpio4)?;

    // Đưa servo về vị trí HOME (0°) khi khởi động
    servo.set_duty(angle_to_duty(0))?;
    info!("Servo initialized at HOME position (0°)");

    // Chớp đèn Xanh lá 1 lần để báo hiệu đã sẵn sàng
    ws2812.write(std::iter::once(RGB8::new(0, 255, 0)))?;
    std::thread::sleep(Duration::from_millis(500));
    ws2812.write(std::iter::once(RGB8::new(0, 0, 0)))?;

    // ── 4. Khởi tạo UDP Socket ──
    let port = 8888;
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;

    info!("Listening for UDP packets on port {}...", port);
    info!("Protocol: [R, G, B, class_id] — 4 bytes");
    info!("  class_id 0..35 → Servo xoay đến góc class_id × 10°");
    info!("  class_id 0xFF  → Tắt LED, servo giữ nguyên");

    let mut buf = [0u8; 4]; // Giao thức mới: 4 byte [R, G, B, class_id]

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, src)) => {
                if size == 4 {
                    let r = buf[0];
                    let g = buf[1];
                    let b = buf[2];
                    let class_id = buf[3];

                    // Điều khiển đèn LED
                    let _ = ws2812.write(std::iter::once(RGB8::new(r, g, b)));

                    // Điều khiển Servo đĩa xoay (chỉ xoay khi có muỗi)
                    if class_id != CLASS_ID_NONE && (class_id as u32) < NUM_SPECIES {
                        let target_angle = class_id as u32 * DEGREES_PER_SPECIES;
                        let duty = angle_to_duty(target_angle);
                        let _ = servo.set_duty(duty);
                        info!(
                            "Nhận từ AI ({}): RGB({},{},{}) | Loài #{} → Servo xoay {}°",
                            src, r, g, b, class_id, target_angle
                        );
                    } else if class_id == CLASS_ID_NONE {
                        // Không phát hiện muỗi → Tắt LED, servo giữ nguyên vị trí
                        info!("Không phát hiện muỗi → LED tắt, servo giữ nguyên");
                    }
                } else if size == 3 {
                    // Tương thích ngược với giao thức cũ (chỉ LED, không servo)
                    let _ = ws2812.write(std::iter::once(RGB8::new(buf[0], buf[1], buf[2])));
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

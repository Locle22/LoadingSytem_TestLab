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
//  Hằng số cấu hình Servo SG90 360° (Continuous)
// ──────────────────────────────────────────────

/// Tần số xung PWM chuẩn cho Servo (50 Hz = chu kỳ 20ms)
const SERVO_FREQ_HZ: u32 = 50;

/// Độ phân giải PWM: 14-bit → 16384 bước
const SERVO_RESOLUTION: u32 = 14;

/// Xung 1.5ms = DỪNG (Neutral). Duty = 1.5 / 20.0 × 16384 ≈ 1229
const DUTY_STOP: u32 = 1229;

/// Xung quay thuận (CW) tốc độ chậm: 1.35ms → Duty ≈ 1106
/// (Tốc độ chậm giúp xoay chính xác hơn, tránh quán tính trượt)
const DUTY_SLOW_CW: u32 = 1106;

/// Xung quay ngược (CCW) tốc độ chậm: 1.65ms → Duty ≈ 1352
const DUTY_SLOW_CCW: u32 = 1352;

/// ┌──────────────────────────────────────────────────────────────┐
/// │  THÔNG SỐ HIỆU CHUẨN (CALIBRATION)                        │
/// │  Đây là số mili-giây cần thiết để servo xoay được 1 độ.    │
/// │  Giá trị mặc định: 6 ms/độ (ở tốc độ chậm).               │
/// │                                                              │
/// │  CÁCH HIỆU CHUẨN:                                           │
/// │  1. Dán một mũi tên lên trục servo                          │
/// │  2. Cho servo quay 360° (gửi class_id = 35 từ vị trí 0)    │
/// │  3. Nếu nó quay QUÁ nhiều → TĂNG giá trị (ví dụ: 7)       │
/// │     Nếu nó quay THIẾU     → GIẢM giá trị (ví dụ: 5)       │
/// └──────────────────────────────────────────────────────────────┘
const MS_PER_DEGREE: u32 = 6;

/// Số loài muỗi trong dataset
const NUM_SPECIES: u32 = 36;

/// Góc cách nhau giữa mỗi loài (36 × 10° = 360°)
const DEGREES_PER_SPECIES: u32 = 10;

/// Mã class_id đặc biệt: Không có muỗi → giữ nguyên servo
const CLASS_ID_NONE: u8 = 0xFF;

/// Chân GPIO kết nối tín hiệu điều khiển Servo.
/// ┌─────────────────────────────────────────────┐
/// │  Sơ đồ đấu nối Servo SG90 360°:            │
/// │  - Dây Cam (Signal) → GPIO 4 (ESP32 S3)    │
/// │  - Dây Đỏ (VCC)    → 5V (Nguồn ngoài)     │
/// │  - Dây Nâu (GND)   → GND chung             │
/// └─────────────────────────────────────────────┘

// ──────────────────────────────────────────────
//  Hàm điều khiển Servo 360° Continuous
// ──────────────────────────────────────────────

/// Xoay servo từ góc hiện tại đến góc đích, chọn chiều ngắn nhất.
///
/// Servo SG90 360° không có cảm biến vị trí, nên ta phải:
/// 1. Tính góc quay cần thiết (delta)
/// 2. Chọn chiều xoay ngắn nhất (CW hoặc CCW)
/// 3. Bật xung quay trong khoảng thời gian = delta × MS_PER_DEGREE
/// 4. Dừng servo (gửi xung neutral 1.5ms)
/// 5. Cập nhật vị trí hiện tại trong phần mềm
fn rotate_to(
    servo: &mut LedcDriver<'_>,
    current_angle: &mut i32,
    target_angle: i32,
) {
    // Tính góc quay ngắn nhất trên vòng tròn 360°
    let mut delta = target_angle - *current_angle;

    // Chuẩn hóa delta về khoảng -180..+180 (chọn chiều ngắn nhất)
    while delta > 180 {
        delta -= 360;
    }
    while delta < -180 {
        delta += 360;
    }

    if delta == 0 {
        return; // Đã ở đúng vị trí, không cần xoay
    }

    // Chọn chiều xoay
    let duty = if delta > 0 { DUTY_SLOW_CW } else { DUTY_SLOW_CCW };
    let abs_delta = delta.unsigned_abs();
    let spin_time_ms = abs_delta * MS_PER_DEGREE;

    info!(
        "  Servo: {}° → {}° (Δ={}°, {}ms, {})",
        *current_angle,
        target_angle,
        delta,
        spin_time_ms,
        if delta > 0 { "CW ↻" } else { "CCW ↺" }
    );

    // Bật motor quay
    let _ = servo.set_duty(duty);

    // Chờ đủ thời gian xoay
    std::thread::sleep(Duration::from_millis(spin_time_ms as u64));

    // Dừng motor
    let _ = servo.set_duty(DUTY_STOP);

    // Cập nhật vị trí hiện tại (trong phần mềm)
    *current_angle = target_angle;
}

// ──────────────────────────────────────────────
//  Main
// ──────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    // Required to link patches for ESP-IDF runtime
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Starting ESP32 — Mosquito Sorter (LED + Servo SG90 360°)");

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
    info!("WiFi Connected!");

    // ── 2. Cấu hình Đèn LED WS2812 (Chân 48) ──
    let led_pin = peripherals.pins.gpio48;
    let channel = peripherals.rmt.channel0;
    let mut ws2812 = Ws2812Esp32Rmt::new(channel, led_pin)?;

    // ── 3. Cấu hình Servo SG90 360° (Chân GPIO 4) ──
    let timer_config = TimerConfig::default().frequency(Hertz(SERVO_FREQ_HZ));
    let timer = LedcTimerDriver::new(peripherals.ledc.timer0, &timer_config)?;
    let mut servo = LedcDriver::new(peripherals.ledc.channel0, &timer, peripherals.pins.gpio4)?;

    // Dừng servo tại vị trí khởi động (HOME = 0°)
    servo.set_duty(DUTY_STOP)?;
    let mut current_angle: i32 = 0;
    info!("Servo initialized at HOME (0°), calibration: {}ms/deg", MS_PER_DEGREE);

    // Chớp đèn Xanh lá 1 lần → Sẵn sàng!
    ws2812.write(std::iter::once(RGB8::new(0, 255, 0)))?;
    std::thread::sleep(Duration::from_millis(500));
    ws2812.write(std::iter::once(RGB8::new(0, 0, 0)))?;

    // ── 4. Khởi tạo UDP Socket ──
    let port = 8888;
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;

    info!("Listening on UDP port {} — Protocol: [R,G,B,class_id]", port);
    info!("  class_id 0..35 → LED sáng + Servo xoay đến class_id × 10°");
    info!("  class_id 0xFF  → LED tắt, servo giữ nguyên");

    let mut buf = [0u8; 4]; // Giao thức: [R, G, B, class_id]

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

                    // Điều khiển Servo đĩa xoay
                    if class_id != CLASS_ID_NONE && (class_id as u32) < NUM_SPECIES {
                        let target_angle = (class_id as u32 * DEGREES_PER_SPECIES) as i32;

                        info!(
                            "Nhận từ AI ({}): RGB({},{},{}) | Loài #{} → Đích {}°",
                            src, r, g, b, class_id, target_angle
                        );

                        rotate_to(&mut servo, &mut current_angle, target_angle);
                    } else if class_id == CLASS_ID_NONE {
                        info!("Không có muỗi → LED tắt, servo giữ nguyên ({}°)", current_angle);
                    }
                } else if size == 3 {
                    // Tương thích ngược giao thức cũ (chỉ LED)
                    let _ = ws2812.write(std::iter::once(RGB8::new(buf[0], buf[1], buf[2])));
                } else {
                    warn!("Gói tin lỗi! Kích thước: {} bytes", size);
                }
            }
            Err(e) => {
                warn!("Lỗi đọc UDP: {}", e);
            }
        }
    }
}

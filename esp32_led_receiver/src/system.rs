use esp_idf_hal::ledc::{LedcDriver, LedcTimerDriver, config::TimerConfig};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::Hertz;
use esp_idf_hal::spi::{config::DriverConfig, SpiConfig, SpiDeviceDriver, SpiDriver};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, ClientConfiguration, Configuration, EspWifi};
use log::{info, warn, error};
use smart_leds::{SmartLedsWrite, RGB8};
use std::net::UdpSocket;
use std::time::Duration;
use std::thread;
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use mfrc522::Mfrc522;
use esp_idf_svc::handle::RawHandle;

use crate::servo::ContinuousServo;
use crate::sorter::Sorter;
use crate::conveyor::Conveyor;

// ──────────────────────────────────────────────
// Hằng số hệ thống
// ──────────────────────────────────────────────
const SERVO_FREQ_HZ: u32 = 50;
const NUM_SPECIES: u32 = 36;
const DEGREES_PER_SPECIES: u32 = 10;
const CLASS_ID_NONE: u8 = 0xFF;
const AUTHORIZED_UID: [u8; 4] = [189, 7, 16, 6];

/// Tầng 3: Điều phối hệ thống & Chạy tự động (System Orchestrator)
/// Quản lý việc kết nối phần cứng và khởi động các chu trình tự động.
pub fn run() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Starting ESP32 — Mosquito Sorter & Conveyor Belt (Layered Architecture)");

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // ── 1. Kết nối WiFi ──
    let mut wifi = EspWifi::new(peripherals.modem, sys_loop, Some(nvs))?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: "The Loc".try_into().unwrap(),
        password: "66666666".try_into().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;

    // Thiết lập hostname để tự động phân giải mosquito-sorter.local trên máy tính
    let hostname = std::ffi::CString::new("mosquito-sorter").unwrap();
    unsafe {
        let netif = wifi.sta_netif();
        let netif_ptr = netif.handle();
        let _ = esp_idf_sys::esp_netif_set_hostname(netif_ptr, hostname.as_ptr());
    }

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

    // ── 3. Cấu hình Timer cho 2 Servo ──
    let timer_config = TimerConfig::new()
        .frequency(Hertz(SERVO_FREQ_HZ))
        .resolution(esp_idf_hal::ledc::config::Resolution::Bits14);

    // Khởi tạo Sorter (Servo đĩa xoay - Chân 4)
    let timer0 = LedcTimerDriver::new(peripherals.ledc.timer0, &timer_config)?;
    let raw_sorter = LedcDriver::new(peripherals.ledc.channel0, &timer0, peripherals.pins.gpio4)?;
    let mut sorter = Sorter::new(ContinuousServo::new(raw_sorter));
    sorter.reset_to_home()?;

    // Khởi tạo Conveyor (Servo băng chuyền - Chân 5)
    let timer1 = LedcTimerDriver::new(peripherals.ledc.timer1, &timer_config)?;
    let raw_conveyor = LedcDriver::new(peripherals.ledc.channel1, &timer1, peripherals.pins.gpio5)?;
    let is_conveyor_running = Arc::new(AtomicBool::new(true));
    let mut conveyor = Conveyor::new(ContinuousServo::new(raw_conveyor), Arc::clone(&is_conveyor_running));
    conveyor.start()?;

    // Chớp đèn Xanh lá 1 lần báo hiệu hệ thống đã sẵn sàng
    ws2812.write(std::iter::once(RGB8::new(0, 255, 0)))?;
    std::thread::sleep(Duration::from_millis(500));
    ws2812.write(std::iter::once(RGB8::new(0, 0, 0)))?;

    // ── 4. Cấu hình SPI & Đầu đọc RFID RC522 ──
    let spi_driver = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio12, // SCK
        peripherals.pins.gpio11, // MOSI
        Some(peripherals.pins.gpio13), // MISO
        &DriverConfig::new(),
    )?;
    
    let spi_config = SpiConfig::new().baudrate(Hertz(1_000_000).into());
    let spi_device = SpiDeviceDriver::new(spi_driver, Some(peripherals.pins.gpio10), &spi_config)?;
    
    // Di chuyển bộ điều khiển băng chuyền và timer vào thread RFID chạy ngầm
    thread::spawn(move || {
        let _t = timer1; // Giữ timer1 không bị drop khi ra khỏi luồng chính
        let mut active_conveyor = conveyor;
        
        let spi_interface = mfrc522::comm::blocking::spi::SpiInterface::new(spi_device);
        let mut rfid = match Mfrc522::new(spi_interface).init() {
            Ok(rfid) => rfid,
            Err(_) => {
                error!("Failed to initialize MFRC522!");
                return;
            }
        };

        info!("RFID Scanner Thread Started! Polling for cards...");

        loop {
            if let Ok(atqa) = rfid.reqa() {
                if let Ok(uid) = rfid.select(&atqa) {
                    let uid_bytes = uid.as_bytes();
                    info!("💳 THẺ ĐÃ ĐƯỢC QUẸT! Mã UID: {:?}", uid_bytes);

                    let mut matched = false;
                    if uid_bytes.len() >= 4 && uid_bytes[0..4] == AUTHORIZED_UID {
                        matched = true;
                    }

                    if matched {
                        match active_conveyor.toggle() {
                            Ok(running) => {
                                if running {
                                    info!("🚀 Băng chuyền: TIẾP TỤC CHẠY");
                                } else {
                                    info!("🛑 Băng chuyền: DỪNG KHẨN CẤP (E-STOP)!");
                                }
                            }
                            Err(e) => {
                                error!("Lỗi khi đảo trạng thái băng chuyền: {:?}", e);
                            }
                        }
                    } else {
                        warn!("❌ Thẻ không hợp lệ! Bỏ qua lệnh E-STOP.");
                    }

                    // Tránh quẹt lặp lại
                    thread::sleep(Duration::from_secs(2));
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    });

    // ── 5. Khởi tạo UDP Socket để nhận lệnh từ Laptop ──
    let port = 8888;
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;
    let mut buf = [0u8; 8]; 

    info!("Listening on UDP port {}...", port);

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _src)) => {
                if size == 0 {
                    continue;
                }

                let opcode = buf[0];

                match opcode {
                    // ── Opcode 0x01: Mosquito classification command ──
                    // Format: [0x01, R, G, B, class_id] (5 bytes)
                    0x01 if size >= 5 => {
                        let r = buf[1];
                        let g = buf[2];
                        let b = buf[3];
                        let class_id = buf[4];

                        info!("OP 0x01 Classify — RGB({},{},{}) class_id={}", r, g, b, class_id);

                        // Cập nhật đèn LED WS2812
                        let _ = ws2812.write(std::iter::once(RGB8::new(r, g, b)));

                        // Cập nhật góc quay đĩa phân loại
                        if class_id != CLASS_ID_NONE && (class_id as u32) < NUM_SPECIES {
                            let target_angle = (class_id as u32 * DEGREES_PER_SPECIES) as i32;
                            if let Err(e) = sorter.rotate_to(target_angle) {
                                error!("Lỗi khi xoay đĩa phân loại: {:?}", e);
                            }
                        }
                    }

                    // ── Opcode 0x02: Absolute rotation ──
                    // Format: [0x02, angle_high, angle_low] (3 bytes)
                    0x02 if size >= 3 => {
                        let angle = ((buf[1] as i16) << 8) | (buf[2] as i16);
                        info!("OP 0x02 Rotate — angle={}", angle);

                        if let Err(e) = sorter.rotate_to(angle as i32) {
                            error!("Lỗi khi xoay tuyệt đối: {:?}", e);
                        }
                    }

                    // ── Opcode 0x03: Reset to home position ──
                    // Format: [0x03] (1 byte)
                    0x03 => {
                        info!("OP 0x03 Reset Home");

                        if let Err(e) = sorter.reset_to_home() {
                            error!("Lỗi khi reset về vị trí gốc: {:?}", e);
                        }
                    }

                    // ── Backwards compatibility: legacy 4-byte format ──
                    // Format: [R, G, B, class_id] (exactly 4 bytes, first byte is NOT a known opcode)
                    _ if size == 4 => {
                        let r = buf[0];
                        let g = buf[1];
                        let b = buf[2];
                        let class_id = buf[3];

                        info!("Legacy 4-byte — RGB({},{},{}) class_id={}", r, g, b, class_id);

                        // Cập nhật đèn LED WS2812
                        let _ = ws2812.write(std::iter::once(RGB8::new(r, g, b)));

                        // Cập nhật góc quay đĩa phân loại
                        if class_id != CLASS_ID_NONE && (class_id as u32) < NUM_SPECIES {
                            let target_angle = (class_id as u32 * DEGREES_PER_SPECIES) as i32;
                            if let Err(e) = sorter.rotate_to(target_angle) {
                                error!("Lỗi khi xoay đĩa phân loại: {:?}", e);
                            }
                        }
                    }

                    _ => {
                        warn!("Unknown opcode 0x{:02X} with size {}", opcode, size);
                    }
                }
            }
            Err(e) => {
                warn!("UDP Receive Error: {}", e);
            }
        }
    }
}

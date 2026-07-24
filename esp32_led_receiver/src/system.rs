use esp_idf_hal::ledc::{LedcDriver, LedcTimerDriver, config::TimerConfig};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::Hertz;
use esp_idf_hal::spi::{config::DriverConfig, SpiConfig, SpiDeviceDriver, SpiDriver};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs};
use esp_idf_svc::wifi::{AuthMethod, AccessPointConfiguration, ClientConfiguration, Configuration, EspWifi};
use esp_idf_svc::http::server::{EspHttpServer, Configuration as HttpConfig, Method};
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
const AUTHORIZED_UID: [u8; 4] = [108, 252, 202, 6];

fn url_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => result.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let h1 = bytes[i + 1] as char;
                let h2 = bytes[i + 2] as char;
                if let Ok(val) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                    result.push(val);
                    i += 2;
                } else {
                    result.push(bytes[i]);
                }
            }
            b => result.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

fn parse_form_body(body: &str) -> (String, String) {
    let mut ssid = String::new();
    let mut password = String::new();
    for pair in body.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == "ssid" {
                ssid = url_decode(v);
            } else if k == "password" {
                password = url_decode(v);
            }
        }
    }
    (ssid, password)
}

fn start_ap_webconfig_mode(
    wifi: &mut EspWifi,
    nvs: EspDefaultNvsPartition,
    ws2812: &mut Ws2812Esp32Rmt,
) -> anyhow::Result<()> {
    info!("📡 Bắt đầu chế độ Access Point (AP Mode) để cấu hình WiFi...");

    // Bật đèn LED màu Cam hiển thị chế độ AP Config
    let _ = ws2812.write(std::iter::once(RGB8::new(255, 140, 0)));

    let ap_config = AccessPointConfiguration {
        ssid: "Mosquito-Sorter-AP".try_into().unwrap(),
        channel: 1,
        auth_method: AuthMethod::None,
        ..Default::default()
    };

    let _ = wifi.stop();
    wifi.set_configuration(&Configuration::AccessPoint(ap_config))?;
    wifi.start()?;

    info!("📶 AP Mode Started! SSID: 'Mosquito-Sorter-AP'");
    info!("🌐 Hãy kết nối WiFi 'Mosquito-Sorter-AP' và truy cập: http://192.168.71.1 (hoặc IP AP mặc định)");

    let mut http_config = HttpConfig::default();
    http_config.stack_size = 10240;
    http_config.max_uri_handlers = 16;
    http_config.max_open_sockets = 7;
    http_config.max_resp_headers = 16;
    http_config.uri_match_wildcard = true;

    let mut server = EspHttpServer::new(&http_config)?;

    // Handle /favicon.ico to prevent 404 warnings
    server.fn_handler("/favicon.ico", Method::Get, |request| -> Result<(), esp_idf_svc::io::EspIOError> {
        let mut response = request.into_ok_response()?;
        response.write(b"")?;
        Ok(())
    })?;

    // GET / : Giao diện Web Cấu hình WiFi (phù hợp với mọi Captive Portal)
    server.fn_handler("/", Method::Get, |request| -> Result<(), esp_idf_svc::io::EspIOError> {
        let html = r#"<!DOCTYPE html>
<html lang="vi">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Cấu hình WiFi ESP32</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; background: #0b0c10; color: #c5c6c7; display: flex; align-items: center; justify-content: center; min-height: 100vh; padding: 20px; }
        .card { background: #1f2833; border: 1px solid #45a29e; border-radius: 16px; width: 100%; max-width: 420px; padding: 30px; box-shadow: 0 10px 25px rgba(0,0,0,0.6); text-align: center; }
        h2 { color: #66fcf1; font-size: 1.5rem; margin-bottom: 10px; }
        p { font-size: 0.9rem; color: #c5c6c7; margin-bottom: 25px; line-height: 1.4; }
        .input-group { text-align: left; margin-bottom: 20px; }
        label { display: block; font-size: 0.85rem; font-weight: 600; color: #66fcf1; margin-bottom: 6px; }
        input[type="text"], input[type="password"] { width: 100%; padding: 12px 15px; border-radius: 8px; border: 1px solid #45a29e; background: #0b0c10; color: #fff; font-size: 1rem; outline: none; transition: border-color 0.3s; }
        input:focus { border-color: #66fcf1; box-shadow: 0 0 8px rgba(102, 252, 241, 0.4); }
        button { width: 100%; padding: 14px; background: linear-gradient(135deg, #45a29e, #66fcf1); color: #0b0c10; border: none; border-radius: 8px; font-weight: 700; font-size: 1.05rem; cursor: pointer; transition: transform 0.2s, opacity 0.2s; }
        button:hover { opacity: 0.9; transform: translateY(-1px); }
        .badge { display: inline-block; background: rgba(255, 165, 0, 0.2); color: #ffa500; border: 1px solid #ffa500; border-radius: 20px; padding: 4px 12px; font-size: 0.8rem; margin-bottom: 15px; font-weight: 600; }
    </style>
</head>
<body>
    <div class="card">
        <span class="badge">📡 CHẾ ĐỘ CẤU HÌNH AP</span>
        <h2>Cấu hình WiFi ESP32</h2>
        <p>Nhập Tên (SSID) và Mật khẩu mạng WiFi để ESP32 kết nối tự động:</p>
        <form method="POST" action="/save">
            <div class="input-group">
                <label for="ssid">Tên WiFi (SSID)</label>
                <input type="text" id="ssid" name="ssid" placeholder="Nhập tên WiFi..." required />
            </div>
            <div class="input-group">
                <label for="password">Mật khẩu WiFi</label>
                <input type="password" id="password" name="password" placeholder="Nhập mật khẩu..." required />
            </div>
            <button type="submit">💾 Lưu & Kết nối</button>
        </form>
    </div>
</body>
</html>"#;
        let mut response = request.into_ok_response()?;
        response.write(html.as_bytes())?;
        Ok(())
    })?;

    // POST /save : Lưu SSID & Password vào NVS và Restart
    let nvs_clone = nvs.clone();
    server.fn_handler("/save", Method::Post, move |mut request| -> Result<(), esp_idf_svc::io::EspIOError> {
        let mut buf = [0u8; 1024];
        let len = match request.read(&mut buf) {
            Ok(n) => n,
            Err(_) => 0,
        };
        let body = String::from_utf8_lossy(&buf[..len]);
        let (ssid, password) = parse_form_body(&body);

        info!("📝 Nhận thông tin WiFi mới từ Web: SSID='{}'", ssid);

        if !ssid.is_empty() {
            if let Ok(mut nvs_handle) = EspNvs::new(nvs_clone.clone(), "wifi_creds", true) {
                let _ = nvs_handle.set_str("ssid", &ssid);
                let _ = nvs_handle.set_str("password", &password);
                info!("💾 Đã lưu SSID và Password mới vào NVS!");
            }
        }

        let html = r#"<!DOCTYPE html>
<html lang="vi">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Đã lưu cấu hình</title>
    <style>
        body { font-family: sans-serif; background: #0b0c10; color: #66fcf1; text-align: center; padding: 50px 20px; }
        .card { background: #1f2833; border-radius: 16px; max-width: 400px; margin: 0 auto; padding: 30px; border: 1px solid #45a29e; }
        h2 { margin-bottom: 15px; }
        p { color: #c5c6c7; font-size: 0.95rem; }
    </style>
</head>
<body>
    <div class="card">
        <h2>✅ Đã lưu cấu hình WiFi!</h2>
        <p>ESP32 đang khởi động lại để thử kết nối vào mạng WiFi mới...</p>
    </div>
</body>
</html>"#;
        let mut response = request.into_ok_response()?;
        response.write(html.as_bytes())?;

        thread::spawn(|| {
            thread::sleep(Duration::from_secs(2));
            unsafe { esp_idf_sys::esp_restart(); }
        });

        Ok(())
    })?;

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

/// Tầng 3: Điều phối hệ thống & Chạy tự động (System Orchestrator)
/// Quản lý việc kết nối phần cứng và khởi động các chu trình tự động.
pub fn run() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Starting ESP32 — Mosquito Sorter & Conveyor Belt (Layered Architecture)");

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // ── 0. Cấu hình Đèn LED WS2812 (Chân 48) ban đầu ──
    let led_pin = peripherals.pins.gpio48;
    let channel = peripherals.rmt.channel0;
    let mut ws2812 = Ws2812Esp32Rmt::new(channel, led_pin)?;

    // ── 1. Đọc WiFi SSID & Password từ NVS ──
    let mut nvs_handle = EspNvs::new(nvs.clone(), "wifi_creds", true).ok();
    let mut ssid_buf = [0u8; 64];
    let mut pass_buf = [0u8; 64];

    let (ssid, password) = if let Some(ref mut nvs_dev) = nvs_handle {
        let saved_ssid = nvs_dev.get_str("ssid", &mut ssid_buf).ok().flatten();
        let saved_pass = nvs_dev.get_str("password", &mut pass_buf).ok().flatten();
        if let (Some(s), Some(p)) = (saved_ssid, saved_pass) {
            info!("📖 Đọc thông tin WiFi từ NVS: SSID='{}'", s);
            (s.to_string(), p.to_string())
        } else {
            info!("ℹ️ Chưa có WiFi trong NVS, sử dụng cấu hình mặc định: SSID='Gia5'");
            ("Gia5".to_string(), "Gi@123456789".to_string())
        }
    } else {
        ("Gia5".to_string(), "Gi@123456789".to_string())
    };

    // ── 2. Thử kết nối WiFi ở Chế độ Station (STA Mode) trong 10 giây ──
    let mut wifi = EspWifi::new(peripherals.modem, sys_loop, Some(nvs.clone()))?;

    let sta_config = ClientConfiguration {
        ssid: ssid.as_str().try_into().unwrap(),
        password: password.as_str().try_into().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    };

    wifi.set_configuration(&Configuration::Client(sta_config))?;

    // Thiết lập hostname để tự động phân giải mosquito-sorter.local trên máy tính
    let hostname = std::ffi::CString::new("mosquito-sorter").unwrap();
    unsafe {
        let netif = wifi.sta_netif();
        let netif_ptr = netif.handle();
        let _ = esp_idf_sys::esp_netif_set_hostname(netif_ptr, hostname.as_ptr());
    }

    wifi.start()?;
    let _ = wifi.connect();

    info!("⏳ Đang thử kết nối vào WiFi '{}' (Thời gian chờ tối đa 10 giây)...", ssid);

    let mut connected = false;
    for i in 1..=20 {
        if wifi.is_connected()? {
            connected = true;
            break;
        }
        info!("  ...đang chờ WiFi kết nối ({}/10s)", i as f32 * 0.5);
        thread::sleep(Duration::from_millis(500));
    }

    if !connected {
        warn!("⚠️ Kết nối WiFi '{}' THẤT BẠI sau 10s!", ssid);
        // Chuyển sang AP Mode để người dùng kết nối cấu hình lại WiFi
        start_ap_webconfig_mode(&mut wifi, nvs, &mut ws2812)?;
        return Ok(());
    }

    info!("✅ WiFi Connected thành công ở STA Mode!");

    // Spawning background thread for UDP broadcast beacon (auto-discovery)
    if let Ok(beacon_socket) = UdpSocket::bind("0.0.0.0:0") {
        let _ = beacon_socket.set_broadcast(true);
        thread::spawn(move || {
            let broadcast_addr = "255.255.255.255:8889";
            let payload = b"mosquito-sorter-beacon";
            loop {
                let _ = beacon_socket.send_to(payload, broadcast_addr);
                thread::sleep(Duration::from_secs(1));
            }
        });
    }

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
            Ok(mut rfid_dev) => {
                match rfid_dev.version() {
                    Ok(v) if v != 0x00 && v != 0xFF => {
                        info!("RFID Scanner initialized successfully (Version: 0x{:02X}).", v);
                        Some(rfid_dev)
                    }
                    _ => {
                        warn!("MFRC522 not detected on SPI bus. RFID features disabled.");
                        None
                    }
                }
            }
            Err(_) => {
                warn!("Failed to initialize MFRC522! RFID features disabled.");
                None
            }
        };

        info!("Conveyor & RFID Scanner Thread Started!");

        loop {
            if let Some(ref mut reader) = rfid {
                if let Ok(atqa) = reader.reqa() {
                    if let Ok(uid) = reader.select(&atqa) {
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

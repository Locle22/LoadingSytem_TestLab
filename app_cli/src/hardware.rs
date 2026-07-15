//! Tầng 2 — Device Abstraction Layer (Hardware Backend Trait)
//!
//! Định nghĩa interface chung `HardwareBackend` cho mọi nền tảng phần cứng.
//! Hiện tại hỗ trợ 2 backend:
//! - [`Esp32Backend`]: Giao tiếp qua UDP với ESP32 S3.
//! - [`PlcBackend`]:   Stub cho PLC công nghiệp (nhóm khác phát triển sau).

use std::sync::atomic::{AtomicI32, Ordering};

use crate::iot::UdpSender;

// ──────────────────────────────────────────────
//  Trait: HardwareBackend (Tầng 2 API)
// ──────────────────────────────────────────────

/// Interface chung cho mọi nền tảng điều khiển servo phần cứng.
///
/// Tầng 3 (Orchestration) sẽ gọi các hàm này mà không cần biết
/// bên dưới là ESP32, PLC hay bất kỳ thiết bị nào khác.
pub trait HardwareBackend: Send + Sync {
    /// Tên hiển thị của backend (ví dụ: "ESP32 S3", "PLC Siemens S7").
    fn name(&self) -> &str;

    /// Gửi lệnh phân loại muỗi (bật LED + xoay Servo đến ô loài muỗi).
    fn send_mosquito_command(&self, r: u8, g: u8, b: u8, class_id: u8);

    /// Gửi lệnh xoay servo đến một góc tuyệt đối (đơn vị: độ).
    fn rotate_servo(&self, angle: i32);

    /// Gửi lệnh reset servo về vị trí ban đầu (0 độ).
    fn reset_home(&self);

    /// Lấy góc hiện tại của đĩa xoay (theo dõi phía Laptop).
    fn get_current_angle(&self) -> i32;
}

// ──────────────────────────────────────────────
//  Esp32Backend — Giao tiếp qua UDP
// ──────────────────────────────────────────────

/// Backend điều khiển servo thông qua ESP32 S3 via UDP.
///
/// Sử dụng giao thức opcode mở rộng:
/// - `0x01 [R, G, B, class_id]` — Lệnh phân loại muỗi
/// - `0x02 [angle_high, angle_low]` — Xoay tuyệt đối
/// - `0x03` — Reset về Home
pub struct Esp32Backend {
    sender: UdpSender,
    current_angle: AtomicI32,
}

impl Esp32Backend {
    /// Khởi tạo kết nối UDP đến ESP32.
    pub fn new(ip: &str, port: u16) -> anyhow::Result<Self> {
        let sender = UdpSender::new(ip, port)?;
        Ok(Self {
            sender,
            current_angle: AtomicI32::new(0),
        })
    }
}

impl HardwareBackend for Esp32Backend {
    fn name(&self) -> &str {
        "ESP32 S3"
    }

    fn send_mosquito_command(&self, r: u8, g: u8, b: u8, class_id: u8) {
        self.sender.send_opcode_mosquito(r, g, b, class_id);
        if class_id != 0xFF && (class_id as u32) < 36 {
            let angle = (class_id as i32) * 10;
            self.current_angle.store(angle, Ordering::SeqCst);
        }
    }

    fn rotate_servo(&self, angle: i32) {
        self.sender.send_opcode_rotate(angle);
        self.current_angle.store(angle, Ordering::SeqCst);
    }

    fn reset_home(&self) {
        self.sender.send_opcode_reset();
        self.current_angle.store(0, Ordering::SeqCst);
    }

    fn get_current_angle(&self) -> i32 {
        self.current_angle.load(Ordering::SeqCst)
    }
}

// ──────────────────────────────────────────────
//  PlcBackend — Stub cho PLC công nghiệp
// ──────────────────────────────────────────────

/// Backend stub cho PLC công nghiệp.
///
/// Mọi phương thức chỉ ghi log ra console.
/// Nhóm phát triển PLC sẽ cài đặt giao thức Modbus/TCP sau.
pub struct PlcBackend {
    current_angle: AtomicI32,
}

impl PlcBackend {
    pub fn new() -> Self {
        println!("  ⚙ Chế độ điều khiển PLC công nghiệp");
        Self {
            current_angle: AtomicI32::new(0),
        }
    }
}

impl HardwareBackend for PlcBackend {
    fn name(&self) -> &str {
        "PLC công nghiệp"
    }

    fn send_mosquito_command(&self, r: u8, g: u8, b: u8, class_id: u8) {
        println!(
            "  [PLC] Lệnh phân loại muỗi: RGB=({},{},{}), class_id={}",
            r, g, b, class_id
        );
    }

    fn rotate_servo(&self, angle: i32) {
        println!("  [PLC] Xoay servo đến góc: {}°", angle);
        self.current_angle.store(angle, Ordering::SeqCst);
    }

    fn reset_home(&self) {
        println!("  [PLC] Reset servo về vị trí ban đầu (0°)");
        self.current_angle.store(0, Ordering::SeqCst);
    }

    fn get_current_angle(&self) -> i32 {
        self.current_angle.load(Ordering::SeqCst)
    }
}

// ──────────────────────────────────────────────
//  DisconnectedBackend — Chế độ mất kết nối ESP32
// ──────────────────────────────────────────────

/// Backend dự phòng khi không thể kết nối tới ESP32 S3 thật.
pub struct DisconnectedBackend {
    current_angle: AtomicI32,
}

impl DisconnectedBackend {
    pub fn new() -> Self {
        println!("  ⚠ Chế độ điều khiển phần cứng tạm thời VÔ HIỆU HÓA do mất kết nối ESP32 S3.");
        Self {
            current_angle: AtomicI32::new(0),
        }
    }
}

impl HardwareBackend for DisconnectedBackend {
    fn name(&self) -> &str {
        "ESP32 S3 (Mất kết nối)"
    }

    fn send_mosquito_command(&self, _r: u8, _g: u8, _b: u8, _class_id: u8) {}

    fn rotate_servo(&self, _angle: i32) {}

    fn reset_home(&self) {}

    fn get_current_angle(&self) -> i32 {
        self.current_angle.load(Ordering::SeqCst)
    }
}


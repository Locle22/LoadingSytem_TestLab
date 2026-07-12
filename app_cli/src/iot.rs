use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::UdpSocket;

// ──────────────────────────────────────────────
//  Giao thức UDP: [R, G, B, class_id] — 4 bytes
// ──────────────────────────────────────────────

/// Mã class_id đặc biệt: Không phát hiện muỗi nào → tắt LED, servo giữ nguyên.
pub const CLASS_ID_NONE: u8 = 0xFF;

/// Giao thức gửi gói tin UDP không chờ (Non-blocking UDP Sender)
pub struct UdpSender {
    socket: UdpSocket,
    target_addr: String,
}

impl UdpSender {
    /// Khởi tạo kết nối đến ESP32 S3.
    /// `ip`: Địa chỉ IP của ESP32 (vd: "192.168.1.10")
    pub fn new(ip: &str, port: u16) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?; // Bind ngẫu nhiên 1 port của hệ điều hành
        socket.set_nonblocking(true)?; // Quan trọng: Đảm bảo không block Thread AI

        let target_addr = format!("{}:{}", ip, port);
        Ok(Self {
            socket,
            target_addr,
        })
    }

    /// Gửi lệnh điều khiển LED + Servo sang ESP32.
    ///
    /// Giao thức 4 byte: `[R, G, B, class_id]`
    /// - `R, G, B`: Mã màu đèn LED cảnh báo
    /// - `class_id`: Chỉ `0xFF` = không phát hi số loài muỗi (0..35), ESP32 sẽ xoay servo đến góc `class_id * 10`
    ///   Giá trị đặc biệtện → tắt LED, servo giữ nguyên.
    pub fn send_command(&self, r: u8, g: u8, b: u8, class_id: u8) {
        let payload = [r, g, b, class_id];
        // Fire-and-forget: Bắn gói tin và bỏ qua lỗi mạng.
        let _ = self.socket.send_to(&payload, &self.target_addr);
    }
}

/// Chuyển đổi ID loài muỗi (0..35) thành mã màu RGB.
/// - Mỗi loài sẽ có một màu sắc duy nhất dựa trên vòng tròn màu HSV (Hue phân bổ đều 360 độ).
pub fn get_species_color(class_id: u8) -> (u8, u8, u8) {
    // 36 loài -> Mỗi loài cách nhau 10 độ trên vòng tròn màu
    let hue = (class_id as f32 * 10.0) % 360.0;
    
    // Saturation = 1.0, Value = 1.0 (Màu rực rỡ nhất)
    let c = 1.0;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = 0.0;

    let (r_prime, g_prime, b_prime) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r_prime + m) * 255.0) as u8,
        ((g_prime + m) * 255.0) as u8,
        ((b_prime + m) * 255.0) as u8,
    )
}

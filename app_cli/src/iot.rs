use std::net::{UdpSocket, SocketAddr};

// ──────────────────────────────────────────────
//  Giao thức UDP: [R, G, B, class_id] — 4 bytes
// ──────────────────────────────────────────────

/// Mã class_id đặc biệt: Không phát hiện muỗi nào → tắt LED, servo giữ nguyên.
pub const CLASS_ID_NONE: u8 = 0xFF;

/// Giao thức gửi gói tin UDP không chờ (Non-blocking UDP Sender)
pub struct UdpSender {
    socket: UdpSocket,
    target_addr: SocketAddr,
}

impl UdpSender {
    /// Khởi tạo kết nối đến ESP32 S3.
    /// `ip`: Địa chỉ IP hoặc Hostname của ESP32 (vd: "mosquito-sorter.local")
    pub fn new(ip: &str, port: u16) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?; // Bind ngẫu nhiên 1 port của hệ điều hành
        socket.set_nonblocking(true)?; // Quan trọng: Đảm bảo không block Thread AI

        use std::net::ToSocketAddrs;
        let addr_str = format!("{}:{}", ip, port);
        let target_addr = addr_str.to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow::anyhow!("Không thể phân giải địa chỉ: {}", addr_str))?;

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
    /// Gửi lệnh phân loại muỗi bằng Opcode 0x01.
    /// Giao thức 5 byte: `[0x01, R, G, B, class_id]`
    pub fn send_opcode_mosquito(&self, r: u8, g: u8, b: u8, class_id: u8) {
        let payload = [0x01, r, g, b, class_id];
        let _ = self.socket.send_to(&payload, &self.target_addr);
    }

    /// Gửi lệnh xoay tuyệt đối bằng Opcode 0x02.
    /// Giao thức 3 byte: `[0x02, angle_high, angle_low]`
    pub fn send_opcode_rotate(&self, angle: i32) {
        let angle_u16 = (angle as i16) as u16;
        let high = (angle_u16 >> 8) as u8;
        let low = (angle_u16 & 0xFF) as u8;
        let payload = [0x02, high, low];
        let _ = self.socket.send_to(&payload, &self.target_addr);
    }

    /// Gửi lệnh reset về Home bằng Opcode 0x03.
    /// Giao thức 1 byte: `[0x03]`
    pub fn send_opcode_reset(&self) {
        let payload = [0x03];
        let _ = self.socket.send_to(&payload, &self.target_addr);
    }

    /// Gửi lệnh điều khiển LED + Servo sang ESP32 (Giao thức cũ 4-byte).
    pub fn send_command(&self, r: u8, g: u8, b: u8, class_id: u8) {
        let payload = [r, g, b, class_id];
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

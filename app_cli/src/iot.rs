use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::UdpSocket;

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

    /// Gửi 3 byte màu [R, G, B] sang ESP32.
    pub fn send_color(&self, r: u8, g: u8, b: u8) {
        let payload = [r, g, b];
        // Bắn gói tin và bỏ qua lỗi (Fire-and-forget), không cần thiết làm sập ứng dụng nếu mạng lag.
        let _ = self.socket.send_to(&payload, &self.target_addr);
    }
}

/// Chuyển đổi tên loài muỗi thành mã màu RGB.
/// - Các loài muỗi cực kỳ nguy hiểm (Aedes, Culex, Anopheles) sẽ được gán màu riêng biệt.
/// - Các loài muỗi khác sẽ được tạo mã màu ngẫu nhiên nhưng CỐ ĐỊNH dựa trên Hash.
pub fn get_species_color(species_name: &str) -> (u8, u8, u8) {
    let name_lower = species_name.to_lowercase();

    // Nhóm 1: Muỗi vằn (Truyền sốt xuất huyết) -> CẢNH BÁO ĐỎ
    if name_lower.contains("aedes") {
        return (255, 0, 0); // Đỏ
    }
    // Nhóm 2: Muỗi Culex (Truyền viêm não Nhật Bản) -> CẢNH BÁO XANH DƯƠNG
    else if name_lower.contains("culex") {
        return (0, 0, 255); // Xanh dương
    }
    // Nhóm 3: Muỗi Anopheles (Truyền sốt rét) -> CẢNH BÁO XANH LÁ
    else if name_lower.contains("anopheles") {
        return (0, 255, 0); // Xanh lá
    }

    // Nhóm 4: Các loài muỗi khác (Tạo mã màu ngẫu nhiên qua Hash)
    let mut hasher = DefaultHasher::new();
    species_name.hash(&mut hasher);
    let hash_val = hasher.finish();

    // Trích xuất 3 byte cuối từ mã Hash để làm dải R, G, B
    let r = (hash_val & 0xFF) as u8;
    let g = ((hash_val >> 8) & 0xFF) as u8;
    let b = ((hash_val >> 16) & 0xFF) as u8;

    // Giới hạn độ sáng (để tránh LED quá chói hoặc tối thui, ta ép sáng)
    let r = r.max(50).min(200);
    let g = g.max(50).min(200);
    let b = b.max(50).min(200);

    (r, g, b)
}

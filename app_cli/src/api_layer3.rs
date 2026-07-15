//! Tầng 3 — High-Level Orchestration API
//!
//! Các hàm nghiệp vụ cấp cao để điều khiển đĩa xoay phân loại muỗi.
//! Mọi hàm trong module này đều gọi xuống Tầng 2 (`HardwareBackend`).

use crate::hardware::HardwareBackend;
use crate::iot::get_species_color;

// ──────────────────────────────────────────────
//  Hằng số đĩa xoay
// ──────────────────────────────────────────────

/// Tổng số ô trên đĩa xoay.
pub const NUM_SLOTS: usize = 36;

/// Góc mỗi ô chiếm trên đĩa (360° / 36 ô = 10°/ô).
pub const DEGREES_PER_SLOT: i32 = 360 / NUM_SLOTS as i32;

// ──────────────────────────────────────────────
//  Đặt tên ô (Slot Naming): A-Z, A0-J0
// ──────────────────────────────────────────────

/// Trả về tên hiển thị của một ô dựa trên chỉ số (0-based).
///
/// Quy tắc đặt tên:
/// - Ô 0..25: A, B, C, ..., Z
/// - Ô 26..35: A0, B0, C0, ..., J0
/// - (Mở rộng tương lai: A1..Z1, A2..Z2, ...)
pub fn slot_name(index: usize) -> String {
    if index < 26 {
        // A..Z
        let ch = (b'A' + index as u8) as char;
        ch.to_string()
    } else {
        // Tính số thế hệ (generation) và vị trí trong thế hệ đó
        let remaining = index - 26;
        let generation = remaining / 26;
        let pos = remaining % 26;
        let ch = (b'A' + pos as u8) as char;
        format!("{}{}", ch, generation)
    }
}

/// Chuyển đổi tên ô thành chỉ số (0-based). Trả về `None` nếu không hợp lệ.
///
/// Ví dụ: "A" → 0, "Z" → 25, "A0" → 26, "J0" → 35
pub fn slot_index(name: &str) -> Option<usize> {
    let name = name.trim().to_uppercase();
    let chars: Vec<char> = name.chars().collect();

    if chars.len() == 1 && chars[0].is_ascii_uppercase() {
        // A..Z → 0..25
        Some((chars[0] as u8 - b'A') as usize)
    } else if chars.len() == 2 && chars[0].is_ascii_uppercase() && chars[1].is_ascii_digit() {
        // A0..Z0 → 26..51, A1..Z1 → 52..77, ...
        let pos = (chars[0] as u8 - b'A') as usize;
        let gen = (chars[1] as u8 - b'0') as usize;
        let index = 26 + gen * 26 + pos;
        Some(index)
    } else {
        None
    }
}

/// Tính góc mặc định (vị trí ban đầu) của một ô trên đĩa xoay.
pub fn slot_angle(slot_index: usize) -> i32 {
    (slot_index as i32) * DEGREES_PER_SLOT
}

/// Trả về danh sách tất cả các ô kèm tên và góc.
pub fn all_slots() -> Vec<(usize, String, i32)> {
    (0..NUM_SLOTS)
        .map(|i| (i, slot_name(i), slot_angle(i)))
        .collect()
}

// ──────────────────────────────────────────────
//  API Tầng 3 — Orchestration Functions
// ──────────────────────────────────────────────

/// Đưa đĩa xoay về vị trí ban đầu (0 độ).
pub fn reset_home(backend: &dyn HardwareBackend) {
    backend.reset_home();
}

/// Xoay đĩa sang trái (ngược chiều kim đồng hồ) N độ.
pub fn rotate_left(backend: &dyn HardwareBackend, n_degrees: i32) {
    let current = backend.get_current_angle();
    let new_angle = normalize_angle(current - n_degrees);
    backend.rotate_servo(new_angle);
}

/// Xoay đĩa sang phải (thuận chiều kim đồng hồ) N độ.
pub fn rotate_right(backend: &dyn HardwareBackend, n_degrees: i32) {
    let current = backend.get_current_angle();
    let new_angle = normalize_angle(current + n_degrees);
    backend.rotate_servo(new_angle);
}

/// Xoay một ô cụ thể (ví dụ: ô "B") đến một góc bất kỳ.
///
/// Cách hoạt động: Tính khoảng cách cần xoay = `target_angle - slot_angle(slot)`,
/// sau đó xoay toàn bộ đĩa bằng đúng khoảng cách đó.
pub fn rotate_slot_to_angle(backend: &dyn HardwareBackend, slot_idx: usize, target_angle: i32) {
    let current_disk_angle = backend.get_current_angle();
    let original_slot_angle = slot_angle(slot_idx);

    // Vị trí thực tế hiện tại của ô = vị trí gốc + góc đĩa hiện tại
    let actual_slot_pos = normalize_angle(original_slot_angle + current_disk_angle);
    let delta = shortest_rotation(actual_slot_pos, target_angle);

    let new_disk_angle = normalize_angle(current_disk_angle + delta);
    backend.rotate_servo(new_disk_angle);
}

/// Di chuyển ô `from` (ví dụ: "C") đến vị trí hiện tại của ô `to` (ví dụ: "B").
///
/// Cách hoạt động:
/// - Tính vị trí gốc của ô `to` trên đĩa.
/// - Tính khoảng cách giữa 2 ô.
/// - Chọn chiều quay ngắn nhất.
pub fn move_slot_to_slot(backend: &dyn HardwareBackend, from_idx: usize, to_idx: usize) {
    let from_angle = slot_angle(from_idx);
    let to_angle = slot_angle(to_idx);

    // Khoảng cách giữa 2 ô (trên đĩa cố định, không phụ thuộc góc quay hiện tại)
    let delta = shortest_rotation(from_angle, to_angle);

    let current = backend.get_current_angle();
    let new_angle = normalize_angle(current + delta);
    backend.rotate_servo(new_angle);
}

/// Gửi lệnh mô phỏng nhận diện muỗi giả (cho chế độ Simulation).
pub fn simulate_mosquito(backend: &dyn HardwareBackend, class_id: u8) {
    let (r, g, b) = get_species_color(class_id);
    backend.send_mosquito_command(r, g, b, class_id);
}

// ──────────────────────────────────────────────
//  Helpers
// ──────────────────────────────────────────────

/// Chuẩn hóa góc về khoảng [0, 360).
fn normalize_angle(angle: i32) -> i32 {
    ((angle % 360) + 360) % 360
}

/// Tính chiều quay ngắn nhất từ `from` đến `to` (kết quả trong [-180, 180]).
fn shortest_rotation(from: i32, to: i32) -> i32 {
    let mut delta = normalize_angle(to) - normalize_angle(from);
    if delta > 180 {
        delta -= 360;
    }
    if delta < -180 {
        delta += 360;
    }
    delta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_naming() {
        assert_eq!(slot_name(0), "A");
        assert_eq!(slot_name(25), "Z");
        assert_eq!(slot_name(26), "A0");
        assert_eq!(slot_name(35), "J0");
    }

    #[test]
    fn test_slot_index_lookup() {
        assert_eq!(slot_index("A"), Some(0));
        assert_eq!(slot_index("Z"), Some(25));
        assert_eq!(slot_index("A0"), Some(26));
        assert_eq!(slot_index("J0"), Some(35));
        assert_eq!(slot_index("invalid"), None);
    }

    #[test]
    fn test_normalize_angle() {
        assert_eq!(normalize_angle(370), 10);
        assert_eq!(normalize_angle(-10), 350);
        assert_eq!(normalize_angle(0), 0);
        assert_eq!(normalize_angle(360), 0);
    }

    #[test]
    fn test_shortest_rotation() {
        // 10° → 350° = quay trái 20° (ngắn hơn quay phải 340°)
        assert_eq!(shortest_rotation(10, 350), -20);
        // 350° → 10° = quay phải 20°
        assert_eq!(shortest_rotation(350, 10), 20);
    }
}

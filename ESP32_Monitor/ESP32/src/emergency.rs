// =============================================================================
// ESP32/src/emergency.rs
// =============================================================================
// Xử lý tín hiệu dừng khẩn cấp phần cứng qua GPIO4 (FR-12).
//
// GPIO4 được cấu hình là input pull-up. Khi tín hiệu kéo xuống LOW,
// nút dừng khẩn cấp được kích hoạt → toàn bộ motor phải dừng ngay.
//
// Trong phiên bản hiện tại, sử dụng phương pháp polling (kiểm tra mỗi vòng lặp).
// Phiên bản production nên sử dụng interrupt (ngắt) để phản hồi nhanh hơn.
// =============================================================================

use esp_println::println;

/// Chân GPIO dùng cho tín hiệu dừng khẩn cấp phần cứng.
/// GPIO4 trên ESP32-S3 được chọn vì dễ truy cập trên các development board.
pub const EMERGENCY_STOP_GPIO: u8 = 4;

/// Trạng thái nút dừng khẩn cấp phần cứng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmergencyStopState {
    /// Bình thường — không có tín hiệu dừng khẩn cấp.
    Normal,
    /// Đã kích hoạt — nút dừng khẩn cấp đang được nhấn.
    Activated,
}

/// Bộ quản lý dừng khẩn cấp phần cứng.
///
/// Theo dõi trạng thái GPIO4 và phát hiện cạnh xuống (falling edge)
/// để kích hoạt e-stop. Có tích hợp debounce đơn giản.
///
/// # Cách sử dụng
/// ```ignore
/// let mut estop = HardwareEmergencyStop::new();
/// // Trong vòng lặp chính:
/// if estop.check_and_update(gpio4_is_low) {
///     // Kích hoạt e-stop cho tất cả motor
/// }
/// ```
pub struct HardwareEmergencyStop {
    /// Trạng thái hiện tại của nút e-stop.
    state: EmergencyStopState,
    /// Trạng thái GPIO lần đọc trước (dùng cho phát hiện cạnh).
    previous_pin_low: bool,
    /// Bộ đếm debounce để lọc nhiễu nút nhấn.
    debounce_counter: u8,
}

/// Số lần đọc liên tiếp cần thiết để xác nhận thay đổi trạng thái (debounce).
/// Giúp lọc nhiễu cơ khí từ nút nhấn.
const DEBOUNCE_THRESHOLD: u8 = 3;

impl HardwareEmergencyStop {
    /// Tạo bộ quản lý e-stop mới, trạng thái ban đầu: Normal.
    ///
    /// # Lưu ý
    /// GPIO4 cần được cấu hình là input pull-up ở bên ngoài (trong main.rs).
    /// Module này chỉ xử lý logic trạng thái, không cấu hình GPIO trực tiếp.
    pub fn new() -> Self {
        println!(
            "[E-STOP] Khởi tạo hardware e-stop trên GPIO{} (pull-up, active-low)",
            EMERGENCY_STOP_GPIO
        );
        Self {
            state: EmergencyStopState::Normal,
            previous_pin_low: false,
            debounce_counter: 0,
        }
    }

    /// Kiểm tra và cập nhật trạng thái e-stop dựa trên mức GPIO.
    ///
    /// # Arguments
    /// * `pin_is_low` - `true` nếu GPIO4 đang ở mức LOW (nút được nhấn).
    ///
    /// # Returns
    /// `true` nếu phát hiện kích hoạt e-stop mới (cạnh xuống sau debounce).
    /// `false` nếu không có thay đổi hoặc đang trong quá trình debounce.
    pub fn check_and_update(&mut self, pin_is_low: bool) -> bool {
        if pin_is_low {
            // GPIO LOW → nút đang được nhấn (active-low)
            if self.debounce_counter < DEBOUNCE_THRESHOLD {
                self.debounce_counter += 1;
            }

            // Đã vượt ngưỡng debounce và chưa kích hoạt → kích hoạt e-stop
            if self.debounce_counter >= DEBOUNCE_THRESHOLD
                && self.state == EmergencyStopState::Normal
            {
                self.state = EmergencyStopState::Activated;
                println!("[E-STOP] !!! HARDWARE E-STOP ACTIVATED (GPIO{} LOW) !!!", EMERGENCY_STOP_GPIO);
                return true;
            }
        } else {
            // GPIO HIGH → nút không được nhấn
            self.debounce_counter = 0;
            // Không tự động reset — cần gọi reset() rõ ràng từ Host
        }

        self.previous_pin_low = pin_is_low;
        false
    }

    /// Lấy trạng thái hiện tại của nút e-stop.
    pub fn get_state(&self) -> EmergencyStopState {
        self.state
    }

    /// Kiểm tra e-stop có đang kích hoạt không.
    pub fn is_activated(&self) -> bool {
        self.state == EmergencyStopState::Activated
    }

    /// Reset trạng thái e-stop về Normal.
    /// Chỉ được gọi khi:
    /// 1. Nút e-stop đã được nhả ra (GPIO HIGH)
    /// 2. Host gửi lệnh reset rõ ràng
    ///
    /// # Returns
    /// `true` nếu reset thành công, `false` nếu nút vẫn đang được nhấn.
    pub fn reset(&mut self, pin_is_low: bool) -> bool {
        if pin_is_low {
            // Nút vẫn đang được nhấn — không cho phép reset
            println!("[E-STOP] Không thể reset: nút vẫn đang nhấn");
            return false;
        }

        self.state = EmergencyStopState::Normal;
        self.debounce_counter = 0;
        self.previous_pin_low = false;
        println!("[E-STOP] Đã reset — trạng thái: Normal");
        true
    }
}

// =============================================================================
// RaspberryPi/src/emergency.rs
// =============================================================================
// Xử lý dừng khẩn cấp (Emergency Stop) qua GPIO, độc lập với SPI (FR-12).
//
// GPIO25 được sử dụng làm tín hiệu E-Stop output:
//   - HIGH (3.3V) = hoạt động bình thường
//   - LOW  (0V)   = dừng khẩn cấp kích hoạt
//
// Module này hoạt động độc lập với giao thức SPI, đảm bảo rằng
// ngay cả khi SPI bị treo, tín hiệu E-Stop vẫn được kích hoạt
// qua đường GPIO trực tiếp tới phần cứng an toàn.
// =============================================================================

use rppal::gpio::{Gpio, OutputPin};

// ── Hằng số cấu hình GPIO E-Stop ────────────────────────────────────────────

/// Chân GPIO dùng cho tín hiệu Emergency Stop output.
/// GPIO25 trên Raspberry Pi 4 (pin vật lý 22).
const ESTOP_GPIO_PIN: u8 = 25;

// ── Error Type ───────────────────────────────────────────────────────────────

/// Lỗi liên quan đến xử lý Emergency Stop qua GPIO.
#[derive(Debug, thiserror::Error)]
pub enum EmergencyError {
    /// Không thể khởi tạo GPIO subsystem.
    #[error("Lỗi GPIO: {0}")]
    GpioError(String),
}

// ── Emergency Stop Handler ───────────────────────────────────────────────────

/// Bộ xử lý tín hiệu Emergency Stop qua GPIO.
///
/// Hoạt động độc lập với SPI — khi E-Stop được kích hoạt,
/// GPIO25 chuyển sang LOW để báo hiệu cho mạch an toàn phần cứng
/// (relay, contactor) ngắt nguồn motor ngay lập tức.
///
/// # Nguyên lý an toàn
/// - Trạng thái mặc định: HIGH (bình thường)
/// - Kích hoạt E-Stop: LOW (fail-safe — mất điện = dừng)
/// - Giải phóng: phải gọi `release()` chủ động
pub struct EmergencyStopHandler {
    /// Chân GPIO output cho tín hiệu E-Stop.
    pin: OutputPin,
    /// Trạng thái E-Stop hiện tại: true = đang kích hoạt.
    is_active: bool,
}

impl EmergencyStopHandler {
    /// Khởi tạo Emergency Stop handler.
    ///
    /// Đặt GPIO25 ở mức HIGH (hoạt động bình thường).
    ///
    /// # Errors
    /// Trả về `EmergencyError::GpioError` nếu không thể truy cập GPIO
    /// (ví dụ: chạy trên máy không phải RPi, hoặc thiếu quyền).
    pub fn new() -> Result<Self, EmergencyError> {
        let gpio = Gpio::new().map_err(|e| {
            EmergencyError::GpioError(format!("Không thể khởi tạo GPIO: {}", e))
        })?;

        let mut pin = gpio.get(ESTOP_GPIO_PIN).map_err(|e| {
            EmergencyError::GpioError(format!(
                "Không thể lấy GPIO pin {}: {}",
                ESTOP_GPIO_PIN, e
            ))
        })?.into_output();

        // Trạng thái mặc định: HIGH = bình thường
        pin.set_high();
        log::info!(
            "Emergency Stop handler khởi tạo thành công trên GPIO{}. Trạng thái: BÌNH THƯỜNG (HIGH)",
            ESTOP_GPIO_PIN
        );

        Ok(Self {
            pin,
            is_active: false,
        })
    }

    /// Kích hoạt Emergency Stop qua GPIO.
    ///
    /// Chuyển GPIO25 sang LOW để báo hiệu dừng khẩn cấp cho phần cứng.
    /// Phương thức này là idempotent — gọi nhiều lần không gây lỗi.
    ///
    /// # Hành vi
    /// - GPIO25 → LOW (kích hoạt relay/contactor ngắt nguồn)
    /// - Log cảnh báo ở mức ERROR (luôn hiển thị)
    /// - Đặt cờ `is_active = true`
    pub fn activate(&mut self) {
        self.pin.set_low();
        self.is_active = true;
        log::error!(
            "!!! EMERGENCY STOP KÍCH HOẠT !!! GPIO{} → LOW. Motor sẽ bị ngắt nguồn.",
            ESTOP_GPIO_PIN
        );
    }

    /// Giải phóng Emergency Stop (cho phép hoạt động trở lại).
    ///
    /// Chuyển GPIO25 về HIGH. Chỉ nên gọi sau khi đã xác nhận
    /// nguyên nhân gây dừng khẩn cấp đã được xử lý.
    ///
    /// # Hành vi
    /// - GPIO25 → HIGH (cho phép relay/contactor đóng lại)
    /// - Log ở mức WARN (cảnh báo: hệ thống đang khôi phục)
    /// - Đặt cờ `is_active = false`
    pub fn release(&mut self) {
        self.pin.set_high();
        self.is_active = false;
        log::warn!(
            "Emergency Stop đã giải phóng. GPIO{} → HIGH. Hệ thống có thể hoạt động trở lại.",
            ESTOP_GPIO_PIN
        );
    }

    /// Kiểm tra Emergency Stop có đang kích hoạt không.
    pub fn is_active(&self) -> bool {
        self.is_active
    }
}

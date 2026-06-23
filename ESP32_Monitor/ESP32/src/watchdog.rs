// =============================================================================
// ESP32/src/watchdog.rs
// =============================================================================
// Watchdog phần mềm để phát hiện mất liên lạc với Host (FR-15).
//
// Nếu Node không nhận được lệnh hợp lệ nào trong WATCHDOG_TIMEOUT_MS,
// Node PHẢI tự động dừng khẩn cấp toàn bộ motor.
//
// Sử dụng cách tiếp cận dựa trên bộ đếm (counter-based) vì không có
// timer phần cứng trực tiếp trong môi trường no_std đơn giản.
// Trong thực tế, nên sử dụng esp-hal timer peripheral cho độ chính xác cao.
// =============================================================================

use esp_println::println;
use spi_protocol::config::WATCHDOG_TIMEOUT_MS;

/// Số tick tương đương 1 ms trong vòng lặp chính.
/// Giá trị này cần được hiệu chỉnh (calibrate) theo tốc độ thực tế
/// của vòng lặp chính trên phần cứng ESP32-S3.
/// Mặc định: 1 tick = 1ms (giả định vòng lặp chạy mỗi ~1ms).
const TICKS_PER_MS: u64 = 1;

/// Ngưỡng timeout tính theo số tick.
const WATCHDOG_TIMEOUT_TICKS: u64 = WATCHDOG_TIMEOUT_MS * TICKS_PER_MS;

/// Watchdog phần mềm theo dõi thời gian kể từ lệnh hợp lệ cuối cùng (FR-15).
///
/// # Cách hoạt động
/// - Mỗi khi nhận được lệnh hợp lệ, gọi `feed()` để reset bộ đếm.
/// - Mỗi vòng lặp chính, gọi `tick()` để tăng bộ đếm.
/// - Gọi `is_expired()` để kiểm tra watchdog đã hết hạn chưa.
///
/// # Lưu ý
/// Trong production, nên thay bộ đếm bằng timer phần cứng ESP32-S3
/// để đảm bảo độ chính xác thời gian.
pub struct SoftwareWatchdog {
    /// Bộ đếm tick kể từ lần feed() cuối cùng.
    ticks_since_last_feed: u64,
    /// Watchdog đã được kích hoạt (bắt đầu đếm) hay chưa.
    enabled: bool,
    /// Watchdog đã hết hạn và đã kích hoạt e-stop.
    triggered: bool,
}

impl SoftwareWatchdog {
    /// Tạo watchdog mới, mặc định chưa kích hoạt.
    /// Watchdog sẽ được kích hoạt khi nhận lệnh đầu tiên từ Host.
    pub fn new() -> Self {
        println!("[WDT] Watchdog khởi tạo — timeout: {}ms", WATCHDOG_TIMEOUT_MS);
        Self {
            ticks_since_last_feed: 0,
            enabled: false,
            triggered: false,
        }
    }

    /// Kích hoạt watchdog và reset bộ đếm.
    /// Gọi khi nhận được lệnh đầu tiên từ Host.
    pub fn start(&mut self) {
        self.enabled = true;
        self.triggered = false;
        self.ticks_since_last_feed = 0;
        println!("[WDT] Watchdog đã kích hoạt");
    }

    /// Reset bộ đếm watchdog — "cho chó ăn" (feed the watchdog).
    /// Gọi mỗi khi nhận được lệnh hợp lệ từ Host.
    pub fn feed(&mut self) {
        self.ticks_since_last_feed = 0;
        self.triggered = false;
    }

    /// Tăng bộ đếm tick. Gọi mỗi vòng lặp chính.
    ///
    /// # Arguments
    /// * `elapsed_ms` - Số millisecond đã trôi qua kể từ lần tick trước.
    ///   Trong trường hợp đơn giản, truyền 1 nếu mỗi vòng lặp ≈ 1ms.
    pub fn tick(&mut self, elapsed_ms: u64) {
        if self.enabled && !self.triggered {
            self.ticks_since_last_feed += elapsed_ms;
        }
    }

    /// Kiểm tra watchdog đã hết hạn chưa (FR-15).
    ///
    /// Trả về `true` nếu:
    /// - Watchdog đã được kích hoạt (enabled)
    /// - Thời gian kể từ lần feed cuối vượt quá WATCHDOG_TIMEOUT_MS
    /// - Chưa được xử lý (triggered lần đầu)
    ///
    /// Sau khi trả về true lần đầu, đánh dấu `triggered = true`
    /// để tránh kích hoạt e-stop nhiều lần liên tục.
    pub fn is_expired(&mut self) -> bool {
        if self.enabled && !self.triggered && self.ticks_since_last_feed >= WATCHDOG_TIMEOUT_TICKS {
            self.triggered = true;
            println!(
                "[WDT] !!! WATCHDOG EXPIRED !!! Không nhận lệnh trong {}ms — kích hoạt E-Stop",
                WATCHDOG_TIMEOUT_MS
            );
            return true;
        }
        false
    }

    /// Kiểm tra watchdog đã kích hoạt chưa (đã từng hết hạn).
    pub fn has_triggered(&self) -> bool {
        self.triggered
    }

    /// Kiểm tra watchdog có đang hoạt động không.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Vô hiệu hoá watchdog hoàn toàn. Dùng khi Node bị e-stop
    /// và cần Host gửi lệnh reset rõ ràng.
    pub fn disable(&mut self) {
        self.enabled = false;
        println!("[WDT] Watchdog đã vô hiệu hoá");
    }
}

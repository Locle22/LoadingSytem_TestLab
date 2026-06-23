// =============================================================================
// spi-protocol/src/config.rs
// =============================================================================
// Tập trung toàn bộ hằng số cấu hình hệ thống (CQ-03, CQ-04).
// Không có magic number rải rác trong code.
// Mọi giá trị nghiệp vụ đều được đặt tên rõ ràng và định nghĩa tại đây.
// =============================================================================

// ── Frame Protocol Constants ─────────────────────────────────────────────────

/// Kích thước cố định của khung truyền (FR-05).
/// Cả Host và Node đều PHẢI đọc/ghi đúng số byte này cho mỗi giao dịch SPI.
pub const FRAME_SIZE: usize = 32;

/// Byte đồng bộ đầu khung (FR-06).
/// Giá trị 0xAA (10101010 binary) dễ nhận diện trên oscilloscope
/// và giúp phát hiện lệch byte khi đồng bộ lại.
pub const START_OF_FRAME: u8 = 0xAA;

/// Kích thước tối đa vùng payload bên trong khung truyền.
/// FRAME_SIZE(32) - SOF(1) - SEQ(1) - CMD(1) - ACT_ID(1) - PLD_LEN(1) - FLAGS(1) - CRC(2) = 24
pub const PAYLOAD_MAX_SIZE: usize = 24;

// ── Actuator Configuration ───────────────────────────────────────────────────

/// Số lượng motor (actuator) trên mỗi Node.
pub const ACTUATOR_COUNT: u8 = 2;

/// Motor 0: chỉ hỗ trợ điều khiển theo vận tốc.
pub const ACTUATOR_ID_VELOCITY_MOTOR: u8 = 0;

/// Motor 1: chỉ hỗ trợ điều khiển theo vị trí (có thể chỉnh vận tốc xoay).
pub const ACTUATOR_ID_POSITION_MOTOR: u8 = 1;

/// Actuator ID đặc biệt cho lệnh broadcast (áp dụng tất cả motor).
/// Dùng cho EmergencyStop, ControlledStop broadcast.
pub const ACTUATOR_ID_BROADCAST: u8 = 0xFF;

// ── Timing Configuration (FR-13, FR-15) ──────────────────────────────────────

/// Timeout mặc định cho mỗi lệnh gửi từ Host (milliseconds) (FR-13).
/// Host PHẢI nhận được phản hồi trong thời gian này, nếu không sẽ gửi lại.
pub const DEFAULT_TIMEOUT_MS: u64 = 100;

/// Số lần gửi lại tối đa khi không nhận được phản hồi (FR-13).
/// Sau khi vượt quá, báo lỗi `MaxRetriesExceeded` cho lớp gọi phía trên.
pub const MAX_RETRY_COUNT: u8 = 3;

/// Chu kỳ gửi heartbeat từ Host xuống Node (milliseconds).
pub const HEARTBEAT_INTERVAL_MS: u64 = 100;

/// Watchdog timeout trên Node (milliseconds) (FR-15).
/// Nếu Node không nhận lệnh hợp lệ nào trong khoảng thời gian này,
/// Node PHẢI tự động dừng khẩn cấp toàn bộ motor.
pub const WATCHDOG_TIMEOUT_MS: u64 = 500;

// ── Safety Limits (FR-19) ────────────────────────────────────────────────────
// Node PHẢI kiểm tra các giới hạn này trước khi thực thi lệnh,
// KHÔNG phụ thuộc vào việc Host gửi giá trị đúng.

/// Vận tốc tối đa cho phép (steps/s).
pub const MAX_VELOCITY_STEPS_PER_SEC: u32 = 10_000;

/// Gia tốc tối đa cho phép (steps/s²).
pub const MAX_ACCELERATION_STEPS_PER_SEC2: u32 = 50_000;

/// Giới hạn vị trí tối thiểu (steps). Giá trị âm = chiều ngược.
pub const POSITION_LIMIT_MIN: i32 = -1_000_000;

/// Giới hạn vị trí tối đa (steps).
pub const POSITION_LIMIT_MAX: i32 = 1_000_000;

// ── Frame Flags ──────────────────────────────────────────────────────────────

/// Bit mask cho cờ ưu tiên cao trong trường `flags` của khung truyền.
/// EmergencyStop PHẢI đặt bit này (FR-12).
pub const FLAG_HIGH_PRIORITY: u8 = 0x80;

// ── Frame Field Offsets ──────────────────────────────────────────────────────
// Vị trí (byte offset) của từng trường trong khung truyền 32 bytes.
// Dùng khi serialize/deserialize thủ công.

/// Offset của byte đồng bộ (Start-of-Frame).
pub const OFFSET_SOF: usize = 0;

/// Offset của số thứ tự khung.
pub const OFFSET_SEQUENCE_NUMBER: usize = 1;

/// Offset của mã lệnh.
pub const OFFSET_COMMAND_CODE: usize = 2;

/// Offset của định danh actuator.
pub const OFFSET_ACTUATOR_ID: usize = 3;

/// Offset của chiều dài payload.
pub const OFFSET_PAYLOAD_LENGTH: usize = 4;

/// Offset bắt đầu vùng payload.
pub const OFFSET_PAYLOAD_START: usize = 5;

/// Offset của trường flags.
pub const OFFSET_FLAGS: usize = 29;

/// Offset bắt đầu CRC-16 (2 bytes, little-endian).
pub const OFFSET_CRC: usize = 30;

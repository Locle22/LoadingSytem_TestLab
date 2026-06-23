// =============================================================================
// spi-protocol/src/error.rs
// =============================================================================
// Định nghĩa toàn bộ mã lỗi và kiểu lỗi dưới dạng enumeration (CQ-06).
// Tuyệt đối không sử dụng số nguyên trần trong logic điều khiển.
// Mỗi lỗi được phân loại theo nhóm với dải giá trị riêng biệt (FR-16).
// =============================================================================

/// Mã lỗi hệ thống, phân loại theo nhóm chức năng (FR-16, CQ-06).
///
/// Mỗi nhóm chiếm một dải giá trị `u8` riêng biệt để dễ dàng phân loại
/// khi truyền qua khung phản hồi. Giá trị `0x00` được dành riêng cho "không có lỗi".
///
/// # Nhóm lỗi
/// - `0x01–0x1F`: Lỗi giao thức (protocol)
/// - `0x20–0x3F`: Lỗi lệnh (command)
/// - `0x40–0x5F`: Lỗi phần cứng motor
/// - `0x60–0x7F`: Lỗi hệ thống
/// - `0x80–0x9F`: Lỗi truyền thông
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub enum ErrorCode {
    // ── Protocol Errors (0x01–0x1F) ──────────────────────────────────────────

    /// Byte đầu khung không phải START_OF_FRAME (0xAA).
    InvalidStartOfFrame = 0x01,

    /// Mã kiểm tra toàn vẹn CRC-16 không khớp.
    ChecksumMismatch = 0x02,

    /// Chiều dài payload vượt quá giới hạn cho phép.
    InvalidPayloadLength = 0x03,

    /// Số thứ tự khung không khớp với kỳ vọng.
    SequenceNumberMismatch = 0x04,

    /// Phát hiện khung trùng lặp (cùng sequence number).
    DuplicateFrame = 0x05,

    /// Nhận được phản hồi không mong đợi (không khớp với lệnh đã gửi).
    UnexpectedResponse = 0x06,

    // ── Command Errors (0x20–0x3F) ───────────────────────────────────────────

    /// Mã lệnh không nằm trong tập lệnh hợp lệ.
    UnknownCommand = 0x20,

    /// Tham số lệnh không hợp lệ (giá trị ngoài phạm vi cho phép).
    InvalidParameter = 0x21,

    /// Actuator ID không tồn tại trên Node.
    InvalidActuatorId = 0x22,

    /// Lệnh không được phép trong trạng thái hiện tại của motor.
    /// Ví dụ: gửi lệnh di chuyển khi motor đang ở trạng thái Disabled.
    CommandNotAllowedInState = 0x23,

    /// Loại lệnh không phù hợp với khả năng của actuator.
    /// Ví dụ: gửi PositionControl cho Motor 0 (velocity-only).
    ActuatorTypeMismatch = 0x24,

    // ── Motor/Hardware Errors (0x40–0x5F) ────────────────────────────────────

    /// Driver động cơ báo lỗi phần cứng.
    MotorDriverFault = 0x40,

    /// Vị trí đích vượt quá giới hạn hành trình (FR-19).
    PositionLimitExceeded = 0x41,

    /// Vận tốc yêu cầu vượt quá giới hạn an toàn (FR-19).
    VelocityLimitExceeded = 0x42,

    /// Gia tốc yêu cầu vượt quá giới hạn an toàn (FR-19).
    AccelerationLimitExceeded = 0x43,

    /// Quá trình homing thất bại (không tìm được sensor home).
    HomingFailed = 0x44,

    /// Lỗi encoder (mất tín hiệu, đếm sai).
    EncoderError = 0x45,

    /// Nhiệt độ motor hoặc driver vượt ngưỡng an toàn.
    OverTemperature = 0x46,

    /// Dòng điện motor vượt ngưỡng an toàn.
    OverCurrent = 0x47,

    // ── System Errors (0x60–0x7F) ────────────────────────────────────────────

    /// Watchdog kích hoạt do mất liên lạc với Host (FR-15).
    WatchdogTriggered = 0x60,

    /// Lệnh dừng khẩn cấp đang hoạt động.
    EmergencyStopActive = 0x61,

    /// Node chưa sẵn sàng nhận lệnh.
    NodeNotReady = 0x62,

    /// Lỗi nội bộ không xác định.
    InternalError = 0x63,

    // ── Communication Errors (0x80–0x9F) ─────────────────────────────────────

    /// Không nhận được phản hồi trong thời gian timeout (FR-13).
    Timeout = 0x80,

    /// Lỗi tầng truyền dẫn vật lý (SPI bus error).
    TransportError = 0x81,

    /// Đã gửi lại tối đa số lần cho phép mà vẫn không thành công (FR-13).
    MaxRetriesExceeded = 0x82,
}

impl ErrorCode {
    /// Chuyển đổi từ giá trị `u8` sang `ErrorCode`.
    /// Trả về `None` nếu giá trị không hợp lệ.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::InvalidStartOfFrame),
            0x02 => Some(Self::ChecksumMismatch),
            0x03 => Some(Self::InvalidPayloadLength),
            0x04 => Some(Self::SequenceNumberMismatch),
            0x05 => Some(Self::DuplicateFrame),
            0x06 => Some(Self::UnexpectedResponse),

            0x20 => Some(Self::UnknownCommand),
            0x21 => Some(Self::InvalidParameter),
            0x22 => Some(Self::InvalidActuatorId),
            0x23 => Some(Self::CommandNotAllowedInState),
            0x24 => Some(Self::ActuatorTypeMismatch),

            0x40 => Some(Self::MotorDriverFault),
            0x41 => Some(Self::PositionLimitExceeded),
            0x42 => Some(Self::VelocityLimitExceeded),
            0x43 => Some(Self::AccelerationLimitExceeded),
            0x44 => Some(Self::HomingFailed),
            0x45 => Some(Self::EncoderError),
            0x46 => Some(Self::OverTemperature),
            0x47 => Some(Self::OverCurrent),

            0x60 => Some(Self::WatchdogTriggered),
            0x61 => Some(Self::EmergencyStopActive),
            0x62 => Some(Self::NodeNotReady),
            0x63 => Some(Self::InternalError),

            0x80 => Some(Self::Timeout),
            0x81 => Some(Self::TransportError),
            0x82 => Some(Self::MaxRetriesExceeded),

            _ => None,
        }
    }

    /// Chuyển đổi sang giá trị `u8` để đặt vào khung truyền.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Kiểm tra lỗi thuộc nhóm giao thức.
    pub fn is_protocol_error(self) -> bool {
        let v = self.as_u8();
        (0x01..=0x1F).contains(&v)
    }

    /// Kiểm tra lỗi thuộc nhóm lệnh.
    pub fn is_command_error(self) -> bool {
        let v = self.as_u8();
        (0x20..=0x3F).contains(&v)
    }

    /// Kiểm tra lỗi thuộc nhóm phần cứng motor.
    pub fn is_hardware_error(self) -> bool {
        let v = self.as_u8();
        (0x40..=0x5F).contains(&v)
    }

    /// Kiểm tra lỗi thuộc nhóm hệ thống.
    pub fn is_system_error(self) -> bool {
        let v = self.as_u8();
        (0x60..=0x7F).contains(&v)
    }

    /// Kiểm tra lỗi thuộc nhóm truyền thông.
    pub fn is_communication_error(self) -> bool {
        let v = self.as_u8();
        (0x80..=0x9F).contains(&v)
    }
}

/// Lỗi cấp giao thức, bao bọc `ErrorCode` với ngữ cảnh phân loại.
///
/// Lớp Protocol và lớp Application sử dụng kiểu này để phân biệt
/// nguồn gốc lỗi mà không cần kiểm tra giá trị số bên trong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub enum ProtocolError {
    /// Lỗi xảy ra khi validate khung truyền (sai SOF, sai CRC, sai length).
    FrameValidation(ErrorCode),

    /// Lỗi truyền thông (timeout, transport error, max retries).
    Communication(ErrorCode),

    /// Lỗi liên quan đến lệnh (lệnh không hợp lệ, tham số sai, actuator sai).
    Command(ErrorCode),

    /// Lỗi phần cứng hoặc hệ thống (motor fault, watchdog, e-stop).
    System(ErrorCode),
}

impl ProtocolError {
    /// Trích xuất `ErrorCode` bên trong.
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::FrameValidation(e)
            | Self::Communication(e)
            | Self::Command(e)
            | Self::System(e) => *e,
        }
    }

    /// Tạo `ProtocolError` tự động phân loại từ `ErrorCode`.
    pub fn from_error_code(code: ErrorCode) -> Self {
        if code.is_protocol_error() {
            Self::FrameValidation(code)
        } else if code.is_command_error() {
            Self::Command(code)
        } else if code.is_hardware_error() || code.is_system_error() {
            Self::System(code)
        } else {
            Self::Communication(code)
        }
    }
}

// Implement Display khi feature "std" được bật, phục vụ logging trên Host.
#[cfg(feature = "std")]
impl core::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}(0x{:02X})", self, self.as_u8())
    }
}

#[cfg(feature = "std")]
impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FrameValidation(e) => write!(f, "FrameValidation: {}", e),
            Self::Communication(e) => write!(f, "Communication: {}", e),
            Self::Command(e) => write!(f, "Command: {}", e),
            Self::System(e) => write!(f, "System: {}", e),
        }
    }
}

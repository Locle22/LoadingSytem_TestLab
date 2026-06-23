// =============================================================================
// spi-protocol/src/commands.rs
// =============================================================================
// Định nghĩa tập lệnh điều khiển, trạng thái motor, và cấu trúc payload
// dưới dạng enumeration (CQ-06). Không sử dụng số nguyên trần.
//
// Tập lệnh đáp ứng FR-10 (tối thiểu 10 nhóm lệnh).
// Payload được serialize/deserialize thủ công (little-endian) vào [u8; 24].
// =============================================================================

use crate::config::{
    ACTUATOR_COUNT, ACTUATOR_ID_BROADCAST, ACTUATOR_ID_POSITION_MOTOR,
    ACTUATOR_ID_VELOCITY_MOTOR, MAX_ACCELERATION_STEPS_PER_SEC2, MAX_VELOCITY_STEPS_PER_SEC,
    POSITION_LIMIT_MAX, POSITION_LIMIT_MIN,
};
use crate::error::ErrorCode;

// ─── Command Code Enum ───────────────────────────────────────────────────────

/// Mã lệnh điều khiển (FR-10, CQ-06).
///
/// Mỗi nhóm lệnh chiếm một dải giá trị `u8` riêng:
/// - `0x01–0x0F`: Hệ thống (ping)
/// - `0x10–0x1F`: Điều khiển nguồn
/// - `0x20–0x2F`: Điều khiển chuyển động
/// - `0x30–0x3F`: Dừng
/// - `0x40–0x4F`: Truy vấn
/// - `0xA0–0xAF`: Phản hồi
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub enum CommandCode {
    // ── Hệ thống (0x01–0x0F) ────────────────────────────────────────────────
    /// Kiểm tra kết nối (heartbeat). Host → Node.
    Ping = 0x01,
    /// Phản hồi heartbeat. Node → Host.
    PingResponse = 0x02,

    // ── Điều khiển nguồn (0x10–0x1F) ────────────────────────────────────────
    /// Kích hoạt driver motor cho phép vận hành. Host → Node.
    MotorEnable = 0x10,
    /// Vô hiệu hoá driver motor, ngăn vận hành. Host → Node.
    MotorDisable = 0x11,

    // ── Điều khiển chuyển động (0x20–0x2F) ──────────────────────────────────
    /// Ra lệnh di chuyển tới vị trí xác định. Chỉ cho Motor 1. Host → Node.
    PositionControl = 0x20,
    /// Ra lệnh chạy theo vận tốc xác định. Chỉ cho Motor 0. Host → Node.
    VelocityControl = 0x21,
    /// Đưa motor về vị trí gốc tham chiếu. Chỉ cho Motor 1. Host → Node.
    Homing = 0x22,

    // ── Dừng (0x30–0x3F) ────────────────────────────────────────────────────
    /// Giảm tốc và dừng motor theo biên dạng an toàn. Host → Node.
    ControlledStop = 0x30,
    /// Dừng motor ngay lập tức, ưu tiên cao nhất (FR-12). Host → Node.
    EmergencyStop = 0x31,

    // ── Truy vấn (0x40–0x4F) ────────────────────────────────────────────────
    /// Đọc trạng thái vận hành hiện tại. Host → Node.
    StatusQuery = 0x40,
    /// Phản hồi trạng thái vận hành. Node → Host.
    StatusResponse = 0x41,

    // ── Phản hồi (0xA0–0xAF) ────────────────────────────────────────────────
    /// Xác nhận lệnh đã thực thi thành công. Node → Host. (FR-11)
    Ack = 0xA0,
    /// Báo lệnh thất bại kèm mã lỗi. Node → Host. (FR-11)
    Nack = 0xA1,
}

impl CommandCode {
    /// Chuyển đổi từ giá trị `u8` sang `CommandCode`.
    /// Trả về `None` nếu giá trị không nằm trong tập lệnh hợp lệ.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Ping),
            0x02 => Some(Self::PingResponse),
            0x10 => Some(Self::MotorEnable),
            0x11 => Some(Self::MotorDisable),
            0x20 => Some(Self::PositionControl),
            0x21 => Some(Self::VelocityControl),
            0x22 => Some(Self::Homing),
            0x30 => Some(Self::ControlledStop),
            0x31 => Some(Self::EmergencyStop),
            0x40 => Some(Self::StatusQuery),
            0x41 => Some(Self::StatusResponse),
            0xA0 => Some(Self::Ack),
            0xA1 => Some(Self::Nack),
            _ => None,
        }
    }

    /// Chuyển đổi sang giá trị `u8` để đặt vào khung truyền.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Kiểm tra lệnh có phải là phản hồi (response) từ Node không.
    pub fn is_response(self) -> bool {
        matches!(
            self,
            Self::PingResponse | Self::StatusResponse | Self::Ack | Self::Nack
        )
    }

    /// Kiểm tra lệnh có phải là lệnh ảnh hưởng tới chuyển động vật lý không.
    /// Các lệnh này BẮT BUỘC phải có phản hồi Ack/Nack (FR-11).
    pub fn requires_acknowledgement(self) -> bool {
        matches!(
            self,
            Self::MotorEnable
                | Self::MotorDisable
                | Self::PositionControl
                | Self::VelocityControl
                | Self::Homing
                | Self::ControlledStop
                | Self::EmergencyStop
        )
    }

    /// Kiểm tra lệnh có phải là EmergencyStop không (FR-12).
    pub fn is_emergency_stop(self) -> bool {
        matches!(self, Self::EmergencyStop)
    }
}

// ─── Motor State Enum ────────────────────────────────────────────────────────

/// Trạng thái hoạt động của motor (CQ-06).
///
/// Trạng thái mặc định khi khởi động là `Disabled` (FR-18).
/// Chỉ chuyển sang trạng thái khác sau khi nhận lệnh `MotorEnable` hợp lệ.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub enum MotorState {
    /// Ngắt nguồn điều khiển. Trạng thái mặc định khi khởi động (FR-18).
    Disabled = 0x00,
    /// Đã kích hoạt, sẵn sàng nhận lệnh di chuyển.
    Idle = 0x01,
    /// Đang thực hiện lệnh di chuyển (position hoặc velocity).
    Moving = 0x02,
    /// Đang thực hiện quá trình homing.
    Homing = 0x03,
    /// Đang giảm tốc dừng (controlled stop).
    Stopping = 0x04,
    /// Lỗi phần cứng — cần xử lý trước khi vận hành tiếp.
    Fault = 0x05,
    /// Đã dừng khẩn cấp — cần reset trước khi vận hành tiếp.
    EmergencyStopped = 0x06,
}

impl MotorState {
    /// Chuyển đổi từ `u8`.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Disabled),
            0x01 => Some(Self::Idle),
            0x02 => Some(Self::Moving),
            0x03 => Some(Self::Homing),
            0x04 => Some(Self::Stopping),
            0x05 => Some(Self::Fault),
            0x06 => Some(Self::EmergencyStopped),
            _ => None,
        }
    }

    /// Chuyển đổi sang `u8`.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Kiểm tra motor có cho phép nhận lệnh di chuyển không.
    pub fn can_accept_motion_command(self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Kiểm tra motor có đang ở trạng thái an toàn không.
    pub fn is_safe_state(self) -> bool {
        matches!(self, Self::Disabled | Self::Idle)
    }
}

// ─── Actuator Capability ─────────────────────────────────────────────────────

/// Khả năng điều khiển của từng loại actuator (CQ-06).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub enum ActuatorCapability {
    /// Motor chỉ hỗ trợ điều khiển theo vận tốc (Motor 0).
    VelocityOnly = 0x00,
    /// Motor chỉ hỗ trợ điều khiển theo vị trí, có thể chỉnh vận tốc xoay (Motor 1).
    PositionOnly = 0x01,
}

impl ActuatorCapability {
    /// Trả về khả năng của actuator dựa trên ID.
    pub fn from_actuator_id(id: u8) -> Option<Self> {
        match id {
            ACTUATOR_ID_VELOCITY_MOTOR => Some(Self::VelocityOnly),
            ACTUATOR_ID_POSITION_MOTOR => Some(Self::PositionOnly),
            _ => None,
        }
    }

    /// Kiểm tra lệnh có phù hợp với khả năng của actuator không.
    /// Trả về `Some(ErrorCode)` nếu không phù hợp.
    pub fn validate_command(self, command: CommandCode) -> Option<ErrorCode> {
        match (self, command) {
            // Velocity-only motor KHÔNG chấp nhận position control hoặc homing
            (Self::VelocityOnly, CommandCode::PositionControl) => {
                Some(ErrorCode::ActuatorTypeMismatch)
            }
            (Self::VelocityOnly, CommandCode::Homing) => Some(ErrorCode::ActuatorTypeMismatch),

            // Position-only motor KHÔNG chấp nhận velocity control
            (Self::PositionOnly, CommandCode::VelocityControl) => {
                Some(ErrorCode::ActuatorTypeMismatch)
            }

            // Các lệnh khác (enable, disable, stop, e-stop, query) hợp lệ cho tất cả
            _ => None,
        }
    }
}

// ─── Payload Structures ──────────────────────────────────────────────────────
// Serialize/deserialize thủ công vào [u8] slice, little-endian.
// Không dùng heap allocation (no_std compatible).
// Kiểu dữ liệu (kích thước, dấu) được quy định rõ ràng (FR-07, CQ-07).

/// Payload cho lệnh `VelocityControl` (8 bytes).
///
/// Dùng cho Motor 0 (velocity-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub struct VelocityCommand {
    /// Vận tốc đích (steps/s). Có dấu: dương = chiều thuận, âm = chiều ngược.
    pub target_velocity: i32,
    /// Gia tốc (steps/s²). Không dấu, luôn dương.
    pub acceleration: u32,
}

/// Kích thước khi serialize `VelocityCommand`.
pub const VELOCITY_COMMAND_SIZE: usize = 8;

impl VelocityCommand {
    /// Kiểm tra tham số có nằm trong giới hạn an toàn không (FR-19).
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let abs_velocity = self.target_velocity.unsigned_abs();
        if abs_velocity > MAX_VELOCITY_STEPS_PER_SEC {
            return Err(ErrorCode::VelocityLimitExceeded);
        }
        if self.acceleration > MAX_ACCELERATION_STEPS_PER_SEC2 {
            return Err(ErrorCode::AccelerationLimitExceeded);
        }
        if self.acceleration == 0 {
            return Err(ErrorCode::InvalidParameter);
        }
        Ok(())
    }

    /// Serialize thành byte slice (little-endian).
    pub fn serialize(&self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        if buf.len() < VELOCITY_COMMAND_SIZE {
            return Err(ErrorCode::InvalidPayloadLength);
        }
        buf[0..4].copy_from_slice(&self.target_velocity.to_le_bytes());
        buf[4..8].copy_from_slice(&self.acceleration.to_le_bytes());
        Ok(())
    }

    /// Deserialize từ byte slice (little-endian).
    pub fn deserialize(buf: &[u8]) -> Result<Self, ErrorCode> {
        if buf.len() < VELOCITY_COMMAND_SIZE {
            return Err(ErrorCode::InvalidPayloadLength);
        }
        Ok(Self {
            target_velocity: i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            acceleration: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
        })
    }
}

/// Payload cho lệnh `PositionControl` (12 bytes).
///
/// Dùng cho Motor 1 (position-only, có thể chỉnh vận tốc xoay).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub struct PositionCommand {
    /// Vị trí đích (steps). Có dấu.
    pub target_position: i32,
    /// Vận tốc tối đa khi di chuyển (steps/s). Không dấu.
    pub max_velocity: u32,
    /// Gia tốc (steps/s²). Không dấu, luôn dương.
    pub acceleration: u32,
}

/// Kích thước khi serialize `PositionCommand`.
pub const POSITION_COMMAND_SIZE: usize = 12;

impl PositionCommand {
    /// Kiểm tra tham số có nằm trong giới hạn an toàn không (FR-19).
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.target_position < POSITION_LIMIT_MIN
            || self.target_position > POSITION_LIMIT_MAX
        {
            return Err(ErrorCode::PositionLimitExceeded);
        }
        if self.max_velocity > MAX_VELOCITY_STEPS_PER_SEC {
            return Err(ErrorCode::VelocityLimitExceeded);
        }
        if self.max_velocity == 0 {
            return Err(ErrorCode::InvalidParameter);
        }
        if self.acceleration > MAX_ACCELERATION_STEPS_PER_SEC2 {
            return Err(ErrorCode::AccelerationLimitExceeded);
        }
        if self.acceleration == 0 {
            return Err(ErrorCode::InvalidParameter);
        }
        Ok(())
    }

    /// Serialize thành byte slice (little-endian).
    pub fn serialize(&self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        if buf.len() < POSITION_COMMAND_SIZE {
            return Err(ErrorCode::InvalidPayloadLength);
        }
        buf[0..4].copy_from_slice(&self.target_position.to_le_bytes());
        buf[4..8].copy_from_slice(&self.max_velocity.to_le_bytes());
        buf[8..12].copy_from_slice(&self.acceleration.to_le_bytes());
        Ok(())
    }

    /// Deserialize từ byte slice (little-endian).
    pub fn deserialize(buf: &[u8]) -> Result<Self, ErrorCode> {
        if buf.len() < POSITION_COMMAND_SIZE {
            return Err(ErrorCode::InvalidPayloadLength);
        }
        Ok(Self {
            target_position: i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            max_velocity: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            acceleration: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
        })
    }
}

/// Payload cho `StatusResponse` (10 bytes).
///
/// Node gửi về Host để báo cáo trạng thái hiện tại của một motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub struct StatusPayload {
    /// Trạng thái hoạt động hiện tại của motor.
    pub motor_state: MotorState,
    /// Mã lỗi hiện tại. `0x00` = không có lỗi.
    pub error_code: u8,
    /// Vị trí hiện tại (steps). Có dấu.
    pub current_position: i32,
    /// Vận tốc hiện tại (steps/s). Có dấu.
    pub current_velocity: i32,
}

/// Kích thước khi serialize `StatusPayload`.
pub const STATUS_PAYLOAD_SIZE: usize = 10;

impl StatusPayload {
    /// Serialize thành byte slice (little-endian).
    pub fn serialize(&self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        if buf.len() < STATUS_PAYLOAD_SIZE {
            return Err(ErrorCode::InvalidPayloadLength);
        }
        buf[0] = self.motor_state.as_u8();
        buf[1] = self.error_code;
        buf[2..6].copy_from_slice(&self.current_position.to_le_bytes());
        buf[6..10].copy_from_slice(&self.current_velocity.to_le_bytes());
        Ok(())
    }

    /// Deserialize từ byte slice (little-endian).
    pub fn deserialize(buf: &[u8]) -> Result<Self, ErrorCode> {
        if buf.len() < STATUS_PAYLOAD_SIZE {
            return Err(ErrorCode::InvalidPayloadLength);
        }
        let motor_state =
            MotorState::from_u8(buf[0]).ok_or(ErrorCode::InvalidParameter)?;
        Ok(Self {
            motor_state,
            error_code: buf[1],
            current_position: i32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]),
            current_velocity: i32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]),
        })
    }
}

/// Payload cho `Nack` (3 bytes).
///
/// Node gửi về Host khi lệnh thất bại, kèm thông tin lệnh gốc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub struct NackPayload {
    /// Mã lỗi cụ thể.
    pub error_code: u8,
    /// Mã lệnh gốc đã thất bại.
    pub failed_command: u8,
    /// Số thứ tự khung của lệnh gốc đã thất bại.
    pub failed_sequence: u8,
}

/// Kích thước khi serialize `NackPayload`.
pub const NACK_PAYLOAD_SIZE: usize = 3;

impl NackPayload {
    /// Serialize thành byte slice.
    pub fn serialize(&self, buf: &mut [u8]) -> Result<(), ErrorCode> {
        if buf.len() < NACK_PAYLOAD_SIZE {
            return Err(ErrorCode::InvalidPayloadLength);
        }
        buf[0] = self.error_code;
        buf[1] = self.failed_command;
        buf[2] = self.failed_sequence;
        Ok(())
    }

    /// Deserialize từ byte slice.
    pub fn deserialize(buf: &[u8]) -> Result<Self, ErrorCode> {
        if buf.len() < NACK_PAYLOAD_SIZE {
            return Err(ErrorCode::InvalidPayloadLength);
        }
        Ok(Self {
            error_code: buf[0],
            failed_command: buf[1],
            failed_sequence: buf[2],
        })
    }
}

// ─── Actuator ID Validation ──────────────────────────────────────────────────

/// Kiểm tra actuator ID có hợp lệ không.
///
/// Hợp lệ nếu ID nằm trong phạm vi `[0, ACTUATOR_COUNT)` hoặc là `ACTUATOR_ID_BROADCAST`.
pub fn validate_actuator_id(id: u8) -> Result<(), ErrorCode> {
    if id < ACTUATOR_COUNT || id == ACTUATOR_ID_BROADCAST {
        Ok(())
    } else {
        Err(ErrorCode::InvalidActuatorId)
    }
}

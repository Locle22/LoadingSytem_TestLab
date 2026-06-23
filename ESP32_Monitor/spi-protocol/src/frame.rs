// =============================================================================
// spi-protocol/src/frame.rs
// =============================================================================
// Đóng gói và giải mã khung truyền cố định 32 bytes (FR-05 → FR-09).
//
// Layout:
//   Byte 0      : SOF (0xAA)
//   Byte 1      : Sequence Number (u8)
//   Byte 2      : Command Code (u8)
//   Byte 3      : Actuator ID (u8)
//   Byte 4      : Payload Length (u8, 0–24)
//   Byte 5–28   : Payload ([u8; 24], zero-padded)
//   Byte 29     : Flags (u8, bit 7 = HIGH_PRIORITY)
//   Byte 30–31  : CRC-16 (u16, little-endian) trên bytes [0..30)
//
// Khung không hợp lệ bị loại bỏ ngay tại đây, KHÔNG chuyển lên
// lớp Application (FR-09).
// =============================================================================

use crate::checksum::{compute_crc, verify_crc};
use crate::commands::CommandCode;
use crate::config::{
    FLAG_HIGH_PRIORITY, FRAME_SIZE, OFFSET_ACTUATOR_ID, OFFSET_COMMAND_CODE, OFFSET_CRC,
    OFFSET_FLAGS, OFFSET_PAYLOAD_LENGTH, OFFSET_PAYLOAD_START, OFFSET_SEQUENCE_NUMBER, OFFSET_SOF,
    PAYLOAD_MAX_SIZE, START_OF_FRAME,
};
use crate::error::{ErrorCode, ProtocolError};

/// Khung truyền cố định 32 bytes (FR-05, FR-06).
///
/// Cấu trúc này đại diện cho một đơn vị dữ liệu truyền qua SPI.
/// Kích thước cố định giúp bên nhận xác định số byte cần đọc
/// mà không cần phân tích trước nội dung (FR-05).
///
/// # Trường bắt buộc (FR-06)
///
/// | Trường | Offset | Size | Mô tả |
/// |--------|--------|------|-------|
/// | SOF | 0 | 1 | Byte đồng bộ, luôn 0xAA |
/// | SEQ | 1 | 1 | Số thứ tự khung |
/// | CMD | 2 | 1 | Mã lệnh (`CommandCode`) |
/// | ACT_ID | 3 | 1 | Định danh actuator |
/// | PLD_LEN | 4 | 1 | Chiều dài payload hợp lệ |
/// | PAYLOAD | 5–28 | 24 | Dữ liệu lệnh, zero-padded |
/// | FLAGS | 29 | 1 | Cờ ưu tiên |
/// | CRC16 | 30–31 | 2 | Mã toàn vẹn CRC-16 LE |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// Byte đồng bộ đầu khung. Luôn = `START_OF_FRAME` (0xAA).
    pub start_of_frame: u8,

    /// Số thứ tự khung (0–255, wrap-around).
    /// Dùng để khớp cặp lệnh – phản hồi và phát hiện khung bị lặp/mất (FR-06).
    pub sequence_number: u8,

    /// Mã lệnh (FR-06). Xem `CommandCode` enum.
    pub command_code: CommandCode,

    /// Định danh thiết bị chấp hành (FR-06).
    /// - `0`: Motor 0 (velocity-only)
    /// - `1`: Motor 1 (position-only)
    /// - `0xFF`: Broadcast (tất cả motor)
    pub actuator_id: u8,

    /// Số byte hợp lệ trong vùng payload (0–24).
    pub payload_length: u8,

    /// Vùng payload cố định 24 bytes. Phần không sử dụng được zero-pad.
    pub payload: [u8; PAYLOAD_MAX_SIZE],

    /// Byte cờ. Bit 7 = `FLAG_HIGH_PRIORITY` (dùng cho EmergencyStop, FR-12).
    pub flags: u8,

    /// Mã kiểm tra toàn vẹn CRC-16, little-endian (FR-08).
    pub crc: u16,
}

impl Frame {
    /// Tạo khung truyền mới, tự động đặt SOF và tính CRC.
    ///
    /// # Arguments
    /// * `sequence_number` - Số thứ tự khung.
    /// * `command_code` - Mã lệnh.
    /// * `actuator_id` - Định danh actuator (0, 1, hoặc 0xFF).
    /// * `payload_data` - Dữ liệu payload (tối đa 24 bytes).
    /// * `flags` - Byte cờ (bit 7 = HIGH_PRIORITY cho EmergencyStop).
    ///
    /// # Errors
    /// Trả về `ProtocolError::FrameValidation(InvalidPayloadLength)` nếu payload > 24 bytes.
    pub fn new(
        sequence_number: u8,
        command_code: CommandCode,
        actuator_id: u8,
        payload_data: &[u8],
        flags: u8,
    ) -> Result<Self, ProtocolError> {
        if payload_data.len() > PAYLOAD_MAX_SIZE {
            return Err(ProtocolError::FrameValidation(
                ErrorCode::InvalidPayloadLength,
            ));
        }

        let mut payload = [0u8; PAYLOAD_MAX_SIZE];
        payload[..payload_data.len()].copy_from_slice(payload_data);

        let mut frame = Self {
            start_of_frame: START_OF_FRAME,
            sequence_number,
            command_code,
            actuator_id,
            payload_length: payload_data.len() as u8,
            payload,
            flags,
            crc: 0, // Sẽ được tính bên dưới
        };

        // Tính CRC trên toàn bộ khung trừ 2 bytes CRC cuối
        let mut temp_buf = [0u8; FRAME_SIZE];
        frame.serialize_without_crc(&mut temp_buf);
        frame.crc = compute_crc(&temp_buf[..OFFSET_CRC]);

        Ok(frame)
    }

    /// Tạo khung EmergencyStop broadcast với cờ ưu tiên cao (FR-12).
    ///
    /// Lệnh EmergencyStop PHẢI có FLAG_HIGH_PRIORITY và actuator_id = BROADCAST.
    pub fn new_emergency_stop(sequence_number: u8) -> Result<Self, ProtocolError> {
        Self::new(
            sequence_number,
            CommandCode::EmergencyStop,
            crate::config::ACTUATOR_ID_BROADCAST,
            &[],
            FLAG_HIGH_PRIORITY,
        )
    }

    /// Tạo khung Ack response.
    pub fn new_ack(
        sequence_number: u8,
        actuator_id: u8,
        acked_sequence: u8,
    ) -> Result<Self, ProtocolError> {
        Self::new(
            sequence_number,
            CommandCode::Ack,
            actuator_id,
            &[acked_sequence],
            0,
        )
    }

    /// Tạo khung Nack response với mã lỗi.
    pub fn new_nack(
        sequence_number: u8,
        actuator_id: u8,
        error_code: ErrorCode,
        failed_command: CommandCode,
        failed_sequence: u8,
    ) -> Result<Self, ProtocolError> {
        let payload = [
            error_code.as_u8(),
            failed_command.as_u8(),
            failed_sequence,
        ];
        Self::new(
            sequence_number,
            CommandCode::Nack,
            actuator_id,
            &payload,
            0,
        )
    }

    /// Serialize khung truyền thành mảng byte cố định 32 bytes.
    ///
    /// Đây là dạng wire format sẵn sàng gửi qua Transport.
    pub fn serialize(&self) -> [u8; FRAME_SIZE] {
        let mut buf = [0u8; FRAME_SIZE];
        self.serialize_without_crc(&mut buf);
        buf[OFFSET_CRC] = (self.crc & 0xFF) as u8; // CRC low byte
        buf[OFFSET_CRC + 1] = ((self.crc >> 8) & 0xFF) as u8; // CRC high byte
        buf
    }

    /// Deserialize từ mảng byte 32 bytes và validate (FR-09).
    ///
    /// Khung không hợp lệ bị loại bỏ ngay tại đây:
    /// - Sai byte đồng bộ SOF
    /// - CRC-16 không khớp
    /// - Payload length vượt quá giới hạn
    /// - Mã lệnh không hợp lệ
    ///
    /// # Errors
    /// Trả về `ProtocolError::FrameValidation` với `ErrorCode` tương ứng.
    pub fn deserialize(buf: &[u8; FRAME_SIZE]) -> Result<Self, ProtocolError> {
        // 1. Kiểm tra byte đồng bộ (FR-09)
        if buf[OFFSET_SOF] != START_OF_FRAME {
            return Err(ProtocolError::FrameValidation(
                ErrorCode::InvalidStartOfFrame,
            ));
        }

        // 2. Kiểm tra CRC-16 (FR-08, FR-09)
        let received_crc = u16::from_le_bytes([buf[OFFSET_CRC], buf[OFFSET_CRC + 1]]);
        if !verify_crc(&buf[..OFFSET_CRC], received_crc) {
            return Err(ProtocolError::FrameValidation(ErrorCode::ChecksumMismatch));
        }

        // 3. Kiểm tra payload length (FR-09)
        let payload_length = buf[OFFSET_PAYLOAD_LENGTH];
        if payload_length as usize > PAYLOAD_MAX_SIZE {
            return Err(ProtocolError::FrameValidation(
                ErrorCode::InvalidPayloadLength,
            ));
        }

        // 4. Kiểm tra command code hợp lệ (FR-09)
        let command_code = CommandCode::from_u8(buf[OFFSET_COMMAND_CODE]).ok_or(
            ProtocolError::FrameValidation(ErrorCode::UnknownCommand),
        )?;

        // 5. Trích xuất payload
        let mut payload = [0u8; PAYLOAD_MAX_SIZE];
        payload.copy_from_slice(&buf[OFFSET_PAYLOAD_START..OFFSET_PAYLOAD_START + PAYLOAD_MAX_SIZE]);

        Ok(Self {
            start_of_frame: buf[OFFSET_SOF],
            sequence_number: buf[OFFSET_SEQUENCE_NUMBER],
            command_code,
            actuator_id: buf[OFFSET_ACTUATOR_ID],
            payload_length,
            payload,
            flags: buf[OFFSET_FLAGS],
            crc: received_crc,
        })
    }

    /// Kiểm tra frame có phải là EmergencyStop không (FR-12).
    pub fn is_emergency(&self) -> bool {
        self.command_code.is_emergency_stop()
    }

    /// Kiểm tra frame có cờ ưu tiên cao không.
    pub fn is_high_priority(&self) -> bool {
        self.flags & FLAG_HIGH_PRIORITY != 0
    }

    /// Lấy phần payload hợp lệ (chỉ `payload_length` bytes đầu).
    pub fn valid_payload(&self) -> &[u8] {
        &self.payload[..self.payload_length as usize]
    }

    /// Serialize các trường (trừ CRC) vào buffer.
    /// Dùng nội bộ để tính CRC.
    fn serialize_without_crc(&self, buf: &mut [u8; FRAME_SIZE]) {
        buf[OFFSET_SOF] = self.start_of_frame;
        buf[OFFSET_SEQUENCE_NUMBER] = self.sequence_number;
        buf[OFFSET_COMMAND_CODE] = self.command_code.as_u8();
        buf[OFFSET_ACTUATOR_ID] = self.actuator_id;
        buf[OFFSET_PAYLOAD_LENGTH] = self.payload_length;
        buf[OFFSET_PAYLOAD_START..OFFSET_PAYLOAD_START + PAYLOAD_MAX_SIZE]
            .copy_from_slice(&self.payload);
        buf[OFFSET_FLAGS] = self.flags;
        // CRC bytes (30–31) giữ nguyên 0, sẽ được điền sau
    }
}

// ─── Unit Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{
        NackPayload, PositionCommand, StatusPayload, VelocityCommand,
        POSITION_COMMAND_SIZE, STATUS_PAYLOAD_SIZE, VELOCITY_COMMAND_SIZE,
    };
    use crate::config::ACTUATOR_ID_BROADCAST;

    // ── Serialize/Deserialize Round-Trip ──────────────────────────────────────

    #[test]
    fn test_frame_round_trip_no_payload() {
        let frame = Frame::new(42, CommandCode::Ping, 0, &[], 0).unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn test_frame_round_trip_with_payload() {
        let payload = [1, 2, 3, 4, 5, 6, 7, 8];
        let frame = Frame::new(1, CommandCode::VelocityControl, 0, &payload, 0).unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        assert_eq!(frame, decoded);
        assert_eq!(decoded.payload_length, 8);
        assert_eq!(&decoded.payload[..8], &payload);
    }

    #[test]
    fn test_frame_round_trip_max_payload() {
        let payload = [0xCC; PAYLOAD_MAX_SIZE];
        let frame = Frame::new(255, CommandCode::StatusResponse, 1, &payload, 0).unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        assert_eq!(frame, decoded);
        assert_eq!(decoded.payload_length as usize, PAYLOAD_MAX_SIZE);
    }

    #[test]
    fn test_frame_round_trip_emergency_stop() {
        let frame = Frame::new_emergency_stop(99).unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        assert_eq!(frame, decoded);
        assert!(decoded.is_emergency());
        assert!(decoded.is_high_priority());
        assert_eq!(decoded.actuator_id, ACTUATOR_ID_BROADCAST);
    }

    // ── SOF Validation ───────────────────────────────────────────────────────

    #[test]
    fn test_reject_invalid_sof() {
        let frame = Frame::new(1, CommandCode::Ping, 0, &[], 0).unwrap();
        let mut buf = frame.serialize();
        buf[OFFSET_SOF] = 0xBB; // Wrong SOF
        let result = Frame::deserialize(&buf);
        assert_eq!(
            result,
            Err(ProtocolError::FrameValidation(ErrorCode::InvalidStartOfFrame))
        );
    }

    // ── CRC Validation ───────────────────────────────────────────────────────

    #[test]
    fn test_reject_corrupted_crc() {
        let frame = Frame::new(1, CommandCode::Ping, 0, &[], 0).unwrap();
        let mut buf = frame.serialize();
        buf[OFFSET_CRC] ^= 0xFF; // Corrupt CRC
        let result = Frame::deserialize(&buf);
        assert_eq!(
            result,
            Err(ProtocolError::FrameValidation(ErrorCode::ChecksumMismatch))
        );
    }

    #[test]
    fn test_reject_corrupted_data() {
        let frame = Frame::new(1, CommandCode::MotorEnable, 0, &[], 0).unwrap();
        let mut buf = frame.serialize();
        buf[5] = 0xFF; // Corrupt a payload byte
        let result = Frame::deserialize(&buf);
        assert_eq!(
            result,
            Err(ProtocolError::FrameValidation(ErrorCode::ChecksumMismatch))
        );
    }

    // ── Payload Length Validation ─────────────────────────────────────────────

    #[test]
    fn test_reject_payload_too_large() {
        let oversized = [0u8; PAYLOAD_MAX_SIZE + 1];
        let result = Frame::new(1, CommandCode::Ping, 0, &oversized, 0);
        assert_eq!(
            result,
            Err(ProtocolError::FrameValidation(ErrorCode::InvalidPayloadLength))
        );
    }

    #[test]
    fn test_reject_invalid_payload_length_in_wire() {
        let frame = Frame::new(1, CommandCode::Ping, 0, &[], 0).unwrap();
        let mut buf = frame.serialize();
        buf[OFFSET_PAYLOAD_LENGTH] = 25; // > PAYLOAD_MAX_SIZE
        // Recompute CRC so it passes CRC check but fails payload length check
        let new_crc = crate::checksum::compute_crc(&buf[..OFFSET_CRC]);
        buf[OFFSET_CRC] = (new_crc & 0xFF) as u8;
        buf[OFFSET_CRC + 1] = ((new_crc >> 8) & 0xFF) as u8;
        let result = Frame::deserialize(&buf);
        assert_eq!(
            result,
            Err(ProtocolError::FrameValidation(ErrorCode::InvalidPayloadLength))
        );
    }

    // ── Command Code Validation ──────────────────────────────────────────────

    #[test]
    fn test_reject_unknown_command() {
        let frame = Frame::new(1, CommandCode::Ping, 0, &[], 0).unwrap();
        let mut buf = frame.serialize();
        buf[OFFSET_COMMAND_CODE] = 0xFE; // Unknown command
        // Recompute CRC
        let new_crc = crate::checksum::compute_crc(&buf[..OFFSET_CRC]);
        buf[OFFSET_CRC] = (new_crc & 0xFF) as u8;
        buf[OFFSET_CRC + 1] = ((new_crc >> 8) & 0xFF) as u8;
        let result = Frame::deserialize(&buf);
        assert_eq!(
            result,
            Err(ProtocolError::FrameValidation(ErrorCode::UnknownCommand))
        );
    }

    // ── Ack/Nack Frame ───────────────────────────────────────────────────────

    #[test]
    fn test_ack_frame_round_trip() {
        let frame = Frame::new_ack(10, 0, 9).unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        assert_eq!(decoded.command_code, CommandCode::Ack);
        assert_eq!(decoded.actuator_id, 0);
        assert_eq!(decoded.valid_payload(), &[9]); // acked sequence
    }

    #[test]
    fn test_nack_frame_round_trip() {
        let frame = Frame::new_nack(
            10,
            1,
            ErrorCode::VelocityLimitExceeded,
            CommandCode::VelocityControl,
            9,
        )
        .unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        assert_eq!(decoded.command_code, CommandCode::Nack);
        let nack = NackPayload::deserialize(decoded.valid_payload()).unwrap();
        assert_eq!(nack.error_code, ErrorCode::VelocityLimitExceeded.as_u8());
        assert_eq!(nack.failed_command, CommandCode::VelocityControl.as_u8());
        assert_eq!(nack.failed_sequence, 9);
    }

    // ── Payload Struct Round-Trips ───────────────────────────────────────────

    #[test]
    fn test_velocity_command_in_frame() {
        let cmd = VelocityCommand {
            target_velocity: -5000,
            acceleration: 1000,
        };
        let mut payload_buf = [0u8; VELOCITY_COMMAND_SIZE];
        cmd.serialize(&mut payload_buf).unwrap();

        let frame = Frame::new(5, CommandCode::VelocityControl, 0, &payload_buf, 0).unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        let decoded_cmd = VelocityCommand::deserialize(decoded.valid_payload()).unwrap();
        assert_eq!(cmd, decoded_cmd);
    }

    #[test]
    fn test_position_command_in_frame() {
        let cmd = PositionCommand {
            target_position: 100_000,
            max_velocity: 8000,
            acceleration: 2000,
        };
        let mut payload_buf = [0u8; POSITION_COMMAND_SIZE];
        cmd.serialize(&mut payload_buf).unwrap();

        let frame = Frame::new(7, CommandCode::PositionControl, 1, &payload_buf, 0).unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        let decoded_cmd = PositionCommand::deserialize(decoded.valid_payload()).unwrap();
        assert_eq!(cmd, decoded_cmd);
    }

    #[test]
    fn test_status_payload_in_frame() {
        use crate::commands::MotorState;

        let status = StatusPayload {
            motor_state: MotorState::Moving,
            error_code: 0x00,
            current_position: -12345,
            current_velocity: 3000,
        };
        let mut payload_buf = [0u8; STATUS_PAYLOAD_SIZE];
        status.serialize(&mut payload_buf).unwrap();

        let frame = Frame::new(20, CommandCode::StatusResponse, 1, &payload_buf, 0).unwrap();
        let buf = frame.serialize();
        let decoded = Frame::deserialize(&buf).unwrap();
        let decoded_status = StatusPayload::deserialize(decoded.valid_payload()).unwrap();
        assert_eq!(status, decoded_status);
    }

    // ── Frame Properties ─────────────────────────────────────────────────────

    #[test]
    fn test_frame_size_is_32() {
        let frame = Frame::new(0, CommandCode::Ping, 0, &[], 0).unwrap();
        let buf = frame.serialize();
        assert_eq!(buf.len(), 32);
    }

    #[test]
    fn test_sof_always_0xaa() {
        let frame = Frame::new(0, CommandCode::Ping, 0, &[], 0).unwrap();
        let buf = frame.serialize();
        assert_eq!(buf[0], 0xAA);
    }

    #[test]
    fn test_valid_payload_slice() {
        let payload = [10, 20, 30];
        let frame = Frame::new(0, CommandCode::Ping, 0, &payload, 0).unwrap();
        assert_eq!(frame.valid_payload(), &[10, 20, 30]);
    }

    #[test]
    fn test_high_priority_flag() {
        let frame = Frame::new(0, CommandCode::EmergencyStop, 0xFF, &[], FLAG_HIGH_PRIORITY).unwrap();
        assert!(frame.is_high_priority());

        let normal = Frame::new(0, CommandCode::Ping, 0, &[], 0).unwrap();
        assert!(!normal.is_high_priority());
    }
}

// =============================================================================
// ESP32/src/node_protocol.rs
// =============================================================================
// Lớp Protocol cho Node ESP32-S3 (FR-05 → FR-12).
//
// Chịu trách nhiệm:
// - Deserialize khung truyền từ byte thô → Frame struct
// - Validate tính toàn vẹn (SOF, CRC, payload length) — thực ra Frame::deserialize
//   đã làm việc này, nhưng protocol layer bổ sung validate ngữ cảnh
// - Theo dõi sequence number để phát hiện khung bị lặp/mất
// - Ưu tiên EmergencyStop (FR-12) — xử lý trước mọi lệnh khác
// - Xây dựng khung phản hồi (Ack, Nack, StatusResponse, PingResponse)
//
// KHÔNG chứa logic điều khiển motor — đó là việc của node_app.rs.
// =============================================================================

use esp_println::println;
use spi_protocol::commands::{CommandCode, StatusPayload, STATUS_PAYLOAD_SIZE};
use spi_protocol::config::FRAME_SIZE;
use spi_protocol::error::{ErrorCode, ProtocolError};
use spi_protocol::frame::Frame;

// ─── NodeProtocol ────────────────────────────────────────────────────────────

/// Lớp Protocol phía Node — xử lý khung truyền và quản lý sequence.
///
/// # Trách nhiệm
/// 1. Giải mã (deserialize) khung truyền thô từ Transport layer
/// 2. Validate khung (SOF, CRC, command code, payload length)
/// 3. Theo dõi sequence number — phát hiện khung trùng lặp
/// 4. Phát hiện và ưu tiên EmergencyStop (FR-12)
/// 5. Tạo khung phản hồi (Ack, Nack, StatusResponse, PingResponse)
///
/// # Không làm
/// - Không kiểm tra tham số motor (velocity, position limits) — việc của node_app
/// - Không gọi motor driver — việc của node_app
/// - Không gửi/nhận byte thô — việc của Transport
pub struct NodeProtocol {
    /// Sequence number tiếp theo mà Node kỳ vọng nhận từ Host.
    /// Dùng để phát hiện khung trùng lặp (DuplicateFrame).
    last_received_seq: Option<u8>,

    /// Sequence number tiếp theo cho khung phản hồi từ Node.
    /// Node tự quản lý sequence riêng cho các frame gửi đi.
    next_response_seq: u8,

    /// Đếm số khung hợp lệ đã nhận — dùng cho thống kê/debug.
    frames_received: u32,

    /// Đếm số khung bị loại bỏ (invalid) — dùng cho thống kê/debug.
    frames_rejected: u32,
}

impl NodeProtocol {
    /// Tạo protocol handler mới.
    pub fn new() -> Self {
        println!("[PROTO] Khởi tạo Node Protocol handler");
        Self {
            last_received_seq: None,
            next_response_seq: 0,
            frames_received: 0,
            frames_rejected: 0,
        }
    }

    /// Deserialize và validate khung truyền từ buffer byte thô.
    ///
    /// Thực hiện các bước validate theo thứ tự:
    /// 1. Deserialize frame (SOF, CRC, payload length, command code) — FR-09
    /// 2. Kiểm tra sequence number trùng lặp
    /// 3. Ưu tiên EmergencyStop — bỏ qua kiểm tra sequence (FR-12)
    ///
    /// # Arguments
    /// * `raw_data` - Buffer 32 bytes nhận từ Transport layer.
    ///
    /// # Returns
    /// * `Ok(Frame)` - Khung hợp lệ, sẵn sàng cho Application layer.
    /// * `Err(ProtocolError)` - Khung không hợp lệ, bị loại bỏ.
    pub fn process_incoming(&mut self, raw_data: &[u8; FRAME_SIZE]) -> Result<Frame, ProtocolError> {
        // Bước 1: Deserialize — validate SOF, CRC, payload length, command code (FR-09)
        let frame = match Frame::deserialize(raw_data) {
            Ok(f) => f,
            Err(e) => {
                self.frames_rejected += 1;
                println!(
                    "[PROTO] Khung bị loại: {:?} (tổng loại: {})",
                    e, self.frames_rejected
                );
                return Err(e);
            }
        };

        // Bước 2: EmergencyStop luôn được ưu tiên — bỏ qua kiểm tra sequence (FR-12)
        if frame.is_emergency() {
            println!(
                "[PROTO] !!! EMERGENCY STOP nhận được (seq={}) — ưu tiên cao nhất !!!",
                frame.sequence_number
            );
            self.last_received_seq = Some(frame.sequence_number);
            self.frames_received += 1;
            return Ok(frame);
        }

        // Bước 3: Kiểm tra sequence number trùng lặp
        if let Some(last_seq) = self.last_received_seq {
            if frame.sequence_number == last_seq {
                self.frames_rejected += 1;
                println!(
                    "[PROTO] Khung trùng lặp: seq={} (tổng loại: {})",
                    frame.sequence_number, self.frames_rejected
                );
                return Err(ProtocolError::FrameValidation(ErrorCode::DuplicateFrame));
            }
        }

        // Khung hợp lệ — cập nhật sequence tracker
        self.last_received_seq = Some(frame.sequence_number);
        self.frames_received += 1;

        println!(
            "[PROTO] Khung hợp lệ: cmd={:?}, seq={}, actuator={}, payload_len={} (tổng nhận: {})",
            frame.command_code,
            frame.sequence_number,
            frame.actuator_id,
            frame.payload_length,
            self.frames_received
        );

        Ok(frame)
    }

    /// Tạo khung Ack response (FR-11).
    ///
    /// Xác nhận lệnh đã được thực thi thành công.
    ///
    /// # Arguments
    /// * `actuator_id` - ID actuator đã thực thi lệnh.
    /// * `acked_sequence` - Sequence number của lệnh được xác nhận.
    pub fn build_ack(
        &mut self,
        actuator_id: u8,
        acked_sequence: u8,
    ) -> Result<[u8; FRAME_SIZE], ProtocolError> {
        let seq = self.next_response_seq;
        self.next_response_seq = self.next_response_seq.wrapping_add(1);

        let frame = Frame::new_ack(seq, actuator_id, acked_sequence)?;
        println!(
            "[PROTO] Gửi ACK: seq={}, acked_seq={}, actuator={}",
            seq, acked_sequence, actuator_id
        );
        Ok(frame.serialize())
    }

    /// Tạo khung Nack response với mã lỗi (FR-11).
    ///
    /// Báo lệnh thất bại kèm thông tin lệnh gốc và lý do.
    ///
    /// # Arguments
    /// * `actuator_id` - ID actuator liên quan.
    /// * `error_code` - Mã lỗi cụ thể.
    /// * `failed_command` - Mã lệnh đã thất bại.
    /// * `failed_sequence` - Sequence number của lệnh đã thất bại.
    pub fn build_nack(
        &mut self,
        actuator_id: u8,
        error_code: ErrorCode,
        failed_command: CommandCode,
        failed_sequence: u8,
    ) -> Result<[u8; FRAME_SIZE], ProtocolError> {
        let seq = self.next_response_seq;
        self.next_response_seq = self.next_response_seq.wrapping_add(1);

        let frame = Frame::new_nack(seq, actuator_id, error_code, failed_command, failed_sequence)?;
        println!(
            "[PROTO] Gửi NACK: seq={}, error={:?}, failed_cmd={:?}, failed_seq={}",
            seq, error_code, failed_command, failed_sequence
        );
        Ok(frame.serialize())
    }

    /// Tạo khung PingResponse.
    ///
    /// Phản hồi heartbeat từ Host — xác nhận Node còn hoạt động.
    ///
    /// # Arguments
    /// * `ping_sequence` - Sequence number của lệnh Ping gốc (đặt trong payload).
    pub fn build_ping_response(
        &mut self,
        ping_sequence: u8,
    ) -> Result<[u8; FRAME_SIZE], ProtocolError> {
        let seq = self.next_response_seq;
        self.next_response_seq = self.next_response_seq.wrapping_add(1);

        // Payload chứa sequence number của Ping gốc để Host khớp cặp
        let frame = Frame::new(
            seq,
            CommandCode::PingResponse,
            0, // actuator_id không quan trọng cho Ping
            &[ping_sequence],
            0, // không có cờ đặc biệt
        )?;

        println!("[PROTO] Gửi PingResponse: seq={}, ping_seq={}", seq, ping_sequence);
        Ok(frame.serialize())
    }

    /// Tạo khung StatusResponse.
    ///
    /// Phản hồi truy vấn trạng thái từ Host.
    ///
    /// # Arguments
    /// * `actuator_id` - ID actuator được truy vấn.
    /// * `status` - Thông tin trạng thái hiện tại.
    pub fn build_status_response(
        &mut self,
        actuator_id: u8,
        status: &StatusPayload,
    ) -> Result<[u8; FRAME_SIZE], ProtocolError> {
        let seq = self.next_response_seq;
        self.next_response_seq = self.next_response_seq.wrapping_add(1);

        // Serialize StatusPayload vào buffer
        let mut payload_buf = [0u8; STATUS_PAYLOAD_SIZE];
        status
            .serialize(&mut payload_buf)
            .map_err(|e| ProtocolError::Command(e))?;

        let frame = Frame::new(
            seq,
            CommandCode::StatusResponse,
            actuator_id,
            &payload_buf,
            0,
        )?;

        println!(
            "[PROTO] Gửi StatusResponse: seq={}, actuator={}, state={:?}, pos={}, vel={}",
            seq, actuator_id, status.motor_state, status.current_position, status.current_velocity
        );
        Ok(frame.serialize())
    }

    /// Lấy số khung hợp lệ đã nhận — dùng cho debug/monitoring.
    pub fn get_frames_received(&self) -> u32 {
        self.frames_received
    }

    /// Lấy số khung bị loại bỏ — dùng cho debug/monitoring.
    pub fn get_frames_rejected(&self) -> u32 {
        self.frames_rejected
    }

    /// Lấy sequence number phản hồi hiện tại — dùng cho debug.
    pub fn get_current_response_seq(&self) -> u8 {
        self.next_response_seq
    }
}

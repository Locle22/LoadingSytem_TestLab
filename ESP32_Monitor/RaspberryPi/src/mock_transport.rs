// =============================================================================
// RaspberryPi/src/mock_transport.rs
// =============================================================================
// Mock Transport cho development và unit testing trên máy dev (Windows/Mac).
//
// Khi build trên Raspberry Pi (Linux), sử dụng SpiMaster thật từ spi_master.rs.
// Khi build trên máy dev (Windows), sử dụng MockTransport này để test logic
// Protocol và Application layer mà không cần phần cứng SPI.
// =============================================================================

use spi_protocol::config::FRAME_SIZE;
use spi_protocol::transport::{Transport, TransportError};
use spi_protocol::frame::Frame;
use spi_protocol::commands::CommandCode;

use std::collections::VecDeque;

/// Mock Transport dùng cho development và testing.
///
/// Lưu các frame gửi đi vào queue, và trả về các response đã được
/// cấu hình trước khi receive.
pub struct MockTransport {
    /// Các frame đã gửi (để kiểm tra trong test).
    sent_frames: Vec<[u8; FRAME_SIZE]>,
    /// Các response được cấu hình trả về khi receive_raw.
    responses: VecDeque<[u8; FRAME_SIZE]>,
    /// Tự động tạo Ack response cho mỗi lệnh gửi.
    auto_ack: bool,
    /// Sequence counter cho auto-generated responses.
    response_seq: u8,
}

impl MockTransport {
    /// Tạo MockTransport mới với auto-ack enabled.
    pub fn new() -> Self {
        log::info!("[MockTransport] Khởi tạo — chế độ development/testing");
        Self {
            sent_frames: Vec::new(),
            responses: VecDeque::new(),
            auto_ack: true,
            response_seq: 0,
        }
    }

    /// Tạo MockTransport không auto-ack (cho test case cụ thể).
    #[allow(dead_code)]
    pub fn without_auto_ack() -> Self {
        Self {
            sent_frames: Vec::new(),
            responses: VecDeque::new(),
            auto_ack: false,
            response_seq: 0,
        }
    }

    /// Thêm response vào queue để trả về khi receive_raw.
    #[allow(dead_code)]
    pub fn enqueue_response(&mut self, response: [u8; FRAME_SIZE]) {
        self.responses.push_back(response);
    }

    /// Lấy danh sách các frame đã gửi (cho verification trong test).
    #[allow(dead_code)]
    pub fn sent_frames(&self) -> &[[u8; FRAME_SIZE]] {
        &self.sent_frames
    }

    /// Lấy frame cuối cùng đã gửi.
    #[allow(dead_code)]
    pub fn last_sent_frame(&self) -> Option<&[u8; FRAME_SIZE]> {
        self.sent_frames.last()
    }

    /// Tạo auto-ack response cho frame vừa gửi.
    fn generate_auto_ack(&mut self, sent_buf: &[u8; FRAME_SIZE]) {
        if !self.auto_ack {
            return;
        }

        // Thử deserialize frame đã gửi để lấy thông tin
        if let Ok(sent_frame) = Frame::deserialize(sent_buf) {
            let response = match sent_frame.command_code {
                CommandCode::Ping => {
                    // Tạo PingResponse
                    Frame::new(
                        self.response_seq,
                        CommandCode::PingResponse,
                        0,
                        &[sent_frame.sequence_number],
                        0,
                    )
                }
                CommandCode::StatusQuery => {
                    // Tạo StatusResponse giả
                    let status_payload = [
                        0x01, // MotorState::Idle
                        0x00, // error_code = 0 (no error)
                        0x00, 0x00, 0x00, 0x00, // position = 0
                        0x00, 0x00, 0x00, 0x00, // velocity = 0
                    ];
                    Frame::new(
                        self.response_seq,
                        CommandCode::StatusResponse,
                        sent_frame.actuator_id,
                        &status_payload,
                        0,
                    )
                }
                _ => {
                    // Tạo Ack cho các lệnh khác
                    Frame::new_ack(
                        self.response_seq,
                        sent_frame.actuator_id,
                        sent_frame.sequence_number,
                    )
                }
            };

            if let Ok(response_frame) = response {
                self.responses.push_back(response_frame.serialize());
                self.response_seq = self.response_seq.wrapping_add(1);
            }
        }
    }
}

impl Transport for MockTransport {
    fn send_raw(&mut self, data: &[u8; FRAME_SIZE]) -> Result<(), TransportError> {
        log::trace!("[MockTransport] send_raw: {} bytes", FRAME_SIZE);
        self.sent_frames.push(*data);
        self.generate_auto_ack(data);
        Ok(())
    }

    fn receive_raw(&mut self, buffer: &mut [u8; FRAME_SIZE]) -> Result<(), TransportError> {
        match self.responses.pop_front() {
            Some(response) => {
                buffer.copy_from_slice(&response);
                log::trace!("[MockTransport] receive_raw: trả về response");
                Ok(())
            }
            None => {
                log::trace!("[MockTransport] receive_raw: không có response");
                Err(TransportError::Timeout)
            }
        }
    }

    fn transfer(
        &mut self,
        tx_buf: &[u8; FRAME_SIZE],
        rx_buf: &mut [u8; FRAME_SIZE],
    ) -> Result<(), TransportError> {
        self.send_raw(tx_buf)?;
        self.receive_raw(rx_buf)
    }
}

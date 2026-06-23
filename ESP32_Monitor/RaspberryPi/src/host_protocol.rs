// =============================================================================
// RaspberryPi/src/host_protocol.rs
// =============================================================================
// Lớp Giao thức (Protocol Layer) trên Host — Raspberry Pi 4.
//
// Chịu trách nhiệm:
//   - Quản lý sequence number (u8, wrapping) (FR-14)
//   - Đóng gói Frame → serialize → gửi qua Transport
//   - Nhận phản hồi → deserialize → validate
//   - Timeout + retry logic (FR-13)
//   - Emergency Stop bypass: gửi ngay, không xếp hàng (FR-12)
//   - Khớp sequence number phản hồi với lệnh đã gửi (FR-14)
//   - Logging toàn bộ sự kiện (CQ-10)
//
// Module này KHÔNG biết ngữ nghĩa lệnh (enable/disable/position/velocity).
// Nó chỉ biết: "gửi Frame, chờ Frame phản hồi, retry nếu thất bại".
// =============================================================================

use std::thread;
use std::time::{Duration, Instant};

use spi_protocol::config::{DEFAULT_TIMEOUT_MS, MAX_RETRY_COUNT};
use spi_protocol::{
    CommandCode, ErrorCode, Frame, NackPayload, ProtocolError, Transport, TransportError,
    FRAME_SIZE,
};

// ── Error Type ───────────────────────────────────────────────────────────────

/// Lỗi phát sinh từ lớp Protocol.
///
/// Bao gồm lỗi giao thức (từ shared library) và lỗi vận hành cấp Host.
#[derive(Debug, thiserror::Error)]
pub enum HostProtocolError {
    /// Lỗi giao thức từ thư viện chia sẻ.
    #[error("Lỗi giao thức: {0}")]
    Protocol(ProtocolError),

    /// Lỗi truyền dẫn (SPI bus error, device not responding...).
    #[error("Lỗi truyền dẫn: {0:?}")]
    Transport(TransportError),

    /// Đã gửi lại tối đa số lần mà vẫn không nhận được phản hồi hợp lệ.
    #[error("Vượt quá số lần gửi lại tối đa ({0} lần)")]
    MaxRetriesExceeded(u8),

    /// Timeout chờ phản hồi.
    #[error("Timeout chờ phản hồi sau {0}ms")]
    Timeout(u64),

    /// Node trả về Nack với mã lỗi cụ thể.
    #[error("Node từ chối lệnh: error_code=0x{:02X}, failed_cmd=0x{:02X}, failed_seq={}", .0.error_code, .0.failed_command, .0.failed_sequence)]
    Nacked(NackPayload),

    /// Sequence number phản hồi không khớp với lệnh đã gửi.
    #[error("Sequence number không khớp: gửi={sent}, nhận={received}")]
    SequenceMismatch { sent: u8, received: u8 },
}

impl From<ProtocolError> for HostProtocolError {
    fn from(e: ProtocolError) -> Self {
        Self::Protocol(e)
    }
}

impl From<TransportError> for HostProtocolError {
    fn from(e: TransportError) -> Self {
        Self::Transport(e)
    }
}

// ── Kiểu kết quả phản hồi ───────────────────────────────────────────────────

/// Kết quả phản hồi từ Node sau khi gửi lệnh thành công.
#[derive(Debug, Clone)]
pub struct Response {
    /// Frame phản hồi đã được validate.
    pub frame: Frame,
}

// ── Host Protocol Handler ────────────────────────────────────────────────────

/// Bộ xử lý giao thức tầng Protocol trên Host.
///
/// Generic over `T: Transport` để có thể sử dụng với:
/// - `SpiMaster` (phần cứng thật)
/// - `MockTransport` (unit testing)
///
/// # Trách nhiệm
/// - Quản lý bộ đếm sequence number (u8, wrapping 0→255→0)
/// - Gửi frame và chờ phản hồi với timeout
/// - Retry logic: tối đa `MAX_RETRY_COUNT` lần
/// - Emergency Stop: gửi ngay lập tức, không xếp hàng
/// - Validate frame phản hồi (đã xử lý bởi `Frame::deserialize`)
/// - Khớp sequence number phản hồi với lệnh đã gửi
pub struct HostProtocol<T: Transport> {
    /// Lớp truyền dẫn (SPI, UART, Mock...).
    transport: T,
    /// Bộ đếm sequence number hiện tại (wrapping u8).
    sequence_counter: u8,
    /// Timeout cho mỗi lần gửi lệnh (milliseconds).
    timeout_ms: u64,
    /// Số lần retry tối đa.
    max_retries: u8,
}

impl<T: Transport> HostProtocol<T> {
    /// Khởi tạo Protocol handler với transport và cấu hình mặc định.
    pub fn new(transport: T) -> Self {
        log::info!(
            "HostProtocol khởi tạo: timeout={}ms, max_retries={}",
            DEFAULT_TIMEOUT_MS,
            MAX_RETRY_COUNT
        );

        Self {
            transport,
            sequence_counter: 0,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_retries: MAX_RETRY_COUNT,
        }
    }

    /// Khởi tạo Protocol handler với cấu hình timeout và retry tùy chỉnh.
    pub fn with_config(transport: T, timeout_ms: u64, max_retries: u8) -> Self {
        log::info!(
            "HostProtocol khởi tạo (custom): timeout={}ms, max_retries={}",
            timeout_ms,
            max_retries
        );

        Self {
            transport,
            sequence_counter: 0,
            timeout_ms,
            max_retries,
        }
    }

    /// Lấy reference mutable đến transport (để test hoặc cấu hình thêm).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Lấy sequence number hiện tại (cho debug/test).
    pub fn current_sequence(&self) -> u8 {
        self.sequence_counter
    }

    // ── Sequence Number Management ───────────────────────────────────────────

    /// Cấp phát sequence number tiếp theo (wrapping u8: 0→1→...→255→0).
    ///
    /// Mỗi lệnh gửi đi PHẢI có sequence number duy nhất để:
    /// - Khớp cặp lệnh ↔ phản hồi
    /// - Phát hiện khung bị mất hoặc trùng lặp (FR-14)
    fn next_sequence(&mut self) -> u8 {
        let seq = self.sequence_counter;
        self.sequence_counter = self.sequence_counter.wrapping_add(1);
        seq
    }

    // ── Core Send/Receive ────────────────────────────────────────────────────

    /// Gửi một Frame và chờ phản hồi với timeout + retry.
    ///
    /// Đây là phương thức trung tâm của lớp Protocol. Mọi lệnh
    /// từ lớp Application đều đi qua đây.
    ///
    /// # Quy trình
    /// 1. Serialize frame → byte array
    /// 2. Gửi qua transport (SPI transfer full-duplex)
    /// 3. Chờ phản hồi trong thời gian timeout
    /// 4. Deserialize + validate phản hồi
    /// 5. Kiểm tra sequence number khớp
    /// 6. Nếu thất bại → retry (tối đa MAX_RETRY_COUNT lần)
    ///
    /// # Arguments
    /// * `command_code` - Mã lệnh cần gửi.
    /// * `actuator_id` - ID thiết bị chấp hành.
    /// * `payload` - Dữ liệu payload (tối đa 24 bytes).
    /// * `flags` - Cờ frame (FLAG_HIGH_PRIORITY cho EmergencyStop).
    ///
    /// # Errors
    /// - `MaxRetriesExceeded` — đã retry hết mà vẫn thất bại
    /// - `Nacked` — Node trả về Nack với mã lỗi
    /// - `SequenceMismatch` — sequence number phản hồi không khớp
    /// - `Protocol` — lỗi đóng gói/giải mã frame
    /// - `Transport` — lỗi SPI bus
    pub fn send_command(
        &mut self,
        command_code: CommandCode,
        actuator_id: u8,
        payload: &[u8],
        flags: u8,
    ) -> Result<Response, HostProtocolError> {
        let seq = self.next_sequence();

        log::info!(
            "Gửi lệnh: cmd={:?}, actuator_id={}, seq={}, payload_len={}, flags=0x{:02X}",
            command_code, actuator_id, seq, payload.len(), flags
        );

        // Tạo frame lệnh
        let frame = Frame::new(seq, command_code, actuator_id, payload, flags)?;
        let tx_buf = frame.serialize();

        // Retry loop
        let mut last_error: Option<HostProtocolError> = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                log::warn!(
                    "Retry lần {}/{}: cmd={:?}, seq={}",
                    attempt, self.max_retries, command_code, seq
                );
            }

            match self.send_and_receive(&tx_buf, seq, command_code) {
                Ok(response) => {
                    log::info!(
                        "Nhận phản hồi thành công: cmd={:?}, seq={}, response_cmd={:?}",
                        command_code, seq, response.frame.command_code
                    );
                    return Ok(response);
                }
                Err(e) => {
                    log::warn!(
                        "Lần gửi {}: thất bại — {}",
                        attempt + 1, e
                    );
                    last_error = Some(e);
                }
            }
        }

        // Đã vượt quá số lần retry
        let total_attempts = self.max_retries + 1;
        log::error!(
            "Vượt quá số lần gửi lại tối đa ({} lần) cho cmd={:?}, seq={}. Lỗi cuối: {:?}",
            total_attempts, command_code, seq, last_error
        );
        Err(HostProtocolError::MaxRetriesExceeded(total_attempts))
    }

    /// Gửi EmergencyStop broadcast — ưu tiên cao nhất, không retry (FR-12).
    ///
    /// Lệnh EmergencyStop PHẢI:
    /// - Gửi ngay lập tức, không xếp hàng chờ
    /// - Sử dụng FLAG_HIGH_PRIORITY
    /// - Gửi broadcast (ACTUATOR_ID_BROADCAST = 0xFF)
    /// - Vẫn chờ Ack nhưng chỉ thử 1 lần (không retry)
    ///
    /// # Errors
    /// Trả về lỗi nếu giao dịch SPI thất bại hoặc Node Nack.
    /// Tuy nhiên, lỗi ở đây KHÔNG nên ngăn cản việc kích hoạt
    /// E-Stop GPIO (xử lý ở lớp Application).
    pub fn send_emergency_stop(&mut self) -> Result<Response, HostProtocolError> {
        let seq = self.next_sequence();

        log::error!(
            "!!! GỬI EMERGENCY STOP !!! seq={}, broadcast, high_priority",
            seq
        );

        let frame = Frame::new_emergency_stop(seq)?;
        let tx_buf = frame.serialize();

        // EmergencyStop: gửi 1 lần duy nhất, không retry
        // Vì tính cấp bách, không nên trì hoãn bằng retry
        self.send_and_receive(&tx_buf, seq, CommandCode::EmergencyStop)
    }

    /// Gửi Ping để kiểm tra kết nối (heartbeat).
    ///
    /// Mong đợi nhận được PingResponse từ Node.
    pub fn send_ping(&mut self) -> Result<Response, HostProtocolError> {
        self.send_command(CommandCode::Ping, 0, &[], 0)
    }

    // ── Internal: Send and Receive Single Transaction ────────────────────────

    /// Thực hiện một giao dịch SPI: gửi frame → chờ phản hồi → validate.
    ///
    /// # Quy trình chi tiết
    /// 1. Gửi tx_buf qua transport (SPI transfer)
    /// 2. Phản hồi đầu tiên từ SPI transfer thường là dummy/stale data
    /// 3. Poll nhận phản hồi thật trong thời gian timeout
    /// 4. Deserialize và validate frame phản hồi
    /// 5. Kiểm tra: phản hồi phải là Ack, Nack, PingResponse, hoặc StatusResponse
    /// 6. Kiểm tra sequence number khớp (trừ StatusResponse có seq riêng)
    fn send_and_receive(
        &mut self,
        tx_buf: &[u8; FRAME_SIZE],
        expected_seq: u8,
        sent_command: CommandCode,
    ) -> Result<Response, HostProtocolError> {
        // Bước 1: Gửi frame lệnh qua SPI
        self.transport.send_raw(tx_buf)?;

        // Bước 2: Chờ phản hồi trong thời gian timeout
        let deadline = Instant::now() + Duration::from_millis(self.timeout_ms);

        // Khoảng cách giữa các lần poll (microseconds)
        // Bắt đầu nhỏ, tăng dần để cân bằng giữa latency và CPU usage
        let poll_interval_us: u64 = 500;

        loop {
            // Kiểm tra timeout
            if Instant::now() >= deadline {
                log::warn!(
                    "Timeout ({}ms) chờ phản hồi cho seq={}",
                    self.timeout_ms, expected_seq
                );
                return Err(HostProtocolError::Timeout(self.timeout_ms));
            }

            // Thử nhận phản hồi
            let mut rx_buf = [0u8; FRAME_SIZE];
            match self.transport.receive_raw(&mut rx_buf) {
                Ok(()) => {
                    // Thử deserialize (validate SOF, CRC, command code, payload length)
                    match Frame::deserialize(&rx_buf) {
                        Ok(response_frame) => {
                            // Frame hợp lệ — kiểm tra nội dung
                            return self.validate_response(
                                response_frame,
                                expected_seq,
                                sent_command,
                            );
                        }
                        Err(ProtocolError::FrameValidation(ErrorCode::InvalidStartOfFrame)) => {
                            // SOF sai → có thể là dummy data, tiếp tục poll
                            log::trace!("Nhận được frame không hợp lệ (sai SOF), tiếp tục poll...");
                        }
                        Err(e) => {
                            // Lỗi nghiêm trọng hơn (CRC sai, command sai...)
                            log::warn!("Frame phản hồi không hợp lệ: {}", e);
                            return Err(HostProtocolError::Protocol(e));
                        }
                    }
                }
                Err(TransportError::Timeout) => {
                    // Transport-level timeout — tiếp tục poll cho đến deadline
                    log::trace!("Transport timeout, tiếp tục poll...");
                }
                Err(e) => {
                    // Lỗi bus nghiêm trọng
                    log::error!("Lỗi transport khi nhận phản hồi: {:?}", e);
                    return Err(HostProtocolError::Transport(e));
                }
            }

            // Nghỉ ngắn trước lần poll tiếp theo
            thread::sleep(Duration::from_micros(poll_interval_us));
        }
    }

    /// Validate frame phản hồi từ Node.
    ///
    /// Kiểm tra:
    /// 1. Command code phải là response type (Ack, Nack, PingResponse, StatusResponse)
    /// 2. Nếu Ack: payload[0] phải khớp expected_seq
    /// 3. Nếu Nack: parse NackPayload và trả về lỗi
    /// 4. Nếu PingResponse: sequence phải khớp
    /// 5. Nếu StatusResponse: cho phép (trả về dữ liệu trạng thái)
    fn validate_response(
        &self,
        response: Frame,
        expected_seq: u8,
        _sent_command: CommandCode,
    ) -> Result<Response, HostProtocolError> {
        log::debug!(
            "Validate phản hồi: cmd={:?}, seq={}, payload_len={}",
            response.command_code, response.sequence_number, response.payload_length
        );

        match response.command_code {
            CommandCode::Ack => {
                // Ack phải chứa sequence number của lệnh được xác nhận
                let acked_seq = if response.payload_length >= 1 {
                    response.payload[0]
                } else {
                    // Ack không có payload → dùng sequence number của frame
                    response.sequence_number
                };

                if acked_seq != expected_seq {
                    log::warn!(
                        "Ack sequence mismatch: expected={}, got={}",
                        expected_seq, acked_seq
                    );
                    return Err(HostProtocolError::SequenceMismatch {
                        sent: expected_seq,
                        received: acked_seq,
                    });
                }

                log::debug!("Ack hợp lệ cho seq={}", expected_seq);
                Ok(Response { frame: response })
            }

            CommandCode::Nack => {
                // Parse NackPayload để lấy thông tin lỗi chi tiết
                let nack_payload = NackPayload::deserialize(response.valid_payload())
                    .map_err(|e| HostProtocolError::Protocol(
                        ProtocolError::FrameValidation(e),
                    ))?;

                log::error!(
                    "Nhận NACK: error_code=0x{:02X}, failed_cmd=0x{:02X}, failed_seq={}",
                    nack_payload.error_code,
                    nack_payload.failed_command,
                    nack_payload.failed_sequence
                );

                Err(HostProtocolError::Nacked(nack_payload))
            }

            CommandCode::PingResponse => {
                // PingResponse: sequence number phải khớp
                if response.sequence_number != expected_seq {
                    log::warn!(
                        "PingResponse sequence mismatch: expected={}, got={}",
                        expected_seq, response.sequence_number
                    );
                    // Chấp nhận luôn cho PingResponse vì heartbeat không cần chính xác tuyệt đối
                }
                log::debug!("PingResponse nhận được cho seq={}", expected_seq);
                Ok(Response { frame: response })
            }

            CommandCode::StatusResponse => {
                // StatusResponse: chứa dữ liệu trạng thái motor
                log::debug!(
                    "StatusResponse nhận được: actuator_id={}, payload_len={}",
                    response.actuator_id, response.payload_length
                );
                Ok(Response { frame: response })
            }

            other => {
                // Phản hồi không mong đợi
                log::warn!(
                    "Phản hồi không mong đợi: cmd={:?} (kỳ vọng Ack/Nack/PingResponse/StatusResponse)",
                    other
                );
                Err(HostProtocolError::Protocol(
                    ProtocolError::FrameValidation(ErrorCode::UnexpectedResponse),
                ))
            }
        }
    }
}

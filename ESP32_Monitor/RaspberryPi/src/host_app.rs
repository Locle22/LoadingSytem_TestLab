// =============================================================================
// RaspberryPi/src/host_app.rs
// =============================================================================
// Lớp Ứng dụng (Application Layer) trên Host — Raspberry Pi 4.
//
// Cung cấp API cấp cao cho điều khiển motor:
//   - ping()                           → kiểm tra kết nối
//   - enable_motor(actuator_id)        → kích hoạt driver
//   - disable_motor(actuator_id)       → vô hiệu hoá driver
//   - move_to_position(pos, vel, acc)  → di chuyển tới vị trí (Motor 1)
//   - set_velocity(vel, acc)           → đặt vận tốc (Motor 0)
//   - home()                           → homing (Motor 1)
//   - controlled_stop(actuator_id)     → dừng an toàn
//   - emergency_stop()                 → dừng khẩn cấp (broadcast)
//   - query_status(actuator_id)        → truy vấn trạng thái
//
// Module này sử dụng HostProtocol (lớp Protocol) để gửi/nhận frame,
// và EmergencyStopHandler (lớp GPIO) cho E-Stop phần cứng.
// =============================================================================

use spi_protocol::commands::{POSITION_COMMAND_SIZE, VELOCITY_COMMAND_SIZE};
use spi_protocol::config::{
    ACTUATOR_ID_BROADCAST, ACTUATOR_ID_POSITION_MOTOR, ACTUATOR_ID_VELOCITY_MOTOR,
};
use spi_protocol::{
    CommandCode, ErrorCode, PositionCommand, StatusPayload, Transport,
    VelocityCommand,
};

use crate::host_protocol::{HostProtocol, HostProtocolError};

// ── Error Type ───────────────────────────────────────────────────────────────

/// Lỗi từ lớp Application.
///
/// Bao gồm lỗi từ Protocol layer và lỗi validation tham số ở tầng Application.
#[derive(Debug, thiserror::Error)]
pub enum MotorControlError {
    /// Lỗi từ lớp Protocol (timeout, retry, transport, nack...).
    #[error("Lỗi giao thức: {0}")]
    Protocol(#[from] HostProtocolError),

    /// Tham số lệnh không hợp lệ (validate trước khi gửi).
    #[error("Tham số không hợp lệ: {0}")]
    InvalidParameter(String),

    /// Lỗi nghiệp vụ khác.
    #[error("Lỗi lệnh: {0:?}")]
    CommandError(ErrorCode),
}

// ── Motor Controller ─────────────────────────────────────────────────────────

/// Bộ điều khiển motor cấp cao (Application Layer).
///
/// Generic over `T: Transport` để có thể sử dụng với SPI thật hoặc Mock.
///
/// # Kiến trúc phân tầng
/// ```text
/// ┌─────────────────────────────────┐
/// │   MotorController (host_app)    │ ← API cấp cao, validate tham số
/// ├─────────────────────────────────┤
/// │   HostProtocol (host_protocol)  │ ← Framing, seq#, timeout, retry
/// ├─────────────────────────────────┤
/// │   SpiMaster (spi_master)        │ ← Byte thô qua SPI bus
/// └─────────────────────────────────┘
/// ```
pub struct MotorController<T: Transport> {
    /// Protocol handler — xử lý framing, sequence, timeout, retry.
    protocol: HostProtocol<T>,
}

impl<T: Transport> MotorController<T> {
    /// Khởi tạo MotorController với transport mặc định.
    pub fn new(transport: T) -> Self {
        log::info!("MotorController khởi tạo");
        Self {
            protocol: HostProtocol::new(transport),
        }
    }

    /// Khởi tạo MotorController với cấu hình timeout/retry tùy chỉnh.
    pub fn with_config(transport: T, timeout_ms: u64, max_retries: u8) -> Self {
        log::info!(
            "MotorController khởi tạo (custom): timeout={}ms, max_retries={}",
            timeout_ms, max_retries
        );
        Self {
            protocol: HostProtocol::with_config(transport, timeout_ms, max_retries),
        }
    }

    /// Truy cập Protocol handler (cho test hoặc debug).
    pub fn protocol_mut(&mut self) -> &mut HostProtocol<T> {
        &mut self.protocol
    }

    // ── Ping / Heartbeat ─────────────────────────────────────────────────────

    /// Kiểm tra kết nối với Node (heartbeat).
    ///
    /// Gửi lệnh Ping và mong đợi PingResponse.
    /// Dùng để kiểm tra Node có đang hoạt động và phản hồi không.
    ///
    /// # Returns
    /// `Ok(())` nếu Node phản hồi PingResponse.
    pub fn ping(&mut self) -> Result<(), MotorControlError> {
        log::info!("Ping: kiểm tra kết nối với Node...");
        self.protocol.send_ping()?;
        log::info!("Ping: Node phản hồi thành công");
        Ok(())
    }

    // ── Motor Enable / Disable ───────────────────────────────────────────────

    /// Kích hoạt driver motor cho phép vận hành.
    ///
    /// # Arguments
    /// * `actuator_id` - ID motor (0=velocity, 1=position, 0xFF=broadcast).
    ///
    /// # Errors
    /// - `InvalidParameter` nếu actuator_id không hợp lệ
    /// - `Protocol(Nacked)` nếu Node từ chối (ví dụ: motor đang fault)
    pub fn enable_motor(&mut self, actuator_id: u8) -> Result<(), MotorControlError> {
        self.validate_actuator_id(actuator_id)?;
        log::info!("Kích hoạt motor: actuator_id={}", actuator_id);

        self.protocol.send_command(
            CommandCode::MotorEnable,
            actuator_id,
            &[],
            0,
        )?;

        log::info!("Motor {} đã được kích hoạt", actuator_id);
        Ok(())
    }

    /// Vô hiệu hoá driver motor, ngăn vận hành.
    ///
    /// Motor sẽ chuyển sang trạng thái Disabled và không nhận lệnh di chuyển.
    ///
    /// # Arguments
    /// * `actuator_id` - ID motor (0=velocity, 1=position, 0xFF=broadcast).
    pub fn disable_motor(&mut self, actuator_id: u8) -> Result<(), MotorControlError> {
        self.validate_actuator_id(actuator_id)?;
        log::info!("Vô hiệu hoá motor: actuator_id={}", actuator_id);

        self.protocol.send_command(
            CommandCode::MotorDisable,
            actuator_id,
            &[],
            0,
        )?;

        log::info!("Motor {} đã được vô hiệu hoá", actuator_id);
        Ok(())
    }

    // ── Position Control (Motor 1 only) ──────────────────────────────────────

    /// Di chuyển motor tới vị trí xác định.
    ///
    /// Lệnh này chỉ dành cho Motor 1 (ACTUATOR_ID_POSITION_MOTOR = 1).
    /// Tham số được validate cả ở Host (trước khi gửi) và Node (trước khi thực thi).
    ///
    /// # Arguments
    /// * `target_position` - Vị trí đích (steps), có dấu.
    /// * `max_velocity` - Vận tốc tối đa (steps/s), không dấu.
    /// * `acceleration` - Gia tốc (steps/s²), không dấu.
    ///
    /// # Errors
    /// - `InvalidParameter` nếu vị trí/vận tốc/gia tốc ngoài giới hạn
    /// - `Protocol(Nacked)` nếu Node từ chối (motor chưa enable, wrong state...)
    pub fn move_to_position(
        &mut self,
        target_position: i32,
        max_velocity: u32,
        acceleration: u32,
    ) -> Result<(), MotorControlError> {
        let cmd = PositionCommand {
            target_position,
            max_velocity,
            acceleration,
        };

        // Validate tham số ở Host trước khi gửi (FR-19)
        cmd.validate().map_err(|e| {
            log::error!("Tham số PositionControl không hợp lệ: {:?}", e);
            MotorControlError::CommandError(e)
        })?;

        // Serialize payload
        let mut payload = [0u8; POSITION_COMMAND_SIZE];
        cmd.serialize(&mut payload).map_err(|e| {
            MotorControlError::CommandError(e)
        })?;

        log::info!(
            "Di chuyển Motor 1: position={}, velocity={}, acceleration={}",
            target_position, max_velocity, acceleration
        );

        self.protocol.send_command(
            CommandCode::PositionControl,
            ACTUATOR_ID_POSITION_MOTOR,
            &payload,
            0,
        )?;

        log::info!(
            "Lệnh PositionControl đã được Node xác nhận: target={}",
            target_position
        );
        Ok(())
    }

    // ── Velocity Control (Motor 0 only) ──────────────────────────────────────

    /// Đặt vận tốc cho motor.
    ///
    /// Lệnh này chỉ dành cho Motor 0 (ACTUATOR_ID_VELOCITY_MOTOR = 0).
    ///
    /// # Arguments
    /// * `target_velocity` - Vận tốc đích (steps/s), có dấu. Dương=thuận, âm=ngược.
    /// * `acceleration` - Gia tốc (steps/s²), không dấu.
    ///
    /// # Errors
    /// - `InvalidParameter` nếu vận tốc/gia tốc ngoài giới hạn
    /// - `Protocol(Nacked)` nếu Node từ chối
    pub fn set_velocity(
        &mut self,
        target_velocity: i32,
        acceleration: u32,
    ) -> Result<(), MotorControlError> {
        let cmd = VelocityCommand {
            target_velocity,
            acceleration,
        };

        // Validate tham số ở Host trước khi gửi (FR-19)
        cmd.validate().map_err(|e| {
            log::error!("Tham số VelocityControl không hợp lệ: {:?}", e);
            MotorControlError::CommandError(e)
        })?;

        // Serialize payload
        let mut payload = [0u8; VELOCITY_COMMAND_SIZE];
        cmd.serialize(&mut payload).map_err(|e| {
            MotorControlError::CommandError(e)
        })?;

        log::info!(
            "Đặt vận tốc Motor 0: velocity={}, acceleration={}",
            target_velocity, acceleration
        );

        self.protocol.send_command(
            CommandCode::VelocityControl,
            ACTUATOR_ID_VELOCITY_MOTOR,
            &payload,
            0,
        )?;

        log::info!(
            "Lệnh VelocityControl đã được Node xác nhận: velocity={}",
            target_velocity
        );
        Ok(())
    }

    // ── Homing (Motor 1 only) ────────────────────────────────────────────────

    /// Đưa motor về vị trí gốc tham chiếu (homing).
    ///
    /// Chỉ dành cho Motor 1 (ACTUATOR_ID_POSITION_MOTOR = 1).
    /// Motor sẽ di chuyển cho đến khi gặp sensor home, sau đó reset vị trí về 0.
    ///
    /// # Errors
    /// - `Protocol(Nacked)` nếu Node từ chối (motor chưa enable, đang busy...)
    pub fn home(&mut self) -> Result<(), MotorControlError> {
        log::info!("Bắt đầu homing Motor 1...");

        self.protocol.send_command(
            CommandCode::Homing,
            ACTUATOR_ID_POSITION_MOTOR,
            &[],
            0,
        )?;

        log::info!("Lệnh Homing đã được Node xác nhận. Motor 1 đang tìm vị trí gốc.");
        Ok(())
    }

    // ── Controlled Stop ──────────────────────────────────────────────────────

    /// Giảm tốc và dừng motor an toàn theo biên dạng.
    ///
    /// Khác với EmergencyStop: motor giảm tốc dần thay vì ngắt đột ngột.
    ///
    /// # Arguments
    /// * `actuator_id` - ID motor (0=velocity, 1=position, 0xFF=broadcast).
    pub fn controlled_stop(&mut self, actuator_id: u8) -> Result<(), MotorControlError> {
        self.validate_actuator_id(actuator_id)?;
        log::info!("Dừng an toàn motor: actuator_id={}", actuator_id);

        self.protocol.send_command(
            CommandCode::ControlledStop,
            actuator_id,
            &[],
            0,
        )?;

        log::info!("Lệnh ControlledStop đã được Node xác nhận: actuator_id={}", actuator_id);
        Ok(())
    }

    // ── Emergency Stop ───────────────────────────────────────────────────────

    /// Dừng khẩn cấp TOÀN BỘ motor — ưu tiên cao nhất (FR-12).
    ///
    /// Hành vi:
    /// 1. Gửi EmergencyStop broadcast qua SPI (FLAG_HIGH_PRIORITY)
    /// 2. Lớp Application (caller) nên kích hoạt GPIO E-Stop song song
    ///
    /// Lệnh này gửi broadcast (ACTUATOR_ID_BROADCAST = 0xFF),
    /// không retry, và có cờ ưu tiên cao nhất.
    ///
    /// # Lưu ý
    /// Ngay cả khi SPI thất bại, caller vẫn PHẢI kích hoạt GPIO E-Stop.
    /// Phương thức này trả về Result nhưng caller nên xử lý lỗi bằng cách
    /// log warning, KHÔNG nên dừng quy trình E-Stop vì lỗi SPI.
    pub fn emergency_stop(&mut self) -> Result<(), MotorControlError> {
        log::error!("!!! EMERGENCY STOP — DỪNG KHẨN CẤP TOÀN BỘ MOTOR !!!");

        // Gửi qua SPI — dùng send_emergency_stop cho priority cao nhất
        match self.protocol.send_emergency_stop() {
            Ok(_) => {
                log::error!("EmergencyStop: Node đã xác nhận dừng khẩn cấp");
                Ok(())
            }
            Err(e) => {
                // Log lỗi nhưng vẫn trả về — caller PHẢI kích hoạt GPIO E-Stop
                log::error!(
                    "EmergencyStop SPI thất bại: {}. Caller PHẢI kích hoạt GPIO E-Stop!",
                    e
                );
                Err(MotorControlError::Protocol(e))
            }
        }
    }

    // ── Status Query ─────────────────────────────────────────────────────────

    /// Truy vấn trạng thái hiện tại của motor.
    ///
    /// Gửi StatusQuery và parse StatusResponse payload.
    ///
    /// # Arguments
    /// * `actuator_id` - ID motor cần truy vấn (0 hoặc 1).
    ///
    /// # Returns
    /// `StatusPayload` chứa:
    /// - `motor_state`: trạng thái hoạt động (Disabled, Idle, Moving...)
    /// - `error_code`: mã lỗi hiện tại (0x00 = không lỗi)
    /// - `current_position`: vị trí hiện tại (steps)
    /// - `current_velocity`: vận tốc hiện tại (steps/s)
    pub fn query_status(&mut self, actuator_id: u8) -> Result<StatusPayload, MotorControlError> {
        self.validate_actuator_id_no_broadcast(actuator_id)?;

        log::debug!("Truy vấn trạng thái motor: actuator_id={}", actuator_id);

        let response = self.protocol.send_command(
            CommandCode::StatusQuery,
            actuator_id,
            &[],
            0,
        )?;

        // Parse StatusPayload từ phản hồi
        let status = StatusPayload::deserialize(response.frame.valid_payload())
            .map_err(|e| {
                log::error!("Không thể parse StatusPayload: {:?}", e);
                MotorControlError::CommandError(e)
            })?;

        log::info!(
            "Trạng thái Motor {}: state={:?}, error=0x{:02X}, pos={}, vel={}",
            actuator_id, status.motor_state, status.error_code,
            status.current_position, status.current_velocity
        );

        Ok(status)
    }

    // ── Validation Helpers ───────────────────────────────────────────────────

    /// Validate actuator ID (bao gồm broadcast).
    fn validate_actuator_id(&self, id: u8) -> Result<(), MotorControlError> {
        match id {
            ACTUATOR_ID_VELOCITY_MOTOR
            | ACTUATOR_ID_POSITION_MOTOR
            | ACTUATOR_ID_BROADCAST => Ok(()),
            _ => {
                log::error!("Actuator ID không hợp lệ: {}", id);
                Err(MotorControlError::InvalidParameter(
                    format!("Actuator ID {} không hợp lệ. Hợp lệ: 0, 1, hoặc 0xFF (broadcast)", id),
                ))
            }
        }
    }

    /// Validate actuator ID (KHÔNG bao gồm broadcast — cho StatusQuery).
    fn validate_actuator_id_no_broadcast(&self, id: u8) -> Result<(), MotorControlError> {
        match id {
            ACTUATOR_ID_VELOCITY_MOTOR | ACTUATOR_ID_POSITION_MOTOR => Ok(()),
            _ => {
                log::error!("Actuator ID không hợp lệ cho StatusQuery: {}", id);
                Err(MotorControlError::InvalidParameter(
                    format!(
                        "Actuator ID {} không hợp lệ cho StatusQuery. Hợp lệ: 0 hoặc 1",
                        id
                    ),
                ))
            }
        }
    }
}

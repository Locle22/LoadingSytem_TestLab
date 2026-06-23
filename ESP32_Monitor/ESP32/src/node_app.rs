// =============================================================================
// ESP32/src/node_app.rs
// =============================================================================
// Lớp Application cho Node ESP32-S3 — điều phối lệnh và quản lý motor.
//
// Chịu trách nhiệm:
// - Nhận Frame đã validate từ Protocol layer
// - Kiểm tra watchdog (FR-15)
// - Ưu tiên EmergencyStop (FR-12)
// - Validate actuator ID và khả năng (capability)
// - Validate tham số lệnh (velocity, acceleration, position limits) (FR-19)
// - Dispatch lệnh tới MotorDriver tương ứng
// - Gửi Ack/Nack phản hồi cho mọi lệnh (FR-11)
// - Xử lý hardware e-stop từ GPIO4
//
// Đây là lớp cao nhất, kết nối Protocol ↔ MotorDriver ↔ Watchdog ↔ E-Stop.
// =============================================================================

use esp_println::println;
use spi_protocol::commands::{
    validate_actuator_id, ActuatorCapability, CommandCode, MotorState, PositionCommand,
    StatusPayload, VelocityCommand,
};
use spi_protocol::config::{
    ACTUATOR_COUNT, ACTUATOR_ID_BROADCAST, FRAME_SIZE,
};
use spi_protocol::error::ErrorCode;
use spi_protocol::frame::Frame;

use crate::emergency::HardwareEmergencyStop;
use crate::motor_hal::{MotorDriver, StubMotorDriver};
use crate::node_protocol::NodeProtocol;
use crate::watchdog::SoftwareWatchdog;

// ─── NodeApplication ─────────────────────────────────────────────────────────

/// Lớp Application — bộ não chính của Node ESP32-S3.
///
/// Quản lý 2 motor (Motor 0: velocity-only, Motor 1: position-only),
/// protocol handler, watchdog, và hardware e-stop.
///
/// # Luồng xử lý lệnh
/// ```text
/// SPI RX → Protocol (validate) → Application (dispatch) → MotorDriver
///                                      ↓
///                              Protocol (build response)
///                                      ↓
///                                   SPI TX
/// ```
pub struct NodeApplication {
    /// Motor 0: velocity-only (ACTUATOR_ID_VELOCITY_MOTOR = 0).
    motors: [StubMotorDriver; ACTUATOR_COUNT as usize],

    /// Protocol handler — xử lý frame và tạo response.
    protocol: NodeProtocol,

    /// Watchdog phần mềm — phát hiện mất liên lạc Host (FR-15).
    watchdog: SoftwareWatchdog,

    /// Hardware e-stop — theo dõi GPIO4 (FR-12).
    emergency: HardwareEmergencyStop,

    /// Cờ đánh dấu hệ thống đang ở trạng thái e-stop.
    /// Khi true, chỉ chấp nhận Ping, StatusQuery, và MotorDisable.
    emergency_active: bool,
}

impl NodeApplication {
    /// Tạo NodeApplication mới — tất cả motor Disabled (FR-18).
    pub fn new() -> Self {
        println!("============================================================");
        println!("[APP] Khởi tạo Node Application");
        println!("[APP] Motor count: {}", ACTUATOR_COUNT);
        println!("[APP] Motor 0: Velocity-only, Motor 1: Position-only");
        println!("[APP] Trạng thái khởi động: TẤT CẢ motor Disabled (FR-18)");
        println!("============================================================");

        Self {
            motors: [
                StubMotorDriver::new(0), // Motor 0: velocity-only
                StubMotorDriver::new(1), // Motor 1: position-only
            ],
            protocol: NodeProtocol::new(),
            watchdog: SoftwareWatchdog::new(),
            emergency: HardwareEmergencyStop::new(),
            emergency_active: false,
        }
    }

    /// Xử lý khung truyền thô từ SPI — điểm vào chính.
    ///
    /// # Luồng xử lý
    /// 1. Protocol layer deserialize và validate khung
    /// 2. Feed watchdog (lệnh hợp lệ → reset timer)
    /// 3. Kiểm tra EmergencyStop ưu tiên (FR-12)
    /// 4. Validate actuator ID
    /// 5. Validate actuator capability
    /// 6. Dispatch lệnh cụ thể
    /// 7. Tạo response (Ack/Nack)
    ///
    /// # Arguments
    /// * `raw_data` - Buffer 32 bytes nhận từ SPI slave.
    ///
    /// # Returns
    /// * `Some([u8; 32])` - Khung phản hồi cần gửi lại cho Host.
    /// * `None` - Không cần phản hồi (lỗi nghiêm trọng ở protocol level).
    pub fn process_raw_frame(&mut self, raw_data: &[u8; FRAME_SIZE]) -> Option<[u8; FRAME_SIZE]> {
        // Bước 1: Protocol layer validate khung truyền
        let frame = match self.protocol.process_incoming(raw_data) {
            Ok(f) => f,
            Err(_e) => {
                // Khung không hợp lệ ở mức protocol (sai SOF, CRC, v.v.)
                // Không gửi phản hồi vì không biết sequence/command để NACK
                println!("[APP] Khung bị loại bởi protocol layer — không phản hồi");
                return None;
            }
        };

        // Bước 2: Feed watchdog — nhận được lệnh hợp lệ (FR-15)
        if !self.watchdog.is_enabled() {
            // Kích hoạt watchdog khi nhận lệnh đầu tiên
            self.watchdog.start();
        }
        self.watchdog.feed();

        // Bước 3: Dispatch lệnh
        self.dispatch_command(&frame)
    }

    /// Dispatch lệnh đã validate tới handler tương ứng.
    ///
    /// EmergencyStop được xử lý đầu tiên, trước mọi kiểm tra khác (FR-12).
    fn dispatch_command(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        let cmd = frame.command_code;
        let actuator_id = frame.actuator_id;
        let seq = frame.sequence_number;

        // ── EmergencyStop — ưu tiên cao nhất, bỏ qua mọi kiểm tra (FR-12) ──
        if cmd == CommandCode::EmergencyStop {
            return self.handle_emergency_stop(frame);
        }

        // ── Nếu đang e-stop, chỉ chấp nhận một số lệnh nhất định ──
        if self.emergency_active {
            match cmd {
                CommandCode::Ping | CommandCode::StatusQuery | CommandCode::MotorDisable => {
                    // Cho phép tiếp tục xử lý
                }
                _ => {
                    println!(
                        "[APP] Từ chối lệnh {:?}: hệ thống đang E-Stop",
                        cmd
                    );
                    return self.send_nack(
                        actuator_id,
                        ErrorCode::EmergencyStopActive,
                        cmd,
                        seq,
                    );
                }
            }
        }

        // ── Xử lý lệnh theo loại ──
        match cmd {
            CommandCode::Ping => self.handle_ping(frame),
            CommandCode::StatusQuery => self.handle_status_query(frame),
            CommandCode::MotorEnable => self.handle_motor_enable(frame),
            CommandCode::MotorDisable => self.handle_motor_disable(frame),
            CommandCode::VelocityControl => self.handle_velocity_control(frame),
            CommandCode::PositionControl => self.handle_position_control(frame),
            CommandCode::Homing => self.handle_homing(frame),
            CommandCode::ControlledStop => self.handle_controlled_stop(frame),

            // Lệnh phản hồi (PingResponse, StatusResponse, Ack, Nack) —
            // Node không xử lý vì đây là lệnh Node gửi đi, không nhận
            CommandCode::PingResponse
            | CommandCode::StatusResponse
            | CommandCode::Ack
            | CommandCode::Nack => {
                println!(
                    "[APP] Bỏ qua lệnh response {:?} — Node không xử lý response",
                    cmd
                );
                None
            }

            // EmergencyStop đã xử lý ở trên
            CommandCode::EmergencyStop => unreachable!(),
        }
    }

    // ─── Command Handlers ────────────────────────────────────────────────────

    /// Xử lý Ping — trả về PingResponse (heartbeat).
    fn handle_ping(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        println!("[APP] Ping nhận được (seq={})", frame.sequence_number);
        match self.protocol.build_ping_response(frame.sequence_number) {
            Ok(response) => Some(response),
            Err(e) => {
                println!("[APP] Lỗi tạo PingResponse: {:?}", e);
                None
            }
        }
    }

    /// Xử lý StatusQuery — trả về StatusResponse cho actuator được chỉ định.
    fn handle_status_query(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        let actuator_id = frame.actuator_id;

        // Validate actuator ID
        if let Err(error_code) = validate_actuator_id(actuator_id) {
            return self.send_nack(actuator_id, error_code, CommandCode::StatusQuery, frame.sequence_number);
        }

        // Nếu broadcast, trả về status Motor 0 (quy ước đơn giản)
        let motor_idx = if actuator_id == ACTUATOR_ID_BROADCAST {
            0usize
        } else {
            actuator_id as usize
        };

        let status = StatusPayload {
            motor_state: self.motors[motor_idx].get_state(),
            error_code: self.motors[motor_idx].get_error(),
            current_position: self.motors[motor_idx].get_position(),
            current_velocity: self.motors[motor_idx].get_velocity(),
        };

        match self.protocol.build_status_response(actuator_id, &status) {
            Ok(response) => Some(response),
            Err(e) => {
                println!("[APP] Lỗi tạo StatusResponse: {:?}", e);
                None
            }
        }
    }

    /// Xử lý MotorEnable — kích hoạt driver motor.
    fn handle_motor_enable(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        let actuator_id = frame.actuator_id;
        let seq = frame.sequence_number;

        // Validate actuator ID
        if let Err(error_code) = validate_actuator_id(actuator_id) {
            return self.send_nack(actuator_id, error_code, CommandCode::MotorEnable, seq);
        }

        // Broadcast enable: kích hoạt tất cả motor
        if actuator_id == ACTUATOR_ID_BROADCAST {
            return self.handle_broadcast_enable(seq);
        }

        let motor_idx = actuator_id as usize;
        match self.motors[motor_idx].enable() {
            Ok(()) => self.send_ack(actuator_id, seq),
            Err(error_code) => {
                self.send_nack(actuator_id, error_code, CommandCode::MotorEnable, seq)
            }
        }
    }

    /// Xử lý MotorDisable — vô hiệu hoá driver motor.
    fn handle_motor_disable(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        let actuator_id = frame.actuator_id;
        let seq = frame.sequence_number;

        if let Err(error_code) = validate_actuator_id(actuator_id) {
            return self.send_nack(actuator_id, error_code, CommandCode::MotorDisable, seq);
        }

        // Broadcast disable: vô hiệu hoá tất cả motor
        if actuator_id == ACTUATOR_ID_BROADCAST {
            return self.handle_broadcast_disable(seq);
        }

        let motor_idx = actuator_id as usize;
        match self.motors[motor_idx].disable() {
            Ok(()) => {
                // Nếu tất cả motor đã disable và đang e-stop, cho phép xoá cờ e-stop
                self.check_emergency_clear();
                self.send_ack(actuator_id, seq)
            }
            Err(error_code) => {
                self.send_nack(actuator_id, error_code, CommandCode::MotorDisable, seq)
            }
        }
    }

    /// Xử lý VelocityControl — đặt vận tốc cho Motor 0 (FR-19).
    fn handle_velocity_control(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        let actuator_id = frame.actuator_id;
        let seq = frame.sequence_number;

        // Bước 1: Validate actuator ID
        if let Err(error_code) = validate_actuator_id(actuator_id) {
            return self.send_nack(actuator_id, error_code, CommandCode::VelocityControl, seq);
        }

        // Bước 2: Validate actuator capability — VelocityControl chỉ cho Motor 0
        if actuator_id != ACTUATOR_ID_BROADCAST {
            if let Some(capability) = ActuatorCapability::from_actuator_id(actuator_id) {
                if let Some(error_code) = capability.validate_command(CommandCode::VelocityControl) {
                    println!(
                        "[APP] Actuator {} không hỗ trợ VelocityControl: {:?}",
                        actuator_id, error_code
                    );
                    return self.send_nack(actuator_id, error_code, CommandCode::VelocityControl, seq);
                }
            }
        }

        // Bước 3: Deserialize payload
        let cmd = match VelocityCommand::deserialize(frame.valid_payload()) {
            Ok(c) => c,
            Err(error_code) => {
                println!("[APP] VelocityCommand deserialize thất bại: {:?}", error_code);
                return self.send_nack(actuator_id, error_code, CommandCode::VelocityControl, seq);
            }
        };

        // Bước 4: Validate tham số (FR-19) — kiểm tra velocity, acceleration limits
        if let Err(error_code) = cmd.validate() {
            println!(
                "[APP] VelocityCommand tham số không hợp lệ: {:?} (vel={}, accel={})",
                error_code, cmd.target_velocity, cmd.acceleration
            );
            return self.send_nack(actuator_id, error_code, CommandCode::VelocityControl, seq);
        }

        // Bước 5: Dispatch tới motor driver
        let motor_idx = actuator_id as usize;
        match self.motors[motor_idx].set_velocity(cmd.target_velocity, cmd.acceleration) {
            Ok(()) => self.send_ack(actuator_id, seq),
            Err(error_code) => {
                self.send_nack(actuator_id, error_code, CommandCode::VelocityControl, seq)
            }
        }
    }

    /// Xử lý PositionControl — di chuyển tới vị trí đích cho Motor 1 (FR-19).
    fn handle_position_control(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        let actuator_id = frame.actuator_id;
        let seq = frame.sequence_number;

        // Bước 1: Validate actuator ID
        if let Err(error_code) = validate_actuator_id(actuator_id) {
            return self.send_nack(actuator_id, error_code, CommandCode::PositionControl, seq);
        }

        // Bước 2: Validate capability — PositionControl chỉ cho Motor 1
        if actuator_id != ACTUATOR_ID_BROADCAST {
            if let Some(capability) = ActuatorCapability::from_actuator_id(actuator_id) {
                if let Some(error_code) = capability.validate_command(CommandCode::PositionControl) {
                    println!(
                        "[APP] Actuator {} không hỗ trợ PositionControl: {:?}",
                        actuator_id, error_code
                    );
                    return self.send_nack(actuator_id, error_code, CommandCode::PositionControl, seq);
                }
            }
        }

        // Bước 3: Deserialize payload
        let cmd = match PositionCommand::deserialize(frame.valid_payload()) {
            Ok(c) => c,
            Err(error_code) => {
                println!("[APP] PositionCommand deserialize thất bại: {:?}", error_code);
                return self.send_nack(actuator_id, error_code, CommandCode::PositionControl, seq);
            }
        };

        // Bước 4: Validate tham số (FR-19) — position, velocity, acceleration limits
        if let Err(error_code) = cmd.validate() {
            println!(
                "[APP] PositionCommand tham số không hợp lệ: {:?} (pos={}, vel={}, accel={})",
                error_code, cmd.target_position, cmd.max_velocity, cmd.acceleration
            );
            return self.send_nack(actuator_id, error_code, CommandCode::PositionControl, seq);
        }

        // Bước 5: Dispatch tới motor driver
        let motor_idx = actuator_id as usize;
        match self.motors[motor_idx].move_to_position(
            cmd.target_position,
            cmd.max_velocity,
            cmd.acceleration,
        ) {
            Ok(()) => self.send_ack(actuator_id, seq),
            Err(error_code) => {
                self.send_nack(actuator_id, error_code, CommandCode::PositionControl, seq)
            }
        }
    }

    /// Xử lý Homing — đưa Motor 1 về vị trí gốc tham chiếu.
    fn handle_homing(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        let actuator_id = frame.actuator_id;
        let seq = frame.sequence_number;

        if let Err(error_code) = validate_actuator_id(actuator_id) {
            return self.send_nack(actuator_id, error_code, CommandCode::Homing, seq);
        }

        // Validate capability — Homing chỉ cho Motor 1 (position-only)
        if actuator_id != ACTUATOR_ID_BROADCAST {
            if let Some(capability) = ActuatorCapability::from_actuator_id(actuator_id) {
                if let Some(error_code) = capability.validate_command(CommandCode::Homing) {
                    println!(
                        "[APP] Actuator {} không hỗ trợ Homing: {:?}",
                        actuator_id, error_code
                    );
                    return self.send_nack(actuator_id, error_code, CommandCode::Homing, seq);
                }
            }
        }

        let motor_idx = actuator_id as usize;
        match self.motors[motor_idx].start_homing() {
            Ok(()) => self.send_ack(actuator_id, seq),
            Err(error_code) => {
                self.send_nack(actuator_id, error_code, CommandCode::Homing, seq)
            }
        }
    }

    /// Xử lý ControlledStop — giảm tốc dừng motor an toàn.
    fn handle_controlled_stop(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        let actuator_id = frame.actuator_id;
        let seq = frame.sequence_number;

        if let Err(error_code) = validate_actuator_id(actuator_id) {
            return self.send_nack(actuator_id, error_code, CommandCode::ControlledStop, seq);
        }

        // Broadcast: dừng tất cả motor
        if actuator_id == ACTUATOR_ID_BROADCAST {
            return self.handle_broadcast_controlled_stop(seq);
        }

        let motor_idx = actuator_id as usize;
        match self.motors[motor_idx].controlled_stop() {
            Ok(()) => self.send_ack(actuator_id, seq),
            Err(error_code) => {
                self.send_nack(actuator_id, error_code, CommandCode::ControlledStop, seq)
            }
        }
    }

    /// Xử lý EmergencyStop — dừng tất cả motor ngay lập tức (FR-12).
    ///
    /// EmergencyStop luôn thành công và ảnh hưởng tất cả motor (broadcast).
    /// Không cần validate actuator ID hay capability.
    fn handle_emergency_stop(&mut self, frame: &Frame) -> Option<[u8; FRAME_SIZE]> {
        println!("[APP] !!! PROCESSING EMERGENCY STOP !!!");

        // Dừng khẩn cấp TẤT CẢ motor — không kiểm tra trạng thái
        for i in 0..(ACTUATOR_COUNT as usize) {
            self.motors[i].emergency_stop();
        }

        // Đặt cờ e-stop toàn hệ thống
        self.emergency_active = true;

        // EmergencyStop luôn trả về ACK (luôn thành công)
        self.send_ack(ACTUATOR_ID_BROADCAST, frame.sequence_number)
    }

    // ─── Broadcast Helpers ───────────────────────────────────────────────────

    /// Broadcast enable: kích hoạt tất cả motor.
    fn handle_broadcast_enable(&mut self, seq: u8) -> Option<[u8; FRAME_SIZE]> {
        println!("[APP] Broadcast MotorEnable cho tất cả {} motor", ACTUATOR_COUNT);

        let mut last_error: Option<ErrorCode> = None;
        for i in 0..(ACTUATOR_COUNT as usize) {
            if let Err(e) = self.motors[i].enable() {
                println!("[APP] Motor {} enable thất bại: {:?}", i, e);
                last_error = Some(e);
            }
        }

        match last_error {
            None => self.send_ack(ACTUATOR_ID_BROADCAST, seq),
            Some(error_code) => {
                self.send_nack(ACTUATOR_ID_BROADCAST, error_code, CommandCode::MotorEnable, seq)
            }
        }
    }

    /// Broadcast disable: vô hiệu hoá tất cả motor.
    fn handle_broadcast_disable(&mut self, seq: u8) -> Option<[u8; FRAME_SIZE]> {
        println!("[APP] Broadcast MotorDisable cho tất cả {} motor", ACTUATOR_COUNT);

        let mut last_error: Option<ErrorCode> = None;
        for i in 0..(ACTUATOR_COUNT as usize) {
            if let Err(e) = self.motors[i].disable() {
                println!("[APP] Motor {} disable thất bại: {:?}", i, e);
                last_error = Some(e);
            }
        }

        self.check_emergency_clear();

        match last_error {
            None => self.send_ack(ACTUATOR_ID_BROADCAST, seq),
            Some(error_code) => {
                self.send_nack(ACTUATOR_ID_BROADCAST, error_code, CommandCode::MotorDisable, seq)
            }
        }
    }

    /// Broadcast controlled stop: dừng tất cả motor an toàn.
    fn handle_broadcast_controlled_stop(&mut self, seq: u8) -> Option<[u8; FRAME_SIZE]> {
        println!("[APP] Broadcast ControlledStop cho tất cả {} motor", ACTUATOR_COUNT);

        let mut last_error: Option<ErrorCode> = None;
        for i in 0..(ACTUATOR_COUNT as usize) {
            if let Err(e) = self.motors[i].controlled_stop() {
                println!("[APP] Motor {} controlled_stop thất bại: {:?}", i, e);
                last_error = Some(e);
            }
        }

        match last_error {
            None => self.send_ack(ACTUATOR_ID_BROADCAST, seq),
            Some(error_code) => {
                self.send_nack(ACTUATOR_ID_BROADCAST, error_code, CommandCode::ControlledStop, seq)
            }
        }
    }

    // ─── Watchdog & E-Stop Public API ────────────────────────────────────────

    /// Kiểm tra watchdog — gọi mỗi vòng lặp chính.
    ///
    /// Nếu watchdog hết hạn (FR-15), kích hoạt e-stop tất cả motor.
    ///
    /// # Arguments
    /// * `elapsed_ms` - Thời gian đã trôi qua kể từ lần kiểm tra trước.
    pub fn check_watchdog(&mut self, elapsed_ms: u64) {
        self.watchdog.tick(elapsed_ms);

        if self.watchdog.is_expired() {
            println!("[APP] !!! WATCHDOG TIMEOUT — kích hoạt E-Stop toàn bộ !!!");
            // Dừng khẩn cấp tất cả motor
            for i in 0..(ACTUATOR_COUNT as usize) {
                self.motors[i].emergency_stop();
            }
            self.emergency_active = true;
        }
    }

    /// Kiểm tra hardware e-stop GPIO — gọi mỗi vòng lặp chính.
    ///
    /// # Arguments
    /// * `gpio4_is_low` - `true` nếu GPIO4 đang ở mức LOW (nút e-stop nhấn).
    pub fn check_hardware_estop(&mut self, gpio4_is_low: bool) {
        if self.emergency.check_and_update(gpio4_is_low) {
            println!("[APP] !!! HARDWARE E-STOP — kích hoạt E-Stop toàn bộ !!!");
            for i in 0..(ACTUATOR_COUNT as usize) {
                self.motors[i].emergency_stop();
            }
            self.emergency_active = true;
        }
    }

    /// Kiểm tra hệ thống có đang ở trạng thái e-stop không.
    pub fn is_emergency_active(&self) -> bool {
        self.emergency_active
    }

    // ─── Private Helpers ─────────────────────────────────────────────────────

    /// Gửi Ack response.
    fn send_ack(&mut self, actuator_id: u8, acked_seq: u8) -> Option<[u8; FRAME_SIZE]> {
        match self.protocol.build_ack(actuator_id, acked_seq) {
            Ok(response) => Some(response),
            Err(e) => {
                println!("[APP] Lỗi tạo ACK: {:?}", e);
                None
            }
        }
    }

    /// Gửi Nack response với mã lỗi.
    fn send_nack(
        &mut self,
        actuator_id: u8,
        error_code: ErrorCode,
        failed_cmd: CommandCode,
        failed_seq: u8,
    ) -> Option<[u8; FRAME_SIZE]> {
        match self
            .protocol
            .build_nack(actuator_id, error_code, failed_cmd, failed_seq)
        {
            Ok(response) => Some(response),
            Err(e) => {
                println!("[APP] Lỗi tạo NACK: {:?}", e);
                None
            }
        }
    }

    /// Kiểm tra xem có thể xoá cờ e-stop không.
    /// Xoá khi tất cả motor đã disable sau e-stop.
    fn check_emergency_clear(&mut self) {
        if !self.emergency_active {
            return;
        }

        // Kiểm tra tất cả motor đã disabled
        let all_disabled = (0..(ACTUATOR_COUNT as usize))
            .all(|i| self.motors[i].get_state() == MotorState::Disabled);

        if all_disabled && !self.emergency.is_activated() {
            self.emergency_active = false;
            println!("[APP] E-Stop đã xoá — tất cả motor Disabled, hệ thống sẵn sàng");
        }
    }
}

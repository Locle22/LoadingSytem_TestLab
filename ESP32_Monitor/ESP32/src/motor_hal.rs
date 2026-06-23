// =============================================================================
// ESP32/src/motor_hal.rs
// =============================================================================
// Lớp trừu tượng phần cứng motor (Hardware Abstraction Layer).
//
// Định nghĩa trait `MotorDriver` cho phép thay thế driver thật bằng stub
// trong quá trình phát triển. `StubMotorDriver` chỉ in lệnh ra serial,
// đội ngũ phần cứng sẽ implement driver thật sau.
//
// Mọi motor PHẢI khởi động ở trạng thái Disabled (FR-18).
// =============================================================================

use esp_println::println;
use spi_protocol::error::ErrorCode;
use spi_protocol::commands::MotorState;

// ─── MotorDriver Trait ───────────────────────────────────────────────────────

/// Giao diện trừu tượng cho driver điều khiển motor.
///
/// Đội ngũ phần cứng sẽ implement trait này với driver thật (TMC2209, A4988, v.v.).
/// Trong giai đoạn phát triển, sử dụng `StubMotorDriver` để mô phỏng.
pub trait MotorDriver {
    /// Kích hoạt driver motor, cho phép vận hành.
    /// Chuyển từ Disabled → Idle.
    fn enable(&mut self) -> Result<(), ErrorCode>;

    /// Vô hiệu hoá driver motor, ngăn vận hành.
    /// Chuyển về Disabled từ bất kỳ trạng thái nào (trừ EmergencyStopped).
    fn disable(&mut self) -> Result<(), ErrorCode>;

    /// Đặt vận tốc đích (steps/s) và gia tốc (steps/s²).
    /// Chỉ hợp lệ cho motor velocity-only (Motor 0).
    fn set_velocity(&mut self, target_velocity: i32, acceleration: u32) -> Result<(), ErrorCode>;

    /// Di chuyển tới vị trí đích với vận tốc tối đa và gia tốc.
    /// Chỉ hợp lệ cho motor position-only (Motor 1).
    fn move_to_position(
        &mut self,
        target_position: i32,
        max_velocity: u32,
        acceleration: u32,
    ) -> Result<(), ErrorCode>;

    /// Bắt đầu quá trình homing (tìm điểm gốc tham chiếu).
    /// Chỉ hợp lệ cho motor position-only (Motor 1).
    fn start_homing(&mut self) -> Result<(), ErrorCode>;

    /// Giảm tốc và dừng motor theo biên dạng an toàn.
    fn controlled_stop(&mut self) -> Result<(), ErrorCode>;

    /// Dừng motor ngay lập tức — ưu tiên cao nhất (FR-12).
    /// Luôn thành công, chuyển sang trạng thái EmergencyStopped.
    fn emergency_stop(&mut self);

    /// Lấy trạng thái hoạt động hiện tại của motor.
    fn get_state(&self) -> MotorState;

    /// Lấy vị trí hiện tại (steps).
    fn get_position(&self) -> i32;

    /// Lấy vận tốc hiện tại (steps/s).
    fn get_velocity(&self) -> i32;

    /// Lấy mã lỗi hiện tại. 0x00 = không có lỗi.
    fn get_error(&self) -> u8;
}

// ─── StubMotorDriver ─────────────────────────────────────────────────────────

/// Driver motor giả lập — chỉ in lệnh ra serial qua `esp_println::println!`.
///
/// Dùng trong giai đoạn phát triển khi chưa có phần cứng motor thật.
/// Theo dõi trạng thái nội bộ (MotorState) và khởi động ở Disabled (FR-18).
pub struct StubMotorDriver {
    /// ID của motor (0 hoặc 1) để phân biệt trong log.
    motor_id: u8,
    /// Trạng thái hiện tại — mặc định Disabled (FR-18).
    state: MotorState,
    /// Vị trí giả lập (steps).
    position: i32,
    /// Vận tốc giả lập (steps/s).
    velocity: i32,
    /// Mã lỗi hiện tại.
    error_code: u8,
}

impl StubMotorDriver {
    /// Tạo StubMotorDriver mới, khởi động ở trạng thái Disabled (FR-18).
    pub fn new(motor_id: u8) -> Self {
        println!("[STUB] Motor {} khởi tạo — trạng thái: Disabled (FR-18)", motor_id);
        Self {
            motor_id,
            state: MotorState::Disabled, // FR-18: Khởi động ở trạng thái an toàn
            position: 0,
            velocity: 0,
            error_code: 0x00,
        }
    }
}

impl MotorDriver for StubMotorDriver {
    fn enable(&mut self) -> Result<(), ErrorCode> {
        println!("[STUB] Motor {} → MotorEnable", self.motor_id);

        // Chỉ cho phép enable từ trạng thái Disabled
        match self.state {
            MotorState::Disabled => {
                self.state = MotorState::Idle;
                self.error_code = 0x00;
                println!("[STUB] Motor {} chuyển sang Idle", self.motor_id);
                Ok(())
            }
            MotorState::EmergencyStopped => {
                println!("[STUB] Motor {} từ chối: đang EmergencyStopped", self.motor_id);
                Err(ErrorCode::EmergencyStopActive)
            }
            _ => {
                println!("[STUB] Motor {} từ chối: trạng thái {:?} không cho phép enable",
                         self.motor_id, self.state);
                Err(ErrorCode::CommandNotAllowedInState)
            }
        }
    }

    fn disable(&mut self) -> Result<(), ErrorCode> {
        println!("[STUB] Motor {} → MotorDisable", self.motor_id);

        // Cho phép disable từ hầu hết trạng thái, trừ EmergencyStopped
        match self.state {
            MotorState::EmergencyStopped => {
                println!("[STUB] Motor {} từ chối: đang EmergencyStopped", self.motor_id);
                Err(ErrorCode::EmergencyStopActive)
            }
            _ => {
                self.state = MotorState::Disabled;
                self.velocity = 0;
                println!("[STUB] Motor {} chuyển sang Disabled", self.motor_id);
                Ok(())
            }
        }
    }

    fn set_velocity(&mut self, target_velocity: i32, acceleration: u32) -> Result<(), ErrorCode> {
        println!(
            "[STUB] Motor {} → VelocityControl: velocity={}, acceleration={}",
            self.motor_id, target_velocity, acceleration
        );

        // Kiểm tra motor có ở trạng thái cho phép nhận lệnh chuyển động không
        if !self.state.can_accept_motion_command() && self.state != MotorState::Moving {
            println!("[STUB] Motor {} từ chối: trạng thái {:?}", self.motor_id, self.state);
            return Err(ErrorCode::CommandNotAllowedInState);
        }

        // Giả lập: cập nhật vận tốc và chuyển sang Moving
        self.velocity = target_velocity;
        self.state = MotorState::Moving;
        println!("[STUB] Motor {} chuyển sang Moving, velocity={}", self.motor_id, target_velocity);
        Ok(())
    }

    fn move_to_position(
        &mut self,
        target_position: i32,
        max_velocity: u32,
        acceleration: u32,
    ) -> Result<(), ErrorCode> {
        println!(
            "[STUB] Motor {} → PositionControl: pos={}, max_vel={}, accel={}",
            self.motor_id, target_position, max_velocity, acceleration
        );

        if !self.state.can_accept_motion_command() {
            println!("[STUB] Motor {} từ chối: trạng thái {:?}", self.motor_id, self.state);
            return Err(ErrorCode::CommandNotAllowedInState);
        }

        // Giả lập: cập nhật vị trí đích và chuyển sang Moving
        self.position = target_position; // Trong thực tế, vị trí sẽ thay đổi dần
        self.state = MotorState::Moving;
        println!("[STUB] Motor {} chuyển sang Moving, target_pos={}", self.motor_id, target_position);
        Ok(())
    }

    fn start_homing(&mut self) -> Result<(), ErrorCode> {
        println!("[STUB] Motor {} → Homing", self.motor_id);

        if !self.state.can_accept_motion_command() {
            println!("[STUB] Motor {} từ chối: trạng thái {:?}", self.motor_id, self.state);
            return Err(ErrorCode::CommandNotAllowedInState);
        }

        self.state = MotorState::Homing;
        println!("[STUB] Motor {} chuyển sang Homing", self.motor_id);
        Ok(())
    }

    fn controlled_stop(&mut self) -> Result<(), ErrorCode> {
        println!("[STUB] Motor {} → ControlledStop", self.motor_id);

        match self.state {
            MotorState::Moving | MotorState::Homing => {
                self.state = MotorState::Stopping;
                self.velocity = 0;
                println!("[STUB] Motor {} chuyển sang Stopping", self.motor_id);
                // Trong thực tế, sẽ giảm tốc dần rồi chuyển về Idle
                // Giả lập: chuyển thẳng về Idle
                self.state = MotorState::Idle;
                println!("[STUB] Motor {} đã dừng → Idle", self.motor_id);
                Ok(())
            }
            MotorState::Idle | MotorState::Disabled => {
                // Đã dừng rồi — không cần làm gì
                println!("[STUB] Motor {} đã ở trạng thái dừng {:?}", self.motor_id, self.state);
                Ok(())
            }
            MotorState::EmergencyStopped => {
                println!("[STUB] Motor {} từ chối: đang EmergencyStopped", self.motor_id);
                Err(ErrorCode::EmergencyStopActive)
            }
            _ => {
                // Stopping, Fault — vẫn chấp nhận controlled stop
                self.velocity = 0;
                self.state = MotorState::Idle;
                Ok(())
            }
        }
    }

    fn emergency_stop(&mut self) {
        // EmergencyStop luôn thành công, ưu tiên cao nhất (FR-12)
        println!("[STUB] Motor {} → !!! EMERGENCY STOP !!!", self.motor_id);
        self.state = MotorState::EmergencyStopped;
        self.velocity = 0;
        println!("[STUB] Motor {} chuyển sang EmergencyStopped", self.motor_id);
    }

    fn get_state(&self) -> MotorState {
        self.state
    }

    fn get_position(&self) -> i32 {
        self.position
    }

    fn get_velocity(&self) -> i32 {
        self.velocity
    }

    fn get_error(&self) -> u8 {
        self.error_code
    }
}

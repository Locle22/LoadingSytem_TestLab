use crate::servo::ContinuousServo;
use std::time::Duration;

// ──────────────────────────────────────────────
// Hằng số chu kỳ xung điều khiển (Duty Cycles)
// ──────────────────────────────────────────────
const DUTY_STOP: u32 = 1229;
const DUTY_SLOW_CW: u32 = 1106;
const DUTY_SLOW_CCW: u32 = 1352;
const MS_PER_DEGREE: u32 = 6;

/// Tầng 2: Thiết lập Bộ phân loại muỗi (Sorter Controller)
/// Quản lý vị trí (góc quay) và thực thi chuyển hướng đĩa phân loại.
pub struct Sorter<'a> {
    servo: ContinuousServo<'a>,
    current_angle: i32,
}

impl<'a> Sorter<'a> {
    /// Tạo mới một bộ phân loại sử dụng Servo tương ứng.
    pub fn new(servo: ContinuousServo<'a>) -> Self {
        Self {
            servo,
            current_angle: 0,
        }
    }

    /// Đưa đĩa quay về vị trí ban đầu (0 độ).
    pub fn reset_to_home(&mut self) -> anyhow::Result<()> {
        self.rotate_to(0)
    }

    /// Xoay mâm phân loại đến một góc mục tiêu bất kỳ (sử dụng giải thuật quay ngắn nhất).
    pub fn rotate_to(&mut self, target_angle: i32) -> anyhow::Result<()> {
        let mut delta = target_angle - self.current_angle;
        
        // Chuẩn hóa góc quay ngắn nhất trong khoảng [-180, 180]
        while delta > 180 {
            delta -= 360;
        }
        while delta < -180 {
            delta += 360;
        }

        if delta == 0 {
            return Ok(());
        }

        let duty = if delta > 0 { DUTY_SLOW_CW } else { DUTY_SLOW_CCW };
        let abs_delta = delta.unsigned_abs();
        let spin_time_ms = abs_delta * MS_PER_DEGREE;

        // Bắt đầu quay
        self.servo.set_duty(duty)?;
        std::thread::sleep(Duration::from_millis(spin_time_ms as u64));
        
        // Dừng quay
        self.servo.set_duty(DUTY_STOP)?;

        self.current_angle = target_angle;
        Ok(())
    }

    /// Lấy góc hiện tại của đĩa xoay.
    pub fn get_current_angle(&self) -> i32 {
        self.current_angle
    }
}

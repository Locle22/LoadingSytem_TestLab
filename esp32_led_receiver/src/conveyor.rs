use crate::servo::ContinuousServo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ──────────────────────────────────────────────
// Hằng số chu kỳ xung điều khiển (Duty Cycles)
// ──────────────────────────────────────────────
const DUTY_STOP: u32 = 1229;
// Đặt duty 1150 (quay chậm vừa đủ vượt qua vùng chết deadband của Servo 360)
const DUTY_MIN_SPEED_CW: u32 = 1150; 

/// Tầng 2: Thiết lập Băng chuyền (Conveyor Controller)
/// Quản lý vận hành băng chuyền kéo mẫu muỗi.
pub struct Conveyor<'a> {
    servo: ContinuousServo<'a>,
    is_running: Arc<AtomicBool>,
}

impl<'a> Conveyor<'a> {
    /// Tạo mới bộ điều khiển băng chuyền từ Servo và biến cờ trạng thái đa luồng.
    pub fn new(servo: ContinuousServo<'a>, is_running: Arc<AtomicBool>) -> Self {
        Self { servo, is_running }
    }

    /// Kích hoạt băng chuyền chạy liên tục ở tốc độ tối thiểu.
    pub fn start(&mut self) -> anyhow::Result<()> {
        self.servo.set_duty(DUTY_MIN_SPEED_CW)?;
        self.is_running.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Kích hoạt phanh dừng khẩn cấp băng chuyền.
    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.servo.set_duty(DUTY_STOP)?;
        self.is_running.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Đảo ngược trạng thái hoạt động hiện tại (Bật -> Tắt hoặc ngược lại).
    pub fn toggle(&mut self) -> anyhow::Result<bool> {
        let current_state = self.is_running.load(Ordering::SeqCst);
        let new_state = !current_state;
        if new_state {
            self.start()?;
        } else {
            self.stop()?;
        }
        Ok(new_state)
    }

    /// Lấy trạng thái hoạt động hiện tại của băng chuyền.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

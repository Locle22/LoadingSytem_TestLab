use esp_idf_hal::ledc::LedcDriver;

/// Tầng 1: Trừu tượng hóa Servo (Hardware Abstraction Layer)
/// Bao bọc LedcDriver để cung cấp các lệnh điều khiển tốc độ/hướng quay cơ bản.
pub struct ContinuousServo<'a> {
    driver: LedcDriver<'a>,
}

impl<'a> ContinuousServo<'a> {
    /// Khởi tạo đối tượng Servo từ driver Ledc đã cấu hình.
    pub fn new(driver: LedcDriver<'a>) -> Self {
        Self { driver }
    }

    /// Thiết lập chu kỳ xung (Duty Cycle) để thay đổi chiều/tốc độ quay.
    pub fn set_duty(&mut self, duty: u32) -> anyhow::Result<()> {
        self.driver.set_duty(duty)?;
        Ok(())
    }
}

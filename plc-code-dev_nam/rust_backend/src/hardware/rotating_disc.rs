// =============================================================================
// LoadingSystem - Rotating Disc Controller (Core: Level 2-5)
// =============================================================================
// Bộ điều khiển chính đĩa xoay, thực hiện toàn bộ logic quy đổi góc-xung,
// phát lệnh Modbus tới PLC và tích hợp cơ chế tự học Level 5.
//
// Mô hình toán học:
//   Encoder 17-bit: 131,072 xung/vòng (360°)
//   P = round(θ × 131,072 / 360)
//   F = round(ω × 131,072 / 360)
//
// Level 2: rotate_degrees(), return_to_origin()
// Level 4: inject_manual_override()
// Level 5: Tự động cập nhật offset_pulses qua learning algorithm
// =============================================================================

use crate::calibration::learning;
use crate::calibration::profile::CalibrationProfile;
use crate::error::LoadingError;
use crate::hardware::modbus_client::{ModbusInterface, COIL_TRIGGER_M0, REG_PULSE_POSITION};

/// Hướng quay đĩa xoay
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    /// Quay sang trái (xung âm)
    Left,
    /// Quay sang phải (xung dương)
    Right,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Left => write!(f, "Left"),
            Direction::Right => write!(f, "Right"),
        }
    }
}

/// Bộ điều khiển đĩa xoay - generic over Modbus interface.
/// Sử dụng generic `M: ModbusInterface` để cho phép swap giữa
/// RealModbusClient (production) và MockModbusClient (testing).
pub struct RotatingDiscController<M: ModbusInterface> {
    /// Client Modbus (thật hoặc mock)
    modbus: M,
    /// Độ phân giải encoder (131,072 xung/vòng cho 17-bit)
    encoder_resolution: u32,
    /// Tần số phát xung mặc định (Hz)
    default_frequency: u32,
    /// Profile hiệu chuẩn (Level 5)
    calibration: CalibrationProfile,
    /// Góc quay tích lũy hiện tại (tracking phần mềm)
    current_angle: f64,
}

impl<M: ModbusInterface> RotatingDiscController<M> {
    /// Tạo controller mới
    ///
    /// # Arguments
    /// * `modbus` - Client Modbus (thật hoặc mock)
    /// * `encoder_resolution` - Độ phân giải encoder (131072)
    /// * `default_frequency` - Tần số xung mặc định (Hz)
    /// * `calibration` - Profile hiệu chuẩn đã load từ đĩa
    pub fn new(
        modbus: M,
        encoder_resolution: u32,
        default_frequency: u32,
        calibration: CalibrationProfile,
    ) -> Self {
        RotatingDiscController {
            modbus,
            encoder_resolution,
            default_frequency,
            calibration,
            current_angle: 0.0,
        }
    }

    // =========================================================================
    // Level 2: Hàm nguyên thủy (Primitives)
    // =========================================================================

    /// Quy đổi góc (độ) sang số xung.
    /// Công thức: P = round(θ × encoder_resolution / 360)
    ///
    /// # Examples
    /// ```text
    /// 90°  → 32,768 xung
    /// 45°  → 16,384 xung
    /// 180° → 65,536 xung
    /// 360° → 131,072 xung
    /// ```
    pub fn degrees_to_pulses(&self, degrees: f64) -> i32 {
        ((degrees * self.encoder_resolution as f64) / 360.0).round() as i32
    }

    /// Quy đổi số xung ngược lại thành góc (độ)
    pub fn pulses_to_degrees(&self, pulses: i32) -> f64 {
        (pulses as f64 * 360.0) / self.encoder_resolution as f64
    }

    /// Quay đĩa xoay một góc θ theo hướng chỉ định.
    /// Tự động áp dụng offset bù sai số từ Level 5.
    ///
    /// # Arguments
    /// * `degrees` - Góc cần quay (giá trị dương)
    /// * `direction` - Hướng quay (Left/Right)
    /// Kiểm tra xem đĩa xoay có đang phát xung (đang di chuyển) hay không
    pub async fn is_moving(&mut self) -> Result<bool, LoadingError> {
        let coils = self.modbus.read_coils(COIL_TRIGGER_M0, 1).await?;
        Ok(coils.first().copied().unwrap_or(false))
    }

    /// Quay đĩa xoay một góc θ theo hướng chỉ định.
    /// Tự động áp dụng offset bù sai số từ Level 5.
    ///
    /// # Arguments
    /// * `degrees` - Góc cần quay (giá trị dương)
    /// * `direction` - Hướng quay (Left/Right)
    /// Quay đĩa xoay một góc θ theo hướng chỉ định kèm tần số phát xung tùy chỉnh.
    pub async fn rotate_degrees_with_freq(
        &mut self,
        degrees: f64,
        direction: Direction,
        custom_freq: Option<u32>,
    ) -> Result<(), LoadingError> {
        if degrees < 0.0 {
            return Err(LoadingError::Command(
                "Degrees must be positive. Use Direction to specify rotation direction.".into(),
            ));
        }

        // Bảo vệ phần cứng: Không phát xung mới nếu PLC đang băm xung chưa xong
        if self.is_moving().await? {
            return Err(LoadingError::Hardware(
                "Rotating disc is currently moving. Command rejected for hardware safety.".into(),
            ));
        }

        let raw_pulses = self.degrees_to_pulses(degrees);

        // Áp dụng offset bù từ calibration (Level 5)
        let final_pulses = match direction {
            Direction::Right => raw_pulses + self.calibration.offset_pulses,
            Direction::Left => -(raw_pulses) - self.calibration.offset_pulses,
        };

        let freq = custom_freq.unwrap_or(self.default_frequency);

        tracing::info!(
            "Rotate {:.2}° {} @ {}Hz → raw={}p, offset={}p, final={}p",
            degrees,
            direction,
            freq,
            raw_pulses,
            self.calibration.offset_pulses,
            final_pulses
        );

        self.execute_pulse_command(final_pulses, freq)
            .await?;

        // Cập nhật góc tracking nội bộ
        match direction {
            Direction::Right => self.current_angle += degrees,
            Direction::Left => self.current_angle -= degrees,
        }

        Ok(())
    }

    /// Quay đĩa xoay một góc θ theo hướng chỉ định (dùng tần số mặc định).
    pub async fn rotate_degrees(
        &mut self,
        degrees: f64,
        direction: Direction,
    ) -> Result<(), LoadingError> {
        self.rotate_degrees_with_freq(degrees, direction, None).await
    }

    /// Đưa đĩa xoay về vị trí gốc (0°).
    /// Tính toán delta xung dựa trên góc tích lũy hiện tại.
    pub async fn return_to_origin(&mut self) -> Result<(), LoadingError> {
        if self.is_moving().await? {
            return Err(LoadingError::Hardware(
                "Rotating disc is currently moving. Command rejected for hardware safety.".into(),
            ));
        }

        let return_pulses = -self.degrees_to_pulses(self.current_angle);

        tracing::info!(
            "Return to origin from {:.2}° → {}p",
            self.current_angle,
            return_pulses
        );

        self.execute_pulse_command(return_pulses, self.default_frequency)
            .await?;
        self.current_angle = 0.0;

        Ok(())
    }

    // =========================================================================
    // Level 4 & 5: Can thiệp thủ công và tự học
    // =========================================================================

    /// Can thiệp thủ công (Level 4) với tự học (Level 5).
    /// Khi chuyên gia nhấn Fine Tune trên HMI, hàm này:
    /// 1. Quy đổi góc điều chỉnh sang xung
    /// 2. Cập nhật offset thông qua thuật toán EMA
    /// 3. Ghi lại lịch sử hiệu chuẩn
    ///
    /// # Arguments
    /// * `adjustment_degrees` - Góc hiệu chỉnh (dương = phải, âm = trái)
    pub fn inject_manual_override(&mut self, adjustment_degrees: f64) {
        let delta_pulses = self.degrees_to_pulses(adjustment_degrees);

        let old_offset = self.calibration.offset_pulses;
        self.calibration.offset_pulses = learning::compute_offset_update(
            self.calibration.offset_pulses,
            delta_pulses,
            self.calibration.learning_coefficient,
        );

        self.calibration
            .add_history_entry(adjustment_degrees, delta_pulses);

        tracing::info!(
            "Manual override: adj={:.2}°, delta={}p, offset: {} → {}",
            adjustment_degrees,
            delta_pulses,
            old_offset,
            self.calibration.offset_pulses
        );
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Lấy góc quay tích lũy hiện tại
    pub fn get_current_angle(&self) -> f64 {
        self.current_angle
    }

    /// Lấy reference tới calibration profile
    pub fn get_calibration(&self) -> &CalibrationProfile {
        &self.calibration
    }

    /// Lấy mutable reference tới calibration profile
    pub fn get_calibration_mut(&mut self) -> &mut CalibrationProfile {
        &mut self.calibration
    }

    /// Lấy reference tới Modbus client (hữu ích cho testing)
    pub fn get_modbus(&self) -> &M {
        &self.modbus
    }

    /// Lấy encoder resolution
    pub fn get_encoder_resolution(&self) -> u32 {
        self.encoder_resolution
    }

    // =========================================================================
    // Internal: Ghi lệnh xung xuống PLC qua Modbus
    // =========================================================================

    /// Ghi lệnh phát xung xuống PLC qua Modbus RTU.
    /// Quy trình:
    /// 1. Phân tách giá trị 32-bit thành hai thanh ghi 16-bit
    /// 2. Ghi đồng thời 4 thanh ghi: D100, D101 (vị trí), D102, D103 (tần số)
    /// 3. SET coil M0 để PLC thực thi lệnh DDRVI
    async fn execute_pulse_command(
        &mut self,
        pulses: i32,
        frequency: u32,
    ) -> Result<(), LoadingError> {
        // Phân tách 32-bit signed thành cặp 16-bit (little-endian cho PLC Delta)
        let p_low = (pulses & 0xFFFF) as u16;
        let p_high = ((pulses >> 16) & 0xFFFF) as u16;
        let f_low = (frequency & 0xFFFF) as u16;
        let f_high = ((frequency >> 16) & 0xFFFF) as u16;

        // Ghi cụm 4 thanh ghi liên tiếp: D100, D101, D102, D103
        // Địa chỉ Modbus gốc: 0x1064
        self.modbus
            .write_multiple_registers(REG_PULSE_POSITION, &[p_low, p_high, f_low, f_high])
            .await?;

        // Kích hoạt coil M0 (0x0800) → PLC thực thi DDRVI
        self.modbus
            .write_single_coil(COIL_TRIGGER_M0, true)
            .await?;

        tracing::debug!(
            "Pulse command sent: pulses={} [0x{:04X}, 0x{:04X}], freq={} [0x{:04X}, 0x{:04X}]",
            pulses, p_low, p_high, frequency, f_low, f_high
        );

        Ok(())
    }
}

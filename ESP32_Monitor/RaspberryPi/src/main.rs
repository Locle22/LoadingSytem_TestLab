// =============================================================================
// RaspberryPi/src/main.rs
// =============================================================================
// Điểm khởi chạy ứng dụng Host trên Raspberry Pi 4.
//
// Tổ chức module:
//   - spi_master:     Lớp Truyền dẫn SPI (chỉ byte thô) — CHỈ trên RPi
//   - mock_transport: Transport giả lập cho development/testing
//   - host_protocol:  Lớp Giao thức (framing, seq#, timeout, retry)
//   - host_app:       Lớp Ứng dụng (API điều khiển motor cấp cao)
//   - emergency:      Xử lý E-Stop qua GPIO (độc lập SPI) — CHỈ trên RPi
//
// Build trên Raspberry Pi thật:
//   cargo build --features rpi
//
// Build trên máy dev (Windows/Mac) dùng MockTransport:
//   cargo build
// =============================================================================

// ── Module declarations ──────────────────────────────────────────────────────

// SPI Master và Emergency GPIO chỉ compile trên RPi (cần rppal, Linux-only)
#[cfg(feature = "rpi")]
mod spi_master;
#[cfg(feature = "rpi")]
mod emergency;

// Mock transport luôn available cho development/testing
mod mock_transport;

// Protocol và Application layer luôn compile (platform-independent)
mod host_protocol;
mod host_app;

use spi_protocol::config::{ACTUATOR_ID_POSITION_MOTOR, ACTUATOR_ID_VELOCITY_MOTOR};

fn main() {
    // ── 1. Khởi tạo logging (CQ-10) ─────────────────────────────────────────
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .format_timestamp_millis()
    .init();

    log::info!("═══════════════════════════════════════════════════════════");
    log::info!("  RPi4 Motor Control Host — Khởi động");
    log::info!("═══════════════════════════════════════════════════════════");

    // ── 2. Chọn Transport dựa trên platform ──────────────────────────────────
    #[cfg(feature = "rpi")]
    {
        run_with_real_hardware();
    }

    #[cfg(not(feature = "rpi"))]
    {
        run_with_mock_transport();
    }

    log::info!("═══════════════════════════════════════════════════════════");
    log::info!("  RPi4 Motor Control Host — Kết thúc");
    log::info!("═══════════════════════════════════════════════════════════");
}

// ── Production mode: SPI thật trên Raspberry Pi ──────────────────────────────

#[cfg(feature = "rpi")]
fn run_with_real_hardware() {
    use crate::spi_master::SpiMaster;
    use crate::host_app::MotorController;

    // Khởi tạo SPI Transport
    let spi = match SpiMaster::new() {
        Ok(spi) => {
            log::info!("SPI Master khởi tạo thành công");
            spi
        }
        Err(e) => {
            log::error!("Không thể khởi tạo SPI Master: {:?}", e);
            log::error!("Kiểm tra: SPI đã bật trong raspi-config? Quyền truy cập /dev/spidev0.0?");
            std::process::exit(1);
        }
    };

    // Khởi tạo Emergency Stop GPIO
    let mut estop = match emergency::EmergencyStopHandler::new() {
        Ok(handler) => {
            log::info!("Emergency Stop handler khởi tạo thành công (GPIO25)");
            handler
        }
        Err(e) => {
            log::error!("Không thể khởi tạo Emergency Stop handler: {}", e);
            std::process::exit(1);
        }
    };

    // Khởi tạo Motor Controller
    let mut controller = MotorController::new(spi);
    log::info!("MotorController sẵn sàng (SPI thật)");

    // Chạy demo
    if let Err(e) = run_demo(&mut controller) {
        log::error!("Demo thất bại: {}", e);
        // Kích hoạt E-Stop qua cả SPI và GPIO
        let _ = controller.emergency_stop();
        estop.activate();
        log::error!("Emergency Stop đã kích hoạt qua cả SPI và GPIO");
    }
}

// ── Development mode: MockTransport trên Windows/Mac ─────────────────────────

#[cfg(not(feature = "rpi"))]
fn run_with_mock_transport() {
    use crate::mock_transport::MockTransport;
    use crate::host_app::MotorController;

    log::info!("╔══════════════════════════════════════════════════════════╗");
    log::info!("║  CHẾ ĐỘ DEVELOPMENT — Sử dụng MockTransport           ║");
    log::info!("║  Để chạy trên RPi thật: cargo build --features rpi     ║");
    log::info!("╚══════════════════════════════════════════════════════════╝");

    let mock = MockTransport::new();
    let mut controller = MotorController::new(mock);
    log::info!("MotorController sẵn sàng (MockTransport)");

    // Chạy demo với MockTransport
    if let Err(e) = run_demo(&mut controller) {
        log::error!("Demo thất bại: {}", e);
    }
}

/// Quy trình demo điều khiển motor — hoạt động với cả SPI thật và MockTransport.
///
/// Minh hoạ cách sử dụng API MotorController theo trình tự:
/// 1. Ping kiểm tra kết nối
/// 2. Enable motor
/// 3. Gửi lệnh điều khiển
/// 4. Truy vấn trạng thái
/// 5. Dừng motor
/// 6. Disable motor
fn run_demo<T: spi_protocol::Transport>(
    controller: &mut host_app::MotorController<T>,
) -> Result<(), Box<dyn std::error::Error>> {
    // ── Bước 1: Kiểm tra kết nối ────────────────────────────────────────────
    log::info!("─── Bước 1: Ping kiểm tra kết nối ───");
    controller.ping()?;
    log::info!("Node phản hồi OK");

    // ── Bước 2: Kích hoạt cả 2 motor ────────────────────────────────────────
    log::info!("─── Bước 2: Kích hoạt motor ───");
    controller.enable_motor(ACTUATOR_ID_VELOCITY_MOTOR)?;
    log::info!("Motor 0 (velocity) đã kích hoạt");

    controller.enable_motor(ACTUATOR_ID_POSITION_MOTOR)?;
    log::info!("Motor 1 (position) đã kích hoạt");

    // ── Bước 3: Đặt vận tốc cho Motor 0 ─────────────────────────────────────
    log::info!("─── Bước 3: Đặt vận tốc Motor 0 ───");
    controller.set_velocity(5000, 2000)?;
    log::info!("Motor 0 đang chạy: 5000 steps/s, gia tốc 2000 steps/s²");

    // ── Bước 4: Di chuyển Motor 1 tới vị trí ─────────────────────────────────
    log::info!("─── Bước 4: Di chuyển Motor 1 tới vị trí ───");
    controller.move_to_position(50_000, 8000, 3000)?;
    log::info!("Motor 1 đang di chuyển tới vị trí 50000 steps");

    // ── Bước 5: Truy vấn trạng thái ─────────────────────────────────────────
    log::info!("─── Bước 5: Truy vấn trạng thái ───");
    let status_0 = controller.query_status(ACTUATOR_ID_VELOCITY_MOTOR)?;
    log::info!("Motor 0: state={:?}, pos={}, vel={}",
        status_0.motor_state, status_0.current_position, status_0.current_velocity);

    let status_1 = controller.query_status(ACTUATOR_ID_POSITION_MOTOR)?;
    log::info!("Motor 1: state={:?}, pos={}, vel={}",
        status_1.motor_state, status_1.current_position, status_1.current_velocity);

    // ── Bước 6: Homing Motor 1 ──────────────────────────────────────────────
    log::info!("─── Bước 6: Homing Motor 1 ───");
    controller.home()?;
    log::info!("Motor 1 đang thực hiện homing...");

    // ── Bước 7: Dừng an toàn Motor 0 ────────────────────────────────────────
    log::info!("─── Bước 7: Dừng an toàn Motor 0 ───");
    controller.controlled_stop(ACTUATOR_ID_VELOCITY_MOTOR)?;
    log::info!("Motor 0 đang giảm tốc dừng...");

    // ── Bước 8: Vô hiệu hoá motor ──────────────────────────────────────────
    log::info!("─── Bước 8: Vô hiệu hoá motor ───");
    controller.disable_motor(ACTUATOR_ID_VELOCITY_MOTOR)?;
    controller.disable_motor(ACTUATOR_ID_POSITION_MOTOR)?;
    log::info!("Tất cả motor đã vô hiệu hoá");

    Ok(())
}

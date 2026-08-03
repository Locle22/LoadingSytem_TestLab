// =============================================================================
// Test: Công thức quy đổi Xung ↔ Góc (Pulse-to-Degree Physics Model)
// =============================================================================
// Kiểm chứng công thức: P = round(θ × 18000 / 360)
//
// Bảng giá trị tham chiếu:
//   90°   → 4500 xung
//   45°   → 2250 xung
//   180°  → 9000 xung
//   360°  → 18000 xung
//   1°    → 50 xung
//   0.5°  → 25 xung
// =============================================================================

use loading_system::calibration::profile::CalibrationProfile;
use loading_system::hardware::modbus_client::MockModbusClient;
use loading_system::hardware::rotating_disc::RotatingDiscController;

/// Helper: Tạo controller mock cho testing
fn create_test_controller() -> RotatingDiscController<MockModbusClient> {
    let mock = MockModbusClient::new();
    let calibration = CalibrationProfile::new(0.1);
    RotatingDiscController::new(mock, 18000, 9000, calibration)
}

// ---------------------------------------------------------------------------
// Test Case: Quy đổi góc → xung
// ---------------------------------------------------------------------------

#[test]
fn test_90_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(90.0), 4500);
}

#[test]
fn test_45_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(45.0), 2250);
}

#[test]
fn test_180_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(180.0), 9000);
}

#[test]
fn test_360_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(360.0), 18000);
}

#[test]
fn test_1_degree_to_pulses() {
    let ctrl = create_test_controller();
    // 1° × 18000 / 360 = 50
    assert_eq!(ctrl.degrees_to_pulses(1.0), 50);
}

#[test]
fn test_0_5_degrees_to_pulses() {
    let ctrl = create_test_controller();
    // 0.5° × 18000 / 360 = 25
    assert_eq!(ctrl.degrees_to_pulses(0.5), 25);
}

#[test]
fn test_0_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(0.0), 0);
}

#[test]
fn test_negative_degrees_to_pulses() {
    let ctrl = create_test_controller();
    // Giá trị âm cũng phải quy đổi đúng
    assert_eq!(ctrl.degrees_to_pulses(-90.0), -4500);
}

#[test]
fn test_small_angle_to_pulses() {
    let ctrl = create_test_controller();
    // 0.1° × 18000 / 360 = 5
    assert_eq!(ctrl.degrees_to_pulses(0.1), 5);
}

// ---------------------------------------------------------------------------
// Test Case: Quy đổi xung → góc
// ---------------------------------------------------------------------------

#[test]
fn test_pulses_to_90_degrees() {
    let ctrl = create_test_controller();
    let degrees = ctrl.pulses_to_degrees(4500);
    assert!((degrees - 90.0).abs() < 0.01);
}

#[test]
fn test_pulses_to_45_degrees() {
    let ctrl = create_test_controller();
    let degrees = ctrl.pulses_to_degrees(2250);
    assert!((degrees - 45.0).abs() < 0.01);
}

#[test]
fn test_pulses_to_360_degrees() {
    let ctrl = create_test_controller();
    let degrees = ctrl.pulses_to_degrees(18000);
    assert!((degrees - 360.0).abs() < 0.01);
}

#[test]
fn test_zero_pulses_to_degrees() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.pulses_to_degrees(0), 0.0);
}

// ---------------------------------------------------------------------------
// Test Case: Tính đối xứng (symmetry)
// ---------------------------------------------------------------------------

#[test]
fn test_roundtrip_conversion() {
    let ctrl = create_test_controller();
    for angle in [0.0, 1.0, 15.0, 30.0, 45.0, 60.0, 90.0, 120.0, 180.0, 270.0, 360.0] {
        let pulses = ctrl.degrees_to_pulses(angle);
        let back = ctrl.pulses_to_degrees(pulses);
        assert!(
            (back - angle).abs() < 0.003,
            "Roundtrip failed for {:.1}°: got {:.4}°",
            angle,
            back
        );
    }
}

#[test]
fn test_encoder_resolution() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.get_encoder_resolution(), 18000);
}

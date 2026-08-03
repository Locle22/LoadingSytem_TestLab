# =============================================================================
# Test: Công thức quy đổi Xung ↔ Góc (Python side)
# =============================================================================
# Kiểm chứng công thức Python khớp với Rust:
#   P = round(θ × 18000 / 360)
# =============================================================================

import unittest
from app import degrees_to_pulses, pulses_to_degrees, ENCODER_RESOLUTION


class TestPulseCalculation(unittest.TestCase):
    """Test công thức quy đổi xung-góc phía Python"""

    def test_encoder_resolution_constant(self):
        self.assertEqual(ENCODER_RESOLUTION, 18000)

    # ---- degrees_to_pulses ----

    def test_90_degrees(self):
        self.assertEqual(degrees_to_pulses(90.0), 4500)

    def test_45_degrees(self):
        self.assertEqual(degrees_to_pulses(45.0), 2250)

    def test_180_degrees(self):
        self.assertEqual(degrees_to_pulses(180.0), 9000)

    def test_360_degrees(self):
        self.assertEqual(degrees_to_pulses(360.0), 18000)

    def test_1_degree(self):
        # 1° × 18000 / 360 = 50
        self.assertEqual(degrees_to_pulses(1.0), 50)

    def test_0_5_degrees(self):
        # 0.5° × 18000 / 360 = 25
        self.assertEqual(degrees_to_pulses(0.5), 25)

    def test_0_degrees(self):
        self.assertEqual(degrees_to_pulses(0.0), 0)

    def test_negative_degrees(self):
        self.assertEqual(degrees_to_pulses(-90.0), -4500)

    def test_small_angle(self):
        # 0.1° × 18000 / 360 = 5
        self.assertEqual(degrees_to_pulses(0.1), 5)

    # ---- pulses_to_degrees ----

    def test_pulses_to_90(self):
        result = pulses_to_degrees(4500)
        self.assertAlmostEqual(result, 90.0, places=1)

    def test_pulses_to_45(self):
        result = pulses_to_degrees(2250)
        self.assertAlmostEqual(result, 45.0, places=1)

    def test_pulses_to_360(self):
        result = pulses_to_degrees(18000)
        self.assertAlmostEqual(result, 360.0, places=1)

    def test_zero_pulses(self):
        self.assertEqual(pulses_to_degrees(0), 0.0)

    # ---- Roundtrip consistency ----

    def test_roundtrip(self):
        """Kiểm tra quy đổi khứ hồi: degrees → pulses → degrees"""
        for angle in [0, 1, 15, 30, 45, 60, 90, 120, 180, 270, 360]:
            pulses = degrees_to_pulses(float(angle))
            back = pulses_to_degrees(pulses)
            self.assertAlmostEqual(
                back, float(angle), delta=0.003,
                msg=f"Roundtrip failed for {angle}°"
            )

    # ---- Consistency with Rust ----

    def test_consistency_with_rust_values(self):
        """Giá trị phải khớp 100% với Rust backend"""
        test_cases = [
            (90.0, 4500),
            (45.0, 2250),
            (180.0, 9000),
            (360.0, 18000),
            (1.0, 50),
            (0.5, 25),
        ]
        for degrees, expected_pulses in test_cases:
            with self.subTest(degrees=degrees):
                self.assertEqual(
                    degrees_to_pulses(degrees), expected_pulses,
                    f"Mismatch for {degrees}°: expected {expected_pulses}"
                )


if __name__ == "__main__":
    unittest.main()

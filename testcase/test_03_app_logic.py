# tests/test_03_app_logic.py
import pytest
from unittest.mock import MagicMock

# --- Hằng số ---
CMD_MOVE_TO = 0x01
STATUS_MOVING = 0x01
STATUS_ALARM = 0x03
ALARM_ENCODER_ERR = 0x05
CONF_THRESHOLD = 0.85

# Hàm giả lập (Mô phỏng logic của rpi_serial_controller.py)
def process_ai_detection(mock_serial_ctrl, confidence, species_id):
    if confidence < CONF_THRESHOLD:
        return False
    mock_serial_ctrl.send_cmd(CMD_MOVE_TO, param1=species_id)
    return True


# ====================================================================
# TC3.1: GIÁ TRỊ BIÊN NGƯỠNG TIN CẬY AI (AI CONFIDENCE BVA)
# ====================================================================
# Test sát sườn độ chính xác Float để tránh sai số dấu phẩy động
AI_BOUNDARIES = [
    (0.0, False),
    (0.8499, False), # Ngay dưới biên -> Bỏ qua
    (0.8500, True),  # Ngay tại biên -> Chấp nhận
    (0.8501, True),  # Ngay trên biên -> Chấp nhận
    (1.0, True)
]

@pytest.mark.parametrize("confidence, expected_trigger", AI_BOUNDARIES)
def test_tc3_1_ai_confidence_boundaries(confidence, expected_trigger):
    """Test bộ lọc AI của Python bằng Mocking (Không cần phần cứng thật)"""
    mock_serial = MagicMock()
    
    is_triggered = process_ai_detection(mock_serial, confidence, species_id=3)
    
    assert is_triggered == expected_trigger
    if expected_trigger:
        mock_serial.send_cmd.assert_called_once_with(CMD_MOVE_TO, param1=3)
    else:
        mock_serial.send_cmd.assert_not_called()


# ====================================================================
# TC3.2: GIÁ TRỊ BIÊN TỌA ĐỘ VẬT LÝ (HARDWARE TARGET BOUNDARIES)
# ====================================================================
# [0, 1, 8, 9] là hợp lệ. [10, 15, 255] là vượt biên (Lưu ý: -1 byte = 255)
HW_BOUNDARIES = [
    (0, STATUS_MOVING, None),
    (1, STATUS_MOVING, None),
    (8, STATUS_MOVING, None),
    (9, STATUS_MOVING, None),
    (10, STATUS_ALARM, ALARM_ENCODER_ERR),
    (15, STATUS_ALARM, ALARM_ENCODER_ERR),
    (255, STATUS_ALARM, ALARM_ENCODER_ERR),
]

@pytest.mark.parametrize("target_jar, expected_status, expected_alarm", HW_BOUNDARIES)
def test_tc3_2_hardware_target_boundaries(esp32, target_jar, expected_status, expected_alarm):
    """Bắn trực tiếp tọa độ biên xuống ESP32 để xem phần cứng có bị lỗi Mảng không"""
    esp32.force_state(0x00) # Trở về IDLE
    esp32.ser.reset_input_buffer()
    
    packet = esp32.build_packet(cmd=CMD_MOVE_TO, p1=target_jar, seq=0xCC)
    esp32.send_raw(packet)
    
    resp = esp32.read_response()
    
    assert resp is not None, "LỖI: ESP32 Crash khi nhận tọa độ biên!"
    assert resp["status"] == expected_status
    if expected_alarm:
        assert resp["alarm"] == expected_alarm

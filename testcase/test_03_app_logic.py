# tests/test_03_app_logic.py
import pytest
from unittest.mock import MagicMock, patch

# Giả định chúng ta import module (rpi_serial_controller)
CONF_THRESHOLD = 0.85

def process_ai_detection(mock_serial_ctrl, confidence, species_id):
    """
    Đây là hàm mô phỏng đoạn logic if/else trong vòng lặp chính của rpi_serial_controller.py
    """
    if confidence < CONF_THRESHOLD:
        print(f"Confidence thấp ({confidence}) -> BỎ QUA")
        return False
    else:
        print(f"Xoay đĩa đến lọ {species_id}...")
        # Gửi lệnh Move To
        mock_serial_ctrl.send_cmd(0x01, param1=species_id)
        return True


def test_tc3_2_ai_noise_filtering():
    """
    TC3.2 - Lọc nhiễu AI bằng ngưỡng Confidence.
    """
    # Tạo một Mock object đại diện cho kết nối Serial
    mock_serial = MagicMock()
    
    # 1. Test trường hợp Confidence THẤP (Ví dụ: 0.80) -> Bị bỏ qua
    is_triggered = process_ai_detection(mock_serial, confidence=0.80, species_id=3)
    
    assert is_triggered is False, "Lỗi: AI vẫn nhận diện dù Confidence dưới ngưỡng an toàn!"
    # Kiểm tra xem send_cmd có bị gọi xuống Serial không? Kỳ vọng là KHÔNG gọi.
    mock_serial.send_cmd.assert_not_called()

    # 2. Test trường hợp Confidence CAO (Ví dụ: 0.95) -> Chấp nhận
    is_triggered_2 = process_ai_detection(mock_serial, confidence=0.95, species_id=4)
    
    assert is_triggered_2 is True
    # Kiểm tra xem send_cmd ĐÃ được gọi với đúng target_jar = 4 chưa
    mock_serial.send_cmd.assert_called_with(0x01, param1=4)
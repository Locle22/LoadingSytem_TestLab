# tests/test_01_protocol.py
import pytest
import time
import random

# Định nghĩa các hằng số
CMD_MOVE_TO = 0x01
CMD_GET_STATUS = 0x08
STATUS_ALARM = 0x03
ALARM_ENCODER_ERR = 0x05


# ====================================================================
# TC1.1: CHECKSUM FUZZING (Tự động sinh 50 gói tin hỏng ngẫu nhiên)
# ====================================================================
# Tạo danh sách 50 bộ số ngẫu nhiên (Lệnh, Param1, Sequence)
FUZZING_DATA = [
    (random.randint(1, 8), random.randint(0, 255), random.randint(0, 255))
    for _ in range(50)
]

@pytest.mark.parametrize("cmd, p1, seq", FUZZING_DATA)
def test_tc1_1_checksum_fuzzing(esp32, cmd, p1, seq):
    """Bắn phá Parser bằng 50 gói tin sai checksum liên tiếp."""
    esp32.ser.reset_input_buffer()
    
    # Đóng gói nhưng bật cờ corrupt_checksum = True
    packet = esp32.build_packet(cmd=cmd, p1=p1, seq=seq, corrupt_chk=True)
    esp32.send_raw(packet)
    
    resp = esp32.read_response(timeout=0.05) # Timeout siêu ngắn
    
    # Assert: ESP32 tuyệt đối không được phản hồi các gói tin rác này
    assert resp is None, f"LỖI BẢO MẬT: Lọt lưới gói tin hỏng! Lệnh: {hex(cmd)}"


# ====================================================================
# TC1.2: HEADER COLLISION (Tiêm Header vào giữa luồng Payload)
# ====================================================================
# Các trường hợp Payload chứa byte 0xAA (giống hệt Header)
COLLISION_MATRIX = [
    (0xAA, 0x00, 0x00, 0x00), # 0xAA nằm ở Param 1
    (0x00, 0xAA, 0x00, 0x00), # 0xAA nằm ở Param 2
    (0x00, 0x00, 0xAA, 0x00), # 0xAA nằm ở Data Low
    (0xAA, 0xAA, 0xAA, 0xAA), # Toàn bộ Payload đều là 0xAA
]

@pytest.mark.parametrize("p1, p2, d_lo, d_hi", COLLISION_MATRIX)
def test_tc1_2_header_collision(esp32, p1, p2, d_lo, d_hi):
    """Kiểm tra parser có bị nhầm lẫn điểm bắt đầu của Frame không."""
    esp32.force_state(0x00) # Đưa về IDLE an toàn
    
    packet = esp32.build_packet(cmd=CMD_MOVE_TO, p1=p1, p2=p2, d_lo=d_lo, d_hi=d_hi, seq=0x77)
    esp32.send_raw(packet)
    
    resp = esp32.read_response()
    
    assert resp is not None, "LỖI: ESP32 crash do xử lý Payload chứa 0xAA"
    assert resp["seq"] == 0x77, "LỖI: Trôi Sequence"
    
    # Vì P1 = 0xAA (170) vượt quá số lọ (9), ESP32 phải văng lỗi ALARM thay vì hiểu nhầm
    if p1 >= 10:
        assert resp["status"] == STATUS_ALARM
        assert resp["alarm"] == ALARM_ENCODER_ERR


# ====================================================================
# TC1.3: PACKET FRAGMENTATION & TIMEOUT (Phân mảnh gói tin)
# ====================================================================
def test_tc1_3_fragmentation_success(esp32):
    """Gửi 3 byte, chờ 50ms, gửi 5 byte còn lại -> Phải ghép thành công."""
    esp32.ser.reset_input_buffer()
    packet = esp32.build_packet(cmd=CMD_GET_STATUS, seq=0x1A)
    
    # Chia làm 2 chunk: 3 byte và 5 byte. Delay 50ms (Nằm trong ngưỡng an toàn)
    esp32.send_fragmented(packet, chunks=[3, 5], delay=0.05)
    
    resp = esp32.read_response()
    assert resp is not None and resp["seq"] == 0x1A, "LỖI: Không thể ghép mảnh gói tin!"

def test_tc1_3_buffer_timeout_drop(esp32):
    """Gửi 4 byte, chờ quá lâu (500ms), ESP32 phải tự hủy buffer để chống kẹt."""
    esp32.ser.reset_input_buffer()
    packet = esp32.build_packet(cmd=CMD_GET_STATUS, seq=0x1B)
    
    # Chia làm 2 chunk: 4 byte và 4 byte. Delay 500ms (Vượt timeout của vi điều khiển)
    esp32.send_fragmented(packet, chunks=[4, 4], delay=0.5)
    
    resp = esp32.read_response(timeout=0.2)
    # Lệnh này phải bị ESP32 drop vì timeout
    assert resp is None, "LỖI: Mạch bị kẹt Buffer lock, không chịu timeout dữ liệu cũ!"


# ====================================================================
# TC1.4: SEQUENCE WRAP-AROUND (Tràn biến đếm)
# ====================================================================
# Test chuỗi sequence liên tục vượt qua giới hạn uint8 (255 -> 0)
@pytest.mark.parametrize("seq_stream", [
    [253, 254, 255, 0, 1, 2], # Wrap around chuẩn
    [10, 10, 10],             # Gửi trùng lặp do mạng nhiễu
])
def test_tc1_4_sequence_validation(esp32, seq_stream):
    """Kiểm tra xử lý đồng bộ chuỗi Sequence"""
    esp32.ser.reset_input_buffer()
    
    for s in seq_stream:
        packet = esp32.build_packet(cmd=CMD_GET_STATUS, seq=s)
        esp32.send_raw(packet)
        resp = esp32.read_response()
        
        assert resp is not None
        assert resp["seq"] == s, f"LỖI ĐỒNG BỘ: RPi gửi {s}, ESP32 trả {resp['seq']}"

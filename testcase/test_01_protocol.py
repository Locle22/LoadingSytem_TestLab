# tests/test_01_protocol.py
import pytest
import time

# Opcode Commands (Đồng bộ với C++ ESP32)
CMD_HOME = 0x02
CMD_GET_STATUS = 0x08
STATUS_NOT_READY = 0x04

def test_tc1_1_happy_case(esp32):
    """
    TC1.1 - Truyền nhận gói tin tiêu chuẩn (Happy Case).
    Gửi lệnh GET_STATUS hợp lệ, mong đợi phản hồi chuẩn checksum.
    """
    # 1. Reset input buffer
    esp32.ser.reset_input_buffer()
    
    # 2. Build gói tin hợp lệ, seq = 0x01
    packet = esp32.build_packet(cmd=CMD_GET_STATUS, seq=0x01)
    esp32.send_raw(packet)
    
    # 3. Đọc phản hồi
    resp = esp32.read_response()
    
    # 4. Asserts (Kỳ vọng)
    assert resp is not None, "Lỗi: ESP32 không phản hồi (Timeout)"
    assert resp["valid_checksum"] is True, "Lỗi: ESP32 gửi lên sai Checksum"
    assert resp["seq"] == 0x01, "Lỗi: Sai số Sequence (Không đồng bộ)"
    # Vì hệ thống vừa mới boot, trạng thái mặc định phải là NOT_READY
    assert resp["status"] == STATUS_NOT_READY, f"Lỗi: Trạng thái sai, nhận được {resp['status']}"


def test_tc1_2_bad_checksum(esp32):
    """
    TC1.2 - Lọc gói tin sai Checksum (Data Corruption).
    Cố tình gửi sai byte Checksum, mong đợi ESP32 hủy lệnh và không phản hồi.
    """
    esp32.ser.reset_input_buffer()
    
    # Tạo lệnh CMD_HOME nhưng cố tình tính sai checksum
    packet = esp32.build_packet(cmd=CMD_HOME, corrupt_checksum=True)
    esp32.send_raw(packet)
    
    # Do ESP32 sẽ drop gói tin này, hàm read_response sẽ bị timeout và trả về None
    resp = esp32.read_response()
    
    assert resp is None, "Lỗi: ESP32 vẫn phản hồi dù gói tin bị sai Checksum!"


def test_tc1_3_alignment_recovery(esp32):
    """
    TC1.3 - Khôi phục đồng bộ Header (Alignment Recovery).
    Bơm rác dữ liệu trước khi gửi gói tin chuẩn.
    """
    esp32.ser.reset_input_buffer()
    
    # 1. Bơm rác (Junk data)
    junk_data = b'\xFF\x00\x3B\x55\x12'
    esp32.send_raw(junk_data)
    
    # 2. Gửi lệnh chuẩn ngay lập tức (không delay)
    packet = esp32.build_packet(cmd=CMD_GET_STATUS, seq=0x03)
    esp32.send_raw(packet)
    
    # 3. Đọc phản hồi
    resp = esp32.read_response()
    
    assert resp is not None, "Lỗi: ESP32 bị kẹt do rác dữ liệu, không thể phục hồi luồng"
    assert resp["valid_checksum"] is True
    assert resp["seq"] == 0x03, "Lỗi: Phục hồi luồng nhưng sai dữ liệu lệnh"


def test_tc1_4_buffer_overflow_prevention(esp32):
    """
    TC1.4 - Ngăn chặn tràn bộ đệm (Buffer Overflow).
    Gửi luồng dữ liệu cực lớn, sau đó xem ESP32 còn sống không.
    """
    esp32.ser.reset_input_buffer()
    
    # 1. Gửi 128 byte rác liên tục
    heavy_junk = b'\x00' * 128
    esp32.send_raw(heavy_junk)
    
    # Cần chờ một chút cho ESP32 parse xong đống rác này
    time.sleep(0.2) 
    
    # 2. Gửi lệnh sống còn (Heartbeat)
    packet = esp32.build_packet(cmd=CMD_GET_STATUS, seq=0x99)
    esp32.send_raw(packet)
    
    resp = esp32.read_response()
    
    assert resp is not None, "Lỗi: ESP32 đã bị Crash hoặc Tràn bộ nhớ (Buffer Overflow)!"
    assert resp["seq"] == 0x99, "Lỗi: Sống sót nhưng xử lý sai sequence"
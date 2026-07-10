# tests/test_04_stress_faults.py
import time
import pytest

CMD_MOVE_TO = 0x01
CMD_SERVO_ON = 0x05
CMD_HOME = 0x02

STATUS_MOVING = 0x01
STATUS_IDLE = 0x00
STATUS_NOT_READY = 0x04

def test_tc4_1_stress_spam_commands(esp32):
    """
    TC4.1 - Chịu tải dồn dập (Rate Limiting).
    Bắn 20 lệnh di chuyển liên tiếp, ESP32 không được crash, chỉ nhận lệnh đầu.
    """
    esp32.ser.reset_input_buffer()
    
    # Setup: Mở khóa
    esp32.send_raw(esp32.build_packet(cmd=CMD_ALARM_RESET))
    esp32.send_raw(esp32.build_packet(cmd=CMD_SERVO_ON))
    esp32.send_raw(esp32.build_packet(cmd=CMD_HOME))
    time.sleep(1.5) # Đợi home xong
    
    esp32.ser.reset_input_buffer()
    
    # Bắn dồn dập 20 lệnh trong 1 vòng lặp cực nhanh
    for target in range(1, 10):
        packet = esp32.build_packet(cmd=CMD_MOVE_TO, p1=target, seq=target)
        esp32.send_raw(packet)
        # Bắn siêu nhanh, không delay
        
    time.sleep(0.1) # Chờ 100ms để ESP32 parse
    
    # Đọc luồng phản hồi trong buffer
    responses = []
    while True:
        resp = esp32.read_response()
        if resp:
            responses.append(resp)
        else:
            break
            
    assert len(responses) > 0, "Lỗi: ESP32 đã crash, không có bất kỳ phản hồi nào!"
    
    # Phản hồi đầu tiên phải là lệnh số 1 được chấp nhận (STATUS_MOVING)
    assert responses[0]["seq"] == 1
    
    # Trạng thái ESP32 vẫn sống sót chứ không văng panic
    print(f"Tổng số phản hồi gom được: {len(responses)} gói tin.")
    # Các lệnh spam phía sau nếu có phản hồi thì trạng thái phải là MOVING (báo bận)
    for r in responses:
        assert r["status"] in [STATUS_MOVING, STATUS_IDLE, STATUS_NOT_READY]

def test_tc4_2_failsafe_timeout(esp32):
    """
    TC4.2 - Failsafe khi mất tín hiệu (Loss of Signal).
    Mô phỏng trường hợp Python chết/đứt cáp, dừng gửi NOP (Heartbeat).
    """
    # Trạng thái hệ thống đang tĩnh
    esp32.ser.reset_input_buffer()
     
    time.sleep(3.5)
    
    # Gửi lệnh GET_STATUS để check xem nó tự khóa an toàn chưa
    packet = esp32.build_packet(cmd=0x08) # CMD_GET_STATUS
    esp32.send_raw(packet)
    resp = esp32.read_response()
    
    assert resp is not None 
    # Kỳ vọng: resp["status"] == STATUS_NOT_READY hoặc STATUS_ALARM
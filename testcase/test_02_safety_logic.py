# tests/test_02_safety_logic.py
import time
import pytest

# Opcode & Status
CMD_MOVE_TO = 0x01
CMD_HOME = 0x02
CMD_STOP = 0x03
CMD_SERVO_ON = 0x05
CMD_SERVO_OFF = 0x06
CMD_ALARM_RESET = 0x07

STATUS_IDLE = 0x00
STATUS_MOVING = 0x01
STATUS_ALARM = 0x03
STATUS_NOT_READY = 0x04

ALARM_ENCODER_ERR = 0x05

def test_tc2_1_move_without_servo(esp32):
    """
    TC2.1 - Chặn lệnh dịch chuyển khi chưa cấp nguồn Servo.
    ESP32 vừa reset, gửi MOVE_TO ngay lập tức.
    """
    esp32.ser.reset_input_buffer()
    
    # Gửi lệnh Move To vị trí lọ 3
    packet = esp32.build_packet(cmd=CMD_MOVE_TO, p1=3)
    esp32.send_raw(packet)
    
    resp = esp32.read_response()
    
    assert resp is not None
    # Trạng thái trả về phải là NOT_READY (0x04) do servo chưa bật
    assert resp["status"] == STATUS_NOT_READY, "Lỗi An toàn: ESP32 vẫn nhận lệnh khi chưa bật Servo!"


def test_tc2_2_out_of_bounds(esp32):
    """
    TC2.2 - Xử lý toạ độ vượt biên.
    """
    esp32.ser.reset_input_buffer()
    
    # 1. Bật Servo & Về Home để mở khóa hệ thống
    esp32.send_raw(esp32.build_packet(cmd=CMD_SERVO_ON))
    esp32.read_response()
    esp32.send_raw(esp32.build_packet(cmd=CMD_HOME))
    time.sleep(1) # Chờ mâm xoay về 0
    esp32.ser.reset_input_buffer()
    
    # 2. Gửi lệnh vượt biên (target_jar = 10, trong khi max là 9)
    packet = esp32.build_packet(cmd=CMD_MOVE_TO, p1=10)
    esp32.send_raw(packet)
    
    resp = esp32.read_response()
    
    assert resp is not None
    assert resp["status"] == STATUS_ALARM, "Lỗi: ESP32 không văng trạng thái ALARM khi vượt biên!"
    assert resp["alarm"] == ALARM_ENCODER_ERR, "Lỗi: Sai mã ALARM (kỳ vọng 0x05 - Encoder/Bounds Error)"

    # Khôi phục trạng thái cho bài test sau
    esp32.send_raw(esp32.build_packet(cmd=CMD_ALARM_RESET))
    time.sleep(0.5)


def test_tc2_4_emergency_stop(esp32):
    """
    TC2.4 - Phản ứng Dừng khẩn cấp (Emergency Stop).
    Gửi lệnh dịch chuyển xa, ngay sau đó bắn lệnh STOP.
    """
    esp32.ser.reset_input_buffer()
    
    # Mở khóa hệ thống
    esp32.send_raw(esp32.build_packet(cmd=CMD_ALARM_RESET))
    esp32.send_raw(esp32.build_packet(cmd=CMD_SERVO_ON))
    time.sleep(0.1)

    # 1. Bắt đầu di chuyển dài đến lọ 8
    esp32.send_raw(esp32.build_packet(cmd=CMD_MOVE_TO, p1=8))
    resp1 = esp32.read_response()
    
    # 2. Ngay lập tức bắn lệnh STOP
    t_start = time.time()
    esp32.send_raw(esp32.build_packet(cmd=CMD_STOP))
    resp2 = esp32.read_response()
    t_reaction = time.time() - t_start
    
    assert resp2 is not None
    assert resp2["status"] == STATUS_ALARM, "Lỗi: ESP32 không chuyển sang ALARM khi bị ngắt khẩn cấp"
    assert t_reaction < 0.1, f"Lỗi Real-time: ESP32 phản hồi STOP quá chậm ({t_reaction:.3f}s), bị chặn bởi hàm delay()!"
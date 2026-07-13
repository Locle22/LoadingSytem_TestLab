# tests/test_02_safety_logic.py
import pytest
import time
import threading

# --- Bảng mã Opcodes ---
CMD_MOVE_TO     = 0x01
CMD_HOME        = 0x02
CMD_STOP        = 0x03
CMD_SERVO_ON    = 0x05
CMD_SERVO_OFF   = 0x06
CMD_ALARM_RESET = 0x07

# --- Bảng mã Status & Alarms ---
STATUS_IDLE        = 0x00
STATUS_MOVING      = 0x01
STATUS_IN_POSITION = 0x02
STATUS_ALARM       = 0x03
STATUS_NOT_READY   = 0x04

ALARM_OVER_CURRENT = 0x01
ALARM_ENCODER_ERR  = 0x05


# ====================================================================
# TC2.1: MA TRẬN KHÓA LIÊN ĐỘNG (INVALID INTERLOCKS)
# ====================================================================
# Ma trận định nghĩa: (Trạng_thái_mục_tiêu, Lệnh_tiêm_vào, Tham_số, Trạng_thái_Kỳ_vọng)
INTERLOCK_MATRIX = [
    # 1. Khi chưa bật Servo (NOT_READY): Cấm mọi lệnh chuyển động
    ("NOT_READY", CMD_MOVE_TO, 3, STATUS_NOT_READY),
    ("NOT_READY", CMD_HOME,    0, STATUS_NOT_READY),
    
    # 2. Khi hệ thống đang Rảnh (IDLE): Các lệnh dư thừa bị bỏ qua an toàn
    ("IDLE", CMD_ALARM_RESET, 0, STATUS_IDLE),
    
    # 3. Khi mâm Đang Xoay (MOVING): Chặn đứng các lệnh gây nhiễu quỹ đạo
    ("MOVING", CMD_HOME,     0, STATUS_MOVING),
    ("MOVING", CMD_SERVO_ON, 0, STATUS_MOVING),
    ("MOVING", CMD_MOVE_TO,  5, STATUS_MOVING),
    
    # 4. Khi đang Báo Lỗi (ALARM): Khóa cứng phần cứng cho đến khi được Reset
    ("ALARM", CMD_MOVE_TO, 2, STATUS_ALARM),
    ("ALARM", CMD_HOME,    0, STATUS_ALARM),
    ("ALARM", CMD_SERVO_ON,0, STATUS_ALARM),
]

def setup_state_for_test(esp32, target_state):
    """Hàm Helper: Đưa ESP32 về đúng trạng thái xuất phát trước khi tiêm lỗi"""
    if target_state == "NOT_READY":
        esp32.reset_mcu()
    elif target_state == "IDLE":
        esp32.force_state(STATUS_IDLE)
    elif target_state == "MOVING":
        esp32.force_state(STATUS_IDLE)
        # Bắn lệnh di chuyển xa (tốn vài giây) để giữ nó ở trạng thái MOVING
        esp32.send_raw(esp32.build_packet(cmd=CMD_MOVE_TO, p1=8))
        time.sleep(0.1) # Chờ MCU cập nhật cờ
    elif target_state == "ALARM":
        esp32.force_state(STATUS_IDLE)
        # Cố tình gửi tọa độ 255 để ép phần cứng văng lỗi ALARM
        esp32.send_raw(esp32.build_packet(cmd=CMD_MOVE_TO, p1=255))
        time.sleep(0.1)

@pytest.mark.parametrize("setup_state, inject_cmd, param, expected_status", INTERLOCK_MATRIX)
def test_tc2_1_invalid_interlocks(esp32, setup_state, inject_cmd, param, expected_status):
    """Bắn phá Ma trận trạng thái: Đảm bảo ESP32 từ chối các lệnh nghịch lý."""
    esp32.ser.reset_input_buffer()
    
    # 1. Đưa vi điều khiển vào thế bí (Setup State)
    setup_state_for_test(esp32, setup_state)
    esp32.ser.reset_input_buffer()
    
    # 2. Tiêm lệnh nghịch lý (Inject Command)
    packet = esp32.build_packet(cmd=inject_cmd, p1=param, seq=0x99)
    esp32.send_raw(packet)
    
    # 3. Đọc phản hồi
    resp = esp32.read_response(timeout=0.3)
    
    # Assert
    assert resp is not None, f"LỖI: ESP32 Crash khi nhận lệnh {hex(inject_cmd)} tại trạng thái {setup_state}"
    assert resp["status"] == expected_status, \\
        f"LỖI BẢO MẬT CƠ KHÍ: ESP32 trả về {hex(resp['status'])} thay vì {hex(expected_status)}"


# ====================================================================
# TC2.2: RACE CONDITION (TƯƠNG TRANH THỜI GIAN) - DỪNG KHẨN CẤP
# ====================================================================
def test_tc2_2_race_condition_emergency_stop(esp32):
    """
    Test Timing: Bắn CMD_STOP đúng vào mili-giây mà mâm xoay sắp dừng (Chuyển tiếp trạng thái).
    Đảm bảo cờ STOP (ALARM) đè được cờ IN_POSITION, không sinh ra lỗi Deadlock.
    """
    esp32.force_state(STATUS_IDLE)
    esp32.ser.reset_input_buffer()

    # Tọa độ 9 tốn một khoảng thời gian T (Giả sử: 2.5 giây)
    packet_move = esp32.build_packet(cmd=CMD_MOVE_TO, p1=9, seq=0x10)
    packet_stop = esp32.build_packet(cmd=CMD_STOP, seq=0x11)
    
    # Luồng 1: Bắn lệnh đi
    esp32.send_raw(packet_move)
    
    # Luồng 2 (Thread): Đợi đúng 2.45s rồi bắn STOP để gây tương tranh
    def inject_stop():
        time.sleep(2.45) # Sát viền thời gian chạm đích
        esp32.send_raw(packet_stop)
        
    t = threading.Thread(target=inject_stop)
    t.start()
    t.join()
    
    # Gom tất cả phản hồi trong buffer
    responses = []
    end_time = time.time() + 1.0
    while time.time() < end_time:
        r = esp32.read_response(timeout=0.1)
        if r:
            responses.append(r)
            
    # Lọc ra gói tin phản hồi cuối cùng
    assert len(responses) > 0, "LỖI: Trục trặc giao tiếp khi Race Condition"
    final_resp = responses[-1]
    
    # Yêu cầu An toàn tối cao: Khi có lệnh STOP, mạch BẮT BUỘC phải chuyển sang ALARM (đèn đỏ)
    assert final_resp["status"] == STATUS_ALARM, \\
        "FATAL BUG: Lệnh Dừng khẩn cấp bị đè mất bởi cờ In_Position do dính Race Condition!"


# ====================================================================
# TC2.3: MA TRẬN KÍCH HOẠT VÀ PHỤC HỒI BÁO ĐỘNG (ALARM RECOVERY)
# ====================================================================
def test_tc2_3_strict_alarm_recovery_sequence(esp32):
    """
    Ép mạch văng Alarm. Sau đó kiểm tra chu trình phục hồi bắt buộc:
    ALARM -> (Gửi Reset) -> NOT_READY -> (Gửi Servo On + Home) -> IDLE
    """
    esp32.force_state(STATUS_IDLE)
    esp32.ser.reset_input_buffer()
    
    # 1. Ép văng lỗi (Target 15 > 9)
    esp32.send_raw(esp32.build_packet(cmd=CMD_MOVE_TO, p1=15, seq=0xA1))
    resp1 = esp32.read_response()
    assert resp1["status"] == STATUS_ALARM and resp1["alarm"] == ALARM_ENCODER_ERR
    
    # 2. Gửi lệnh Reset Lỗi
    esp32.send_raw(esp32.build_packet(cmd=CMD_ALARM_RESET, seq=0xA2))
    resp2 = esp32.read_response()
    assert resp2["status"] == STATUS_NOT_READY, "LỖI: Reset lỗi xong phải về NOT_READY để khóa an toàn!"
    
    # 3. Thử láu cá gửi MOVE_TO ngay lập tức (Chưa Home)
    esp32.send_raw(esp32.build_packet(cmd=CMD_MOVE_TO, p1=1, seq=0xA3))
    resp3 = esp32.read_response()
    assert resp3["status"] == STATUS_NOT_READY, "LỖI BẢO MẬT: Chưa Home lại đã cho phép chạy tiếp!"
    
    # 4. Phục hồi chuẩn mực
    esp32.send_raw(esp32.build_packet(cmd=CMD_SERVO_ON, seq=0xA4))
    time.sleep(0.1)
    esp32.send_raw(esp32.build_packet(cmd=CMD_HOME, seq=0xA5))
    time.sleep(1.0) # Đợi mâm quay về cảm biến Home
    
    # 5. Kiểm tra trạng thái cuối
    esp32.send_raw(esp32.build_packet(cmd=CMD_GET_STATUS, seq=0xA6))
    resp_final = esp32.read_response()
    
    assert resp_final["status"] == STATUS_IDLE, "LỖI: Chu trình phục hồi thất bại, không về được IDLE!"

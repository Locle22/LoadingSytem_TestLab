# tests/test_04_stress_faults.py
import pytest
import time
import serial

CMD_MOVE_TO = 0x01
CMD_GET_STATUS = 0x08
STATUS_MOVING = 0x01
STATUS_IDLE = 0x00
STATUS_NOT_READY = 0x04
STATUS_ALARM = 0x03

# ====================================================================
# TC4.1: BÃO LỆNH (COMMAND FLOODING / STRESS TEST)
# ====================================================================
def test_tc4_1_command_flooding(esp32):
    """Bắn dồn dập 100 lệnh/giây để ép tràn Serial Buffer của ESP32"""
    esp32.force_state(STATUS_IDLE)
    
    # Đóng gói 100 lệnh liên tiếp thành 1 mảng byte khổng lồ (800 bytes)
    massive_payload = bytearray()
    for seq in range(100):
        massive_payload.extend(esp32.build_packet(cmd=CMD_MOVE_TO, p1=5, seq=seq))
        
    # Bắn toàn bộ 800 bytes xuống không có một mili-giây delay nào
    esp32.send_raw(massive_payload)
    
    # Kiểm tra khả năng sinh tồn
    responses = []
    start_time = time.time()
    while time.time() - start_time < 1.0:
        r = esp32.read_response(timeout=0.1)
        if r: responses.append(r)
        
    assert len(responses) > 0, "FATAL CRASH: ESP32 sập Watchdog vì tràn Buffer!"
    
    # Lệnh đầu tiên phải thành công, các lệnh sau nếu có phản hồi phải là MOVING (Bận)
    assert responses[0]["status"] == STATUS_MOVING
    for r in responses[1:]:
        assert r["status"] in [STATUS_MOVING, STATUS_IDLE]


# ====================================================================
# TC4.2: NGÂM HỆ THỐNG (SOAK TESTING)
# ====================================================================
@pytest.mark.slow
def test_tc4_2_soak_test_memory_leak(esp32):
    """
    Test Ngâm (Soak Test): Chạy vòng lặp dài để phát hiện Memory Leak.
    Chỉ chạy khi có marker `pytest -m slow`.
    """
    esp32.force_state(STATUS_IDLE)
    
    # Vòng lặp 1000 lần liên tiếp
    for i in range(1000):
        # Tính toán wrap-around cho Sequence (0-255)
        seq_id = i % 256 
        
        esp32.send_raw(esp32.build_packet(cmd=CMD_GET_STATUS, seq=seq_id))
        resp = esp32.read_response(timeout=0.2)
        
        # Nếu chỉ cần 1 lần timeout -> Hệ thống đã bị lag hoặc crash
        assert resp is not None, f"SOAK TEST FAILED: Memory Leak/Crash tại vòng lặp thứ {i}"
        assert resp["seq"] == seq_id, f"Trôi sequence tại vòng lặp {i}"


# ====================================================================
# TC4.3: TIÊM LỖI VẬT LÝ (LOSS OF SIGNAL / BROWNOUT)
# ====================================================================
def test_tc4_3_physical_fault_loss_of_signal(esp32):
    """
    Mô phỏng đứt cáp: Đang quay thì mất Serial.
    Yêu cầu ESP32 phải tự nhảy về trạng thái An toàn thay vì quay vĩnh viễn.
    """
    esp32.force_state(STATUS_IDLE)
    
    # 1. Bắt đầu quay mâm tốn nhiều thời gian
    esp32.send_raw(esp32.build_packet(cmd=CMD_MOVE_TO, p1=8, seq=0x99))
    time.sleep(0.1) # Chờ lệnh đến nơi
    
    # 2. HÀNH ĐỘNG HỦY DIỆT: Đóng ép cổng COM để giả lập đứt cáp USB
    esp32.ser.close()
    
    # 3. Đợi 2 giây cho thuật toán Heartbeat Watchdog của ESP32 tự kích hoạt (Nếu có)
    time.sleep(2.0)
    
    # 4. Cắm cáp lại
    esp32.ser.open()
    time.sleep(0.5)
    esp32.ser.reset_input_buffer()
    
    # 5. Kiểm tra trạng thái hiện tại
    esp32.send_raw(esp32.build_packet(cmd=CMD_GET_STATUS, seq=0xAA))
    resp = esp32.read_response(timeout=1.0)
    
    assert resp is not None, "ESP32 bị treo cứng sau khi mất tín hiệu!"
    
    # YÊU CẦU: Không được nằm ở trạng thái MOVING. Phải khóa an toàn.
    assert resp["status"] in [STATUS_NOT_READY, STATUS_ALARM], \\
        "LỖI AN TOÀN CHẾT NGƯỜI: Mất tín hiệu máy chủ mà mâm cơ khí vẫn tự động quay!"

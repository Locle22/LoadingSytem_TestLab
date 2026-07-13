# tests/conftest.py
import pytest
import serial
import serial.tools.list_ports
import time

# --- Định nghĩa Hằng số ---
HEADER_CMD = 0xAA
HEADER_STATUS = 0x55

STATUS_IDLE = 0x00
STATUS_NOT_READY = 0x04

class ESP32TestAdapter:
    def __init__(self, port, baudrate=115200):
        # Timeout rất ngắn (0.2s) để bắt lỗi realtime
        self.ser = serial.Serial(port, baudrate, timeout=0.2)
        self.reset_mcu()

    def reset_mcu(self):
        """Reset cứng mạch bằng chân DTR/RTS"""
        self.ser.dtr = False
        self.ser.rts = False
        time.sleep(0.1)
        self.ser.dtr = True
        self.ser.rts = True
        time.sleep(2.0) # Chờ boot animation
        self.ser.reset_input_buffer()
        self.ser.reset_output_buffer()

    def build_packet(self, cmd, p1=0, p2=0, d_lo=0, d_hi=0, seq=0, corrupt_chk=False):
        """Đóng gói và tính XOR Checksum"""
        packet = bytearray([HEADER_CMD, cmd, p1, p2, d_lo, d_hi, seq, 0x00])
        chk = 0
        for i in range(7):
            chk ^= packet[i]
        
        # Tiêm lỗi checksum nếu cần
        if corrupt_chk:
            chk ^= 0xFF  
            
        packet[7] = chk
        return packet

    def send_raw(self, raw_bytes):
        """Gửi nguyên khối"""
        self.ser.write(raw_bytes)
        self.ser.flush()

    def send_fragmented(self, packet, chunks, delay):
        """Băm nhỏ gói tin để test Timeout của UART Buffer"""
        idx = 0
        for chunk_size in chunks:
            self.ser.write(packet[idx:idx+chunk_size])
            self.ser.flush()
            idx += chunk_size
            time.sleep(delay)

    def read_response(self, timeout=0.5):
        """Đọc và bóc tách gói tin (Có timeout bảo vệ)"""
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.ser.in_waiting:
                b = self.ser.read(1)
                if b and b[0] == HEADER_STATUS:
                    rest = self.ser.read(7)
                    if len(rest) == 7:
                        full = b + rest
                        chk = 0
                        for i in range(7): chk ^= full[i]
                        return {
                            "raw": full, "status": full[1], "jar": full[2],
                            "alarm": full[3], "seq": full[6],
                            "valid_checksum": (chk == full[7])
                        }
        return None 

    def force_state(self, target_status):
        """Hàm cực kỳ quan trọng: Ép ESP32 vào trạng thái mong muốn để test"""
        self.reset_mcu()
        if target_status == STATUS_IDLE:
            self.send_raw(self.build_packet(cmd=0x05)) # SERVO_ON
            self.read_response()
            self.send_raw(self.build_packet(cmd=0x02)) # HOME
            time.sleep(1.5) # Chờ mâm xoay về điểm 0
        self.ser.reset_input_buffer()

@pytest.fixture(scope="session")
def esp32():
    """Fixture tự động quét và thiết lập kết nối"""
    target_port = None
    for port in serial.tools.list_ports.comports():
        if "CH340" in port.description or "CP2102" in port.description:
            target_port = port.device
            break
            
    if not target_port:
        pytest.fail("Không tìm thấy mạch ESP32!")
        
    print(f"\n[SETUP] Kết nối ESP32 tại {target_port}")
    adapter = ESP32TestAdapter(target_port)
    yield adapter
    print(f"\n[TEARDOWN] Đóng kết nối {target_port}")
    adapter.close()

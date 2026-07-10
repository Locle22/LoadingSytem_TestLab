# tests 
import pytest
import serial
import serial.tools.list_ports
import time
import struct

# --- Định nghĩa hằng số Giao thức ---
HEADER_CMD = 0xAA
HEADER_STATUS = 0x55

class ESP32TestAdapter:
    def __init__(self, port, baudrate=115200):
        # Mở cổng Serial với timeout 0.5s để test các trường hợp mất gói tin
        self.ser = serial.Serial(port, baudrate, timeout=0.5)
        self.reset_mcu()

    def reset_mcu(self):
        """Khởi động lại mạch cứng ESP32 qua chân DTR/RTS để test case luôn tinh khiết"""
        self.ser.dtr = False
        self.ser.rts = False
        time.sleep(0.1)
        self.ser.dtr = True
        self.ser.rts = True
        # Chờ 2 giây để ESP32 chạy xong boot animation LED
        time.sleep(2.0) 
        self.ser.reset_input_buffer()
        self.ser.reset_output_buffer()

    def build_packet(self, cmd, p1=0, p2=0, d_lo=0, d_hi=0, seq=0, corrupt_checksum=False):
        """Đóng gói 8-byte và tính XOR Checksum"""
        packet = bytearray([HEADER_CMD, cmd, p1, p2, d_lo, d_hi, seq, 0x00])
        chk = 0
        for i in range(7):
            chk ^= packet[i]
        
        # Nếu cố tình tạo lỗi checksum để test
        if corrupt_checksum:
            chk ^= 0xFF  
            
        packet[7] = chk
        return packet

    def send_raw(self, raw_bytes):
        """Bơm dữ liệu thô xuống cổng Serial"""
        self.ser.write(raw_bytes)
        self.ser.flush()

    def read_response(self):
        """Đọc và bóc tách gói tin phản hồi từ ESP32"""
        # Tìm Header 0x55
        start_time = time.time()
        while time.time() - start_time < 0.5:
            b = self.ser.read(1)
            if b and b[0] == HEADER_STATUS:
                # Nếu thấy Header, đọc 7 byte còn lại
                rest = self.ser.read(7)
                if len(rest) == 7:
                    full_pkt = b + rest
                    # Tính checksum kiểm chứng
                    chk = 0
                    for i in range(7):
                        chk ^= full_pkt[i]
                    is_valid = (chk == full_pkt[7])
                    
                    return {
                        "raw": full_pkt,
                        "status": full_pkt[1],
                        "jar": full_pkt[2],
                        "alarm": full_pkt[3],
                        "seq": full_pkt[6],
                        "valid_checksum": is_valid
                    }
        return None # Timeout, không có phản hồi

    def close(self):
        self.ser.close()

@pytest.fixture(scope="session")
def esp32():
    """Fixture tự động quét cổng và cấp phát adapter cho các test case"""
    target_port = None
    ports = serial.tools.list_ports.comports()
    for port in ports:
        if "CH340" in port.description or "CP2102" in port.description:
            target_port = port.device
            break
            
    if not target_port:
        pytest.fail("Không tìm thấy mạch ESP32. Vui lòng cắm cáp USB!")
        
    print(f"\n[SETUP] Đã kết nối ESP32 tại {target_port}")
    adapter = ESP32TestAdapter(target_port)
    
    yield adapter  # Cung cấp adapter cho test function chạy
    
    # Teardown sau khi test xong
    print(f"\n[TEARDOWN] Đóng kết nối Serial {target_port}")
    adapter.close()
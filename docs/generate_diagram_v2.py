import base64
import requests
import zlib
import os

mermaid_code = """sequenceDiagram
    participant Web as Web UI (Browser)
    participant Server as Laptop HTTP Server (Axum)
    participant L3 as API Tầng 3 (Orchestration)
    participant L2 as API Tầng 2 (HardwareBackend)
    participant L1 as Tầng 1 (ESP32 / PLC)

    Note over Web: Người dùng bấm "Mô phỏng" loài ID=5
    Web->>Server: POST /api/simulate_mosquito {class_id: 5}
    Note over Server: Nhận request, định tuyến
    Server->>L3: simulate_mosquito(backend, 5)
    Note over L3: Ánh xạ: ID 5 -> màu RGB & góc 50°
    L3->>L2: backend.send_mosquito_command(R, G, B, 5)
    Note over L2: Đóng gói Opcode 0x01: [0x01, R, G, B, 5]
    L2->>L1: Gửi UDP packet đến port 8888 (hoặc Modbus cho PLC)
    Note over L1: Nhận opcode 0x01, giải mã
    L1->>L1: Cập nhật màu LED WS2812
    L1->>L1: Xoay Servo Sorter đến góc 50°
"""

def download_kroki():
    payload = zlib.compress(mermaid_code.encode('utf-8'), 9)
    encoded = base64.urlsafe_b64encode(payload).decode('utf-8')
    url = f"https://kroki.io/mermaid/png/{encoded}"
    print(f"Fetching from Kroki: {url}")
    response = requests.get(url)
    if response.status_code == 200:
        with open("sequence_diagram_v2.png", "wb") as f:
            f.write(response.content)
        print("Success Kroki!")
        return True
    else:
        print(f"Failed Kroki: {response.status_code} - {response.text}")
        return False

def download_mermaid_ink():
    b64 = base64.b64encode(mermaid_code.encode('utf-8')).decode('utf-8')
    url = f"https://mermaid.ink/img/{b64}"
    print(f"Fetching from Mermaid.ink: {url}")
    response = requests.get(url)
    if response.status_code == 200:
        with open("sequence_diagram_v2.png", "wb") as f:
            f.write(response.content)
        print("Success Mermaid.ink!")
        return True
    else:
        print(f"Failed Mermaid.ink: {response.status_code}")
        return False

if __name__ == '__main__':
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    if not download_kroki():
        print("Trying Mermaid.ink...")
        download_mermaid_ink()

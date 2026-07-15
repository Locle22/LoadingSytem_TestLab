import base64
import requests
import zlib

mermaid_code = """sequenceDiagram
    participant Laptop as Tầng AI (Laptop)
    participant Socket as Kết nối mạng (WiFi UDP)
    participant ESP as Tầng IoT (ESP32 Main Thread)
    participant LED as LED WS2812
    participant Sorter as Servo Sorter (GPIO 4)

    Note over Laptop: 1. AI phát hiện loài muỗi ID=5
    Note over Laptop: 2. get_species_color(5) -> (R:50, G:100, B:200)
    Laptop->>Socket: 3. send_command(50, 100, 200, 5) -> gửi [50, 100, 200, 5]
    Socket->>ESP: 4. Gói tin đến port 8888
    Note over ESP: 5. recv_from() trả về buf = [50, 100, 200, 5]
    Note over ESP: 6. Gán r=50, g=100, b=200, class_id=5
    ESP->>LED: 7. write(RGB8(50, 100, 200)) -> LED đổi màu
    Note over ESP: 8. Tính target_angle = 5 * 10 = 50 độ
    ESP->>Sorter: 9. rotate_to(50) -> Servo xoay mâm đến góc 50°"""

def download_kroki():
    # Kroki implementation
    # zlib compress -> base64 urlsafe
    payload = zlib.compress(mermaid_code.encode('utf-8'), 9)
    encoded = base64.urlsafe_b64encode(payload).decode('utf-8')
    url = f"https://kroki.io/mermaid/png/{encoded}"
    print(f"Fetching from Kroki: {url}")
    response = requests.get(url)
    if response.status_code == 200:
        with open("sequence_diagram.png", "wb") as f:
            f.write(response.content)
        print("Success Kroki!")
        return True
    else:
        print(f"Failed Kroki: {response.status_code} - {response.text}")
        return False

def download_mermaid_ink():
    # Simple base64 for mermaid.ink
    # Wait, simple base64 must be standard base64 of the code
    b64 = base64.b64encode(mermaid_code.encode('utf-8')).decode('utf-8')
    url = f"https://mermaid.ink/img/{b64}"
    print(f"Fetching from Mermaid.ink: {url}")
    response = requests.get(url)
    if response.status_code == 200:
        with open("sequence_diagram.png", "wb") as f:
            f.write(response.content)
        print("Success Mermaid.ink!")
        return True
    else:
        print(f"Failed Mermaid.ink: {response.status_code}")
        return False

if not download_kroki():
    print("Trying Mermaid.ink...")
    download_mermaid_ink()

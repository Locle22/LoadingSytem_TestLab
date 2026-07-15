# 🦟 AI Mosquito Detector (V2 - Multi-Platform & Web Control)

Hệ thống Trí tuệ Nhân tạo nhận diện thời gian thực 36 loài muỗi thông qua Camera điện thoại (DroidCam) kết hợp điều khiển mâm xoay phân loại và dải đèn LED RGB qua Vi điều khiển ESP32 S3 hoặc PLC công nghiệp, kèm giao diện Web giám sát & mô phỏng cục bộ.

---

## 🛠 1. Yêu cầu hệ thống (System Requirements)

### Phía Máy tính (PC / Laptop)
* **Hệ điều hành:** Windows 10/11 (Hoặc Linux/macOS).
* **Rust Toolchain:** Phiên bản ổn định mới nhất.
* **Node.js:** Phiên bản 18+ (Dành cho Web UI).
* **Model AI:** File mô hình YOLOv9e định dạng `.onnx` (Nạp tại `app_cli/models/moquito_v19_e150.onnx`).

### Phía Thiết bị ngoại vi
* **Điện thoại:** Cài đặt ứng dụng **DroidCam** làm IP Camera.
* **Vi điều khiển:** **ESP32 S3 WROOM** hoặc **PLC công nghiệp** (hỗ trợ Driver).
* **Ngoại vi kết nối ESP32:** Đèn LED WS2812 (GPIO 48), Servo Sorter (GPIO 4), Servo Conveyor (GPIO 5), RFID RC522 (SPI2).

---

## 🚀 2. Hướng dẫn chạy nhanh từng thành phần (Execution Guide)

### BƯỚC 1: Nạp Code ESP32 S3 (Firmware)
1. Cắm cáp kết nối ESP32 S3 vào máy tính.
2. Cấu hình WiFi (SSID và Mật khẩu) trong file **`esp32_led_receiver/src/system.rs`** (dòng 47-48).
3. Di chuyển vào thư mục và nạp code bằng lệnh:
   ```bash
   cd esp32_led_receiver
   cargo espflash flash --release --monitor
   ```
   *(Sử dụng cờ `--release` để tối ưu hóa kích thước nhị phân vừa vặn phân vùng 1MB của ESP32. Ghi lại địa chỉ IP của mạch hiển thị trên Monitor).*

### BƯỚC 2: Khởi động Laptop Backend (Rust + HTTP Server)
Mở một Terminal mới ở thư mục gốc của dự án và chạy:

* **Chạy với ESP32 S3 thật:**
  ```bash
  cargo run --release -p app_cli -- --model "app_cli/models/moquito_v19_e150.onnx" --camera "http://IP_DROIDCAM:4747/video" --esp-ip "IP_ESP32"
  ```
* **Chạy với chế độ mô phỏng PLC công nghiệp:**
  ```bash
  cargo run --release -p app_cli -- --model "app_cli/models/moquito_v19_e150.onnx" --camera "http://IP_DROIDCAM:4747/video" --plc
  ```

### BƯỚC 3: Khởi động Web UI (Vite)
1. Mở Terminal khác di chuyển vào thư mục Web:
   ```bash
   cd web_ui
   npm install
   npm run dev
   ```
2. Truy cập [http://localhost:5173/](http://localhost:5173/) trên trình duyệt.

---

## 🧪 3. Chế độ kiểm thử nhanh không cần Phần cứng/Camera (Mock Mode)

Để hỗ trợ kiểm thử nhanh thiết kế giao diện Web UI và kiểm tra phản hồi API mà không cần kết nối camera điện thoại hay mạch thật:

1. Chạy Mock Backend bằng Python:
   ```bash
   cd web_ui
   python mock_backend.py
   ```
   *(Mock server sẽ chạy trên cổng `3000` thay thế cho Rust backend).*
2. Chạy Web UI:
   ```bash
   cd web_ui
   npm run dev
   ```
3. Truy cập [http://localhost:5173/](http://localhost:5173/), kích hoạt **Chế độ Mô phỏng (Simulation Mode)** để click chọn loài muỗi giả lập, điều khiển mâm xoay ảo và kiểm thử trực quan.

---

## 📡 4. Giao thức UDP Opcode V2
Gói tin UDP gửi từ Laptop xuống cổng `8888` của ESP32 được cấu trúc theo dạng Opcode ở byte đầu tiên:
* **`0x01 [R, G, B, class_id]` (5 bytes):** Lệnh phân loại muỗi. ESP32 sẽ cập nhật màu dải LED WS2812 theo màu RGB và xoay servo mâm phân loại đến góc `class_id * 10` độ.
* **`0x02 [angle_high, angle_low]` (3 bytes):** Xoay mâm tuyệt đối đến góc gửi kèm (Ghép byte dạng `i16`).
* **`0x03` (1 byte):** Đưa mâm phân loại trở về vị trí gốc (Home 0°).
* **Tương thích ngược:** Gói tin 4-byte kiểu cũ (`[R, G, B, class_id]`) vẫn được ESP32 nhận diện và thực thi tự động.

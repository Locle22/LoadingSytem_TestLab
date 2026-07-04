# 🦟 AI Mosquito Detector (Real-time & IoT)

Hệ thống Trí tuệ Nhân tạo nhận diện thời gian thực 36 loài muỗi thông qua Camera điện thoại (DroidCam) kết hợp cảnh báo vật lý bằng Đèn LED RGB qua Vi điều khiển ESP32 S3 (Giao thức UDP).

## 🛠 1. Yêu cầu hệ thống (System Requirements)

### Phía Máy tính (PC / Laptop)
* **Hệ điều hành:** Windows 10/11 (Hoặc Linux/macOS).
* **Rust Toolchain:** Cài đặt qua [Rustup](https://rustup.rs/).
* **CUDA / GPU (Tùy chọn):** Card đồ họa NVIDIA để tăng tốc độ Inference (nếu không có sẽ tự chạy bằng CPU).
* **Model AI:** File mô hình YOLOv9e định dạng `.onnx` (Do dung lượng >200MB, file này không có trên GitHub, bạn cần tải riêng).

### Phía Thiết bị ngoại vi
* **Điện thoại:** Cài đặt ứng dụng **DroidCam** (Android / iOS) để làm IP Camera.
* **Vi điều khiển:** Mạch **ESP32 S3 WROOM** (Có tích hợp đèn LED WS2812 ở chân GPIO 48).
* **Phần mềm nạp ESP32:** Cài đặt công cụ nạp ROM của Rust bằng lệnh:
  ```bash
  cargo install espflash
  ```

---

## ⚙️ 2. Hướng dẫn Tải & Cấu hình (Configuration)

### Bước 2.1: Chuẩn bị mã nguồn và Model
1. Mở Terminal / PowerShell và clone dự án về máy:
   ```bash
   git clone https://github.com/Locle22/LoadingSytem_TestLab.git
   cd LoadingSytem_TestLab
   ```
2. Chép file Model AI (`moquito_v19_e150.onnx`) của bạn vào thư mục theo đường dẫn sau:
   `app_cli/models/moquito_v19_e150.onnx`

### Bước 2.2: Cấu hình Điện thoại (Camera)
1. Kết nối Điện thoại và Máy tính vào **cùng một mạng WiFi**.
2. Mở ứng dụng **DroidCam** trên điện thoại.
3. Ghi nhớ **IP** và **Port** hiện trên màn hình điện thoại (Ví dụ: `http://192.168.1.5:4747`).

---

## 🚀 3. Hướng dẫn Chạy (Step-by-Step Execution)

Hệ thống hoạt động với 2 thành phần độc lập, bạn cần bật ESP32 trước, sau đó bật AI trên Máy tính.

### BƯỚC 3.1: Chạy ESP32 S3 (Đèn Cảnh Báo IoT)
1. Cắm cáp kết nối ESP32 S3 vào máy tính.
2. Mở một Terminal mới, di chuyển vào thư mục ESP32:
   ```bash
   cd esp32_led_receiver
   ```
3. (Tùy chọn) Sửa Tên WiFi và Mật khẩu trong file `src/main.rs` (dòng 24-25) cho khớp với mạng nhà bạn.
4. Nạp code vào vi điều khiển:
   ```bash
   cargo espflash flash --monitor
   ```
5. Nhìn vào màn hình Terminal, đợi ESP32 kết nối WiFi thành công và **ghi nhớ địa chỉ IP của ESP32** (Ví dụ: `192.168.1.10`). Khi khởi động xong, đèn trên ESP32 sẽ chớp Xanh Lá 1 lần.

### BƯỚC 3.2: Chạy AI Mosquito Detector (PC)
1. Mở Terminal khác tại thư mục gốc của project (nơi chứa file `Cargo.toml`).
2. Thay các thông số IP của DroidCam và IP của ESP32 vào câu lệnh dưới đây rồi chạy:

**Dành cho máy tính có Card NVIDIA (Chạy bằng GPU):**
```bash
cargo run --release --features cuda -p app_cli -- --model "app_cli\models\moquito_v19_e150.onnx" --camera "http://IP_CỦA_DROIDCAM:4747/video" --device "cuda:0" --esp-ip "IP_CỦA_ESP32"
```

**Dành cho máy tính không có Card rời (Chạy bằng CPU):**
```bash
cargo run --release -p app_cli -- --model "app_cli\models\moquito_v19_e150.onnx" --camera "http://IP_CỦA_DROIDCAM:4747/video" --esp-ip "IP_CỦA_ESP32"
```

---

## 🧠 4. Cách hệ thống hoạt động
- Khung hình từ DroidCam sẽ được truyền thẳng lên PC với chất lượng siêu nét gốc của camera, hiển thị ở tốc độ mượt mà 60 FPS (Kiến trúc Đa luồng xử lý giật lag).
- Trí tuệ Nhân tạo YOLOv9e phân tích ngầm ở tốc độ 5 FPS.
- Nếu phát hiện **Aedes** (Sốt xuất huyết), **Culex** (Viêm não NB) hoặc **Anopheles** (Sốt rét), hệ thống lập tức bắn tín hiệu sóng WiFi (UDP) kích hoạt ESP32 phát sáng đèn **Đỏ, Xanh Dương hoặc Xanh Lá**. 
- Nếu là các loài muỗi khác, hệ thống sẽ gán màu sáng tự động theo mã Hash. Tên loại muỗi cũng được in liên tục ở cửa sổ Terminal.

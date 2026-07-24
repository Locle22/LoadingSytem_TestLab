# 🚀 Hướng dẫn Cài đặt & Vận hành Hệ thống Phân loại Muỗi AI
*(Dành cho người mới bắt đầu sau khi clone source code)*

Tài liệu này sẽ hướng dẫn bạn từng bước từ lúc tải code về cho đến khi hệ thống hoạt động hoàn chỉnh (Bao gồm AI nhận diện và phần cứng ESP32).

---

## 🛠️ PHẦN 1: Chuẩn bị Môi trường (Cài đặt các công cụ)

Để chạy được dự án này, máy tính của bạn cần cài đặt các công cụ sau:

1. **Rust Toolchain (Cốt lõi):**
   - Dự án được viết chủ yếu bằng Rust. Truy cập [rustup.rs](https://rustup.rs/) tải file `rustup-init.exe` về cài đặt (chọn cấu hình mặc định).
   - *Lưu ý: Bạn cũng sẽ cần cài đặt **C++ Build Tools** (Visual Studio) theo yêu cầu của Rust.*
   
2. **Cài đặt công cụ nạp code cho ESP32:**
   - Mở Terminal (Command Prompt hoặc PowerShell), chạy lệnh:
     ```bash
     cargo install cargo-espflash
     ```

3. **Node.js (Dành cho Giao diện Web):**
   - Tải và cài đặt Node.js (bản LTS 18+ hoặc 20+) tại [nodejs.org](https://nodejs.org/). Dùng để compile giao diện Frontend.

4. **DroidCam App (Camera trên điện thoại):**
   - Vào Google Play Store trên điện thoại Android, tải và cài đặt ứng dụng **DroidCam**. 

---

## 📂 PHẦN 2: Tải Model AI

Do các mô hình trí tuệ nhân tạo (Model YOLO) thường có dung lượng rất lớn nên không được đưa lên Github. Sau khi clone code, bạn cần thực hiện:

1. Lấy file model **`moquito_v19_e150.onnx`** (do tác giả hoặc người quản lý dự án cung cấp).
2. Tạo thư mục `models` bên trong đường dẫn `yolo_workspace/app_cli/` (nếu chưa có).
3. Copy file `.onnx` vào đường dẫn:
   `yolo_workspace/app_cli/models/moquito_v19_e150.onnx`

---

## ⚡ PHẦN 3: Thiết lập & Nạp code cho Phần cứng (ESP32)

1. Đấu nối dây phần cứng cho mạch ESP32-S3 theo hướng dẫn trong file **`doc/wiring_guide.md`**.
2. Mở file mã nguồn cấu hình: `yolo_workspace/esp32_led_receiver/src/system.rs` bằng trình soạn thảo (như VS Code).
3. Tìm đến dòng 47-48, thay đổi thông tin mạng WiFi nhà bạn:
   ```rust
   // Sửa thành tên và mật khẩu WiFi của bạn
   "Ten_WiFi_Nha_Ban", // SSID
   "Mat_Khau_WiFi"     // Password
   ```
4. Cắm cáp USB Type-C từ ESP32 vào máy tính.
5. Mở Terminal, di chuyển vào thư mục code ESP32 và chạy lệnh nạp code:
   ```bash
   cd yolo_workspace/esp32_led_receiver
   cargo build --release
   cargo espflash flash --release --monitor
   ```
   *(Quá trình biên dịch lần đầu sẽ mất khoảng vài phút. Khi thành công, trên màn hình sẽ in ra các log cấu hình. Bạn có thể bấm `Ctrl + C` để thoát màn hình monitor).*

---

## 🎯 PHẦN 4: Khởi chạy Hệ thống Toàn diện

Khi phần cứng đã nạp code thành công và cắm nguồn, hãy khởi động khối điều khiển PC:

### 1. Chuẩn bị Camera
- Mở ứng dụng **DroidCam** trên điện thoại.
- Đảm bảo điện thoại và máy tính kết nối **Cùng một mạng WiFi**.
- Ghi lại dải **IP** mà DroidCam hiển thị trên màn hình điện thoại (VD: `192.168.1.5`).

### 2. Khởi chạy bằng One-Click Script
Dự án đã được tích hợp sẵn Script khởi chạy tự động:
1. Mở thư mục `yolo_workspace` trên máy tính.
2. Click đúp vào file **`CHAY_HE_THONG.bat`**.
3. Cửa sổ dòng lệnh hiện ra, bạn chỉ việc trả lời các câu hỏi:
   - **Chọn nguồn Camera:** Nhập `1` (DroidCam).
   - **Nhập IP điện thoại:** Nhập dãy IP ở bước trên (VD: `192.168.1.5`).
   - **Chọn thiết bị điều khiển:** Nhập `1` (ESP32).
   - **Nhập IP ESP32:** Cứ nhấn `Enter` để bỏ qua (Hệ thống sẽ tự động tìm quét thiết bị).

### 3. Tận hưởng
- Hệ thống AI sẽ bắt đầu nạp model vào card đồ họa / CPU.
- Sau khoảng 5-10 giây, Trình duyệt Web của bạn sẽ tự động mở lên giao diện tại `http://localhost:3000`.
- Tại đây, bạn sẽ thấy hình ảnh truyền trực tiếp từ điện thoại với các bounding-box nhận diện muỗi.
- Lệnh xoay mâm phân loại và bật LED sẽ tự động được truyền xuống ESP32.

---

## 🛠️ PHẦN 5: Chế độ Dành cho Nhà phát triển (Developer Mode)

Nếu bạn muốn thay đổi giao diện Web UI (Màu sắc, bố cục, thêm tính năng) thay vì chạy trực tiếp bằng file `.bat`, hãy làm theo các bước sau:

1. Chạy Backend (API Server) thủ công:
   ```bash
   cd yolo_workspace
   cargo run --release -p app_cli -- --model "app_cli/models/moquito_v19_e150.onnx" --camera "http://<IP_DROIDCAM>:4747/video"
   ```

2. Mở một Terminal khác, chạy Frontend qua Vite (chế độ hot-reload):
   ```bash
   cd yolo_workspace/web_ui
   npm install
   npm run dev
   ```
3. Truy cập vào `http://localhost:5173` để xem các thay đổi về giao diện theo thời gian thực mỗi khi bạn lưu file code. Mọi thay đổi đều được cập nhật mà không cần phải compile lại toàn bộ.

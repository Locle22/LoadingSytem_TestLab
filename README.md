# 🦟 AI Mosquito Detector — Hệ Thống Phân Loại & Điều Tra Muỗi AI

> **Một hệ thống mô phỏng thông minh hướng tới phục vụ sức khỏe cộng đồng thông qua việc điều tra, giám sát và phân tích dịch tễ muỗi tại địa phương.**

---

## 🌍 1. Mô tả hệ thống & Ý nghĩa y tế cộng đồng

Hệ thống **AI Mosquito Detector** được nghiên cứu và thiết kế là **một hệ thống mô phỏng nhằm mục đích hướng tới phục vụ sức khỏe cộng đồng thông qua việc điều tra, phân tích muỗi tại địa phương**. Trước đây, công tác lập bản đồ sự phân bố của các loài muỗi truyền bệnh (như Sốt xuất huyết Dengue, Sốt rét, Zika, Viêm não Nhật Bản, v.v.) thường đòi hỏi các chuyên gia dịch tễ phải phân loại thủ công dưới kính hiển vi — một quy trình tốn rất nhiều thời gian, công sức và dễ xảy ra sai sót.

Thông qua việc ứng dụng Trí tuệ Nhân tạo (AI) kết hợp cơ điện tử IoT, hệ thống mang lại những đổi mới vượt trội:
* **Tự động nhận diện & phân loại thời gian thực:** Nhận diện chính xác 36 loài muỗi khác nhau bằng mô hình thị giác máy tính chuyên sâu (YOLO) với tốc độ xử lý cao.
* **Mô phỏng dây chuyền phân loại tự động:** Tự động định hướng đĩa xoay và phân tách các mẫu vật muỗi vào đúng lọ thu thập tương ứng phục vụ cho công tác bảo quản và nghiên cứu chuyên sâu.
* **Học tập chủ động (Active Learning - Human-in-the-loop):** Cho phép người vận hành chụp lại các trường hợp muỗi lạ hoặc khó nhận diện ngay trong quá trình chạy hệ thống, gán nhãn thủ công trên giao diện Web và tải về bộ dữ liệu huấn luyện mới (.ZIP) chuẩn YOLO để liên tục tái huấn luyện (retrain), nâng cấp độ chính xác cho AI.
* **Số hóa dữ liệu dịch tễ:** Ghi nhận và thống kê lịch sử xuất hiện của từng loài muỗi theo thời gian thực, tạo tiền đề xây dựng mạng lưới dữ liệu cảnh báo sớm các điểm nóng bùng phát dịch bệnh cho cơ quan y tế địa phương.

---

## ⚙️ 2. Thành phần phần cứng hệ thống

Hệ thống được xây dựng tối ưu trên nền tảng cơ điện tử và IoT nhúng, bao gồm các thành phần linh kiện chủ chốt sau:

* **ESP32 (Vi điều khiển chính):** Sử dụng vi điều khiển ESP32 / ESP32-S3 WROOM đóng vai trò bộ não trung tâm phía phần cứng. Đảm nhận giao tiếp vô tuyến WiFi, thu nhận lệnh truyền thông UDP tốc độ cao từ máy tính và điều khiển trực tiếp toàn bộ cảm biến, động cơ.
* **02 Động cơ Servo SG90s:**
  * **Servo Băng chuyền (Conveyor Servo):** Đóng vai trò chức năng mô phỏng băng chuyền hoạt động, tuần tự đưa mẫu muỗi vào buồng chụp cảm biến AI.
  * **Servo Đĩa xoay (Sorter Servo):** Đóng vai trò cơ cấu đĩa xoay phân loại mẫu, quay chính xác góc chỉ định để đưa từng loài muỗi rớt xuống lọ/ô hứng mẫu tương ứng (đặt tên quy chuẩn từ ô A đến J0).
* **01 Bộ nguồn 5V:** Cung cấp nguồn điện áp ổn định và đủ dòng công suất cho vi điều khiển ESP32, các động cơ servo và module ngoại vi hoạt động liên tục mà không bị sụt áp hay nhiễu tín hiệu.
* **Mạch RFID RC522:** Cảm biến đọc thẻ từ RFID tần số 13.56MHz (giao tiếp SPI), đóng vai trò quản lý phân quyền người thao tác hệ thống (Operator / Admin), định danh khay mẫu hoặc ghi nhận các phiên làm việc dịch tễ.
* *(Ngoại vi hỗ trợ):* Dải đèn LED RGB WS2812 (hiển thị trực quan màu sắc nhận diện đặc trưng của từng loài muỗi theo tín hiệu AI) và Camera điện thoại DroidCam / IP Camera (thu nhận luồng hình ảnh ngõ vào cho mô hình YOLO).

---

## 🏗️ 3. Kiến trúc mã nguồn (Workspace Structure)

Dự án được cấu trúc theo mô hình workspace nhiều tầng (multi-layer architecture), tách biệt rõ ràng giữa xử lý AI lõi, điều phối backend, firmware vi điều khiển và giao diện quản trị Web:

```text
yolo_workspace/
├── 🧠 ai_vision/               # Module lõi thị giác máy tính & xử lý AI
│   ├── src/                    # Code xử lý luồng video camera,추론 YOLOv8/v9/v11 (ONNX Runtime)
│   └── types.rs                # Định nghĩa cấu trúc bounding box và danh sách 36 loài muỗi
│
├── ⚙️ app_cli/                 # Tầng điều phối trung tâm (Backend Orchestrator & HTTP Server)
│   ├── src/
│   │   ├── main.rs             # Điểm khởi chạy CLI, xử lý đa luồng AI & kết nối ngoại vi
│   │   ├── hardware.rs         # Trait trừu tượng hóa phần cứng (Esp32Backend / PlcBackend)
│   │   ├── api_layer3.rs       # API tầng 3: Điều phối góc xoay mâm, quy chuẩn đặt tên ô (A-J0)
│   │   ├── iot.rs              # Giao thức truyền thông UDP Opcode V2 (0x01, 0x02, 0x03)
│   │   └── web_server.rs       # HTTP REST API Server (Axum, cổng 3000) & xử lý Active Learning
│   └── Cargo.toml              # Cấu hình dependency (bao gồm crate `zip` xuất dataset)
│
├── 🔌 esp32_led_receiver/      # Tầng 1: Firmware nhúng cho mạch ESP32-S3 (Rust Embedded)
│   ├── src/
│   │   ├── main.rs             # Khởi tạo WiFi, UDP socket, luồng RFID và vòng lặp chính
│   │   ├── system.rs           # Bộ giải mã UDP Opcode V2, xử lý lệnh quay Servo & LED RGB
│   │   ├── servo.rs            # Driver điều khiển xung PWM cho Servo SG90s
│   │   ├── sorter.rs / conveyor.rs # Logic nghiệp vụ đĩa xoay phân loại và băng chuyền
│   │   └── rfid.rs             # Giao tiếp đọc/ghi thẻ RFID RC522
│   └── Cargo.toml
│
├── 💻 web_ui/                  # Tầng giao diện người dùng (Modern Web Dashboard - Vite + Vanilla JS)
│   ├── index.html              # Layout Dashboard: Live Camera, Active Learning, Lịch sử Real-time
│   ├── src/
│   │   ├── main.js             # Logic điều khiển, đồng bộ status, chụp mẫu & tải ZIP dataset
│   │   └── style.css           # Giao diện Dark Mode sang trọng, hiệu ứng Glow, Modal responsive
│   └── package.json
│
└── 📄 docs/                    # Tài liệu kỹ thuật & Hướng dẫn hệ thống
    ├── setup_guide.md          # Hướng dẫn thiết lập môi trường, biên dịch và vận hành
    └── wiring_guide.md         # Sơ đồ đấu nối chân (Pinout) cho ESP32, Servo, RFID, LED
```

### Các Đặc điểm Kỹ thuật Nổi bật:
* **Giao thức UDP Opcode V2:** Mã hóa lệnh gửi xuống ESP32 bằng byte đầu tiên (`0x01`: Phân loại muỗi tự động kèm màu RGB, `0x02`: Xoay đĩa góc tuyệt đối, `0x03`: Reset Home về 0°), đảm bảo tính phản hồi tức thì và tương thích ngược hoàn toàn.
* **Thu thập & Gán nhãn Trực tiếp (Active Learning):** Tích hợp tính năng giữ khung hình tĩnh (freeze frame), lưu ảnh RAW và nhãn vào `annotations.jsonl`, đóng gói thành file `.ZIP` chuẩn định dạng YOLO (kèm `data.yaml`) sẵn sàng cho huấn luyện lại model.
* **Khả năng Mở rộng Đa nền tảng:** Kiến trúc cho phép chạy linh hoạt trên vi điều khiển ESP32 thực tế, chế độ mô phỏng phần mềm (Simulation/Mock Mode) hoặc kết nối Driver PLC công nghiệp thông qua cờ tham số `--plc`.

---

## 🚀 4. Hướng dẫn sử dụng & Bắt đầu nhanh

Người dùng cần tải bộ mã nguồn về, sau đó thực hiện hướng dẫn chi tiết theo 2 tài liệu chuẩn đã được biên soạn sẵn trong thư mục `docs/`:

1. 📌 **[Hướng dẫn Đấu nối Phần cứng (wiring_guide.md)](docs/wiring_guide.md):**
   * Sơ đồ chân chi tiết (Pinout) kết nối ESP32 với 2 Servo SG90s (Băng chuyền GPIO 5, Đĩa xoay GPIO 4), mạch RFID RC522 (SPI) và đèn LED WS2812 (GPIO 48).
   * Hướng dẫn sử dụng nguồn cấp ngoài 5V an toàn, tránh sụt áp hoặc hư hỏng cổng USB máy tính.

2. 📌 **[Hướng dẫn Cài đặt & Vận hành (setup_guide.md)](docs/setup_guide.md):**
   * Các bước chuẩn bị môi trường: cài đặt Rust Toolchain, Node.js, DroidCam trên điện thoại.
   * Hướng dẫn biên dịch và nạp Firmware xuống ESP32 (`cargo espflash flash --release`).
   * Lệnh khởi chạy Backend trên Laptop (`cargo run -p app_cli`) và khởi chạy Web UI (`npm run dev` trong thư mục `web_ui`).
   * Kịch bản kiểm thử các tính năng nhận diện, điều khiển thủ công và thu thập dữ liệu Active Learning.

---
*Dự án AI Mosquito Detector — Kết hợp Trí tuệ Nhân tạo và Cơ điện tử vì sức khỏe cộng đồng.*

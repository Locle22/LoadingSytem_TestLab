# MosquitoSortingLab 🦟

### KỊCH BẢN KIỂM THỬ (TESTCASE) HỆ THỐNG ESP32 SERIAL COMM

#### 1. Nhóm Kiểm thử Giao thức & Bảo mật dữ liệu (Protocol & Integrity)

Mục tiêu: Đảm bảo ESP32 lọc đúng các gói tin bị nhiễu do đường truyền Serial.

* **TC1.1 - Gói tin hợp lệ (Happy Case):**
* *Hành động:* RPi gửi chuỗi 8 bytes: `[0xAA, 0x08, 0x00, 0x00, 0x00, 0x00, 0x01, Checksum]` (Lệnh `CMD_GET_STATUS`) qua file `rpi_serial_controller.py`.
* *Kỳ vọng:* Hàm `process_packet()` tính toán checksum trùng khớp. ESP32 chớp LED Cyan (nhận data) 1 lần và gửi trả gói `[0x55, STATUS, JAR, ALARM, 0, 0, SEQ, CHK]` lên RPi.


* **TC1.2 - Sai Checksum (Lỗi đường truyền):**
* *Hành động:* Cố tình sửa byte cuối (byte 7) của lệnh `CMD_HOME` thành một giá trị sai trước khi gọi `ser.write(packet)` trong file Python.
* *Kỳ vọng:* ESP32 nhận đủ 8 bytes nhưng tại hàm `process_packet`, điều kiện `if (pkt[7] != expected_chk) return;` được kích hoạt. Lệnh bị hủy, ESP32 không di chuyển, không crash.


* **TC1.3 - Căn chỉnh Header (Alignment Recovery):**
* *Hành động:* Gửi 1 byte rác (ví dụ `0xFF`), sau đó mới gửi gói 8 bytes hợp lệ (bắt đầu bằng `0xAA`).
* *Kỳ vọng:* Đoạn code `if (rx_idx == 0 && b != HEADER_CMD) continue;` sẽ bỏ qua byte `0xFF` và bắt đúng vào gói dữ liệu sau đó.



#### 2. Nhóm Kiểm thử Logic Trạng thái (State Machine Logic)

Mục tiêu: Đảm bảo động cơ không tự ý chạy khi chưa có lệnh cho phép (Safety first).

* **TC2.1 - Ra lệnh di chuyển khi Servo chưa bật:**
* *Hành động:* Ngay sau khi cấp nguồn ESP32 (LED đang màu trắng mờ - `COLOR_SERVO_OFF`), gửi lệnh `CMD_MOVE_TO` đến lọ số 2.
* *Kỳ vọng:* Tại `handle_move_to`, do `!servo_on` là True, LED nháy màu Đỏ (`COLOR_ALARM`) 2 lần. ESP32 phản hồi `STATUS_NOT_READY` và đĩa không quay.


* **TC2.2 - Lỗi biên vị trí (Out of Bounds):**
* *Hành động:* Gửi lệnh `CMD_MOVE_TO` với target_jar = 10 (trong khi index cho phép chỉ từ 0 đến 9 vì `NUM_JARS = 10`).
* *Kỳ vọng:* Hàm `target_jar >= NUM_JARS` trả về true, LED đổi màu Đỏ (`COLOR_ALARM`), gửi về cờ lỗi `ALARM_ENCODER_ERR`.


* **TC2.3 - Luồng di chuyển bình thường:**
* *Hành động:* Gửi `CMD_SERVO_ON`, sau đó gửi `CMD_MOVE_TO` (jar = 3 - Culex pipiens).
* *Kỳ vọng:* 1. LED đổi sang Vàng (`COLOR_MOVING`).
2. Chờ delay giả lập `steps * 300` ms.
3. LED nháy màu Xanh Lá của lọ 3 (`JAR_COLORS[target_jar]`) 3 lần.
4. Trả về trạng thái `STATUS_IN_POSITION`.



#### 3. Nhóm Kiểm thử Controller Python (trên máy tính/RPi)

Mục tiêu: File `rpi_serial_controller.py` đóng vai trò là "bộ não" điều phối, cần kiểm tra tính tự động của nó.

* **TC3.1 - Tự động nhận diện thiết bị (Auto-detect):**
* *Hành động:* Rút cáp ESP32, chạy lệnh `python3 rpi_serial_controller.py`. Sau đó cắm cáp lại và chạy tiếp.
* *Kỳ vọng:* Lần đầu báo "[LỖI] Không tìm thấy cổng Serial nào!". Lần hai code `find_port()` phải nhận diện đúng cổng COM/TTY có chứa "CH340" hoặc "CP2102" và tự động kết nối mà không cần nhập tay.


* **TC3.2 - Phân loại Muỗi & Bỏ qua kết quả nhiễu:**
* *Hành động:* Sửa hàm `fake_ai_detect()` để trả về object với `confidence = 0.80`.
* *Kỳ vọng:* Vòng lặp chính đọc thấy `confidence < conf_threshold` (`0.85`), in ra log "confidence thấp -> bỏ qua" và *tuyệt đối không gửi lệnh* `CMD_MOVE_TO` xuống ESP32.



#### Tối ưu cho phần cứng vật lý (Step Kế tiếp)

Code hiện tại đang giả lập thời gian (`delay(steps * 300);`), khi lắp Servo/Động cơ bước thật vào, hàm `delay()` này sẽ làm đứng toàn bộ hệ thống (Blocking), ESP32 sẽ không thể nhận lệnh `CMD_STOP` (Dừng khẩn cấp) trong lúc đang quay.

**Nhiệm vụ cho Code phần cứng thật:** Cần thay thế `delay()` bằng hàm xuất xung (PWM/Step) trong ngắt Timer, hoặc dùng thư viện `AccelStepper` dạng Non-blocking (`stepper.run()`) trong `void loop()`. 

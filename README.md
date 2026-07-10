# MosquitoSortingLab 🦟 - LoadingSystem  

Bản đặc tả tài liệu kỹ thuật và Kịch bản kiểm thử (Test Cases) hoàn chỉnh dành cho phân hệ thực nghiệm nhúng (STEM).

---

## 1. Tổng Quan Dự Án & Kiến Trúc Phân Hệ
Dự án **LoadingSystem** tích hợp công nghệ xử lý ảnh (AI Edge Vision) và cơ điện tử chính xác để tự động hóa quy trình phân luồng, nhận diện và đóng hũ tự động các mẫu vật thể sinh học/nông sản. Trong phân hệ thực nghiệm này, hệ thống tập trung phân loại **10 loài muỗi** đặc thù với độ chính xác cơ khí yêu cầu nghiêm ngặt $\ge 98\%$.

### Sơ đồ luồng dữ liệu  
```
[Camera Công Nghiệp]
       │ (Quét liên tục bề mặt băng tải)
       ▼
[Raspberry Pi / Jetson Nano (AI Inference)] 
       │ (Mô hình YOLO/MobileNet -> Phân lớp & Tính toán vị trí)
       │ (Giao tiếp USB Serial - Chipset CH340/CP2102 @ 115200 Baud)
       ▼
[ESP32 WROOM Node Controller] 
       │ (Bộ phân giải giao thức 8-Byte, điều khiển máy trạng thái FreeRTOS)
       ▼
[Driver CSD7_02BX1 / Động cơ bước] ──> [Đĩa Xoay Cơ Khí Hứng Mẫu]
```

---

## 2. Đặc Tả Giao Thức Nhị Phân 
Hệ thống sử dụng gói tin cố định kích thước **8 bytes** truyền nhận song phương (Full-duplex/UART) giúp tối ưu hóa băng thông, triệt tiêu độ trễ phân tích cú pháp chuỗi (JSON/String parsing) và chống nhiễu công nghiệp.

### 2.1. Cấu trúc Gói lệnh (TX: RPi → ESP32)
| Byte Vị trí | Tên Trường | Kiểu dữ liệu | Giá trị / Mô tả |
| :---: | :--- | :---: | :--- |
| `Byte 0` | **HEADER_CMD** | `uint8_t` | Luôn cố định là `0xAA` |
| `Byte 1` | **CMD_TYPE** | `uint8_t` | Mã lệnh điều khiển (Xem danh mục Opcode bên dưới) |
| `Byte 2` | **PARAM_1** | `uint8_t` | Tham số thứ nhất (ví dụ: Vị trí lọ mục tiêu `target_jar` từ 0 - 9) |
| `Byte 3` | **PARAM_2** | `uint8_t` | Tham số thứ hai (Mở rộng tính năng điều khiển phụ) |
| `Byte 4` | **DATA_LO** | `uint8_t` | Byte thấp của trường dữ liệu 16-bit (Tốc độ / Gia tốc) |
| `Byte 5` | **DATA_HI** | `uint8_t` | Byte cao của trường dữ liệu 16-bit |
| `Byte 6` | **SEQUENCE** | `uint8_t` | Số tuần tự bản tin (`seq` tăng dần từ 0x00 đến 0xFF để đồng bộ luồng) |
| `Byte 7` | **CHECKSUM** | `uint8_t` | Byte kiểm tra lỗi toán tử XOR: `Byte(0) ^ Byte(1) ^ ... ^ Byte(6)` |

#### Danh mục Opcode Lệnh (`CmdType`):
* `0x00` (**CMD_NOP**): Lệnh trống / Giữ nhịp Heartbeat hệ thống.
* `0x01` (**CMD_MOVE_TO**): Di chuyển đĩa xoay tới vị trí hũ thủy tinh thứ N.
* `0x02` (**CMD_HOME**): Lệnh nội suy đưa cơ cấu về điểm gốc cảm biến giới hạn.
* `0x03` (**CMD_STOP**): Dừng khẩn cấp toàn bộ cơ cấu chấp hành ngay lập tức.
* `0x04` (**CMD_SET_SPEED**): Thiết lập các tham số động học cho động cơ.
* `0x05` (**CMD_SERVO_ON**): Cấp nguồn driver, khóa trục động cơ (Sẵn sàng chạy).
* `0x06` (**CMD_SERVO_OFF**): Ngắt nguồn driver (Thả lỏng trục cơ khí).
* `0x07` (**CMD_ALARM_RESET**): Xóa trạng thái lỗi, khôi phục máy trạng thái sau sự cố.
* `0x08` (**CMD_GET_STATUS**): Truy vấn trạng thái hiện tại của Node vi điều khiển.

### 2.2. Cấu trúc Gói trạng thái (RX: ESP32 → RPi)
| Byte Vị trí | Tên Trường | Kiểu dữ liệu | Giá trị / Mô tả |
| :---: | :--- | :---: | :--- |
| `Byte 0` | **HEADER_STATUS** | `uint8_t` | Luôn cố định là `0x55` |
| `Byte 1` | **STATUS_FLAG** | `uint8_t` | Máy trạng thái hiện tại của ESP32 (Idle, Moving, Alarm...) |
| `Byte 2` | **JAR_POSITION** | `uint8_t` | Vị trí thực tế hiện tại của đĩa xoay (0 - 9) |
| `Byte 3` | **ALARM_CODE** | `uint8_t` | Mã lỗi phần cứng chi tiết nếu có sự cố xảy ra |
| `Byte 4` | `RESERVED_1` | `uint8_t` | Dự phòng giữ chỗ (`0x00`) |
| `Byte 5` | `RESERVED_2` | `uint8_t` | Dự phòng giữ chỗ (`0x00`) |
| `Byte 6` | **SEQUENCE** | `uint8_t` | Khớp đúng giá trị `seq` của gói lệnh nhận được để xác thực phản hồi |
| `Byte 7` | **CHECKSUM** | `uint8_t` | Byte kiểm tra XOR toàn bản tin phản hồi |

#### Trạng thái Hệ thống (`StatusFlag`):
* `0x00` (**STATUS_IDLE**): Hệ thống rảnh, sẵn sàng nhận lệnh mới.
* `0x01` (**STATUS_MOVING**): Cơ cấu đang trong quá trình chuyển động.
* `0x02` (**STATUS_IN_POSITION**): Cơ cấu cơ khí đã đến chính xác vị trí đích.
* `0x03` (**STATUS_ALARM**): Hệ thống rơi vào trạng thái lỗi phần cứng nghiêm trọng.
* `0x04` (**STATUS_NOT_READY**): Hệ thống chưa bật Servo hoặc chưa thực hiện chu trình HOME.

#### Mã báo động phần cứng (`AlarmCode`):
* `0x00` (**ALARM_NONE**): Trạng thái bình thường không có lỗi.
* `0x01` (**ALARM_OVER_CURRENT**): Cảnh báo quá dòng (Động cơ bị kẹt cơ khí nghiêm trọng).
* `0x02` (**ALARM_OVER_VOLTAGE**): Lỗi điện áp nguồn cấp vượt ngưỡng an toàn.
* `0x05` (**ALARM_ENCODER_ERR**): Sai lệch xung phản hồi hoặc tọa độ vượt biên vật lý.

---

## 3. Bản Kịch Bản Kiểm Thử Hoàn Chỉnh  

Dưới đây là ma trận checklist kiểm thử tích hợp phần mềm và phần cứng biên, được thiết kế dưới dạng bảng kiểm soát chất lượng nghiệm thu.

### 3.1. Nhóm 1: Kiểm thử Giao thức & Toàn vẹn Dữ liệu (Protocol & Data Integrity)
*Mục tiêu: Đảm bảo tầng Driver Serial trên ESP32 xử lý chính xác các trường hợp nhiễu bit, lệch pha dữ liệu mà không gây sập hệ thống.*

| Mã TC | Tên Kịch Bản | Tiền Điều Kiện | Hành Động (Input) | Kết Quả Kỳ Vọng (Expected Output) | Trạng Thái |
| :---: | :--- | :--- | :--- | :--- | :---: |
| **TC1.1** | Gói tin tiêu chuẩn (Happy Case) | Thiết bị kết nối Serial, đang ở trạng thái `STATUS_IDLE`. | RPi gửi gói tin `CMD_GET_STATUS` chuẩn mực, tính đúng Checksum. | - LED chớp xanh Cyan (`COLOR_RECEIVING`).<br>- Gửi phản hồi 8-byte khớp mã `seq`. | `[ ]` |
| **TC1.2** | Lỗi Checksum dữ liệu | Thiết bị đang hoạt động bình thường ở trạng thái `STATUS_IDLE`. | Cố tình làm sai lệch byte số 7 (Checksum) của lệnh `CMD_HOME` trên RPi trước khi phát dữ liệu. | - Hàm `process_packet()` phát hiện sai checksum.<br>- Lệnh bị hủy bỏ ngay lập tức, cơ cấu đứng yên.<br>- Không crash phần mềm. | `[ ]` |
| **TC1.3** | Căn chỉnh luồng Header (Alignment Recovery) | ESP32 đang quét vòng lặp `Serial.available()`. | RPi gửi liên tục các byte nhiễu ngẫu nhiên (`0xFF`, `0x00`), sau đó mới gửi gói 8-byte chuẩn xuất phát từ `0xAA`. | - Thuật toán lọc `if(rx_idx == 0 && b != HEADER_CMD) continue;` loại bỏ toàn bộ byte rác.<br>- Bắt trúng byte `0xAA` để phục hồi frame chuẩn xác. | `[ ]` |
| **TC1.4** | Tràn bộ đệm nhận (Buffer Overflow) | ESP32 đang trong trạng thái xử lý logic. | RPi gửi gói tin dài bất thường (ví dụ: 64 bytes liên tục không ngắt quãng). | - Hệ thống ngắt gói cứ sau mỗi `PACKET_SIZE = 8`. <br>- Giải phóng bộ đệm bằng cách đưa `rx_idx = 0` mà không làm tràn bộ nhớ Heap. | `[ ]` |

### 3.2. Nhóm 2: Kiểm thử Logic Máy trạng thái & An Toàn Phần Cứng (State Machine & Hardware Safety)
*Mục tiêu: Ngăn chặn tuyệt đối các xung lỗi kích hoạt động cơ ngoài ý muốn hoặc phá hủy cơ cấu cơ khí.*

| Mã TC | Tên Kịch Bản | Tiền Điều Kiện | Hành Động (Input) | Kết Quả Kỳ Vọng (Expected Output) | Trạng Thái |
| :---: | :--- | :--- | :--- | :--- | :---: |
| **TC2.1** | Chặn dịch chuyển khi chưa Sẵn sàng | ESP32 mới khởi động, trạng thái mặc định là `STATUS_NOT_READY` (Servo chưa bật). | RPi gửi lệnh `CMD_MOVE_TO` yêu cầu quay đến hũ số 5. | - Logic `!servo_on` chặn đứng lệnh.<br>- LED nháy màu Đỏ (`COLOR_ALARM`) 2 lần cảnh báo.<br>- Phản hồi trạng thái chưa sẵn sàng về máy chủ. | `[ ]` |
| **TC2.2** | Kiểm soát lỗi biên vị trí vật lý | Hệ thống đã HOME thành công, Servo hoạt động bình thường. | RPi gửi lệnh `CMD_MOVE_TO` với tham số `target_jar = 10` (Vượt biên `NUM_JARS = 10`). | - Biến điều kiện biên kích hoạt báo lỗi.<br>- LED chuyển sang màu Đỏ đặc (`COLOR_ALARM`).<br>- Trả cờ trạng thái `STATUS_ALARM` kèm mã lỗi `ALARM_ENCODER_ERR`. | `[ ]` |
| **TC2.3** | Chu trình phân loại tuần tự chuẩn | Hệ thống hoạt động tốt, đĩa cơ khí đang nằm tại vị trí hũ số 0. | Gửi lệnh `CMD_SERVO_ON`, tiếp nối bằng lệnh `CMD_MOVE_TO` đến vị trí hũ số 3. | - LED chuyển sang màu Vàng (`COLOR_MOVING`).<br>- Trực quan hóa LED đích nháy màu xanh lá (`JAR_COLORS[3]`) 3 lần.<br>- Cập nhật vị trí thực tế là 3, trả trạng thái hoàn thành. | `[ ]` |
| **TC2.4** | Dừng khẩn cấp giữa chu kỳ (Failsafe) | Hệ thống đang thực thi chuyển động, LED đang có màu vàng (`STATUS_MOVING`). | RPi phát lệnh ngắt dừng khẩn cấp `CMD_STOP`. | - ESP32 lập tức cấu hình mức ưu tiên cao chặn luồng quay.<br>- LED nháy màu đỏ liên tục 5 lần cường độ cao (`50ms`).<br>- Đưa máy trạng thái về an toàn. | `[ ]` |

### 3.3. Nhóm 3: Kiểm thử Tầng Quản Lý & Xử Lý Ảnh Biên (Python AI Controller Testing)
*Mục tiêu: Đảm bảo script điều phối tự động trên Raspberry Pi điều khiển thông minh và xử lý chính xác kết quả mô hình thị giác biên.*

| Mã TC | Tên Kịch Bản | Tiền Điều Kiện | Hành Động (Input) | Kết Quả Kỳ Vọng (Expected Output) | Trạng Thái |
| :---: | :--- | :--- | :--- | :--- | :---: |
| **TC3.1** | Tự động quét nhận diện cổng vật lý | Rút hoặc cắm cáp USB kết nối giữa bộ điều khiển phần cứng và máy tính. | Chạy script điều phối `rpi_serial_controller.py`. | - Hàm `find_port()` quét toàn bộ chuỗi VID/PID hệ thống.<br>- Lọc trúng chipset giao tiếp (`CH340`, `CP2102`) để tự thiết lập kết nối không cần can thiệp thủ công. | `[ ]` |
| **TC3.2** | Bộ lọc ngưỡng tin cậy mô hình AI | Mô hình mạng nơ-ron nhận diện muỗi và trả kết quả với độ chính xác trung bình. | Mô phỏng AI trả về kết quả định danh loài với mức độ tự tin `confidence = 0.80` (ngưỡng hệ thống yêu cầu `0.85`). | - Script kiểm tra điều kiện logic nội bộ.<br>- In nhật ký hệ thống: `"confidence thấp -> bỏ qua"`.<br>- Chặn không gửi lệnh điều khiển xuống Serial để ngừa gạt nhầm. | `[ ]` |
| **TC3.3** | Tự động xử lý phục hồi sau lỗi phần cứng | Cơ cấu cơ khí gửi cờ lỗi `STATUS_ALARM` lên do kẹt động cơ nhẹ. | Script Python nhận chuỗi dữ liệu trạng thái từ Serial. | - Chương trình Python lập tức nhận dạng lỗi.<br>- Tự động kích hoạt luồng khẩn cấp gửi gói tin `CMD_ALARM_RESET` xuống trạm nhúng nhằm khôi phục trạng thái làm việc. | `[ ]` |

### 3.4. Nhóm 4: Kiểm thử Hiệu Năng Cao & Quá Tải Hệ Thống 
*Mục tiêu: Kiểm tra độ ổn định bền bỉ của hệ thống nhúng khi hoạt động liên tục với tải suất công nghiệp.*

| Mã TC | Tên Kịch Bản | Tiền Điều Kiện | Hành Động (Input) | Kết Quả Kỳ Vọng (Expected Output) | Trạng Thái |
| :---: | :--- | :--- | :--- | :--- | :---: |
| **TC4.1** | Spam lệnh tần suất cao (Stress Test) | Cơ cấu đang thực hiện chu trình chuyển động cơ khí chặng dài. | Giả lập camera nhiễu khiến AI liên tục gửi dồn dập 15 lệnh `CMD_MOVE_TO` với các hũ khác nhau trong 1 giây. | - ESP32 không bị tràn Queue hay sập Watchdog Timer.<br>- Hệ thống bỏ qua các gói tin spam và liên tục phản hồi trạng thái `STATUS_NOT_READY` hoặc `STATUS_MOVING`. | `[ ]` |
| **TC4.2** | Mất kết nối đột ngột (Loss of Signal) | Hệ thống đang trong ca vận hành phân loại tự động tự do. | Cố tình ngắt cáp Serial UART vật lý hoặc ngắt kết nối không dây giữa chừng khi đĩa đang quay. | - Mạch nhúng kích hoạt bộ kiểm soát Timeout (Nếu quá 3 giây không nhận gói tin Keepalive/NOP từ máy chủ).<br>- Tự động phanh trục động cơ nhằm bảo vệ an toàn cơ cấu. | `[ ]` |

---

## 4. Hướng Dẫn Tối Ưu Hóa Khi Triển Khai Phần Cứng Thật  

### 4.1. Khắc phục vấn đề Blocking logic (Treo hệ thống) từ mã nguồn mô phỏng
Trong mã nguồn thử nghiệm ban đầu (`esp32_controller.ino`), việc sử dụng hàm chặn luồng `delay(steps * 300);` sẽ **làm đóng băng hoàn toàn vi điều khiển**. Trong suốt thời gian delay này, chip ESP32 không thể đọc thanh ghi Serial, dẫn tới việc bỏ sót lệnh dừng khẩn cấp `CMD_STOP` từ Raspberry Pi gửi xuống.

### 4.2. Khuyến nghị kiến trúc đa nhiệm thời gian thực (FreeRTOS Architecture)
Để đáp ứng các tiêu chuẩn khắt khe về tính tuân thủ coding trong tài liệu nghiệp vụ BRD của hệ thống công nghiệp LoadingSystem, nhóm STEM cần tái cấu trúc mã nguồn sang mô hình **Đa luồng phi chặn (Non-blocking Multi-tasking)** tích hợp sẵn trong nhân FreeRTOS của ESP32:

1.  **Task 1: Luồng Giao Tiếp Ưu Tiên Cao (UART RX Task - Cấu hình mức ưu tiên: 5)**
    * Nhiệm vụ: Liên tục trực đọc buffer phần cứng của bộ UART thông qua cơ chế ngắt (Interrupt). 
    * Hành động: Thực hiện parse gói tin 8-byte, đối chiếu Checksum bằng toán tử XOR tốc độ cao. Nếu gói tin hợp lệ, đẩy thẳng cấu trúc dữ liệu lệnh vào một hàng đợi an toàn thread-safe (**FreeRTOS Message Queue**).
2.  **Task 2: Luồng Nội Suy Động Lực Học (Motor Drive Task - Cấu hình mức ưu tiên: 2)**
    * Nhiệm vụ: Lắng nghe trạng thái từ Message Queue bằng hàm phi chặn `xQueueReceive()`.
    * Hành động: Khi nhận được cấu trúc chuyển dịch hũ mục tiêu, luồng này sử dụng thư viện điều khiển động cơ bước dạng Non-blocking (ví dụ: `AccelStepper` gọi hàm liên tục `stepper.run()` trong vòng lặp chính) hoặc trực tiếp băm chuỗi xung thông qua bộ tạo tần số phần cứng **LEDC/RMT Peripheral** của ESP32.
    * *Ưu điểm:* Nếu một lệnh `CMD_STOP` khẩn cấp xuất hiện trong Queue, Task 1 sẽ bắt được ngay và cập nhật biến toàn cục hoặc gửi tín hiệu Semaphore ngắt trực tiếp xung của Motor Task chỉ trong vài micro-giây.

### 4.3. Sơ đồ đấu nối dây ngoại vi khuyến nghị (Pinout Matrix)
* **WS2812B NeoPixel Status LED:** Chân tín hiệu kết nối trực tiếp đến chân `GPIO 48` hoặc `GPIO 8` tùy biến thể board phát triển (Cấu hình độ sáng vừa phải `< 50` để bảo vệ mắt và tiết kiệm dòng nguồn mạch biên).
* **Driver Điều khiển Động cơ (CSD7_02BX1):**
    * `PULSE_PIN` (Xung cấp tốc độ) -> Cấu hình chân đầu ra PWM tốc độ cao phần cứng.
    * `DIR_PIN` (Cấu hình hướng quay thuận/ngược) -> Chân GPIO Output tiêu chuẩn.
    * `EN_PIN` (Cấp/ngắt momen lực động cơ) -> Chân GPIO nối trực tiếp đến ngắt an toàn hệ thống

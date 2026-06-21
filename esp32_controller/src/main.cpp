#include <Arduino.h>
/*
 * ============================================================
 *   ESP32 WROOM Motor Controller – LoadingSystem V1
 *   Board: ESP32 WROOM + CH340K + WS2812B RGB LED
 *   
 *   Nhận lệnh từ Raspberry Pi qua USB Serial
 *   Hiển thị trạng thái bằng LED RGB (mỗi trạng thái 1 màu)
 * ============================================================
 * 
 *   CÀI THƯ VIỆN:
 *   Arduino IDE → Sketch → Include Library → Manage Libraries
 *   → Tìm "Adafruit NeoPixel" → Install
 *
 *   CẤU HÌNH BOARD:
 *   Tools → Board → ESP32 Arduino → ESP32 Dev Module
 *   Tools → Port  → COMx (CH340)
 * 
 * ============================================================
 *   PROTOCOL: 8 bytes mỗi packet
 *   TX (RPi → ESP32):  [0xAA][CMD][P1][P2][D_LO][D_HI][SEQ][CHK]
 *   RX (ESP32 → RPi):  [0x55][STATUS][JAR][ALARM][0][0][SEQ][CHK]
 * ============================================================
 */

#include <Adafruit_NeoPixel.h>

// ===================== CẤU HÌNH PIN =====================
// Đổi số pin cho đúng với board của bạn:
//   ESP32-S3:     GPIO 48
//   ESP32-C3:     GPIO 8
//   ESP32 WROOM:  GPIO 48 hoặc GPIO 2 (tùy board)
//   Nếu không biết → thử 48 trước, không sáng thì đổi 8, rồi 2

#define RGB_LED_PIN    48     // ESP32-S3 WROOM CH340K built-in RGB LED data pin
#define NUM_LEDS       1      // Board có 1 LED RGB
#define LED_BRIGHTNESS 50     // 0-255 (50 = vừa mắt, không chói)

// Nếu board có LED đơn (không phải WS2812B), dùng fallback
#define FALLBACK_LED_PIN 2    // LED xanh built-in (dùng nếu RGB không hoạt động)

// ===================== KHAI BÁO LED =====================

Adafruit_NeoPixel rgb(NUM_LEDS, RGB_LED_PIN, NEO_GRB + NEO_KHZ800);

// Bảng màu cho từng trạng thái
struct Color { uint8_t r, g, b; };

const Color COLOR_OFF        = {  0,   0,   0};   // Tắt
const Color COLOR_BOOT       = {128,   0, 255};   // Tím      – khởi động
const Color COLOR_READY      = {  0, 255,   0};   // Xanh lá  – sẵn sàng (IDLE)
const Color COLOR_MOVING     = {255, 200,   0};   // Vàng     – đang quay
const Color COLOR_IN_POS     = {  0, 255,   0};   // Xanh lá  – đã đến vị trí
const Color COLOR_HOME       = {  0, 100, 255};   // Xanh dương – đang home
const Color COLOR_ALARM      = {255,   0,   0};   // Đỏ       – lỗi
const Color COLOR_SERVO_OFF  = { 20,  20,  20};   // Trắng mờ – servo tắt
const Color COLOR_RECEIVING  = {  0, 255, 255};   // Cyan     – đang nhận data

// Màu cho từng loài muỗi (10 loài = 10 màu khác nhau)
const Color JAR_COLORS[10] = {
    {255,  50,  50},   // Lọ 0: Đỏ nhạt     – Aedes aegypti
    {255, 100,   0},   // Lọ 1: Cam          – Aedes albopictus
    {255, 255,   0},   // Lọ 2: Vàng         – Anopheles gambiae
    {  0, 255,   0},   // Lọ 3: Xanh lá      – Culex pipiens
    {  0, 255, 128},   // Lọ 4: Xanh ngọc    – Culex quinquefas.
    {  0, 128, 255},   // Lọ 5: Xanh dương   – Anopheles stephensi
    {  0,   0, 255},   // Lọ 6: Xanh đậm     – Aedes polynesiensis
    {128,   0, 255},   // Lọ 7: Tím          – Mansonia uniformis
    {255,   0, 255},   // Lọ 8: Hồng         – Toxorhynchites sp.
    {255,   0, 128},   // Lọ 9: Hồng đậm     – Armigeres subalbatus
};


// ===================== PROTOCOL =====================

#define HEADER_CMD    0xAA
#define HEADER_STATUS 0x55
#define PACKET_SIZE   8

enum CmdType : uint8_t {
    CMD_NOP         = 0x00,
    CMD_MOVE_TO     = 0x01,
    CMD_HOME        = 0x02,
    CMD_STOP        = 0x03,
    CMD_SET_SPEED   = 0x04,
    CMD_SERVO_ON    = 0x05,
    CMD_SERVO_OFF   = 0x06,
    CMD_ALARM_RESET = 0x07,
    CMD_GET_STATUS  = 0x08,
};

enum StatusFlag : uint8_t {
    STATUS_IDLE        = 0x00,
    STATUS_MOVING      = 0x01,
    STATUS_IN_POSITION = 0x02,
    STATUS_ALARM       = 0x03,
    STATUS_NOT_READY   = 0x04,
};

enum AlarmCode : uint8_t {
    ALARM_NONE         = 0x00,
    ALARM_OVER_CURRENT = 0x01,
    ALARM_OVER_VOLTAGE = 0x02,
    ALARM_ENCODER_ERR  = 0x05,
};


// ===================== STATE =====================

bool servo_on = false;
uint8_t current_jar = 0;
uint8_t current_status = STATUS_NOT_READY;
uint8_t current_alarm = ALARM_NONE;
uint8_t rx_buf[PACKET_SIZE];
int rx_idx = 0;

#define NUM_JARS 10


// ===================== LED FUNCTIONS =====================

void led_set(Color c) {
    rgb.setPixelColor(0, rgb.Color(c.r, c.g, c.b));
    rgb.show();
}

void led_off() {
    led_set(COLOR_OFF);
}

void led_flash(Color c, int times, int delay_ms) {
    for (int i = 0; i < times; i++) {
        led_set(c);
        delay(delay_ms);
        led_off();
        delay(delay_ms);
    }
}

void led_fade_in_out(Color c, int duration_ms) {
    // Fade in
    for (int b = 0; b <= LED_BRIGHTNESS; b += 5) {
        rgb.setPixelColor(0, rgb.Color(
            c.r * b / 255,
            c.g * b / 255,
            c.b * b / 255
        ));
        rgb.show();
        delay(duration_ms / (LED_BRIGHTNESS / 5) / 2);
    }
    // Fade out
    for (int b = LED_BRIGHTNESS; b >= 0; b -= 5) {
        rgb.setPixelColor(0, rgb.Color(
            c.r * b / 255,
            c.g * b / 255,
            c.b * b / 255
        ));
        rgb.show();
        delay(duration_ms / (LED_BRIGHTNESS / 5) / 2);
    }
}


// ===================== HELPER =====================

uint8_t calc_checksum(uint8_t* data, int len) {
    uint8_t chk = 0;
    for (int i = 0; i < len; i++) chk ^= data[i];
    return chk;
}

void send_status(uint8_t status, uint8_t jar, uint8_t alarm, uint8_t seq) {
    uint8_t resp[PACKET_SIZE];
    resp[0] = HEADER_STATUS;
    resp[1] = status;
    resp[2] = jar;
    resp[3] = alarm;
    resp[4] = 0x00;
    resp[5] = 0x00;
    resp[6] = seq;
    resp[7] = calc_checksum(resp, 7);
    Serial.write(resp, PACKET_SIZE);
}


// ===================== COMMAND HANDLERS =====================

void handle_move_to(uint8_t target_jar, uint8_t seq) {
    if (!servo_on) {
        led_flash(COLOR_ALARM, 2, 100);
        send_status(STATUS_NOT_READY, current_jar, ALARM_NONE, seq);
        return;
    }

    if (target_jar >= NUM_JARS) {
        led_set(COLOR_ALARM);
        send_status(STATUS_ALARM, current_jar, ALARM_ENCODER_ERR, seq);
        return;
    }

    current_status = STATUS_MOVING;

    // VÀNG = đang di chuyển
    led_set(COLOR_MOVING);

    int steps = abs((int)target_jar - (int)current_jar);
    delay(steps * 300);  // Giả lập thời gian quay

    current_jar = target_jar;
    current_status = STATUS_IN_POSITION;

    // Hiện MÀU CỦA LỌ đích (mỗi loài muỗi 1 màu)
    led_set(JAR_COLORS[target_jar]);

    send_status(STATUS_IN_POSITION, current_jar, ALARM_NONE, seq);
}

void handle_home(uint8_t seq) {
    current_status = STATUS_MOVING;

    // XANH DƯƠNG = đang home
    led_fade_in_out(COLOR_HOME, 1000);

    current_jar = 0;
    current_status = STATUS_IN_POSITION;

    led_set(COLOR_READY);
    send_status(STATUS_IN_POSITION, 0, ALARM_NONE, seq);
}

void handle_servo_on(uint8_t seq) {
    servo_on = true;
    current_status = STATUS_IDLE;

    // XANH LÁ = servo bật, sẵn sàng
    led_flash(COLOR_READY, 2, 200);
    led_set(COLOR_READY);

    send_status(STATUS_IDLE, current_jar, ALARM_NONE, seq);
}

void handle_servo_off(uint8_t seq) {
    servo_on = false;
    current_status = STATUS_NOT_READY;

    // TRẮNG MỜ = servo tắt
    led_fade_in_out(COLOR_READY, 500);
    led_set(COLOR_SERVO_OFF);

    send_status(STATUS_NOT_READY, current_jar, ALARM_NONE, seq);
}

void handle_stop(uint8_t seq) {
    current_status = STATUS_IDLE;

    // ĐỎ nháy nhanh = dừng khẩn cấp
    led_flash(COLOR_ALARM, 5, 50);
    led_set(COLOR_READY);

    send_status(STATUS_IDLE, current_jar, ALARM_NONE, seq);
}

void handle_get_status(uint8_t seq) {
    send_status(current_status, current_jar, current_alarm, seq);
}

void handle_alarm_reset(uint8_t seq) {
    current_alarm = ALARM_NONE;
    current_status = servo_on ? STATUS_IDLE : STATUS_NOT_READY;

    led_flash(COLOR_HOME, 3, 100);
    led_set(servo_on ? COLOR_READY : COLOR_SERVO_OFF);

    send_status(current_status, current_jar, ALARM_NONE, seq);
}


// ===================== PROCESS PACKET =====================

void process_packet(uint8_t* pkt) {
    uint8_t expected_chk = calc_checksum(pkt, 7);
    if (pkt[7] != expected_chk) return;  // checksum sai

    uint8_t cmd = pkt[1];
    uint8_t p1  = pkt[2];
    uint8_t seq = pkt[6];

    switch (cmd) {
        case CMD_MOVE_TO:     handle_move_to(p1, seq);  break;
        case CMD_HOME:        handle_home(seq);         break;
        case CMD_STOP:        handle_stop(seq);         break;
        case CMD_SERVO_ON:    handle_servo_on(seq);     break;
        case CMD_SERVO_OFF:   handle_servo_off(seq);    break;
        case CMD_ALARM_RESET: handle_alarm_reset(seq);  break;
        case CMD_GET_STATUS:  handle_get_status(seq);   break;
        case CMD_NOP:         handle_get_status(seq);   break;
        default: break;
    }
}


// ===================== SETUP & LOOP =====================

void setup() {
    Serial.begin(115200);

    // Khởi tạo RGB LED
    rgb.begin();
    rgb.setBrightness(LED_BRIGHTNESS);
    rgb.show();

    // Animation boot: Tím → Đỏ → Vàng → Xanh lá → Xanh dương → Tím
    Color boot_seq[] = {COLOR_BOOT, COLOR_ALARM, COLOR_MOVING, COLOR_READY, COLOR_HOME, COLOR_BOOT};
    for (int i = 0; i < 6; i++) {
        led_set(boot_seq[i]);
        delay(200);
    }

    // Kết thúc boot: Trắng mờ = chờ lệnh
    led_set(COLOR_SERVO_OFF);

    current_status = STATUS_NOT_READY;
    rx_idx = 0;
}

void loop() {
    while (Serial.available()) {
        uint8_t b = Serial.read();

        if (rx_idx == 0 && b != HEADER_CMD) continue;

        rx_buf[rx_idx++] = b;

        if (rx_idx >= PACKET_SIZE) {
            process_packet(rx_buf);
            rx_idx = 0;
        }
    }
}

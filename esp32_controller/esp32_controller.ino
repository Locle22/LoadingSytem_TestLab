/*
 * ============================================================
 *   ESP32 Motor Controller – LoadingSystem V1
 *   Nhận lệnh từ Raspberry Pi qua USB Serial
 *   Điều khiển LED giả lập motor (chưa có motor thật)
 * ============================================================
 * 
 *   Protocol: 8 bytes mỗi packet
 *   TX (RPi → ESP32):  [0xAA][CMD][P1][P2][D_LO][D_HI][SEQ][CHK]
 *   RX (ESP32 → RPi):  [0x55][STATUS][JAR][ALARM][0][0][SEQ][CHK]
 */

// ===================== PROTOCOL DEFINITION =====================

// Header bytes
#define HEADER_CMD    0xAA
#define HEADER_STATUS 0x55
#define PACKET_SIZE   8

// Command types (RPi -> ESP32)
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

// Status flags (ESP32 -> RPi)
enum StatusFlag : uint8_t {
    STATUS_IDLE        = 0x00,
    STATUS_MOVING      = 0x01,
    STATUS_IN_POSITION = 0x02,
    STATUS_ALARM       = 0x03,
    STATUS_NOT_READY   = 0x04,
};

// Alarm codes
enum AlarmCode : uint8_t {
    ALARM_NONE         = 0x00,
    ALARM_OVER_CURRENT = 0x01,
    ALARM_OVER_VOLTAGE = 0x02,
    ALARM_ENCODER_ERR  = 0x05,
};

// ===================== PIN DEFINITIONS =====================

#define LED_BUILTIN_PIN  2    // LED xanh trên ESP32 DevKit
#define LED_STATUS_PIN   LED_BUILTIN_PIN

// Nếu có motor stepper/servo thật, đấu vào đây:
// #define PULSE_PIN    18
// #define DIR_PIN      19
// #define ENABLE_PIN   21

// ===================== GLOBAL STATE =====================

bool servo_on = false;
uint8_t current_jar = 0;
uint8_t current_status = STATUS_NOT_READY;
uint8_t current_alarm = ALARM_NONE;

// Buffer nhận lệnh
uint8_t rx_buf[PACKET_SIZE];
int rx_idx = 0;

// Số lọ trên đĩa xoay
#define NUM_JARS 10

// ===================== HELPER FUNCTIONS =====================

uint8_t calc_checksum(uint8_t* data, int len) {
    uint8_t chk = 0;
    for (int i = 0; i < len; i++) {
        chk ^= data[i];
    }
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

    // Debug print (dùng Serial2 nếu muốn debug riêng)
    // Serial2.printf("[TX] status=%d jar=%d alarm=%d seq=%d\n",
    //               status, jar, alarm, seq);
}

void blink_led(int times, int delay_ms) {
    for (int i = 0; i < times; i++) {
        digitalWrite(LED_STATUS_PIN, HIGH);
        delay(delay_ms);
        digitalWrite(LED_STATUS_PIN, LOW);
        delay(delay_ms);
    }
}

// ===================== COMMAND HANDLERS =====================

void handle_move_to(uint8_t target_jar, uint8_t seq) {
    if (!servo_on) {
        send_status(STATUS_NOT_READY, current_jar, ALARM_NONE, seq);
        return;
    }

    if (target_jar >= NUM_JARS) {
        send_status(STATUS_ALARM, current_jar, ALARM_ENCODER_ERR, seq);
        return;
    }

    // Tính số bước cần quay
    int steps = abs((int)target_jar - (int)current_jar);

    // === GIẢ LẬP MOTOR ===
    // Trên thật: phát xung Pulse/Direction cho driver
    // Trên giả lập: nháy LED + delay
    current_status = STATUS_MOVING;

    for (int i = 0; i < steps; i++) {
        digitalWrite(LED_STATUS_PIN, HIGH);
        delay(150);
        digitalWrite(LED_STATUS_PIN, LOW);
        delay(150);
        // Trên thật: phát N xung cho 1 bước lọ
        // pulse_generator_step(PULSES_PER_JAR);
    }

    current_jar = target_jar;
    current_status = STATUS_IN_POSITION;

    send_status(STATUS_IN_POSITION, current_jar, ALARM_NONE, seq);
}

void handle_home(uint8_t seq) {
    // Quay về vị trí 0
    current_status = STATUS_MOVING;

    // Giả lập homing
    blink_led(5, 100);

    current_jar = 0;
    current_status = STATUS_IN_POSITION;

    send_status(STATUS_IN_POSITION, 0, ALARM_NONE, seq);
}

void handle_servo_on(uint8_t seq) {
    servo_on = true;
    current_status = STATUS_IDLE;

    // LED sáng = servo ON
    digitalWrite(LED_STATUS_PIN, HIGH);
    delay(500);
    digitalWrite(LED_STATUS_PIN, LOW);

    send_status(STATUS_IDLE, current_jar, ALARM_NONE, seq);
}

void handle_servo_off(uint8_t seq) {
    servo_on = false;
    current_status = STATUS_NOT_READY;

    // LED nháy nhanh = servo OFF
    blink_led(3, 50);

    send_status(STATUS_NOT_READY, current_jar, ALARM_NONE, seq);
}

void handle_stop(uint8_t seq) {
    // Dừng khẩn cấp
    current_status = STATUS_IDLE;

    // Trên thật: dừng phát xung ngay lập tức
    // timer_stop();

    send_status(STATUS_IDLE, current_jar, ALARM_NONE, seq);
}

void handle_get_status(uint8_t seq) {
    send_status(current_status, current_jar, current_alarm, seq);
}

void handle_alarm_reset(uint8_t seq) {
    current_alarm = ALARM_NONE;
    current_status = servo_on ? STATUS_IDLE : STATUS_NOT_READY;

    send_status(current_status, current_jar, ALARM_NONE, seq);
}

// ===================== PROCESS PACKET =====================

void process_packet(uint8_t* pkt) {
    // Verify checksum
    uint8_t expected_chk = calc_checksum(pkt, 7);
    if (pkt[7] != expected_chk) {
        // Checksum sai → bỏ qua
        return;
    }

    uint8_t cmd  = pkt[1];
    uint8_t p1   = pkt[2];  // param1
    uint8_t p2   = pkt[3];  // param2
    uint16_t data = pkt[4] | (pkt[5] << 8);
    uint8_t seq  = pkt[6];

    switch (cmd) {
        case CMD_MOVE_TO:     handle_move_to(p1, seq);    break;
        case CMD_HOME:        handle_home(seq);           break;
        case CMD_STOP:        handle_stop(seq);           break;
        case CMD_SERVO_ON:    handle_servo_on(seq);       break;
        case CMD_SERVO_OFF:   handle_servo_off(seq);      break;
        case CMD_ALARM_RESET: handle_alarm_reset(seq);    break;
        case CMD_GET_STATUS:  handle_get_status(seq);     break;
        case CMD_NOP:         handle_get_status(seq);     break;
        default: break;
    }
}

// ===================== SETUP & LOOP =====================

void setup() {
    Serial.begin(115200);
    pinMode(LED_STATUS_PIN, OUTPUT);

    // Nháy LED 3 lần = ESP32 đã sẵn sàng
    blink_led(3, 200);

    current_status = STATUS_NOT_READY;
    rx_idx = 0;
}

void loop() {
    while (Serial.available()) {
        uint8_t b = Serial.read();

        // Chờ header byte 0xAA
        if (rx_idx == 0 && b != HEADER_CMD) {
            continue;  // Bỏ byte rác
        }

        rx_buf[rx_idx++] = b;

        // Nhận đủ 8 bytes → xử lý
        if (rx_idx >= PACKET_SIZE) {
            process_packet(rx_buf);
            rx_idx = 0;
        }
    }
}

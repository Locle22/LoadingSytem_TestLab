#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
=============================================================
  LOADING SYSTEM - Demo Simulation
  Chạy trên Raspberry Pi OS (VirtualBox)
  
  Mô phỏng toàn bộ luồng:
    Camera → AI nhận diện → Gửi lệnh → STM32 điều khiển motor
  
  Tất cả đều hardcode/giả lập vì chưa có phần cứng thật.
=============================================================
"""

import time
import random
import struct
import sys
import io
from enum import IntEnum
from dataclasses import dataclass
from typing import Optional

# Fix encoding cho Windows console
if sys.platform == "win32":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')


# ============================================================
# 1. ĐỊNH NGHĨA GIAO THỨC SPI (dùng chung với STM32 Rust)
# ============================================================

class CmdType(IntEnum):
    """Lệnh từ RPi gửi xuống STM32"""
    NOP         = 0x00   # Không làm gì (heartbeat)
    MOVE_TO     = 0x01   # Di chuyển đĩa xoay đến lọ N
    HOME        = 0x02   # Về vị trí gốc
    STOP        = 0x03   # Dừng khẩn cấp
    SET_SPEED   = 0x04   # Đặt tốc độ
    SERVO_ON    = 0x05   # Bật servo
    SERVO_OFF   = 0x06   # Tắt servo
    ALARM_RESET = 0x07   # Reset alarm
    GET_STATUS  = 0x08   # Yêu cầu trạng thái


class StatusFlag(IntEnum):
    """Trạng thái STM32 gửi lên RPi"""
    IDLE        = 0x00   # Đang rảnh
    MOVING      = 0x01   # Đang di chuyển
    IN_POSITION = 0x02   # Đã đến vị trí
    ALARM       = 0x03   # Có lỗi
    NOT_READY   = 0x04   # Chưa sẵn sàng


class AlarmCode(IntEnum):
    """Mã lỗi alarm"""
    NONE         = 0x00
    OVER_CURRENT = 0x01
    OVER_VOLTAGE = 0x02
    ENCODER_ERR  = 0x05


HEADER_CMD    = 0xAA
HEADER_STATUS = 0x55


@dataclass
class CmdPacket:
    """Gói lệnh 8 byte: RPi → STM32"""
    cmd: CmdType
    param1: int = 0
    param2: int = 0
    data: int = 0
    seq: int = 0

    def to_bytes(self) -> bytes:
        payload = struct.pack("BBBBHB",
            HEADER_CMD,
            self.cmd,
            self.param1,
            self.param2,
            self.data,
            self.seq
        )
        checksum = 0
        for b in payload:
            checksum ^= b
        return payload + bytes([checksum & 0xFF])

    def __str__(self):
        return (f"[CMD] header=0x{HEADER_CMD:02X} cmd={self.cmd.name} "
                f"param1={self.param1} param2={self.param2} "
                f"data={self.data} seq={self.seq}")


@dataclass
class StatusPacket:
    """Gói trạng thái 8 byte: STM32 → RPi"""
    status: StatusFlag
    jar_pos: int = 0
    alarm: AlarmCode = AlarmCode.NONE
    seq: int = 0

    def to_bytes(self) -> bytes:
        payload = struct.pack("BBBBHB",
            HEADER_STATUS,
            self.status,
            self.jar_pos,
            self.alarm,
            0,
            self.seq
        )
        checksum = 0
        for b in payload:
            checksum ^= b
        return payload + bytes([checksum & 0xFF])

    def __str__(self):
        return (f"[STATUS] status={self.status.name} "
                f"jar_pos={self.jar_pos} alarm={self.alarm.name}")


# ============================================================
# 2. GIẢ LẬP CAMERA
# ============================================================

class FakeCamera:
    """
    Giả lập camera HIGH CLOUD 4K
    Trên thật: dùng OpenCV cv2.VideoCapture(0)
    Trên giả lập: trả về frame giả
    """

    def __init__(self, width=1920, height=1080, fps=30):
        self.width = width
        self.height = height
        self.fps = fps
        self.frame_count = 0
        self.is_open = False

    def open(self):
        print(f"  [CAMERA] Đang kết nối camera USB...")
        time.sleep(0.5)
        self.is_open = True
        print(f"  [CAMERA] ✓ Camera HIGH CLOUD 4K đã kết nối")
        print(f"  [CAMERA]   Resolution: {self.width}x{self.height}")
        print(f"  [CAMERA]   FPS: {self.fps}")
        print(f"  [CAMERA]   Interface: USB UVC")

    def capture_frame(self):
        """
        Trên thật:
            ret, frame = cv2.VideoCapture(0).read()
        Trên giả lập:
            Trả về frame giả (chỉ là dict mô tả)
        """
        if not self.is_open:
            return None

        self.frame_count += 1
        time.sleep(1.0 / self.fps)  # giả lập thời gian capture

        return {
            "frame_id": self.frame_count,
            "width": self.width,
            "height": self.height,
            "timestamp": time.time(),
        }

    def close(self):
        self.is_open = False
        print(f"  [CAMERA] Camera đã ngắt kết nối")


# ============================================================
# 3. GIẢ LẬP AI NHẬN DIỆN
# ============================================================

# Database muỗi (hardcode)
MOSQUITO_DB = {
    0: {"name": "Aedes aegypti",      "vi": "Muỗi vằn",         "danger": "Sốt xuất huyết"},
    1: {"name": "Aedes albopictus",    "vi": "Muỗi hổ châu Á",   "danger": "Zika, Dengue"},
    2: {"name": "Anopheles gambiae",   "vi": "Muỗi Anopheles",    "danger": "Sốt rét"},
    3: {"name": "Culex pipiens",       "vi": "Muỗi nhà",          "danger": "Viêm não Nhật Bản"},
    4: {"name": "Culex quinquefas.",    "vi": "Muỗi Culex",        "danger": "Giun chỉ"},
    5: {"name": "Anopheles stephensi", "vi": "Muỗi Anopheles S.", "danger": "Sốt rét"},
    6: {"name": "Aedes polynesiensis", "vi": "Muỗi Polynesia",    "danger": "Giun chỉ"},
    7: {"name": "Mansonia uniformis",  "vi": "Muỗi Mansonia",     "danger": "Giun chỉ"},
    8: {"name": "Toxorhynchites sp.",  "vi": "Muỗi khổng lồ",     "danger": "Không nguy hiểm"},
    9: {"name": "Armigeres subalbatus","vi": "Muỗi Armigeres",    "danger": "Viêm não Nhật Bản"},
}

NUM_JARS = len(MOSQUITO_DB)  # 10 lọ tương ứng 10 loài


class FakeAI:
    """
    Giả lập model AI nhận diện muỗi
    Trên thật: dùng ONNX Runtime hoặc TensorRT trên Jetson
    Trên giả lập: random kết quả
    """

    def __init__(self):
        self.model_loaded = False
        self.inference_count = 0

    def load_model(self, model_path="mosquito_model.onnx"):
        print(f"  [AI] Đang load model: {model_path}")
        time.sleep(0.8)
        self.model_loaded = True
        print(f"  [AI] ✓ Model loaded thành công")
        print(f"  [AI]   Classes: {NUM_JARS} loài muỗi")
        print(f"  [AI]   Input size: 640x640")
        print(f"  [AI]   Backend: ONNX Runtime (giả lập)")

    def detect(self, frame) -> Optional[dict]:
        """
        Trên thật:
            input = preprocess(frame)
            output = model.run(input)
            return postprocess(output)
        Trên giả lập:
            Random một kết quả nhận diện
        """
        if not self.model_loaded or frame is None:
            return None

        self.inference_count += 1

        # 70% có muỗi, 30% không có (giả lập thực tế)
        if random.random() < 0.3:
            return None  # Không phát hiện muỗi trong frame này

        species_id = random.randint(0, NUM_JARS - 1)
        confidence = random.uniform(0.75, 0.99)

        # Tọa độ bounding box giả
        cx = random.randint(200, 1700)
        cy = random.randint(200, 900)
        w = random.randint(30, 80)
        h = random.randint(20, 60)

        return {
            "species_id": species_id,
            "species_name": MOSQUITO_DB[species_id]["name"],
            "species_vi": MOSQUITO_DB[species_id]["vi"],
            "confidence": confidence,
            "bbox": {"cx": cx, "cy": cy, "w": w, "h": h},
            "frame_id": frame["frame_id"],
        }


# ============================================================
# 4. GIẢ LẬP GIAO TIẾP SPI VỚI STM32
# ============================================================

class FakeSPI:
    """
    Giả lập SPI Master (RPi) giao tiếp với STM32 Slave
    Trên thật: dùng spidev library
        spi = spidev.SpiDev()
        spi.open(0, 0)
        response = spi.xfer2(cmd_bytes)
    Trên giả lập: in ra console và giả phản hồi
    """

    def __init__(self):
        self.is_open = False
        self.seq = 0
        self.current_jar = 0  # vị trí đĩa xoay hiện tại

    def open(self, bus=0, device=0, speed_hz=1_000_000):
        print(f"  [SPI] Đang mở SPI bus={bus} device={device}...")
        time.sleep(0.3)
        self.is_open = True
        print(f"  [SPI] ✓ SPI Master đã kết nối")
        print(f"  [SPI]   Speed: {speed_hz/1_000_000:.1f} MHz")
        print(f"  [SPI]   Mode: Full-duplex")

    def send_command(self, cmd: CmdType, param1=0, param2=0, data=0) -> StatusPacket:
        """Gửi lệnh và nhận phản hồi (full-duplex)"""
        self.seq = (self.seq + 1) & 0xFF

        packet = CmdPacket(
            cmd=cmd,
            param1=param1,
            param2=param2,
            data=data,
            seq=self.seq
        )

        # In raw bytes
        raw = packet.to_bytes()
        hex_str = " ".join(f"{b:02X}" for b in raw)
        print(f"  [SPI TX] {packet}")
        print(f"           Raw: [{hex_str}] ({len(raw)} bytes)")

        # Giả lập STM32 phản hồi
        time.sleep(0.05)  # giả lập latency SPI

        if cmd == CmdType.MOVE_TO:
            # Giả lập: đĩa xoay di chuyển
            old_jar = self.current_jar
            self.current_jar = param1

            # Giả lập thời gian quay đĩa
            jars_to_move = abs(param1 - old_jar)
            move_time = jars_to_move * 0.3  # 0.3s mỗi lọ
            print(f"  [SPI RX] STM32 đang quay đĩa: lọ {old_jar} → lọ {param1} "
                  f"({jars_to_move} bước, ~{move_time:.1f}s)")
            time.sleep(min(move_time, 2.0))  # giới hạn tối đa 2s cho demo

            return StatusPacket(
                status=StatusFlag.IN_POSITION,
                jar_pos=param1,
                seq=self.seq
            )

        elif cmd == CmdType.SERVO_ON:
            return StatusPacket(
                status=StatusFlag.IDLE,
                jar_pos=self.current_jar,
                seq=self.seq
            )

        elif cmd == CmdType.HOME:
            self.current_jar = 0
            time.sleep(1.0)
            return StatusPacket(
                status=StatusFlag.IN_POSITION,
                jar_pos=0,
                seq=self.seq
            )

        elif cmd == CmdType.GET_STATUS:
            return StatusPacket(
                status=StatusFlag.IDLE,
                jar_pos=self.current_jar,
                seq=self.seq
            )

        else:
            return StatusPacket(
                status=StatusFlag.IDLE,
                jar_pos=self.current_jar,
                seq=self.seq
            )

    def close(self):
        self.is_open = False
        print(f"  [SPI] SPI đã đóng")


# ============================================================
# 5. MAIN – CHƯƠNG TRÌNH CHÍNH
# ============================================================

def print_banner():
    print("=" * 64)
    print("   LOADING SYSTEM – Hệ thống Phân Loại Muỗi Tự Động")
    print("   Demo Simulation trên Raspberry Pi OS (VirtualBox)")
    print("=" * 64)
    print()
    print("   Kiến trúc: Camera → AI → SPI → STM32 → Motor")
    print("   Ngôn ngữ:  Python (RPi) + Rust (STM32)")
    print("   Phiên bản: V1 – 1 muỗi/phút (giả lập)")
    print()
    print("=" * 64)


def print_detection(det):
    """In kết quả nhận diện đẹp"""
    info = MOSQUITO_DB[det["species_id"]]
    print(f"  ┌─────────────────────────────────────────────┐")
    print(f"  │  PHÁT HIỆN MUỖI!                            │")
    print(f"  ├─────────────────────────────────────────────┤")
    print(f"  │  Loài  : {info['name']:<35s}│")
    print(f"  │  Tên VN: {info['vi']:<35s}│")
    print(f"  │  Lọ số : {det['species_id']:<35d}│")
    print(f"  │  Conf  : {det['confidence']:.1%}{' ':>30s}│")
    print(f"  │  Bbox  : ({det['bbox']['cx']}, {det['bbox']['cy']})"
          f" {det['bbox']['w']}x{det['bbox']['h']}{' ':>18s}│")
    print(f"  │  Bệnh  : {info['danger']:<35s}│")
    print(f"  └─────────────────────────────────────────────┘")


def main():
    print_banner()

    # ---------- KHỞI TẠO ----------
    print("\n[INIT] Khởi tạo hệ thống...\n")

    camera = FakeCamera(1920, 1080, fps=30)
    ai = FakeAI()
    spi = FakeSPI()

    camera.open()
    print()
    ai.load_model()
    print()
    spi.open()

    # ---------- BẬT SERVO ----------
    print("\n[SYSTEM] Bật servo drive...\n")
    response = spi.send_command(CmdType.SERVO_ON)
    print(f"  [SPI RX] {response}")

    # ---------- HOME ----------
    print("\n[SYSTEM] Đưa đĩa xoay về vị trí gốc (Home)...\n")
    response = spi.send_command(CmdType.HOME)
    print(f"  [SPI RX] {response}")

    # ---------- VÒNG LẶP CHÍNH ----------
    print("\n" + "=" * 64)
    print("  BẮT ĐẦU PHÂN LOẠI – Nhấn Ctrl+C để dừng")
    print("=" * 64)

    conf_threshold = 0.85
    stats = {i: 0 for i in range(NUM_JARS)}
    total_detected = 0
    total_sorted = 0
    total_frames = 0

    try:
        while True:
            total_frames += 1
            print(f"\n--- Frame #{total_frames} ---")

            # Bước 1: Capture ảnh
            t0 = time.time()
            frame = camera.capture_frame()
            t_capture = (time.time() - t0) * 1000

            # Bước 2: AI nhận diện
            t1 = time.time()
            detection = ai.detect(frame)
            t_ai = (time.time() - t1) * 1000

            if detection is None:
                print(f"  [AI] Không phát hiện muỗi "
                      f"(capture: {t_capture:.0f}ms, AI: {t_ai:.0f}ms)")
                time.sleep(1.0)  # chờ 1s rồi chụp tiếp
                continue

            total_detected += 1

            # Bước 3: Kiểm tra confidence
            if detection["confidence"] < conf_threshold:
                print(f"  [AI] Phát hiện {detection['species_vi']} "
                      f"nhưng confidence thấp ({detection['confidence']:.1%} "
                      f"< {conf_threshold:.0%}) → BỎ QUA")
                time.sleep(1.0)
                continue

            # Bước 4: In kết quả
            print_detection(detection)

            # Bước 5: Gửi lệnh di chuyển đĩa xoay
            jar_id = detection["species_id"]
            print(f"\n  [CONTROL] Gửi lệnh: Xoay đĩa đến lọ {jar_id}...\n")
            t2 = time.time()
            response = spi.send_command(CmdType.MOVE_TO, param1=jar_id)
            t_move = (time.time() - t2) * 1000
            print(f"  [SPI RX] {response}")

            # Bước 6: Kiểm tra kết quả
            if response.status == StatusFlag.IN_POSITION:
                total_sorted += 1
                stats[jar_id] += 1
                print(f"\n  ✅ THÀNH CÔNG! {detection['species_vi']} "
                      f"→ Lọ {jar_id} (tổng lọ {jar_id}: {stats[jar_id]} con)")
            elif response.status == StatusFlag.ALARM:
                print(f"\n  ❌ LỖI! Alarm code: {response.alarm.name}")
                print(f"  [CONTROL] Đang reset alarm...")
                spi.send_command(CmdType.ALARM_RESET)

            # Bước 7: In thống kê
            t_total = t_capture + t_ai + t_move
            print(f"\n  ⏱  Thời gian: capture={t_capture:.0f}ms "
                  f"AI={t_ai:.0f}ms motor={t_move:.0f}ms "
                  f"TOTAL={t_total:.0f}ms")
            print(f"  📊 Thống kê: {total_sorted}/{total_detected} con đã phân loại "
                  f"({total_sorted/max(total_detected,1):.0%})")

            # Chờ trước khi xử lý con tiếp (V1: 1 muỗi/phút)
            print(f"\n  ⏳ Chờ muỗi tiếp theo...")
            time.sleep(2.0)  # rút ngắn cho demo (thật: 60s)

    except KeyboardInterrupt:
        print("\n\n" + "=" * 64)
        print("  DỪNG HỆ THỐNG")
        print("=" * 64)

    # ---------- TẮT HỆ THỐNG ----------
    print("\n[SHUTDOWN] Đang tắt hệ thống...\n")

    spi.send_command(CmdType.HOME)
    spi.send_command(CmdType.SERVO_OFF)
    spi.close()
    camera.close()

    # ---------- IN THỐNG KÊ CUỐI ----------
    print("\n" + "=" * 64)
    print("  BÁO CÁO PHÂN LOẠI")
    print("=" * 64)
    print(f"\n  Tổng frames   : {total_frames}")
    print(f"  Phát hiện muỗi: {total_detected}")
    print(f"  Đã phân loại  : {total_sorted}")
    print()
    print(f"  {'Lọ':<5s} {'Loài':<25s} {'Số lượng':<10s}")
    print(f"  {'─'*5} {'─'*25} {'─'*10}")
    for jar_id in range(NUM_JARS):
        name = MOSQUITO_DB[jar_id]["vi"]
        count = stats[jar_id]
        bar = "█" * count
        print(f"  {jar_id:<5d} {name:<25s} {count:<5d} {bar}")
    print(f"\n  Tổng: {total_sorted} con muỗi đã phân loại thành công")
    print("=" * 64)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
============================================================
  RPi Serial Controller – LoadingSystem V1
  Giao tiếp với ESP32/STM32 qua USB Serial
  
  Luồng: Camera (giả lập) → AI (giả lập) → Serial → ESP32
  
  Cách dùng:
    python3 rpi_serial_controller.py              # auto-detect port
    python3 rpi_serial_controller.py /dev/ttyUSB0  # chỉ định port
============================================================
"""

import serial
import serial.tools.list_ports
import struct
import time
import random
import sys
from enum import IntEnum


# ============================================================
# PROTOCOL (giống hệt trong loading_system_demo.py)
# ============================================================

class CmdType(IntEnum):
    NOP         = 0x00
    MOVE_TO     = 0x01
    HOME        = 0x02
    STOP        = 0x03
    SET_SPEED   = 0x04
    SERVO_ON    = 0x05
    SERVO_OFF   = 0x06
    ALARM_RESET = 0x07
    GET_STATUS  = 0x08

class StatusFlag(IntEnum):
    IDLE        = 0x00
    MOVING      = 0x01
    IN_POSITION = 0x02
    ALARM       = 0x03
    NOT_READY   = 0x04

HEADER_CMD    = 0xAA
HEADER_STATUS = 0x55
PACKET_SIZE   = 8


# ============================================================
# DATABASE MUỖI (hardcode)
# ============================================================

MOSQUITO_DB = {
    0: {"name": "Aedes aegypti",       "vi": "Muỗi vằn"},
    1: {"name": "Aedes albopictus",    "vi": "Muỗi hổ châu Á"},
    2: {"name": "Anopheles gambiae",   "vi": "Muỗi Anopheles"},
    3: {"name": "Culex pipiens",       "vi": "Muỗi nhà"},
    4: {"name": "Culex quinquefas.",   "vi": "Muỗi Culex"},
    5: {"name": "Anopheles stephensi", "vi": "Muỗi Anopheles S."},
    6: {"name": "Aedes polynesiensis", "vi": "Muỗi Polynesia"},
    7: {"name": "Mansonia uniformis",  "vi": "Muỗi Mansonia"},
    8: {"name": "Toxorhynchites sp.",  "vi": "Muỗi khổng lồ"},
    9: {"name": "Armigeres subalbatus","vi": "Muỗi Armigeres"},
}

NUM_JARS = len(MOSQUITO_DB)


# ============================================================
# SERIAL COMMUNICATION
# ============================================================

class SerialController:
    def __init__(self, port=None, baudrate=115200):
        self.ser = None
        self.seq = 0
        self.port = port
        self.baudrate = baudrate

    def find_port(self):
        """Tự tìm cổng ESP32/STM32"""
        ports = serial.tools.list_ports.comports()
        print("\n  Các cổng Serial tìm thấy:")
        for p in ports:
            print(f"    {p.device}: {p.description}")
            # ESP32 thường dùng chip CP2102 hoặc CH340
            if any(k in p.description.lower() for k in
                   ["cp210", "ch340", "ch9102", "ftdi", "usb serial",
                    "usb-serial", "uart", "silicon labs", "wch"]):
                print(f"    >>> Có vẻ là ESP32/STM32!")
                return p.device

        if ports:
            # Nếu không nhận diện được, lấy port đầu tiên
            print(f"\n  Không nhận diện rõ, thử dùng: {ports[0].device}")
            return ports[0].device

        return None

    def connect(self):
        """Kết nối serial"""
        if self.port is None:
            self.port = self.find_port()

        if self.port is None:
            print("\n  [LỖI] Không tìm thấy cổng Serial nào!")
            print("  Kiểm tra:")
            print("    - ESP32 đã cắm USB chưa?")
            print("    - Driver CH340/CP2102 đã cài chưa?")
            print("    - VirtualBox USB filter đã bật chưa?")
            return False

        try:
            self.ser = serial.Serial(self.port, self.baudrate, timeout=2)
            time.sleep(2)  # chờ ESP32 reset sau khi mở serial
            self.ser.reset_input_buffer()

            print(f"\n  [SERIAL] Đã kết nối: {self.port} @ {self.baudrate} baud")
            return True

        except serial.SerialException as e:
            print(f"\n  [LỖI] Không mở được {self.port}: {e}")
            return False

    def send_cmd(self, cmd, param1=0, param2=0, data=0):
        """Gửi lệnh 8 bytes, nhận phản hồi 8 bytes"""
        self.seq = (self.seq + 1) & 0xFF

        # Đóng gói
        payload = bytes([
            HEADER_CMD,
            cmd,
            param1 & 0xFF,
            param2 & 0xFF,
            data & 0xFF,
            (data >> 8) & 0xFF,
            self.seq
        ])
        checksum = 0
        for b in payload:
            checksum ^= b
        packet = payload + bytes([checksum & 0xFF])

        # Gửi
        hex_str = " ".join(f"{b:02X}" for b in packet)
        cmd_name = CmdType(cmd).name if cmd in CmdType._value2member_map_ else f"0x{cmd:02X}"
        print(f"  [TX] cmd={cmd_name} p1={param1} p2={param2} seq={self.seq}")
        print(f"       Raw: [{hex_str}]")

        self.ser.write(packet)

        # Nhận phản hồi
        response = self.ser.read(PACKET_SIZE)
        if len(response) < PACKET_SIZE:
            print(f"  [RX] Timeout! Chỉ nhận {len(response)} bytes")
            return None

        # Parse
        header  = response[0]
        status  = response[1]
        jar_pos = response[2]
        alarm   = response[3]
        seq     = response[6]

        if header != HEADER_STATUS:
            print(f"  [RX] Header sai: 0x{header:02X} (expect 0x55)")
            return None

        status_name = StatusFlag(status).name if status in StatusFlag._value2member_map_ else f"0x{status:02X}"
        hex_str = " ".join(f"{b:02X}" for b in response)
        print(f"  [RX] status={status_name} jar={jar_pos} alarm={alarm} seq={seq}")
        print(f"       Raw: [{hex_str}]")

        return {
            "status": status,
            "jar_pos": jar_pos,
            "alarm": alarm,
            "seq": seq,
        }

    def close(self):
        if self.ser and self.ser.is_open:
            self.ser.close()
            print(f"  [SERIAL] Đã ngắt kết nối")


# ============================================================
# AI GIẢ LẬP
# ============================================================

def fake_ai_detect():
    """Giả lập AI nhận diện muỗi (random)"""
    if random.random() < 0.3:
        return None  # 30% không phát hiện

    species_id = random.randint(0, NUM_JARS - 1)
    confidence = random.uniform(0.75, 0.99)
    return {
        "species_id": species_id,
        "confidence": confidence,
    }


# ============================================================
# MAIN
# ============================================================

def main():
    print("=" * 60)
    print("  LOADING SYSTEM – RPi Serial Controller")
    print("  Giao tiếp RPi <-> ESP32 qua USB Serial")
    print("=" * 60)

    # Xác định port
    port = sys.argv[1] if len(sys.argv) > 1 else None

    # Kết nối
    ctrl = SerialController(port=port)
    if not ctrl.connect():
        sys.exit(1)

    # Bật servo
    print("\n--- Bật Servo ---")
    resp = ctrl.send_cmd(CmdType.SERVO_ON)

    # Home
    print("\n--- Home ---")
    resp = ctrl.send_cmd(CmdType.HOME)

    # Vòng lặp chính
    print("\n" + "=" * 60)
    print("  BẮT ĐẦU PHÂN LOẠI – Ctrl+C để dừng")
    print("=" * 60)

    stats = {i: 0 for i in range(NUM_JARS)}
    total = 0
    conf_threshold = 0.85

    try:
        frame = 0
        while True:
            frame += 1
            print(f"\n--- Frame #{frame} ---")

            # AI nhận diện
            det = fake_ai_detect()

            if det is None:
                print("  [AI] Không phát hiện muỗi")
                time.sleep(1)
                continue

            info = MOSQUITO_DB[det["species_id"]]

            if det["confidence"] < conf_threshold:
                print(f"  [AI] {info['vi']} - confidence thấp "
                      f"({det['confidence']:.1%}) -> bỏ qua")
                time.sleep(1)
                continue

            print(f"  [AI] Phát hiện: {info['vi']} "
                  f"({info['name']}) conf={det['confidence']:.1%}")

            # Gửi lệnh xoay đĩa
            jar = det["species_id"]
            print(f"\n  >>> Xoay đĩa đến lọ {jar}...")
            t0 = time.time()
            resp = ctrl.send_cmd(CmdType.MOVE_TO, param1=jar)
            dt = (time.time() - t0) * 1000

            if resp and resp["status"] == StatusFlag.IN_POSITION:
                total += 1
                stats[jar] += 1
                print(f"\n  OK! {info['vi']} -> Lọ {jar} "
                      f"({stats[jar]} con) [{dt:.0f}ms]")
            elif resp and resp["status"] == StatusFlag.ALARM:
                print(f"\n  LOI! Alarm -> reset...")
                ctrl.send_cmd(CmdType.ALARM_RESET)
            else:
                print(f"\n  LOI! Timeout hoặc lỗi giao tiếp")

            print(f"  Tổng: {total} con đã phân loại")
            time.sleep(2)

    except KeyboardInterrupt:
        print("\n\nDừng hệ thống...")

    # Tắt
    ctrl.send_cmd(CmdType.HOME)
    ctrl.send_cmd(CmdType.SERVO_OFF)
    ctrl.close()

    # Báo cáo
    print("\n" + "=" * 60)
    print("  BÁO CÁO")
    print("=" * 60)
    for jar_id in range(NUM_JARS):
        name = MOSQUITO_DB[jar_id]["vi"]
        count = stats[jar_id]
        if count > 0:
            print(f"  Lọ {jar_id}: {name:25s} {count} con")
    print(f"\n  Tổng: {total} con")
    print("=" * 60)


if __name__ == "__main__":
    main()

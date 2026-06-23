# MosquitoSortingLab 🦟

Automated mosquito sorting system using AI vision and embedded control.

## Project Overview

**LoadingSystem** – An industrial-grade automated system that identifies, classifies, and sorts mosquito specimens into corresponding glass jars using:

- **AI Vision**: Camera + Deep Learning model (YOLO/MobileNet) on Jetson Nano / Raspberry Pi
- **Embedded Control**: STM32 microcontroller programmed in **Rust**
- **Servo Drive**: CSD7_02BX1 (RS Automation, 200W) for precision turntable positioning
- **Communication**: SPI protocol (binary packets, enum-based)

## Architecture

```
Camera (4K) → Raspberry Pi→ SPI → STM32 (Rust) → Servo Drive → Motor
                    (AI inference)              (Pulse/Direction)
```

## Quick Start

```bash
# Run the simulation demo (no hardware needed)
python3 loading_system_demo.py
```

Press `Ctrl+C` to stop and see the sorting statistics report.


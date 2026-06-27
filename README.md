# MosquitoSortingLab 🦟

Automated mosquito sorting system using AI vision and embedded control.

## Project Overview

**LoadingSystem** – An industrial-grade automated system that identifies, classifies, and sorts mosquito specimens into corresponding glass jars using:

- **AI Vision**: Camera + Deep Learning model (YOLO/MobileNet) on Jetson Nano / Raspberry Pi
- **Embedded Control**: ESP32 microcontroller programmed in **Rust**
- **Servo Drive**: CSD7_02BX1 (RS Automation, 200W) for precision turntable positioning
- **Communication**: Wifi/Bluetooth

## Architecture

```
Camera (4K) → Raspberry Pi→ wifi/bluetooth → ESP32 (Rust) → Servo Drive → Motor
                    (AI inference)              (Pulse/Direction)
```



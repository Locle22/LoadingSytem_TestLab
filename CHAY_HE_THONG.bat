@echo off
title Mosquito Detector System Starter (V2)
color 0B

echo =================================================================
echo        KHOI DONG HE THONG PHAN LOAI MUOI TU DONG V2
echo =================================================================
echo.

:: 1. Lua chon camera
echo CHON NGUON CAMERA:
echo   [1] DroidCam (Ket noi qua WiFi)
echo   [2] Kinh hien vi / Camera USB cam day (HDMI USB Microscope)
set /p "CAM_MODE=Moi nhap lua chon (1 hoac 2, Mac dinh: 1): "
if "%CAM_MODE%"=="" set "CAM_MODE=1"

if "%CAM_MODE%"=="2" goto USB_CAM

:DROID_CAM
set "DEFAULT_CAM=192.168.1.5"
set /p "CAM_IP=Nhap IP cua dien thoai DroidCam (Mac dinh: %DEFAULT_CAM%): "
if "%CAM_IP%"=="" set "CAM_IP=%DEFAULT_CAM%"
set "CAM_ARG=--camera http://%CAM_IP%:4747/video"
goto CHOOSE_HW

:USB_CAM
set /p "USB_INDEX=Nhap chi so cong USB Camera (Mac dinh: 0): "
if "%USB_INDEX%"=="" set "USB_INDEX=0"
set "CAM_ARG=--usb-cam %USB_INDEX%"
goto CHOOSE_HW

:CHOOSE_HW
echo.
echo CHON THIET BI DIEU KHIEN PHAN CUNG:
echo   [1] ESP32 S3 (Tu dong phan giai hostname hoac UDP Broadcast)
echo   [2] PLC cong nghiep (Che do in gia lap console)
set /p "HW_MODE=Moi nhap lua chon (1 hoac 2, Mac dinh: 1): "
if "%HW_MODE%"=="" set "HW_MODE=1"

set "HW_ARG="
if "%HW_MODE%"=="2" set "HW_ARG=--plc"

echo.
echo -----------------------------------------------------------------
echo   -> Nguon video: %CAM_ARG%
if "%HW_MODE%"=="1" (
    echo   -> Thiet bi:   ESP32 S3 (Auto-Discovery)
) else (
    echo   -> Thiet bi:   PLC cong nghiep (Stub)
)
echo -----------------------------------------------------------------
echo.
echo [*] Dang chuan bi khoi dong he thong...
echo [*] Tu dong mo trinh duyet sau 3 giay...

:: Cho 3 giay va tu dong mo trinh duyet den giao dien
timeout /t 3 /nobreak >nul
start "" "http://localhost:3000"

:: Khoi dong backend Rust
cargo run --release -p app_cli -- --model "app_cli\models\moquito_v19_e150.onnx" %CAM_ARG% %HW_ARG%

pause

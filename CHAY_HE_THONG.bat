@echo off
title Mosquito Detector System Starter (V2)
color 0B

echo =================================================================
echo        KHOI DONG HE THONG PHAN LOAI MUOI TU DONG V2
echo =================================================================
echo.

:: Nhap IP DroidCam
set "DEFAULT_CAM=192.168.1.5"
set /p "CAM_IP=1. Nhap IP cua dien thoai DroidCam (Mac dinh: %DEFAULT_CAM%): "
if "%CAM_IP%"=="" set "CAM_IP=%DEFAULT_CAM%"

:: Nhap IP ESP32
set "DEFAULT_ESP=192.168.1.15"
set /p "ESP_IP=2. Nhap IP cua mach ESP32 S3 (Mac dinh: %DEFAULT_ESP%): "
if "%ESP_IP%"=="" set "ESP_IP=%DEFAULT_ESP%"

:: Lua chon che do Chay voi ESP32 hay PLC
echo.
echo CHON CHE DO DIEU KHIEN:
echo   [1] ESP32 S3 (Thiet bi IoT mac dinh)
echo   [2] PLC cong nghiep (Che do gia lap stub)
set /p "MODE=Moi nhap lua chon (1 hoac 2, Mac dinh: 1): "
if "%MODE%"=="" set "MODE=1"

echo.
echo -----------------------------------------------------------------
echo   -> IP Camera:  http://%CAM_IP%:4747/video
if "%MODE%"=="1" (
    echo   -> IP ESP32:   %ESP_IP%
) else (
    echo   -> Che do:     PLC cong nghiep
)
echo -----------------------------------------------------------------
echo.
echo [*] Dang chuan bi he thong...
echo [*] Tu dong mo trinh duyet sau 3 giay...

:: Cho 3 giay va tu dong mo trinh duyet den giao dien
timeout /t 3 /nobreak >nul
start "" "http://localhost:3000"

:: Khoi dong backend Rust
if "%MODE%"=="1" (
    cargo run --release -p app_cli -- --model "app_cli\models\moquito_v19_e150.onnx" --camera "http://%CAM_IP%:4747/video" --esp-ip "%ESP_IP%"
) else (
    cargo run --release -p app_cli -- --model "app_cli\models\moquito_v19_e150.onnx" --camera "http://%CAM_IP%:4747/video" --plc
)

pause

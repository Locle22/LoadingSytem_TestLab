// =============================================================================
// ESP32/src/main.rs
// =============================================================================
// Entry point cho firmware ESP32-S3 Node.
//
// Khởi tạo peripherals (SPI slave, GPIO), tạo các module (Application,
// Transport, Watchdog, E-Stop), và chạy vòng lặp chính.
//
// Vòng lặp chính:
// 1. Kiểm tra watchdog timeout
// 2. Kiểm tra hardware e-stop (GPIO4)
// 3. Kiểm tra SPI slave có dữ liệu mới
// 4. Xử lý lệnh qua Application layer
// 5. Gửi phản hồi qua SPI slave
//
// LƯU Ý: Code này được thiết kế để COMPILE CORRECT. Khi chạy trên phần cứng
// thật, cần thay thế các TODO trong spi_slave.rs bằng esp-hal API thực tế.
// =============================================================================

#![no_std]
#![no_main]

// ── Extern crates ────────────────────────────────────────────────────────────
// esp-backtrace cung cấp panic handler bắt buộc cho no_std (FR-18: safe state)
use esp_backtrace as _;
use esp_println::println;

// ── Application modules ─────────────────────────────────────────────────────
mod emergency;
mod motor_hal;
mod node_app;
mod node_protocol;
mod spi_slave;
mod watchdog;

use node_app::NodeApplication;
use spi_slave::SpiSlaveTransport;
use spi_protocol::config::FRAME_SIZE;
use spi_protocol::transport::Transport;

// ─── Entry Point ─────────────────────────────────────────────────────────────

#[esp_hal::main]
fn main() -> ! {
    // ── Khởi tạo ESP32-S3 peripherals ──────────────────────────────────────
    println!("\n");
    println!("############################################################");
    println!("#                                                          #");
    println!("#   ESP32-S3 Node — SPI Motor Control Protocol v0.1.0     #");
    println!("#                                                          #");
    println!("############################################################");
    println!("");

    // Khởi tạo HAL — lấy quyền sở hữu peripherals
    let _config = esp_hal::Config::default();

    // TODO: Cấu hình GPIO4 làm input pull-up cho hardware e-stop
    // ```ignore
    // let io = esp_hal::gpio::Io::new(peripherals.GPIO, peripherals.IO_MUX);
    // let estop_pin = io.pins.gpio4.into_pull_up_input();
    // ```

    // TODO: Cấu hình SPI2 ở chế độ slave
    // ```ignore
    // let spi = esp_hal::spi::slave::Spi::new(
    //     peripherals.SPI2,
    //     io.pins.gpio12, // SCLK
    //     io.pins.gpio13, // MOSI
    //     io.pins.gpio11, // MISO
    //     io.pins.gpio10, // CS
    //     esp_hal::spi::SpiMode::Mode0,
    // );
    // let transport = SpiSlaveTransport::new(spi);
    // ```

    // ── Khởi tạo các module ────────────────────────────────────────────────
    let mut transport = SpiSlaveTransport::new();
    let mut app = NodeApplication::new();

    // Giả lập trạng thái GPIO4 (trong production sẽ đọc pin thật)
    let gpio4_is_low = false;

    println!("");
    println!("[MAIN] Hệ thống sẵn sàng — bắt đầu vòng lặp chính");
    println!("[MAIN] Đang chờ lệnh từ Host qua SPI...");
    println!("------------------------------------------------------------");

    // ── Vòng lặp chính ─────────────────────────────────────────────────────
    // Vòng lặp polling — trong production nên dùng interrupt-driven approach
    // để giảm CPU usage và tăng responsiveness.
    loop {
        // Bước 1: Kiểm tra watchdog timeout (FR-15)
        // Mỗi vòng lặp ≈ 1ms (ước lượng, cần calibrate trên phần cứng thật)
        app.check_watchdog(1);

        // Bước 2: Kiểm tra hardware e-stop GPIO4 (FR-12)
        // TODO: Đọc trạng thái GPIO4 thật
        // let gpio4_is_low = !estop_pin.is_high();
        app.check_hardware_estop(gpio4_is_low);

        // Bước 3: Kiểm tra SPI slave có dữ liệu mới từ Master
        let mut rx_buf = [0u8; FRAME_SIZE];
        match transport.receive_raw(&mut rx_buf) {
            Ok(()) => {
                // Có dữ liệu mới — xử lý lệnh qua Application layer
                println!("------------------------------------------------------------");
                println!("[MAIN] Nhận được frame từ Master");

                // Bước 4: Xử lý lệnh
                if let Some(response) = app.process_raw_frame(&rx_buf) {
                    // Bước 5: Gửi phản hồi qua SPI slave
                    match transport.send_raw(&response) {
                        Ok(()) => {
                            println!("[MAIN] Phản hồi đã nạp vào TX buffer");
                        }
                        Err(e) => {
                            println!("[MAIN] Lỗi gửi phản hồi: {:?}", e);
                        }
                    }
                }

                println!("------------------------------------------------------------");
            }
            Err(_) => {
                // Không có dữ liệu mới — bình thường trong polling mode
                // Không in log để tránh spam serial
            }
        }

        // TODO: Delay nhỏ để giảm CPU usage (esp-hal delay hoặc NOP loop)
        // esp_hal::delay::Delay::new().delay_millis(1);
    }
}

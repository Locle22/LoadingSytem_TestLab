use pi_sender::MotorController;
use protocol::DeviceId;
use std::io::{self, Write};

fn main() {
    let esp32_ip = "172.20.10.2:8080";

    println!("=== Motor Controller CLI ===");
    println!("Target ESP32: {}", esp32_ip);
    println!();

    let ctrl = match MotorController::new(esp32_ip) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Không thể khởi tạo MotorController: {}", e);
            return;
        }
    };

    loop {
        println!("┌──────────────────────────────────┐");
        println!("│  1. Set Conveyor Speed            │");
        println!("│  2. Stop Conveyor                 │");
        println!("│  3. Set Turntable Speed            │");
        println!("│  4. Rotate Turntable To Angle      │");
        println!("│  5. Rotate Turntable By Angle      │");
        println!("│  6. Set Turntable Home             │");
        println!("│  7. Stop Turntable                 │");
        println!("│  8. Ping ESP32                     │");
        println!("│  9. EMERGENCY STOP                 │");
        println!("│  0. Exit                           │");
        println!("└──────────────────────────────────┘");
        print!("Chọn lệnh: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let choice = input.trim();

        let result = match choice {
            "1" => {
                let speed = read_i16("Nhập tốc độ (-1000..1000): ");
                ctrl.set_speed(DeviceId::Conveyor, speed)
            }
            "2" => ctrl.stop(DeviceId::Conveyor),
            "3" => {
                let speed = read_i16("Nhập tốc độ (-1000..1000): ");
                ctrl.set_speed(DeviceId::Turntable, speed)
            }
            "4" => {
                let angle = read_i16("Nhập góc đích (0..360): ");
                ctrl.rotate_to(angle)
            }
            "5" => {
                let delta = read_i16("Nhập góc xoay thêm (âm = ngược): ");
                ctrl.rotate_by(delta)
            }
            "6" => ctrl.set_home(),
            "7" => ctrl.stop(DeviceId::Turntable),
            "8" => ctrl.ping(),
            "9" => ctrl.emergency_stop(),
            "0" => {
                println!("Thoát.");
                break;
            }
            _ => {
                println!("❌ Lệnh không hợp lệ!");
                continue;
            }
        };

        match result {
            Ok(bytes) => println!("✅ Đã gửi {} bytes", bytes),
            Err(e) => eprintln!("❌ Lỗi gửi: {}", e),
        }
        println!();
    }
}

/// Helper: đọc một số i16 từ stdin.
fn read_i16(prompt: &str) -> i16 {
    loop {
        print!("{}", prompt);
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        match input.trim().parse::<i16>() {
            Ok(val) => return val,
            Err(_) => println!("❌ Vui lòng nhập một số nguyên hợp lệ."),
        }
    }
}

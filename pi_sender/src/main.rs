use pi_sender::WifiSender;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    // Replace this with the actual IP address of your ESP32
    // We will use port 8080 for this UDP communication.
    let esp32_ip = "172.20.10.2:8080";
    
    println!("Starting Pi UDP Sender...");
    println!("Target ESP32 address: {}", esp32_ip);

    let sender = match WifiSender::new(esp32_ip) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create UDP socket: {}", e);
            return;
        }
    };

    loop {
        print!("Nhập tin nhắn để gửi (hoặc gõ 'exit' để thoát): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let message = input.trim();

        if message == "exit" {
            println!("Đang thoát...");
            break;
        }

        if message.is_empty() {
            continue;
        }
        
        match sender.send_message(message) {
            Ok(bytes) => {
                println!("Đã gửi {} bytes: '{}'", bytes, message);
            }
            Err(e) => {
                eprintln!("Lỗi khi gửi gói tin: {}", e);
            }
        }
    }
}

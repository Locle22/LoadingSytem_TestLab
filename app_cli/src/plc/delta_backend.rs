use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, error};

use crate::hardware::HardwareBackend;
use crate::plc::modbus_client::RealModbusClient;
use crate::plc::rotating_disc::{Direction, RotatingDiscController};
use crate::plc::calibration::profile::CalibrationProfile;

pub enum PlcCommand {
    RotateToAbsolute(i32),
    ResetHome,
}

pub struct DeltaPlcBackend {
    sender: mpsc::Sender<PlcCommand>,
    current_angle: Arc<AtomicI32>,
}

impl DeltaPlcBackend {
    pub fn new(port: String, baud_rate: u32, slave_address: u8) -> Self {
        let (tx, mut rx) = mpsc::channel::<PlcCommand>(32);
        let current_angle = Arc::new(AtomicI32::new(0));
        let angle_clone = current_angle.clone();

        // Khởi tạo luồng ngầm (Background Task) để quản lý Modbus I/O
        tokio::spawn(async move {
            info!("Starting Delta PLC Backend task on {}", port);
            
            // Khởi tạo RealModbusClient
            let client_res = RealModbusClient::new_with_config(
                &port, baud_rate, 8, 1, "Even", slave_address
            ).await;

            match client_res {
                Ok(client) => {
                    info!("Successfully connected to PLC on {}", port);
                    // Dùng profile mặc định cho calibration
                    let profile = CalibrationProfile {
                        offset_pulses: 0,
                        learning_coefficient: 0.1,
                        history: vec![],
                    };
                    
                    let mut controller = RotatingDiscController::new(
                        client,
                        18000, // encoder_resolution
                        9000,  // default_frequency (0.5 vòng/s)
                        profile,
                    );

                    // Xử lý các lệnh gửi tới từ channel
                    while let Some(cmd) = rx.recv().await {
                        match cmd {
                            PlcCommand::RotateToAbsolute(target_angle) => {
                                let curr = controller.get_current_angle() as i32;
                                let mut diff = target_angle - curr;
                                
                                // Thuật toán đường đi ngắn nhất (Shortest path)
                                // Vì đĩa là 360 độ, ta có thể quay trái hoặc phải sao cho góc quay <= 180 độ
                                diff = (diff % 360 + 360) % 360;
                                if diff > 180 {
                                    diff -= 360;
                                }

                                if diff != 0 {
                                    let dir = if diff > 0 { Direction::Right } else { Direction::Left };
                                    let abs_diff = diff.abs() as f64;
                                    
                                    info!("Rotating PLC Disc: {} degrees to {:?}", abs_diff, dir);
                                    if let Err(e) = controller.rotate_degrees(abs_diff, dir).await {
                                        error!("PLC rotation failed: {}", e);
                                    } else {
                                        // Cập nhật lại tracking biến tĩnh sau khi quay thành công
                                        angle_clone.store(controller.get_current_angle() as i32, Ordering::SeqCst);
                                    }
                                }
                            }
                            PlcCommand::ResetHome => {
                                info!("Resetting PLC Disc to Home");
                                if let Err(e) = controller.return_to_origin().await {
                                    error!("PLC return to origin failed: {}", e);
                                } else {
                                    angle_clone.store(0, Ordering::SeqCst);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to connect to PLC on {}: {}", port, e);
                }
            }
        });

        Self {
            sender: tx,
            current_angle,
        }
    }
}

impl HardwareBackend for DeltaPlcBackend {
    fn name(&self) -> &str {
        "PLC Delta DVP14SS2 (Modbus RTU)"
    }

    fn send_mosquito_command(&self, _r: u8, _g: u8, _b: u8, class_id: u8) {
        // Có 12 lọ phân bố đều trên 360 độ -> mỗi lọ cách nhau 30 độ
        // Ta tính toán target angle:
        let target_angle = (class_id as i32) * 30;
        let _ = self.sender.try_send(PlcCommand::RotateToAbsolute(target_angle));
    }

    fn rotate_servo(&self, angle: i32) {
        let _ = self.sender.try_send(PlcCommand::RotateToAbsolute(angle));
    }

    fn reset_home(&self) {
        let _ = self.sender.try_send(PlcCommand::ResetHome);
    }

    fn get_current_angle(&self) -> i32 {
        self.current_angle.load(Ordering::SeqCst)
    }
}

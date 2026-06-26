use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use protocol::{Command, DeviceId, Packet};

// ============================================================
// WifiSender — Low-level UDP transport
// ============================================================

pub struct WifiSender {
    socket: UdpSocket,
    target_addr: String,
}

impl WifiSender {
    /// Creates a new WifiSender bound to an arbitrary local port.
    /// `target_addr` should be in the format "IP:PORT", e.g., "192.168.1.100:8080".
    pub fn new(target_addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;

        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;

        Ok(Self {
            socket,
            target_addr: target_addr.to_string(),
        })
    }

    /// Sends raw bytes to the target address via UDP.
    pub fn send_raw(&self, data: &[u8]) -> std::io::Result<usize> {
        self.socket.send_to(data, &self.target_addr)
    }

    /// Sends a plain text message to the target address via UDP.
    pub fn send_message(&self, message: &str) -> std::io::Result<usize> {
        self.send_raw(message.as_bytes())
    }
}

// ============================================================
// MotorController — Type-safe command API
// ============================================================

/// High-level API for sending motor control commands to ESP32.
///
/// Wraps `WifiSender` and provides type-safe methods for each command.
/// Each method creates a `Command`, wraps it in a `Packet` with an
/// auto-incrementing sequence ID, serializes it, and sends it via UDP.
pub struct MotorController {
    sender: WifiSender,
    seq_counter: AtomicU16,
}

impl MotorController {
    /// Create a new MotorController targeting the given ESP32 address.
    ///
    /// # Example
    /// ```no_run
    /// let ctrl = pi_sender::MotorController::new("172.20.10.2:8080").unwrap();
    /// ```
    pub fn new(target_addr: &str) -> std::io::Result<Self> {
        let sender = WifiSender::new(target_addr)?;
        Ok(Self {
            sender,
            seq_counter: AtomicU16::new(1),
        })
    }

    /// Get the next sequence ID (auto-incrementing).
    fn next_seq(&self) -> u16 {
        self.seq_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a command to ESP32. Returns the number of bytes sent.
    fn send_command(&self, command: Command) -> std::io::Result<usize> {
        let packet = Packet::new(self.next_seq(), command);
        let (buf, len) = packet.serialize();
        self.sender.send_raw(&buf[..len])
    }

    // === Motor Speed Control ===

    /// Set the speed of a specific motor.
    ///
    /// `speed` range: -1000 to 1000. Negative = reverse direction.
    /// Team 2 will map this to actual PWM values on the ESP32.
    pub fn set_speed(&self, device: DeviceId, speed: i16) -> std::io::Result<usize> {
        self.send_command(Command::SetSpeed { device, speed })
    }

    /// Stop a specific motor.
    pub fn stop(&self, device: DeviceId) -> std::io::Result<usize> {
        self.send_command(Command::Stop { device })
    }

    /// Request the current status of a specific motor.
    pub fn get_status(&self, device: DeviceId) -> std::io::Result<usize> {
        self.send_command(Command::GetStatus { device })
    }

    // === Turntable-specific Commands ===

    /// Rotate the turntable to an absolute angle (0–360 degrees).
    pub fn rotate_to(&self, angle_deg: i16) -> std::io::Result<usize> {
        self.send_command(Command::RotateTo { angle_deg })
    }

    /// Rotate the turntable by a relative angle (can be negative).
    pub fn rotate_by(&self, delta_deg: i16) -> std::io::Result<usize> {
        self.send_command(Command::RotateBy { delta_deg })
    }

    /// Set the current turntable position as the home (0°) position.
    pub fn set_home(&self) -> std::io::Result<usize> {
        self.send_command(Command::SetHome)
    }

    // === System Commands ===

    /// Ping the ESP32 to check connectivity.
    pub fn ping(&self) -> std::io::Result<usize> {
        self.send_command(Command::Ping)
    }

    /// Emergency stop — immediately stops ALL motors.
    pub fn emergency_stop(&self) -> std::io::Result<usize> {
        self.send_command(Command::EmergencyStop)
    }
}

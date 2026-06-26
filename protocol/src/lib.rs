//! # Protocol — Shared Command Protocol
//!
//! Thư viện giao thức dùng chung giữa Raspberry Pi (sender) và ESP32 (receiver).
//!
//! ## Packet Format
//! ```text
//! ┌─────────┬─────────┬──────────┬───────────┬──────────┐
//! │ Header  │ Seq ID  │ Command  │ Payload   │ Checksum │
//! │ 2 bytes │ 2 bytes │ 1 byte   │ 0-4 bytes │ 1 byte   │
//! │ 0xAA55  │ u16 LE  │ u8       │ varies    │ XOR all  │
//! └─────────┴─────────┴──────────┴───────────┴──────────┘
//! ```

// ============================================================
// Constants
// ============================================================

/// Magic header bytes to identify a valid packet.
pub const HEADER: [u8; 2] = [0xAA, 0x55];

/// Maximum packet size in bytes (header + seq + cmd + max_payload + checksum).
pub const MAX_PACKET_SIZE: usize = 10;

// Command code constants
const CMD_SET_SPEED: u8 = 0x01;
const CMD_STOP: u8 = 0x02;
const CMD_GET_STATUS: u8 = 0x03;
const CMD_ROTATE_TO: u8 = 0x10;
const CMD_ROTATE_BY: u8 = 0x11;
const CMD_SET_HOME: u8 = 0x12;
const CMD_PING: u8 = 0xF0;
const CMD_EMERGENCY_STOP: u8 = 0xFF;

// Device ID constants
const DEV_CONVEYOR: u8 = 0x01;
const DEV_TURNTABLE: u8 = 0x02;
const DEV_ALL: u8 = 0xFF;

// ============================================================
// DeviceId
// ============================================================

/// Identifies which motor/device a command targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceId {
    /// Servo băng chuyền
    Conveyor,
    /// Servo đĩa xoay
    Turntable,
    /// Tất cả thiết bị (dùng cho EmergencyStop)
    All,
}

impl DeviceId {
    /// Returns a human-readable name for this device.
    pub fn name(&self) -> &'static str {
        match self {
            DeviceId::Conveyor => "Conveyor",
            DeviceId::Turntable => "Turntable",
            DeviceId::All => "All",
        }
    }

    fn to_byte(self) -> u8 {
        match self {
            DeviceId::Conveyor => DEV_CONVEYOR,
            DeviceId::Turntable => DEV_TURNTABLE,
            DeviceId::All => DEV_ALL,
        }
    }

    fn from_byte(b: u8) -> Result<Self, ProtocolError> {
        match b {
            DEV_CONVEYOR => Ok(DeviceId::Conveyor),
            DEV_TURNTABLE => Ok(DeviceId::Turntable),
            DEV_ALL => Ok(DeviceId::All),
            _ => Err(ProtocolError::InvalidDeviceId(b)),
        }
    }
}

// ============================================================
// Command
// ============================================================

/// All supported commands in the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    // --- Lệnh chung cho motor ---
    /// Đặt tốc độ motor. speed: -1000..1000
    SetSpeed { device: DeviceId, speed: i16 },
    /// Dừng 1 motor cụ thể
    Stop { device: DeviceId },
    /// Yêu cầu trạng thái
    GetStatus { device: DeviceId },

    // --- Lệnh riêng cho đĩa xoay ---
    /// Xoay đến góc tuyệt đối (0..360 degree)
    RotateTo { angle_deg: i16 },
    /// Xoay thêm N độ (relative, có thể âm)
    RotateBy { delta_deg: i16 },
    /// Đặt vị trí hiện tại làm gốc (0°)
    SetHome,

    // --- Lệnh hệ thống ---
    /// Kiểm tra kết nối
    Ping,
    /// Dừng TẤT CẢ ngay lập tức
    EmergencyStop,
}

impl Command {
    /// Returns a human-readable description of this command (for serial debug).
    pub fn description(&self) -> &'static str {
        match self {
            Command::SetSpeed { .. } => "SetSpeed",
            Command::Stop { .. } => "Stop",
            Command::GetStatus { .. } => "GetStatus",
            Command::RotateTo { .. } => "RotateTo",
            Command::RotateBy { .. } => "RotateBy",
            Command::SetHome => "SetHome",
            Command::Ping => "Ping",
            Command::EmergencyStop => "EmergencyStop",
        }
    }

    /// Serialize command into the payload portion (command_code + payload_bytes).
    /// Returns (command_code, payload_bytes, payload_len).
    fn serialize_payload(&self) -> (u8, [u8; 4], usize) {
        let mut payload = [0u8; 4];
        match self {
            Command::SetSpeed { device, speed } => {
                payload[0] = device.to_byte();
                let speed_bytes = speed.to_le_bytes();
                payload[1] = speed_bytes[0];
                payload[2] = speed_bytes[1];
                (CMD_SET_SPEED, payload, 3)
            }
            Command::Stop { device } => {
                payload[0] = device.to_byte();
                (CMD_STOP, payload, 1)
            }
            Command::GetStatus { device } => {
                payload[0] = device.to_byte();
                (CMD_GET_STATUS, payload, 1)
            }
            Command::RotateTo { angle_deg } => {
                let bytes = angle_deg.to_le_bytes();
                payload[0] = bytes[0];
                payload[1] = bytes[1];
                (CMD_ROTATE_TO, payload, 2)
            }
            Command::RotateBy { delta_deg } => {
                let bytes = delta_deg.to_le_bytes();
                payload[0] = bytes[0];
                payload[1] = bytes[1];
                (CMD_ROTATE_BY, payload, 2)
            }
            Command::SetHome => (CMD_SET_HOME, payload, 0),
            Command::Ping => (CMD_PING, payload, 0),
            Command::EmergencyStop => (CMD_EMERGENCY_STOP, payload, 0),
        }
    }

    /// Deserialize command from command code + payload bytes.
    fn from_payload(cmd_code: u8, payload: &[u8]) -> Result<Self, ProtocolError> {
        match cmd_code {
            CMD_SET_SPEED => {
                if payload.len() < 3 {
                    return Err(ProtocolError::PayloadTooShort);
                }
                let device = DeviceId::from_byte(payload[0])?;
                let speed = i16::from_le_bytes([payload[1], payload[2]]);
                Ok(Command::SetSpeed { device, speed })
            }
            CMD_STOP => {
                if payload.is_empty() {
                    return Err(ProtocolError::PayloadTooShort);
                }
                let device = DeviceId::from_byte(payload[0])?;
                Ok(Command::Stop { device })
            }
            CMD_GET_STATUS => {
                if payload.is_empty() {
                    return Err(ProtocolError::PayloadTooShort);
                }
                let device = DeviceId::from_byte(payload[0])?;
                Ok(Command::GetStatus { device })
            }
            CMD_ROTATE_TO => {
                if payload.len() < 2 {
                    return Err(ProtocolError::PayloadTooShort);
                }
                let angle_deg = i16::from_le_bytes([payload[0], payload[1]]);
                Ok(Command::RotateTo { angle_deg })
            }
            CMD_ROTATE_BY => {
                if payload.len() < 2 {
                    return Err(ProtocolError::PayloadTooShort);
                }
                let delta_deg = i16::from_le_bytes([payload[0], payload[1]]);
                Ok(Command::RotateBy { delta_deg })
            }
            CMD_SET_HOME => Ok(Command::SetHome),
            CMD_PING => Ok(Command::Ping),
            CMD_EMERGENCY_STOP => Ok(Command::EmergencyStop),
            _ => Err(ProtocolError::UnknownCommand(cmd_code)),
        }
    }
}

// ============================================================
// ProtocolError
// ============================================================

/// Errors that can occur during packet parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    /// Packet is too short to contain even the header.
    PacketTooShort,
    /// Header bytes don't match 0xAA 0x55.
    InvalidHeader,
    /// Checksum verification failed.
    ChecksumMismatch,
    /// Unknown command code.
    UnknownCommand(u8),
    /// Unknown device ID.
    InvalidDeviceId(u8),
    /// Payload is too short for the command.
    PayloadTooShort,
}

impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProtocolError::PacketTooShort => write!(f, "Packet too short"),
            ProtocolError::InvalidHeader => write!(f, "Invalid header (expected 0xAA55)"),
            ProtocolError::ChecksumMismatch => write!(f, "Checksum mismatch"),
            ProtocolError::UnknownCommand(c) => write!(f, "Unknown command code: 0x{:02X}", c),
            ProtocolError::InvalidDeviceId(d) => write!(f, "Invalid device ID: 0x{:02X}", d),
            ProtocolError::PayloadTooShort => write!(f, "Payload too short for command"),
        }
    }
}

// ============================================================
// Packet
// ============================================================

/// A protocol packet containing a command with header, sequence ID, and checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet {
    /// Sequence ID for tracking (monotonically increasing).
    pub seq_id: u16,
    /// The command contained in this packet.
    pub command: Command,
}

impl Packet {
    /// Create a new packet with the given sequence ID and command.
    pub fn new(seq_id: u16, command: Command) -> Self {
        Self { seq_id, command }
    }

    /// Serialize this packet into a byte buffer.
    ///
    /// Returns a fixed-size buffer and the number of valid bytes.
    /// Format: [0xAA, 0x55, seq_lo, seq_hi, cmd_code, ...payload, checksum]
    pub fn serialize(&self) -> ([u8; MAX_PACKET_SIZE], usize) {
        let mut buf = [0u8; MAX_PACKET_SIZE];
        let seq_bytes = self.seq_id.to_le_bytes();
        let (cmd_code, payload, payload_len) = self.command.serialize_payload();

        // Header
        buf[0] = HEADER[0];
        buf[1] = HEADER[1];

        // Seq ID (little-endian)
        buf[2] = seq_bytes[0];
        buf[3] = seq_bytes[1];

        // Command code
        buf[4] = cmd_code;

        // Payload
        for i in 0..payload_len {
            buf[5 + i] = payload[i];
        }

        // Checksum: XOR of all bytes from seq_id to end of payload
        let checksum_range_end = 5 + payload_len;
        let mut checksum: u8 = 0;
        for i in 2..checksum_range_end {
            checksum ^= buf[i];
        }
        buf[checksum_range_end] = checksum;

        let total_len = checksum_range_end + 1;
        (buf, total_len)
    }

    /// Deserialize a packet from raw bytes.
    pub fn deserialize(data: &[u8]) -> Result<Self, ProtocolError> {
        // Minimum packet: header(2) + seq(2) + cmd(1) + checksum(1) = 6 bytes
        if data.len() < 6 {
            return Err(ProtocolError::PacketTooShort);
        }

        // Verify header
        if data[0] != HEADER[0] || data[1] != HEADER[1] {
            return Err(ProtocolError::InvalidHeader);
        }

        // Extract seq_id
        let seq_id = u16::from_le_bytes([data[2], data[3]]);

        // Command code
        let cmd_code = data[4];

        // The payload is everything between cmd_code and the last byte (checksum)
        let payload = &data[5..data.len() - 1];

        // Verify checksum: XOR of bytes from index 2 to (len - 2) inclusive
        let checksum_byte = data[data.len() - 1];
        let mut computed_checksum: u8 = 0;
        for i in 2..(data.len() - 1) {
            computed_checksum ^= data[i];
        }
        if computed_checksum != checksum_byte {
            return Err(ProtocolError::ChecksumMismatch);
        }

        // Parse command
        let command = Command::from_payload(cmd_code, payload)?;

        Ok(Packet { seq_id, command })
    }
}

// ============================================================
// Display implementations for nice serial output
// ============================================================

impl core::fmt::Display for Command {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Command::SetSpeed { device, speed } => {
                write!(f, "SetSpeed {{ device: {}, speed: {} }}", device.name(), speed)
            }
            Command::Stop { device } => {
                write!(f, "Stop {{ device: {} }}", device.name())
            }
            Command::GetStatus { device } => {
                write!(f, "GetStatus {{ device: {} }}", device.name())
            }
            Command::RotateTo { angle_deg } => {
                write!(f, "RotateTo {{ angle_deg: {}° }}", angle_deg)
            }
            Command::RotateBy { delta_deg } => {
                write!(f, "RotateBy {{ delta_deg: {}° }}", delta_deg)
            }
            Command::SetHome => write!(f, "SetHome"),
            Command::Ping => write!(f, "Ping"),
            Command::EmergencyStop => write!(f, "EmergencyStop"),
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_set_speed() {
        let original = Packet::new(1, Command::SetSpeed {
            device: DeviceId::Conveyor,
            speed: 500,
        });
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_serialize_deserialize_negative_speed() {
        let original = Packet::new(42, Command::SetSpeed {
            device: DeviceId::Turntable,
            speed: -300,
        });
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_serialize_deserialize_stop() {
        let original = Packet::new(2, Command::Stop {
            device: DeviceId::Conveyor,
        });
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_serialize_deserialize_rotate_to() {
        let original = Packet::new(3, Command::RotateTo { angle_deg: 180 });
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_serialize_deserialize_rotate_by() {
        let original = Packet::new(4, Command::RotateBy { delta_deg: -45 });
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_serialize_deserialize_set_home() {
        let original = Packet::new(5, Command::SetHome);
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_serialize_deserialize_ping() {
        let original = Packet::new(100, Command::Ping);
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_serialize_deserialize_emergency_stop() {
        let original = Packet::new(999, Command::EmergencyStop);
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_serialize_deserialize_get_status() {
        let original = Packet::new(10, Command::GetStatus {
            device: DeviceId::Turntable,
        });
        let (buf, len) = original.serialize();
        let parsed = Packet::deserialize(&buf[..len]).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_invalid_header() {
        let data = [0x00, 0x00, 0x01, 0x00, CMD_PING, 0x00];
        let result = Packet::deserialize(&data);
        assert_eq!(result, Err(ProtocolError::InvalidHeader));
    }

    #[test]
    fn test_packet_too_short() {
        let data = [0xAA, 0x55, 0x01];
        let result = Packet::deserialize(&data);
        assert_eq!(result, Err(ProtocolError::PacketTooShort));
    }

    #[test]
    fn test_checksum_mismatch() {
        let packet = Packet::new(1, Command::Ping);
        let (mut buf, len) = packet.serialize();
        // Corrupt the checksum
        buf[len - 1] ^= 0xFF;
        let result = Packet::deserialize(&buf[..len]);
        assert_eq!(result, Err(ProtocolError::ChecksumMismatch));
    }

    #[test]
    fn test_unknown_command() {
        // Manually craft a packet with an unknown command code
        let mut data = [0xAA, 0x55, 0x01, 0x00, 0xBB, 0x00];
        // Compute correct checksum for bytes 2..5
        let mut checksum: u8 = 0;
        for i in 2..5 {
            checksum ^= data[i];
        }
        data[5] = checksum;
        let result = Packet::deserialize(&data);
        assert_eq!(result, Err(ProtocolError::UnknownCommand(0xBB)));
    }

    #[test]
    fn test_command_display() {
        let cmd = Command::SetSpeed {
            device: DeviceId::Conveyor,
            speed: 500,
        };
        let s = format!("{}", cmd);
        assert_eq!(s, "SetSpeed { device: Conveyor, speed: 500 }");
    }

    #[test]
    fn test_all_device_ids_roundtrip() {
        for device in [DeviceId::Conveyor, DeviceId::Turntable, DeviceId::All] {
            let b = device.to_byte();
            let parsed = DeviceId::from_byte(b).unwrap();
            assert_eq!(device, parsed);
        }
    }
}

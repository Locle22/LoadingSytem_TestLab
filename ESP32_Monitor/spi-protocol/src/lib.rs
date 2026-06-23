// =============================================================================
// spi-protocol/src/lib.rs
// =============================================================================
// Module root cho thư viện giao thức SPI chia sẻ.
//
// Thư viện này được thiết kế `no_std` by default để tương thích với
// cả Host (Raspberry Pi 4, std) và Node (ESP32-S3, no_std).
//
// Bật feature "std" cho Host, "defmt-logging" cho Node.
// =============================================================================

#![no_std]

// Khi feature "std" được bật (trên Host), cho phép sử dụng std
#[cfg(feature = "std")]
extern crate std;

/// Hằng số cấu hình hệ thống (CQ-03, CQ-04).
/// Tập trung tất cả giá trị cấu hình, không có magic number rải rác.
pub mod config;

/// Mã lỗi và kiểu lỗi (CQ-06, FR-16).
/// Mọi lỗi đều là enum có tên ý nghĩa, phân loại theo nhóm.
pub mod error;

/// Tập lệnh điều khiển, trạng thái motor, payload structs (FR-10, CQ-06).
pub mod commands;

/// CRC-16 checksum cho kiểm tra toàn vẹn dữ liệu (FR-08).
pub mod checksum;

/// Khung truyền cố định 32 bytes (FR-05 → FR-09).
/// Đóng gói, giải mã, và validate khung truyền.
pub mod frame;

/// Giao diện trừu tượng cho lớp Truyền dẫn (FR-02, FR-03).
/// Cho phép thay thế SPI bằng UART/I2C/Mock.
pub mod transport;

// ─── Re-exports ──────────────────────────────────────────────────────────────
// Xuất các kiểu dữ liệu chính để người dùng thư viện không cần
// import từ các sub-module sâu bên trong.

pub use commands::{
    ActuatorCapability, CommandCode, MotorState, NackPayload, PositionCommand, StatusPayload,
    VelocityCommand,
};
pub use config::FRAME_SIZE;
pub use error::{ErrorCode, ProtocolError};
pub use frame::Frame;
pub use transport::{Transport, TransportError};

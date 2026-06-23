// =============================================================================
// RaspberryPi/src/spi_master.rs
// =============================================================================
// Lớp Truyền dẫn SPI Master trên Raspberry Pi 4 (FR-02, FR-03).
//
// Module này chỉ chịu trách nhiệm gửi/nhận byte thô qua SPI bus.
// KHÔNG chứa bất kỳ logic nghiệp vụ nào:
//   - Không phân tích lệnh
//   - Không kiểm tra tham số
//   - Không diễn giải ý nghĩa dữ liệu
//
// Sử dụng thư viện `rppal` để giao tiếp SPI trên Raspberry Pi 4.
// =============================================================================

use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use spi_protocol::{Transport, TransportError, FRAME_SIZE};

// ── Hằng số cấu hình SPI ────────────────────────────────────────────────────

/// Tốc độ clock SPI mặc định (Hz).
/// 1 MHz phù hợp cho giao tiếp ổn định RPi4 ↔ ESP32-S3 qua dây jumper.
const SPI_CLOCK_SPEED_HZ: u32 = 1_000_000;

/// Bus SPI mặc định trên RPi4: SPI0 (GPIO 10=MOSI, 9=MISO, 11=SCLK).
const SPI_BUS: Bus = Bus::Spi0;

/// Chip Select mặc định: CE0 (GPIO 8).
const SPI_SLAVE_SELECT: SlaveSelect = SlaveSelect::Ss0;

/// SPI Mode 0 (CPOL=0, CPHA=0) — phổ biến nhất cho ESP32.
const SPI_MODE: Mode = Mode::Mode0;

// ── SPI Master Implementation ────────────────────────────────────────────────

/// Cấu trúc SPI Master sử dụng `rppal::spi::Spi`.
///
/// Đây là implementation cụ thể của `Transport` trait cho phần cứng
/// Raspberry Pi 4. Chỉ gửi/nhận byte thô, không xử lý logic giao thức.
pub struct SpiMaster {
    /// Handle SPI từ rppal.
    spi: Spi,
}

impl SpiMaster {
    /// Khởi tạo SPI Master với cấu hình mặc định.
    ///
    /// Cấu hình: SPI0, CE0, Mode0, 1 MHz.
    ///
    /// # Errors
    /// Trả về `TransportError::BusError` nếu không thể mở bus SPI
    /// (ví dụ: SPI chưa được bật trong raspi-config).
    pub fn new() -> Result<Self, TransportError> {
        Self::with_config(SPI_BUS, SPI_SLAVE_SELECT, SPI_CLOCK_SPEED_HZ, SPI_MODE)
    }

    /// Khởi tạo SPI Master với cấu hình tùy chỉnh.
    ///
    /// # Arguments
    /// * `bus` - Bus SPI (Spi0 hoặc Spi1).
    /// * `slave_select` - Chip select (Ss0, Ss1, Ss2).
    /// * `clock_speed_hz` - Tốc độ clock SPI (Hz).
    /// * `mode` - SPI mode (Mode0–Mode3).
    ///
    /// # Errors
    /// Trả về `TransportError::BusError` nếu không thể mở bus SPI.
    pub fn with_config(
        bus: Bus,
        slave_select: SlaveSelect,
        clock_speed_hz: u32,
        mode: Mode,
    ) -> Result<Self, TransportError> {
        let spi = Spi::new(bus, slave_select, clock_speed_hz, mode)
            .map_err(|e| {
                log::error!("Không thể khởi tạo SPI bus: {}", e);
                TransportError::BusError
            })?;

        log::info!(
            "SPI Master khởi tạo thành công: bus={:?}, ss={:?}, clock={}Hz, mode={:?}",
            bus, slave_select, clock_speed_hz, mode
        );

        Ok(Self { spi })
    }
}

// ── Transport Trait Implementation ───────────────────────────────────────────
// Chỉ gửi/nhận byte thô — KHÔNG diễn giải nội dung (FR-02).

impl Transport for SpiMaster {
    /// Gửi một khung byte thô 32 bytes qua SPI.
    ///
    /// Thực hiện giao dịch SPI write-only. Dữ liệu nhận về (nếu có)
    /// bị bỏ qua vì đây là hàm gửi một chiều.
    fn send_raw(&mut self, data: &[u8; FRAME_SIZE]) -> Result<(), TransportError> {
        self.spi.write(data).map_err(|e| {
            log::error!("SPI send_raw thất bại: {}", e);
            TransportError::BusError
        })?;
        log::trace!("SPI send_raw: {} bytes gửi thành công", FRAME_SIZE);
        Ok(())
    }

    /// Nhận một khung byte thô 32 bytes từ SPI.
    ///
    /// Thực hiện giao dịch SPI read-only. Host gửi byte 0x00 (dummy)
    /// để tạo clock cho slave truyền dữ liệu.
    fn receive_raw(&mut self, buffer: &mut [u8; FRAME_SIZE]) -> Result<(), TransportError> {
        self.spi.read(buffer).map_err(|e| {
            log::error!("SPI receive_raw thất bại: {}", e);
            TransportError::BusError
        })?;
        log::trace!("SPI receive_raw: {} bytes nhận thành công", FRAME_SIZE);
        Ok(())
    }

    /// Giao dịch full-duplex: gửi `tx_buf` đồng thời nhận vào `rx_buf`.
    ///
    /// Đây là phương thức chính cho SPI full-duplex:
    /// - Host truyền khung lệnh qua MOSI
    /// - Đồng thời nhận phản hồi từ Node qua MISO
    ///
    /// Lưu ý: rppal `transfer` yêu cầu tx và rx buffer cùng kích thước,
    /// điều kiện luôn thỏa vì cả hai đều là `[u8; FRAME_SIZE]`.
    fn transfer(
        &mut self,
        tx_buf: &[u8; FRAME_SIZE],
        rx_buf: &mut [u8; FRAME_SIZE],
    ) -> Result<(), TransportError> {
        // Chuẩn bị buffer cho rppal::spi::transfer (in-place)
        rx_buf.copy_from_slice(tx_buf);
        self.spi.transfer(rx_buf).map_err(|e| {
            log::error!("SPI transfer thất bại: {}", e);
            TransportError::BusError
        })?;
        log::trace!("SPI transfer: {} bytes gửi/nhận thành công", FRAME_SIZE);
        Ok(())
    }
}

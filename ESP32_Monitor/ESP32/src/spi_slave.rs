// =============================================================================
// ESP32/src/spi_slave.rs
// =============================================================================
// Lớp Truyền dẫn SPI Slave cho ESP32-S3 (FR-02, FR-03).
//
// Module này implement trait `Transport` từ thư viện spi-protocol,
// cung cấp khả năng gửi/nhận khung byte thô qua SPI slave interface.
//
// NGUYÊN TẮC (FR-02): Module này KHÔNG chứa bất kỳ logic nghiệp vụ nào.
// Không phân tích lệnh, không kiểm tra tham số, không diễn giải dữ liệu.
// Chỉ chịu trách nhiệm gửi/nhận luồng byte thô qua bus SPI.
//
// LƯU Ý: SPI slave API trong esp-hal v1.0.0-beta.0 còn đang phát triển
// (feature "unstable"). Implementation hiện tại sử dụng placeholder
// với TODO cho các lệnh esp-hal thực tế. Khi tích hợp phần cứng thật,
// cần thay thế các TODO bằng API esp-hal phù hợp.
// =============================================================================

use esp_println::println;
use spi_protocol::config::FRAME_SIZE;
use spi_protocol::transport::{Transport, TransportError};

// ─── SPI Slave Pin Configuration ─────────────────────────────────────────────

/// Chân SCLK (Serial Clock) — Master cung cấp xung clock.
#[allow(dead_code)]
pub const SPI_SCLK_PIN: u8 = 12;

/// Chân MOSI (Master Out Slave In) — Dữ liệu từ Master → Slave.
#[allow(dead_code)]
pub const SPI_MOSI_PIN: u8 = 13;

/// Chân MISO (Master In Slave Out) — Dữ liệu từ Slave → Master.
#[allow(dead_code)]
pub const SPI_MISO_PIN: u8 = 11;

/// Chân CS (Chip Select) — Master chọn Slave, active LOW.
#[allow(dead_code)]
pub const SPI_CS_PIN: u8 = 10;

// ─── SpiSlaveTransport ───────────────────────────────────────────────────────

/// Transport SPI Slave cho ESP32-S3.
///
/// Đóng gói peripheral SPI2 ở chế độ slave. Khi Master bắt đầu giao dịch
/// (kéo CS xuống LOW), slave tự động gửi/nhận dữ liệu theo xung SCLK.
///
/// # Khởi tạo
/// Peripheral SPI được cấu hình trong `main.rs` và truyền vào đây.
/// Module này chỉ giữ reference đến peripheral đã cấu hình.
///
/// # Cấu hình SPI
/// - Mode: Slave
/// - Clock Polarity: CPOL=0 (idle LOW)  
/// - Clock Phase: CPHA=0 (sample trên cạnh lên)
/// - Bit Order: MSB first
/// - Frame Size: 32 bytes cố định
pub struct SpiSlaveTransport {
    /// Buffer TX chuẩn bị sẵn để gửi khi Master bắt đầu giao dịch.
    tx_buffer: [u8; FRAME_SIZE],
    /// Buffer RX lưu dữ liệu nhận được từ Master.
    rx_buffer: [u8; FRAME_SIZE],
    /// Cờ đánh dấu đã có dữ liệu mới trong rx_buffer.
    rx_ready: bool,
    /// Cờ đánh dấu tx_buffer đã được nạp dữ liệu để gửi.
    tx_loaded: bool,
    // TODO: Thêm trường cho esp-hal SPI slave peripheral khi tích hợp phần cứng.
    // Ví dụ: spi: esp_hal::spi::slave::Spi<'static, esp_hal::Blocking>,
}

impl SpiSlaveTransport {
    /// Tạo SpiSlaveTransport mới.
    ///
    /// # Lưu ý
    /// Trong production, hàm này sẽ nhận peripheral SPI đã cấu hình từ main.rs.
    /// Hiện tại chỉ khởi tạo buffer nội bộ.
    ///
    /// # TODO
    /// ```ignore
    /// pub fn new(spi: esp_hal::spi::slave::Spi<'static, esp_hal::Blocking>) -> Self {
    ///     Self { spi, tx_buffer: [0u8; FRAME_SIZE], ... }
    /// }
    /// ```
    pub fn new() -> Self {
        println!(
            "[SPI] Khởi tạo SPI Slave Transport — SCLK=GPIO{}, MOSI=GPIO{}, MISO=GPIO{}, CS=GPIO{}",
            SPI_SCLK_PIN, SPI_MOSI_PIN, SPI_MISO_PIN, SPI_CS_PIN
        );
        println!("[SPI] Frame size: {} bytes, Mode: SPI_MODE_0 (CPOL=0, CPHA=0)", FRAME_SIZE);

        Self {
            tx_buffer: [0u8; FRAME_SIZE],
            rx_buffer: [0u8; FRAME_SIZE],
            rx_ready: false,
            tx_loaded: false,
        }
    }

    /// Nạp dữ liệu vào TX buffer để sẵn sàng gửi khi Master yêu cầu.
    ///
    /// Trong SPI slave, slave không chủ động gửi — phải đợi Master kéo CS
    /// và cung cấp clock. Hàm này chuẩn bị dữ liệu trước.
    pub fn prepare_response(&mut self, data: &[u8; FRAME_SIZE]) {
        self.tx_buffer.copy_from_slice(data);
        self.tx_loaded = true;
    }

    /// Kiểm tra xem có dữ liệu mới từ Master không.
    ///
    /// Trong production, hàm này sẽ kiểm tra cờ ngắt hoặc trạng thái DMA
    /// của peripheral SPI. Hiện tại trả về trạng thái cờ nội bộ.
    pub fn has_pending_data(&self) -> bool {
        self.rx_ready
    }

    /// Lấy dữ liệu đã nhận và xoá cờ ready.
    pub fn take_received_data(&mut self) -> Option<[u8; FRAME_SIZE]> {
        if self.rx_ready {
            self.rx_ready = false;
            Some(self.rx_buffer)
        } else {
            None
        }
    }
}

impl Transport for SpiSlaveTransport {
    /// Gửi khung byte thô qua SPI slave (FR-02).
    ///
    /// Trong SPI slave, "gửi" thực chất là nạp dữ liệu vào TX buffer
    /// và đợi Master thực hiện giao dịch tiếp theo.
    ///
    /// # TODO (esp-hal integration)
    /// ```ignore
    /// fn send_raw(&mut self, data: &[u8; FRAME_SIZE]) -> Result<(), TransportError> {
    ///     let mut rx_dummy = [0u8; FRAME_SIZE];
    ///     self.spi.transfer(&mut rx_dummy, data)
    ///         .map_err(|_| TransportError::BusError)?;
    ///     Ok(())
    /// }
    /// ```
    fn send_raw(&mut self, data: &[u8; FRAME_SIZE]) -> Result<(), TransportError> {
        // Nạp dữ liệu vào TX buffer — sẽ được gửi khi Master bắt đầu giao dịch
        self.prepare_response(data);
        println!("[SPI] TX buffer loaded ({} bytes)", FRAME_SIZE);
        Ok(())

        // TODO: Khi có phần cứng thật, thay bằng esp-hal SPI slave transfer.
        // Lưu ý SPI slave là passive — cần đợi Master clock để thực sự gửi.
    }

    /// Nhận khung byte thô từ SPI slave (FR-02).
    ///
    /// Đọc dữ liệu từ RX buffer sau khi Master hoàn tất giao dịch.
    /// Trong slave mode, hàm này nên block cho đến khi Master thực hiện
    /// giao dịch, hoặc trả về lỗi nếu timeout.
    ///
    /// # TODO (esp-hal integration)
    /// ```ignore
    /// fn receive_raw(&mut self, buffer: &mut [u8; FRAME_SIZE]) -> Result<(), TransportError> {
    ///     let tx_zeros = [0u8; FRAME_SIZE];
    ///     self.spi.transfer(buffer, &tx_zeros)
    ///         .map_err(|_| TransportError::BusError)?;
    ///     Ok(())
    /// }
    /// ```
    fn receive_raw(&mut self, buffer: &mut [u8; FRAME_SIZE]) -> Result<(), TransportError> {
        if self.rx_ready {
            buffer.copy_from_slice(&self.rx_buffer);
            self.rx_ready = false;
            println!("[SPI] RX data consumed ({} bytes)", FRAME_SIZE);
            Ok(())
        } else {
            // Chưa có dữ liệu mới — trong production sẽ block hoặc timeout
            Err(TransportError::Timeout)
        }

        // TODO: Khi có phần cứng thật, thay bằng blocking SPI slave receive.
    }

    /// Giao dịch full-duplex: gửi tx_buf đồng thời nhận vào rx_buf (FR-02).
    ///
    /// Đây là mode tự nhiên nhất cho SPI (synchronous full-duplex).
    /// Slave nạp TX data, đợi Master clock, đồng thời nhận RX data.
    ///
    /// # TODO (esp-hal integration)
    /// ```ignore
    /// fn transfer(
    ///     &mut self,
    ///     tx_buf: &[u8; FRAME_SIZE],
    ///     rx_buf: &mut [u8; FRAME_SIZE],
    /// ) -> Result<(), TransportError> {
    ///     rx_buf.copy_from_slice(&[0u8; FRAME_SIZE]);
    ///     self.spi.transfer(rx_buf, tx_buf)
    ///         .map_err(|_| TransportError::BusError)?;
    ///     Ok(())
    /// }
    /// ```
    fn transfer(
        &mut self,
        tx_buf: &[u8; FRAME_SIZE],
        rx_buf: &mut [u8; FRAME_SIZE],
    ) -> Result<(), TransportError> {
        // Nạp TX data
        self.tx_buffer.copy_from_slice(tx_buf);
        self.tx_loaded = true;

        // Giả lập: trong production, SPI slave sẽ đợi Master clock
        // và thực hiện transfer full-duplex thực sự
        if self.rx_ready {
            rx_buf.copy_from_slice(&self.rx_buffer);
            self.rx_ready = false;
            println!("[SPI] Full-duplex transfer completed ({} bytes)", FRAME_SIZE);
            Ok(())
        } else {
            Err(TransportError::Timeout)
        }

        // TODO: Khi có phần cứng thật, thay bằng esp-hal SPI slave transfer.
    }
}

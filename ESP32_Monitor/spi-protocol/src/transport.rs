// =============================================================================
// spi-protocol/src/transport.rs
// =============================================================================
// Giao diện trừu tượng (abstract contract) cho lớp Truyền dẫn (FR-03).
//
// Lớp Protocol và Application giao tiếp với lớp dưới thông qua trait này,
// cho phép thay thế SPI bằng UART/I2C/Mock trong tương lai mà KHÔNG cần
// thay đổi mã nguồn các lớp phía trên (FR-03).
//
// Transport trait KHÔNG chứa logic nghiệp vụ (FR-02):
// - Không phân tích lệnh
// - Không kiểm tra tham số
// - Không diễn giải ý nghĩa dữ liệu
// Chỉ chịu trách nhiệm gửi/nhận luồng byte thô.
// =============================================================================

use crate::config::FRAME_SIZE;

/// Lỗi tầng truyền dẫn.
///
/// Mỗi implementation cụ thể (SPI, UART, Mock...) có thể định nghĩa
/// kiểu lỗi riêng phù hợp với phần cứng bên dưới.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-logging", derive(defmt::Format))]
pub enum TransportError {
    /// Lỗi bus vật lý (SPI bus error, UART framing error...).
    BusError,
    /// Thiết bị đích không phản hồi.
    DeviceNotResponding,
    /// Buffer truyền/nhận không đủ kích thước.
    BufferTooSmall,
    /// Giao dịch bị timeout ở tầng truyền dẫn.
    Timeout,
    /// Lỗi khác không phân loại được.
    Other,
}

/// Giao diện trừu tượng cho lớp Truyền dẫn (FR-02, FR-03).
///
/// # Nguyên tắc thiết kế
///
/// - **FR-02**: Trait này KHÔNG chứa bất kỳ logic nghiệp vụ nào.
///   Nó chỉ gửi/nhận mảng byte thô có kích thước cố định `FRAME_SIZE`.
///
/// - **FR-03**: Các lớp phía trên (Protocol, Application) chỉ biết đến trait này,
///   không biết implementation cụ thể. Thay đổi SPI → UART chỉ cần đổi
///   struct implement trait, không sửa code lớp trên.
///
/// - **FR-04**: Transport trait nằm ở tầng thấp nhất, không phụ thuộc
///   vào bất kỳ lớp nào ở trên.
///
/// # Implementors
///
/// - `SpiMaster` trên Host (Raspberry Pi 4) — sử dụng `rppal::spi::Spi`
/// - `SpiSlave` trên Node (ESP32-S3) — sử dụng `esp_hal::spi::slave`
/// - `MockTransport` cho unit testing
pub trait Transport {
    /// Gửi một khung byte thô có kích thước cố định `FRAME_SIZE`.
    ///
    /// Implementation chỉ cần đẩy bytes lên bus, KHÔNG diễn giải nội dung.
    ///
    /// # Errors
    /// Trả về `TransportError` nếu giao dịch vật lý thất bại.
    fn send_raw(&mut self, data: &[u8; FRAME_SIZE]) -> Result<(), TransportError>;

    /// Nhận một khung byte thô có kích thước cố định `FRAME_SIZE`.
    ///
    /// Implementation chỉ cần đọc bytes từ bus vào buffer,
    /// KHÔNG kiểm tra nội dung hay tính hợp lệ.
    ///
    /// # Errors
    /// Trả về `TransportError` nếu giao dịch vật lý thất bại.
    fn receive_raw(&mut self, buffer: &mut [u8; FRAME_SIZE]) -> Result<(), TransportError>;

    /// Thực hiện giao dịch full-duplex: gửi `tx_buf` đồng thời nhận vào `rx_buf`.
    ///
    /// Phương thức này phù hợp với đặc tính SPI (full-duplex synchronous).
    /// Đối với các bus bán song công (half-duplex) như UART,
    /// implementation có thể thực hiện tuần tự: gửi trước, nhận sau.
    ///
    /// # Errors
    /// Trả về `TransportError` nếu giao dịch vật lý thất bại.
    fn transfer(
        &mut self,
        tx_buf: &[u8; FRAME_SIZE],
        rx_buf: &mut [u8; FRAME_SIZE],
    ) -> Result<(), TransportError>;
}

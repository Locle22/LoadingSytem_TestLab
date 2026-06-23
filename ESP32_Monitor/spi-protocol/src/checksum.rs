// =============================================================================
// spi-protocol/src/checksum.rs
// =============================================================================
// CRC-16 checksum cho kiểm tra toàn vẹn dữ liệu (FR-08).
//
// Sử dụng CRC-16/IBM-SDLC (CCITT variant):
// - Polynomial: 0x1021
// - Init: 0xFFFF
// - Refin: true, Refout: true
// - XorOut: 0xFFFF
//
// Khả năng phát hiện:
// - 100% lỗi bit đơn
// - 100% lỗi bit đôi
// - 100% lỗi cụm ≤ 16 bit
//
// Phù hợp cho môi trường công nghiệp có nhiễu điện từ (FR-08).
// no_std compatible (dùng crate `crc` với default-features = false).
// =============================================================================

use crc::{Crc, CRC_16_IBM_SDLC};

/// Bộ tính CRC-16 (compile-time constant, zero runtime cost cho khởi tạo).
const CRC_CALCULATOR: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);

/// Tính CRC-16 cho một slice dữ liệu.
///
/// # Arguments
/// * `data` - Dữ liệu cần tính CRC (thường là bytes 0..30 của khung truyền).
///
/// # Returns
/// Giá trị CRC-16 (u16, little-endian khi đặt vào khung truyền).
pub fn compute_crc(data: &[u8]) -> u16 {
    CRC_CALCULATOR.checksum(data)
}

/// Kiểm tra CRC-16 có khớp với giá trị mong đợi không.
///
/// # Arguments
/// * `data` - Dữ liệu đã nhận (không bao gồm 2 bytes CRC).
/// * `expected_crc` - Giá trị CRC-16 đã nhận từ khung truyền.
///
/// # Returns
/// `true` nếu CRC khớp (dữ liệu toàn vẹn), `false` nếu không.
pub fn verify_crc(data: &[u8], expected_crc: u16) -> bool {
    compute_crc(data) == expected_crc
}

// ─── Unit Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_known_value() {
        // CRC-16/IBM-SDLC của "123456789" = 0x906E
        let data = b"123456789";
        let crc = compute_crc(data);
        assert_eq!(crc, 0x906E, "CRC-16/IBM-SDLC of '123456789' should be 0x906E");
    }

    #[test]
    fn test_crc_verify_success() {
        let data = b"Hello SPI";
        let crc = compute_crc(data);
        assert!(verify_crc(data, crc), "CRC verification should succeed for matching data");
    }

    #[test]
    fn test_crc_verify_failure_corrupted_data() {
        let data = b"Hello SPI";
        let crc = compute_crc(data);
        let mut corrupted = *data;
        corrupted[0] ^= 0x01; // Flip 1 bit
        assert!(
            !verify_crc(&corrupted, crc),
            "CRC verification should fail for corrupted data"
        );
    }

    #[test]
    fn test_crc_verify_failure_wrong_crc() {
        let data = b"test data";
        let crc = compute_crc(data);
        assert!(
            !verify_crc(data, crc.wrapping_add(1)),
            "CRC verification should fail for wrong CRC value"
        );
    }

    #[test]
    fn test_crc_empty_data() {
        // CRC of empty data should be deterministic
        let crc1 = compute_crc(&[]);
        let crc2 = compute_crc(&[]);
        assert_eq!(crc1, crc2, "CRC of empty data should be deterministic");
    }

    #[test]
    fn test_crc_single_bit_error_detected() {
        let data = [0xAA, 0x01, 0x20, 0x01, 0x0C, 0x00, 0x00, 0x00];
        let original_crc = compute_crc(&data);

        // Flip each bit in the data and verify CRC detects it
        for byte_idx in 0..data.len() {
            for bit_idx in 0..8 {
                let mut corrupted = data;
                corrupted[byte_idx] ^= 1 << bit_idx;
                assert!(
                    !verify_crc(&corrupted, original_crc),
                    "CRC should detect single-bit error at byte {} bit {}",
                    byte_idx,
                    bit_idx
                );
            }
        }
    }
}

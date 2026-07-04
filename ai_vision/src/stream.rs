use std::io::Read;
use crate::types::DetectorError;

/// Trình đọc luồng MJPEG từ IP Camera.
pub struct MjpegStream {
    reader: reqwest::blocking::Response,
    buffer: Vec<u8>,
}

impl MjpegStream {
    /// Mở kết nối tới DroidCam IP Camera URL.
    pub fn new(url: &str) -> Result<Self, DetectorError> {
        let resp = reqwest::blocking::get(url).map_err(|e| {
            DetectorError::ImageProcessing(format!("Failed to connect to IP camera: {e}"))
        })?;

        // Kiểm tra HTTP Status
        if !resp.status().is_success() {
            return Err(DetectorError::ImageProcessing(format!(
                "Camera returned HTTP {}",
                resp.status()
            )));
        }

        Ok(Self {
            reader: resp,
            buffer: Vec::with_capacity(1024 * 1024), // 1MB buffer
        })
    }

    /// Lấy khung hình tiếp theo (Blocking).
    /// Quét buffer tìm `0xFF 0xD8` (SOI) và `0xFF 0xD9` (EOI).
    pub fn next_frame(&mut self) -> Result<image::DynamicImage, DetectorError> {
        let mut chunk = [0u8; 16384];

        loop {
            let mut start_idx = None;
            let mut end_idx = None;

            // Tìm SOI (Start of Image)
            for i in 0..self.buffer.len().saturating_sub(1) {
                if self.buffer[i] == 0xFF && self.buffer[i + 1] == 0xD8 {
                    start_idx = Some(i);
                    break;
                }
            }

            // Nếu thấy SOI, tiếp tục tìm EOI (End of Image)
            if let Some(start) = start_idx {
                for i in start..self.buffer.len().saturating_sub(1) {
                    if self.buffer[i] == 0xFF && self.buffer[i + 1] == 0xD9 {
                        end_idx = Some(i + 2); // Bao gồm cả EOI
                        break;
                    }
                }
            }

            // Nếu tìm thấy trọn vẹn 1 file JPEG
            if let (Some(start), Some(end)) = (start_idx, end_idx) {
                let frame_data = &self.buffer[start..end];
                let img = image::load_from_memory(frame_data).map_err(|e| {
                    DetectorError::ImageProcessing(format!("Failed to decode JPEG frame: {e}"))
                })?;

                // Loại bỏ frame cũ khỏi buffer (drain), giữ lại phần dư (nếu có)
                self.buffer.drain(0..end);

                return Ok(img);
            } else if start_idx.is_none() && !self.buffer.is_empty() {
                // Tối ưu bộ nhớ: nếu không thấy SOI, xóa sạch buffer (Rác HTTP headers)
                // Nhưng chừa lại 1 byte cuối phòng hờ SOI bị cắt đôi
                if self.buffer.len() > 1 {
                    let last = self.buffer.len() - 1;
                    self.buffer.drain(0..last);
                }
            }

            // Đọc thêm dữ liệu vào mạng nếu chưa đủ 1 frame
            let n = self
                .reader
                .read(&mut chunk)
                .map_err(|e| DetectorError::ImageProcessing(format!("Stream read error: {e}")))?;

            if n == 0 {
                return Err(DetectorError::ImageProcessing(
                    "Camera stream closed unexpectedly".into(),
                ));
            }
            self.buffer.extend_from_slice(&chunk[..n]);
        }
    }
}

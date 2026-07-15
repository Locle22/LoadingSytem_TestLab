use std::io::Read;
use crate::types::DetectorError;

use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};

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
                if self.buffer.len() > 1 {
                    let last = self.buffer.len() - 1;
                    self.buffer.drain(0..last);
                }
            }

            // Đọc thêm dữ liệu từ mạng nếu chưa đủ 1 frame
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

/// Enum đóng gói toàn bộ luồng luân chuyển video của hệ thống.
pub enum VideoStream {
    Mjpeg(MjpegStream),
    Webcam(Box<Camera>),
}

impl VideoStream {
    /// Lấy khung hình tiếp theo (tự động nhận dạng kiểu nguồn).
    pub fn next_frame(&mut self) -> Result<image::DynamicImage, DetectorError> {
        match self {
            VideoStream::Mjpeg(stream) => stream.next_frame(),
            VideoStream::Webcam(camera) => {
                let frame = camera.frame().map_err(|e| {
                    DetectorError::ImageProcessing(format!("Failed to capture USB frame: {e}"))
                })?;
                let decoded = frame.decode_image::<RgbFormat>().map_err(|e| {
                    DetectorError::ImageProcessing(format!("Failed to decode RGB frame: {e}"))
                })?;
                
                let rgb_img: image::RgbImage = decoded;
                Ok(image::DynamicImage::ImageRgb8(rgb_img))
            }
        }
    }
}

/// Khởi tạo camera USB cắm trực tiếp (như kính hiển vi USB HDMI Microscope).
pub fn new_webcam(index: u32) -> Result<Camera, DetectorError> {
    let index = CameraIndex::Index(index);
    let requested = RequestedFormat::new::<RgbFormat>(
        RequestedFormatType::AbsoluteHighestFrameRate
    );
    let mut camera = Camera::new(index, requested).map_err(|e| {
        DetectorError::ImageProcessing(format!("Failed to open USB camera: {e}"))
    })?;
    camera.open_stream().map_err(|e| {
        DetectorError::ImageProcessing(format!("Failed to start USB camera stream: {e}"))
    })?;
    Ok(camera)
}

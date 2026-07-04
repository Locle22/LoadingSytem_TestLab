use ab_glyph::{FontRef, PxScale};
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;

use crate::types::Detection;

/// Vẽ danh sách các nhận diện (Detections) đè lên hình ảnh đầu vào.
pub fn draw_detections(img: &mut RgbImage, detections: &[Detection]) {
    // Thử load font Arial từ Windows. Nếu thất bại, không vẽ chữ mà chỉ vẽ khung.
    let font_data = std::fs::read("C:\\Windows\\Fonts\\arial.ttf").unwrap_or_default();
    let font = FontRef::try_from_slice(&font_data).ok();

    let box_color = Rgb([220, 20, 60]); // Màu đỏ Crimson
    let text_color = Rgb([255, 255, 255]); // Trắng

    for det in detections {
        let x = det.x_min as i32;
        let y = det.y_min as i32;

        // Tính toán chiều rộng, chiều cao (bảo vệ tránh bị âm)
        let w = (det.x_max - det.x_min).max(1.0) as u32;
        let h = (det.y_max - det.y_min).max(1.0) as u32;

        let rect = Rect::at(x, y).of_size(w, h);

        // Vẽ viền hộp dày 3 pixels
        draw_hollow_rect_mut(img, rect, box_color);
        draw_hollow_rect_mut(img, Rect::at(x - 1, y - 1).of_size(w + 2, h + 2), box_color);
        draw_hollow_rect_mut(
            img,
            Rect::at(x + 1, y + 1).of_size(w.saturating_sub(2), h.saturating_sub(2)),
            box_color,
        );

        // Vẽ nhãn (Label) nếu Font khả dụng
        if let Some(ref f) = font {
            let label = format!("{} {:.0}%", det.class_name, det.confidence * 100.0);
            let scale = PxScale::from(20.0);

            // Đo kích thước chữ (ước lượng)
            let text_width = label.len() as u32 * 10;
            let text_height = 24;

            // Vẽ nền cho chữ
            let bg_rect = Rect::at(x - 1, y - text_height as i32)
                .of_size(text_width + 10, text_height as u32);
            draw_filled_rect_mut(img, bg_rect, box_color);

            // Vẽ chữ
            draw_text_mut(img, text_color, x + 2, y - text_height as i32 + 2, scale, f, &label);
        }
    }
}

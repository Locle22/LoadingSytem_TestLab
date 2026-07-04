//! Image pre-processing and model output post-processing.
//!
//! This module owns two critical pipeline stages:
//!
//! 1. **Pre-processing** — read an image from disk, apply *letterbox*
//!    resizing (preserving aspect ratio with gray padding), convert to
//!    RGB, normalise to `[0, 1]`, and emit an NCHW `Array4<f32>` tensor.
//!
//! 2. **Post-processing** — decode the raw `(1, 4+C, N)` output tensor,
//!    apply confidence filtering, convert `cx/cy/w/h` → `x1/y1/x2/y2`,
//!    rescale coordinates back to the original image, and run greedy
//!    per-class Non-Maximum Suppression (NMS).

use ndarray::Array4;

use crate::types::{Detection, DetectorError, LABELS, NUM_CLASSES};

// ──────────────────────────────────────────────
//  Letterbox Metadata
// ──────────────────────────────────────────────

/// Book-keeping from the letterbox resize, required to un-project
/// bounding-box coordinates back to the original image.
pub(crate) struct LetterboxInfo {
    /// Uniform scale factor applied to the image (< 1.0 means shrunk).
    pub scale: f32,
    /// Horizontal padding added to each side (pixels in model space).
    pub pad_x: f32,
    /// Vertical padding added to each side (pixels in model space).
    pub pad_y: f32,
    /// Original image width before any transformation.
    pub orig_width: u32,
    /// Original image height before any transformation.
    pub orig_height: u32,
}

// ──────────────────────────────────────────────
//  Pre-processing
// ──────────────────────────────────────────────

/// Read an image from `image_path`, apply YOLO-style letterbox resize to
/// `(input_w × input_h)`, normalise pixel values to `[0, 1]`, and return
/// an NCHW `f32` tensor together with the [`LetterboxInfo`] needed later
/// for coordinate rescaling.
///
/// # Colour space
/// The `image` crate decodes to RGB by default; no BGR conversion is
/// needed because the Ultralytics training pipeline also uses RGB.
///
/// # Padding value
/// Gray fill `(114, 114, 114)` follows the YOLO letterbox convention.
pub(crate) fn preprocess(
    image_path: &std::path::Path,
    input_w: u32,
    input_h: u32,
) -> Result<(Array4<f32>, LetterboxInfo), DetectorError> {
    let img = image::open(image_path).map_err(|e| {
        DetectorError::ImageProcessing(format!(
            "Cannot open '{}': {e}",
            image_path.display()
        ))
    })?;

    preprocess_image(&img, input_w, input_h)
}

pub(crate) fn preprocess_image(
    img: &image::DynamicImage,
    input_w: u32,
    input_h: u32,
) -> Result<(Array4<f32>, LetterboxInfo), DetectorError> {
    let (orig_w, orig_h) = (img.width(), img.height());

    // ── 2. Compute letterbox geometry ──────────────────────────────
    let scale = f32::min(input_w as f32 / orig_w as f32, input_h as f32 / orig_h as f32);

    let new_w = (orig_w as f32 * scale).round() as u32;
    let new_h = (orig_h as f32 * scale).round() as u32;

    let pad_x = (input_w as f32 - new_w as f32) / 2.0;
    let pad_y = (input_h as f32 - new_h as f32) / 2.0;

    // ── 3. Resize (maintain aspect ratio) ──────────────────────────
    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Triangle);
    let rgb_resized = resized.to_rgb8();

    // ── 4. Paste onto gray canvas ──────────────────────────────────
    let mut canvas =
        image::RgbImage::from_pixel(input_w, input_h, image::Rgb([114u8, 114, 114]));

    image::imageops::overlay(
        &mut canvas,
        &rgb_resized,
        pad_x.round() as i64,
        pad_y.round() as i64,
    );

    // ── 5. Convert to NCHW f32 tensor, normalise /255 ─────────────
    let raw = canvas.as_raw(); // &[u8] — contiguous RGB row-major
    let w = input_w as usize;
    let h = input_h as usize;

    let mut tensor = Array4::<f32>::zeros((1, 3, h, w));
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 3;
            tensor[[0, 0, y, x]] = raw[idx] as f32 / 255.0; // R
            tensor[[0, 1, y, x]] = raw[idx + 1] as f32 / 255.0; // G
            tensor[[0, 2, y, x]] = raw[idx + 2] as f32 / 255.0; // B
        }
    }

    let info = LetterboxInfo {
        scale,
        pad_x,
        pad_y,
        orig_width: orig_w,
        orig_height: orig_h,
    };

    Ok((tensor, info))
}

// ──────────────────────────────────────────────
//  Post-processing
// ──────────────────────────────────────────────

/// Decode the raw YOLO output tensor, filter by confidence, convert
/// coordinates, and apply per-class greedy NMS.
///
/// # Expected tensor layout
///
/// Shape: `[1, 4 + NUM_CLASSES, N]` where:
/// - rows 0‥3  → `cx, cy, w, h` (centre-format, model-space pixels)
/// - rows 4‥39 → per-class confidence scores
/// - `N`       → total number of anchor predictions
///
/// The data is stored in **row-major** order: element `[b, r, c]` is at
/// index `b * (rows * cols) + r * cols + c`.
///
/// This is the **raw** output format produced by YOLOv9 when exported
/// via Ultralytics *without* end-to-end NMS.
pub(crate) fn postprocess(
    shape: &[i64],
    data: &[f32],
    letterbox: &LetterboxInfo,
    conf_threshold: f32,
    iou_threshold: f32,
) -> Result<Vec<Detection>, DetectorError> {
    // ── Validate shape ─────────────────────────────────────────────
    if shape.len() != 3 {
        return Err(DetectorError::Postprocessing(format!(
            "Expected 3-D output tensor, got {}-D with shape {shape:?}",
            shape.len()
        )));
    }

    let rows = shape[1] as usize; // 4 + NUM_CLASSES
    let num_preds = shape[2] as usize; // N predictions

    let expected_rows = 4 + NUM_CLASSES;
    if rows != expected_rows {
        return Err(DetectorError::Postprocessing(format!(
            "Output dim[1] is {rows} but expected {expected_rows} \
             (4 bbox + {NUM_CLASSES} classes). Shape: {shape:?}"
        )));
    }

    // Helper: access element at [0, row, col] in row-major layout.
    // The batch dimension is always 0 so we skip it.
    let at = |row: usize, col: usize| -> f32 {
        data[row * num_preds + col]
    };

    // ── Decode each prediction ─────────────────────────────────────
    let mut candidates: Vec<Detection> = Vec::with_capacity(num_preds / 4);

    for i in 0..num_preds {
        // Find the best class score for this anchor
        let mut best_class: usize = 0;
        let mut best_score: f32 = f32::NEG_INFINITY;
        for c in 0..NUM_CLASSES {
            let score = at(4 + c, i);
            if score > best_score {
                best_score = score;
                best_class = c;
            }
        }

        // Early reject low-confidence predictions
        if best_score < conf_threshold {
            continue;
        }

        // Centre-format → corner-format (model-space)
        let cx = at(0, i);
        let cy = at(1, i);
        let w = at(2, i);
        let h = at(3, i);

        let x1 = cx - w / 2.0;
        let y1 = cy - h / 2.0;
        let x2 = cx + w / 2.0;
        let y2 = cy + h / 2.0;

        // Rescale from letterbox space → original image space
        let x1_orig =
            ((x1 - letterbox.pad_x) / letterbox.scale).clamp(0.0, letterbox.orig_width as f32);
        let y1_orig =
            ((y1 - letterbox.pad_y) / letterbox.scale).clamp(0.0, letterbox.orig_height as f32);
        let x2_orig =
            ((x2 - letterbox.pad_x) / letterbox.scale).clamp(0.0, letterbox.orig_width as f32);
        let y2_orig =
            ((y2 - letterbox.pad_y) / letterbox.scale).clamp(0.0, letterbox.orig_height as f32);

        // Safety: best_class is always < NUM_CLASSES (36) so LABELS[best_class]
        // is guaranteed in-bounds.
        let class_name = LABELS
            .get(best_class)
            .copied()
            .unwrap_or("Unknown");

        candidates.push(Detection {
            class_id: best_class,
            class_name,
            confidence: best_score,
            x_min: x1_orig,
            y_min: y1_orig,
            x_max: x2_orig,
            y_max: y2_orig,
        });
    }

    // ── Non-Maximum Suppression ────────────────────────────────────
    Ok(nms(candidates, iou_threshold))
}

// ──────────────────────────────────────────────
//  Greedy Per-Class NMS
// ──────────────────────────────────────────────

/// Greedy per-class Non-Maximum Suppression.
///
/// 1. Sort candidates by confidence (descending).
/// 2. For each unsuppressed candidate, suppress all later candidates
///    of the **same class** whose IoU exceeds `iou_threshold`.
///
/// Runs in `O(n²)` but `n` is typically small after confidence filtering.
fn nms(mut detections: Vec<Detection>, iou_threshold: f32) -> Vec<Detection> {
    // Sort descending by confidence
    detections.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let n = detections.len();
    let mut suppressed = vec![false; n];
    let mut keep: Vec<Detection> = Vec::with_capacity(n);

    for i in 0..n {
        if suppressed[i] {
            continue;
        }
        keep.push(detections[i].clone());

        // Suppress overlapping boxes of the *same class*
        for j in (i + 1)..n {
            if suppressed[j] {
                continue;
            }
            if detections[j].class_id != detections[i].class_id {
                continue;
            }
            if iou(&detections[i], &detections[j]) > iou_threshold {
                suppressed[j] = true;
            }
        }
    }

    keep
}

/// Intersection-over-Union between two axis-aligned bounding boxes.
fn iou(a: &Detection, b: &Detection) -> f32 {
    let inter_x1 = a.x_min.max(b.x_min);
    let inter_y1 = a.y_min.max(b.y_min);
    let inter_x2 = a.x_max.min(b.x_max);
    let inter_y2 = a.y_max.min(b.y_max);

    let inter_area = (inter_x2 - inter_x1).max(0.0) * (inter_y2 - inter_y1).max(0.0);
    let area_a = (a.x_max - a.x_min) * (a.y_max - a.y_min);
    let area_b = (b.x_max - b.x_min) * (b.y_max - b.y_min);
    let union_area = area_a + area_b - inter_area;

    if union_area <= 0.0 {
        0.0
    } else {
        inter_area / union_area
    }
}

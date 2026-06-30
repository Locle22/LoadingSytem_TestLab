//! **ai_vision** — Core library for YOLOv9e mosquito species detection.
//!
//! This crate provides a single, high-level [`YoloDetector`] that loads
//! an ONNX model and exposes one public method:
//!
//! ```rust,ignore
//! let detector = YoloDetector::new("model.onnx", ComputeDevice::Cpu, None, None)?;
//! let detections: Vec<Detection> = detector.detect("image.jpg")?;
//! ```
//!
//! # Architecture
//!
//! | Module       | Responsibility |
//! |--------------|------------------------------------------------------|
//! | [`types`]    | Shared data types, error hierarchy, class labels.    |
//! | [`core`]     | Session management and pipeline orchestration.       |
//! | [`process`]  | Image pre-processing, tensor post-processing, NMS.   |
//!
//! # Feature flags
//!
//! | Feature      | Effect |
//! |--------------|------------------------------------------------------|
//! | `cuda`       | Enables NVIDIA CUDA Execution Provider.              |
//! | `tensorrt`   | Enables NVIDIA TensorRT Execution Provider.          |

// ── Internal modules ───────────────────────────────────────────────
pub mod types;
pub mod core;
mod process;

// ── Public re-exports (ergonomic top-level access) ─────────────────
pub use crate::core::YoloDetector;
pub use crate::types::{ComputeDevice, Detection, DetectorError, LABELS, NUM_CLASSES};

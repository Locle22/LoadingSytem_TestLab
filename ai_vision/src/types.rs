//! Type definitions for the AI Vision detection pipeline.
//!
//! This module defines all shared data structures used across the library:
//! - [`DetectorError`]: Typed error hierarchy for every failure mode.
//! - [`ComputeDevice`]: Hardware execution provider selection.
//! - [`Detection`]: A single bounding-box result with class metadata.
//! - [`LABELS`]: The 36 mosquito species class names.

use thiserror::Error;

// ──────────────────────────────────────────────
//  Error Hierarchy
// ──────────────────────────────────────────────

/// Exhaustive, typed error hierarchy for the detection pipeline.
///
/// Each variant maps to a distinct phase so callers can programmatically
/// distinguish *where* a failure occurred without parsing error strings.
#[derive(Debug, Error)]
pub enum DetectorError {
    /// Failed to initialise the ONNX Runtime session or load the model file.
    #[error("Model loading failed: {0}")]
    ModelLoad(String),

    /// Failed during image I/O, resize, colour-space conversion, or
    /// tensor normalisation.
    #[error("Image processing failed: {0}")]
    ImageProcessing(String),

    /// The ONNX Runtime returned an error during forward-pass execution.
    #[error("Inference failed: {0}")]
    Inference(String),

    /// Raw model output could not be decoded, filtered, or NMS-suppressed.
    #[error("Post-processing failed: {0}")]
    Postprocessing(String),
}

// ──────────────────────────────────────────────
//  Compute Device
// ──────────────────────────────────────────────

/// Selects the ONNX Runtime Execution Provider at construction time.
///
/// The library transparently maps each variant to the corresponding
/// `ort` Execution Provider.  GPU variants require their Cargo feature
/// to be enabled at compile time; selecting them without the feature
/// returns a clear [`DetectorError::ModelLoad`] at runtime.
#[derive(Debug, Clone)]
pub enum ComputeDevice {
    /// CPU execution — always available, no extra dependencies.
    Cpu,

    /// NVIDIA CUDA execution.
    ///
    /// The inner `i32` is the CUDA device ordinal (e.g. `0`).
    /// Requires the crate feature **`cuda`**.
    /// Falls back to CPU if the driver or hardware is missing.
    Cuda(i32),

    /// NVIDIA TensorRT execution with automatic CUDA → CPU fallback.
    ///
    /// The inner `i32` is the GPU device ordinal.
    /// Requires the crate feature **`tensorrt`**.
    /// The provider chain is: TensorRT → CUDA → CPU.
    TensorRT(i32),
}

// ──────────────────────────────────────────────
//  Detection Result
// ──────────────────────────────────────────────

/// A single object detection produced by the YOLO pipeline.
///
/// Coordinates are in **pixel space of the original input image**
/// (i.e. already rescaled from the letterboxed model input).
/// The bounding box uses the `(x_min, y_min, x_max, y_max)` convention.
#[derive(Debug, Clone)]
pub struct Detection {
    /// Zero-based class index into [`LABELS`].
    pub class_id: usize,

    /// Human-readable species name (borrowed from the static label table).
    pub class_name: &'static str,

    /// Model confidence score in the range `[0.0, 1.0]`.
    pub confidence: f32,

    /// Left edge of the bounding box (pixels).
    pub x_min: f32,

    /// Top edge of the bounding box (pixels).
    pub y_min: f32,

    /// Right edge of the bounding box (pixels).
    pub x_max: f32,

    /// Bottom edge of the bounding box (pixels).
    pub y_max: f32,
}

// ──────────────────────────────────────────────
//  Class Label Table (36 mosquito species)
// ──────────────────────────────────────────────

/// The 36 mosquito species that the YOLOv9e model was trained to classify.
///
/// Index order matches the training dataset's `class_id`.
pub const LABELS: [&str; 36] = [
    "Aedes camptorhynchus",            // 0
    "Aedes hesperonotius",             // 1
    "Aedes notoscriptus",              // 2
    "Aedes ratcliffei",                // 3
    "Aedes vigilax",                   // 4
    "Anopheles annulipes",             // 5
    "Coquillettidia linealis",         // 6
    "Culex annulirostris",             // 7
    "Culex australicus",               // 8
    "Culex globocoxitus",              // 9
    "Culex latus",                     // 10
    "Culex quinquefasciatus",          // 11
    "Aedes alboannulatus",             // 12
    "Aedes alternans",                 // 13
    "Aedes sagax",                     // 14
    "Coquillettidia xanthogaster",     // 15
    "Culex sitiens",                   // 16
    "Male coquillettidia xanthogaster",// 17
    "Mansonia uniformis",              // 18
    "Tripteroides atripes",            // 19
    "Aedes aculeatus",                 // 20
    "Aedes alboseutellatus",           // 21
    "Aedes bitaenorhynchus",           // 22
    "Aedes burpengaryensis",           // 23
    "Aedes garnicola",                 // 24
    "Aedes lineatopennis",             // 25
    "Aedes procox",                    // 26
    "Aedes vittiger",                  // 27
    "Anopheles bancroftii",            // 28
    "Culex arbostensis",               // 29
    "Verralina funerea",               // 30
    "Verrallina masters 52",           // 31
    "Culiseta atra",                   // 32
    "Aedes albosostatus",              // 33
    "Aedes hesperontus",               // 34
    "Aedes_torneri",                   // 35
];

/// Total number of species classes the model can predict.
pub const NUM_CLASSES: usize = LABELS.len();

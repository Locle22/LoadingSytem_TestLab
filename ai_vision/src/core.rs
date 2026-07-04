//! ONNX session management and inference orchestration.
//!
//! [`YoloDetector`] encapsulates the full lifecycle of a YOLO inference
//! session: model loading, hardware device selection, and the
//! `preprocess → inference → postprocess` pipeline.
//!
//! The only public method on this struct is [`YoloDetector::detect`],
//! keeping the API surface minimal and hard to misuse.

use std::path::Path;
use std::sync::Mutex;

use ndarray::Array4;
use ort::session::Session;
use ort::value::Tensor;

use crate::process::{postprocess, preprocess, preprocess_image, LetterboxInfo};
use crate::types::{ComputeDevice, Detection, DetectorError};

// ──────────────────────────────────────────────
//  Model-specific constants (YOLOv9e @ 512×512)
// ──────────────────────────────────────────────

/// Width of the input tensor expected by the model.
const INPUT_WIDTH: u32 = 512;

/// Height of the input tensor expected by the model.
const INPUT_HEIGHT: u32 = 512;

/// Default confidence threshold used when the caller does not specify one.
const DEFAULT_CONF_THRESHOLD: f32 = 0.25;

/// Default IoU threshold for NMS, matching the value stored in the
/// original checkpoint (0.7).
const DEFAULT_IOU_THRESHOLD: f32 = 0.7;

// ──────────────────────────────────────────────
//  YoloDetector
// ──────────────────────────────────────────────

/// The central inference engine.
///
/// Holds an ONNX Runtime [`Session`] (behind a [`Mutex`] for interior
/// mutability, since `Session::run` requires `&mut self`) and all
/// hyper-parameters needed to run the full detection pipeline.
///
/// Instances are **thread-safe** — the `Mutex` ensures exclusive access
/// during inference, so a single `YoloDetector` can be shared across
/// threads behind an `Arc`.
pub struct YoloDetector {
    /// Mutex-wrapped session because `ort::Session::run` takes `&mut self`.
    session: Mutex<Session>,
    input_width: u32,
    input_height: u32,
    conf_threshold: f32,
    iou_threshold: f32,
}

impl YoloDetector {
    // ── Constructor ────────────────────────────────────────────────

    /// Create a new detector from an ONNX model file.
    ///
    /// # Arguments
    /// * `model_path`     — Path to the `.onnx` file.
    /// * `device`         — [`ComputeDevice`] selecting the Execution Provider.
    /// * `conf_threshold` — Override the default confidence cut-off (0.25).
    /// * `iou_threshold`  — Override the default NMS IoU threshold (0.7).
    ///
    /// # Errors
    /// Returns [`DetectorError::ModelLoad`] if the model file is missing,
    /// corrupt, or the requested Execution Provider is unavailable.
    pub fn new(
        model_path: impl AsRef<Path>,
        device: ComputeDevice,
        conf_threshold: Option<f32>,
        iou_threshold: Option<f32>,
    ) -> Result<Self, DetectorError> {
        let session = Self::build_session(model_path.as_ref(), &device)?;

        Ok(Self {
            session: Mutex::new(session),
            input_width: INPUT_WIDTH,
            input_height: INPUT_HEIGHT,
            conf_threshold: conf_threshold.unwrap_or(DEFAULT_CONF_THRESHOLD),
            iou_threshold: iou_threshold.unwrap_or(DEFAULT_IOU_THRESHOLD),
        })
    }

    // ── Public API (single entry point) ────────────────────────────

    /// Run the full detection pipeline on a single image.
    ///
    /// 1. **Pre-process** — letterbox resize + normalise → NCHW tensor.
    /// 2. **Inference**   — forward-pass through the ONNX model.
    /// 3. **Post-process** — decode, filter, rescale, NMS.
    ///
    /// Returns a `Vec<Detection>` sorted by descending confidence.
    ///
    /// # Errors
    /// Propagates errors from any pipeline stage; see [`DetectorError`].
    pub fn detect(&self, image_path: impl AsRef<Path>) -> Result<Vec<Detection>, DetectorError> {
        let image_path = image_path.as_ref();

        // Stage 1 — Pre-process
        let (input_tensor, letterbox_info) =
            preprocess(image_path, self.input_width, self.input_height)?;

        self.detect_tensor(input_tensor, letterbox_info)
    }

    /// Run the full detection pipeline on an in-memory image.
    /// Eliminates disk I/O bottleneck for real-time video streams.
    pub fn detect_image(&self, img: &image::DynamicImage) -> Result<Vec<Detection>, DetectorError> {
        let (input_tensor, letterbox_info) =
            preprocess_image(img, self.input_width, self.input_height)?;

        self.detect_tensor(input_tensor, letterbox_info)
    }

    fn detect_tensor(&self, input_tensor: Array4<f32>, letterbox_info: LetterboxInfo) -> Result<Vec<Detection>, DetectorError> {
        // Stage 2 — Inference
        let raw_output = self.run_inference(input_tensor)?;

        // Stage 3 — Post-process
        let (shape, data) = raw_output
            .try_extract_tensor::<f32>()
            .map_err(|e| {
                DetectorError::Inference(format!("Failed to extract output tensor: {e}"))
            })?;

        postprocess(
            shape,
            data,
            &letterbox_info,
            self.conf_threshold,
            self.iou_threshold,
        )
    }

    // ── Private: Session construction ──────────────────────────────

    /// Build an ONNX Runtime [`Session`] with the requested Execution
    /// Provider, applying safe fallback semantics for GPU variants.
    fn build_session(model_path: &Path, device: &ComputeDevice) -> Result<Session, DetectorError> {
        let mut builder = Session::builder()
            .map_err(|e| DetectorError::ModelLoad(format!("SessionBuilder init failed: {e}")))?;

        let session = match device {
            // ─── CPU (always available) ────────────────────────────
            ComputeDevice::Cpu => builder.commit_from_file(model_path),

            // ─── CUDA ──────────────────────────────────────────────
            ComputeDevice::Cuda(device_id) => {
                #[cfg(feature = "cuda")]
                {
                    use ort::execution_providers::CUDAExecutionProvider;

                    let _device_id = *device_id;
                    builder
                        .with_execution_providers([
                            CUDAExecutionProvider::default().build(),
                        ])
                        .map_err(|e| {
                            DetectorError::ModelLoad(format!("CUDA EP registration failed: {e}"))
                        })?
                        .commit_from_file(model_path)
                }
                #[cfg(not(feature = "cuda"))]
                {
                    let _ = device_id;
                    return Err(DetectorError::ModelLoad(
                        "CUDA support was not compiled. \
                         Rebuild with: cargo build --features cuda"
                            .into(),
                    ));
                }
            }

            // ─── TensorRT (fallback chain: TRT → CUDA → CPU) ──────
            ComputeDevice::TensorRT(device_id) => {
                #[cfg(feature = "tensorrt")]
                {
                    use ort::execution_providers::{
                        CUDAExecutionProvider, TensorRTExecutionProvider,
                    };

                    let _device_id = *device_id;
                    builder
                        .with_execution_providers([
                            TensorRTExecutionProvider::default().build(),
                            CUDAExecutionProvider::default().build(),
                        ])
                        .map_err(|e| {
                            DetectorError::ModelLoad(format!(
                                "TensorRT/CUDA EP registration failed: {e}"
                            ))
                        })?
                        .commit_from_file(model_path)
                }
                #[cfg(not(feature = "tensorrt"))]
                {
                    let _ = device_id;
                    return Err(DetectorError::ModelLoad(
                        "TensorRT support was not compiled. \
                         Rebuild with: cargo build --features tensorrt"
                            .into(),
                    ));
                }
            }
        };

        session.map_err(|e| DetectorError::ModelLoad(format!("Model load failed: {e}")))
    }

    // ── Private: Forward pass ──────────────────────────────────────

    /// Execute the ONNX graph with a prepared NCHW input tensor.
    ///
    /// Returns the first output as an owned [`DynValue`] which contains
    /// the raw detection tensor of shape `(1, 4+C, N)`.
    ///
    /// Ownership is transferred out of `SessionOutputs` so the
    /// `MutexGuard` can be safely dropped before the caller inspects
    /// the tensor data.
    fn run_inference(
        &self,
        input: Array4<f32>,
    ) -> Result<ort::value::DynValue, DetectorError> {
        // Convert the ndarray into an ort Tensor value.
        let input_tensor = Tensor::from_array(input).map_err(|e| {
            DetectorError::Inference(format!("Failed to create ONNX tensor from ndarray: {e}"))
        })?;

        // Acquire the session lock and run inference.
        let mut session = self.session.lock().map_err(|e| {
            DetectorError::Inference(format!("Session lock poisoned: {e}"))
        })?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| {
                DetectorError::Inference(format!("ONNX Runtime forward pass failed: {e}"))
            })?;

        // Extract the first output value as an owned DynValue.
        // We must bind the result to a local variable before returning
        // so the compiler can verify drop order (outputs before session).
        let first_output = outputs
            .into_iter()
            .next()
            .map(|(_name, value)| value)
            .ok_or_else(|| {
                DetectorError::Inference("Model produced zero output tensors".into())
            });

        first_output
    }
}

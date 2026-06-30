//! CLI thin-client for the YOLOv9e mosquito species detector.
//!
//! This binary contains **zero** inference logic.  It is responsible
//! only for:
//!
//! 1. Parsing command-line arguments.
//! 2. Mapping the `--device` string to [`ComputeDevice`].
//! 3. Constructing a [`YoloDetector`] and calling [`YoloDetector::detect`].
//! 4. Pretty-printing results to stdout.

use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use ai_vision::{ComputeDevice, YoloDetector};

// ──────────────────────────────────────────────
//  CLI Definition
// ──────────────────────────────────────────────

/// YOLOv9e Mosquito Species Detector — CLI Interface
#[derive(Parser, Debug)]
#[command(
    name = "mosquito-detector",
    version,
    about = "Detect and classify 36 mosquito species using a YOLOv9e ONNX model."
)]
struct Cli {
    /// Path to the ONNX model file (.onnx).
    #[arg(short, long)]
    model: String,

    /// Path to the input image (jpg, png, bmp, etc.).
    #[arg(short, long)]
    image: String,

    /// Compute device: `cpu`, `cuda:0`, `cuda:1`, `tensorrt:0`, …
    #[arg(short, long, default_value = "cpu")]
    device: String,

    /// Confidence threshold — detections below this score are discarded.
    #[arg(long, default_value_t = 0.25)]
    conf: f32,

    /// IoU threshold for Non-Maximum Suppression.
    #[arg(long, default_value_t = 0.7)]
    iou: f32,
}

// ──────────────────────────────────────────────
//  Entry Point
// ──────────────────────────────────────────────

fn main() {
    if let Err(err) = run() {
        eprintln!("\n  ✖ Error: {err:#}");
        process::exit(1);
    }
}

/// Actual application logic, separated for clean error propagation.
fn run() -> Result<()> {
    let cli = Cli::parse();

    // ── Parse device string ────────────────────────────────────────
    let device = parse_device(&cli.device)
        .with_context(|| format!("Invalid --device value '{}'", cli.device))?;

    // ── Initialise detector ────────────────────────────────────────
    eprintln!("  ⏳ Loading model: {}", cli.model);
    let detector = YoloDetector::new(&cli.model, device, Some(cli.conf), Some(cli.iou))
        .with_context(|| "Failed to initialise YoloDetector")?;
    eprintln!("  ✔ Model loaded successfully.\n");

    // ── Run inference ──────────────────────────────────────────────
    eprintln!("  ⏳ Running inference on: {}", cli.image);
    let detections = detector
        .detect(&cli.image)
        .with_context(|| "Detection failed")?;
    eprintln!("  ✔ Inference complete — {} detection(s).\n", detections.len());

    // ── Display results ────────────────────────────────────────────
    if detections.is_empty() {
        println!("  No mosquitoes detected.");
        return Ok(());
    }

    print_results(&detections);

    Ok(())
}

// ──────────────────────────────────────────────
//  Helpers
// ──────────────────────────────────────────────

/// Parse a device specification string into a [`ComputeDevice`].
///
/// Accepted formats:
/// - `"cpu"`
/// - `"cuda:N"` where N is a non-negative GPU ordinal
/// - `"tensorrt:N"`
fn parse_device(s: &str) -> Result<ComputeDevice> {
    let s = s.trim().to_lowercase();

    if s == "cpu" {
        return Ok(ComputeDevice::Cpu);
    }

    if let Some(id_str) = s.strip_prefix("cuda:") {
        let id: i32 = id_str
            .parse()
            .with_context(|| format!("Invalid CUDA device id '{id_str}'"))?;
        return Ok(ComputeDevice::Cuda(id));
    }

    if let Some(id_str) = s.strip_prefix("tensorrt:") {
        let id: i32 = id_str
            .parse()
            .with_context(|| format!("Invalid TensorRT device id '{id_str}'"))?;
        return Ok(ComputeDevice::TensorRT(id));
    }

    anyhow::bail!(
        "Unrecognised device '{s}'. Expected one of: cpu, cuda:<N>, tensorrt:<N>"
    );
}

/// Pretty-print detection results as a formatted table to stdout.
fn print_results(detections: &[ai_vision::Detection]) {
    // Column widths
    const W_IDX: usize = 4;
    const W_SPECIES: usize = 38;
    const W_CONF: usize = 12;
    const W_BBOX: usize = 40;

    let line = format!(
        "╠{:═>w1$}╪{:═>w2$}╪{:═>w3$}╪{:═>w4$}╣",
        "", "", "", "",
        w1 = W_IDX, w2 = W_SPECIES, w3 = W_CONF, w4 = W_BBOX,
    );

    let top = format!(
        "╔{:═>w1$}╤{:═>w2$}╤{:═>w3$}╤{:═>w4$}╗",
        "", "", "", "",
        w1 = W_IDX, w2 = W_SPECIES, w3 = W_CONF, w4 = W_BBOX,
    );

    let bottom = format!(
        "╚{:═>w1$}╧{:═>w2$}╧{:═>w3$}╧{:═>w4$}╝",
        "", "", "", "",
        w1 = W_IDX, w2 = W_SPECIES, w3 = W_CONF, w4 = W_BBOX,
    );

    println!("{top}");
    println!(
        "║{:^w1$}│{:^w2$}│{:^w3$}│{:^w4$}║",
        "#", "Species", "Confidence", "BBox [x1, y1, x2, y2]",
        w1 = W_IDX, w2 = W_SPECIES, w3 = W_CONF, w4 = W_BBOX,
    );
    println!("{line}");

    for (i, det) in detections.iter().enumerate() {
        let bbox = format!(
            "[{:.1}, {:.1}, {:.1}, {:.1}]",
            det.x_min, det.y_min, det.x_max, det.y_max,
        );
        println!(
            "║{:>w1$}│ {:<w2$}│{:>w3$.4}│ {:<w4$}║",
            i + 1, det.class_name, det.confidence, bbox,
            w1 = W_IDX, w2 = W_SPECIES - 1, w3 = W_CONF, w4 = W_BBOX - 1,
        );
    }

    println!("{bottom}");
}

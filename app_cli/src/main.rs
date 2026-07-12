//! CLI thin-client for the YOLOv9e mosquito species detector.
//!
//! This binary contains **zero** inference logic. It is responsible
//! only for:
//!
//! 1. Parsing command-line arguments.
//! 2. Mapping the `--device` string to [`ComputeDevice`].
//! 3. Constructing a [`YoloDetector`] and calling [`YoloDetector::detect`].
//! 4. Handling Real-time DroidCam streaming or Single Image inference.

mod iot;

use std::process;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use minifb::{Key, Window, WindowOptions};

use ai_vision::draw::draw_detections;
use ai_vision::stream::MjpegStream;
use ai_vision::{ComputeDevice, YoloDetector};

use crate::iot::{get_species_color, UdpSender, CLASS_ID_NONE};

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
    image: Option<String>,

    /// URL to DroidCam IP Camera (e.g., http://192.168.1.5:4747/video).
    #[arg(short, long)]
    camera: Option<String>,

    /// ESP32 S3 IP Address to send UDP color commands (e.g., 192.168.1.10)
    #[arg(long)]
    esp_ip: Option<String>,

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

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.image.is_none() && cli.camera.is_none() {
        anyhow::bail!("You must specify either --image or --camera.");
    }

    // ── Parse device string ────────────────────────────────────────
    let device = parse_device(&cli.device)
        .with_context(|| format!("Invalid --device value '{}'", cli.device))?;

    // ── Initialise detector ────────────────────────────────────────
    eprintln!("  ⏳ Loading model: {}", cli.model);
    let detector = YoloDetector::new(&cli.model, device, Some(cli.conf), Some(cli.iou))
        .with_context(|| "Failed to initialise YoloDetector")?;
    eprintln!("  ✔ Model loaded successfully.\n");

    if let Some(url) = &cli.camera {
        run_realtime(&detector, url, cli.esp_ip.as_deref())?;
    } else if let Some(image_path) = &cli.image {
        run_single_image(&detector, image_path)?;
    }

    Ok(())
}

// ──────────────────────────────────────────────
//  Real-time Mode (DroidCam)
// ──────────────────────────────────────────────

fn run_realtime(detector: &YoloDetector, url: &str, esp_ip: Option<&str>) -> Result<()> {
    eprintln!("  ⏳ Connecting to DroidCam: {}", url);
    let mut stream = MjpegStream::new(url).with_context(|| "Failed to connect to IP Camera")?;
    eprintln!("  ✔ Connected! Press ESC to exit.");

    // Thiết lập kết nối UDP tới ESP32 (Nếu có cung cấp IP)
    let udp_sender = if let Some(ip) = esp_ip {
        eprintln!("  ⏳ Kết nối UDP đến ESP32 tại {ip}:8888...");
        let sender = UdpSender::new(ip, 8888).with_context(|| "Failed to bind UDP socket")?;
        eprintln!("  ✔ ESP32 IoT Integration: ACTIVE");
        Some(sender)
    } else {
        None
    };

    // Shared State between threads
    let shared_frame = Arc::new(RwLock::new(None::<image::DynamicImage>));
    let shared_detections = Arc::new(RwLock::new(Vec::<ai_vision::types::Detection>::new()));

    let is_running = Arc::new(RwLock::new(true));

    // ── Thread 1: Camera (Network Fetcher) ──
    let frame_clone = Arc::clone(&shared_frame);
    let running_clone = Arc::clone(&is_running);
    thread::spawn(move || {
        while *running_clone.read().unwrap() {
            if let Ok(frame) = stream.next_frame() {
                // Instantly update the latest frame, dropping the old one
                *frame_clone.write().unwrap() = Some(frame);
            }
        }
    });

    // ── Thread 2: AI Inference ──
    // Note: We need a cheap way to share the detector. Since we can't easily move the reference,
    // we assume the detector is just cloned or we can just run the detection here if we pass an Arc.
    // Wait, since detector is a reference in run_realtime, we can't easily pass it to a thread without Arc.
    // Instead of Arc-ing detector, we can use std::thread::scope to spawn threads that borrow from the local stack!
    std::thread::scope(|s| {
        let frame_clone = Arc::clone(&shared_frame);
        let det_clone = Arc::clone(&shared_detections);
        let running_clone = Arc::clone(&is_running);

        s.spawn(move || {
            let inference_interval = Duration::from_millis(200); // 5 FPS
            let mut last_inference_time = Instant::now();

            let action_cooldown = Duration::from_secs(5);
            let mut last_action_time = Instant::now() - action_cooldown; // Allow immediate first action

            while *running_clone.read().unwrap() {
                if last_inference_time.elapsed() >= inference_interval {
                    // Try to get the latest frame
                    let frame_opt = {
                        frame_clone.read().unwrap().clone()
                    };

                    if let Some(frame) = frame_opt {
                        // Run AI without blocking the network or UI
                        if let Ok(detections) = detector.detect_image(&frame) {
                            // In kết quả ra Serial / Terminal
                            if !detections.is_empty() {
                                let species_list: Vec<String> = detections
                                    .iter()
                                    .map(|d| format!("{} ({:.0}%)", d.class_name, d.confidence * 100.0))
                                    .collect();
                                println!("🦟 Đã phát hiện: {}", species_list.join(", "));

                                // Giao tiếp với ESP32 S3 (Điều khiển đèn LED + Servo đĩa xoay)
                                if let Some(ref sender) = udp_sender {
                                    if last_action_time.elapsed() >= action_cooldown {
                                        last_action_time = Instant::now();
                                        // Lấy loài có độ tin cậy cao nhất
                                        let best_match = &detections[0];
                                        let class_id = best_match.class_id as u8;
                                        let (r, g, b) = get_species_color(class_id);
                                        sender.send_command(r, g, b, class_id);
                                    }
                                }
                            } else {
                                // Tắt đèn, giữ nguyên servo
                                if let Some(ref sender) = udp_sender {
                                    if last_action_time.elapsed() >= action_cooldown {
                                        sender.send_command(0, 0, 0, CLASS_ID_NONE);
                                    }
                                }
                            }

                            *det_clone.write().unwrap() = detections;
                        }
                        last_inference_time = Instant::now();
                    } else {
                        thread::sleep(Duration::from_millis(10));
                    }
                } else {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        });

        // ── Thread 3: UI Loop (Main Thread) ──
        // Đợi khung hình đầu tiên để tự động khớp độ phân giải gốc của DroidCam
        let mut initial_w = 640;
        let mut initial_h = 480;
        eprintln!("  ⏳ Đang lấy độ phân giải từ DroidCam...");
        while *is_running.read().unwrap() {
            if let Some(frame) = shared_frame.read().unwrap().as_ref() {
                initial_w = frame.width() as usize;
                initial_h = frame.height() as usize;
                eprintln!("  ✔ Độ phân giải luồng video: {}x{}", initial_w, initial_h);
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        let mut window = Window::new(
            "Mosquito Detector - Live 60FPS (Press ESC to Exit)",
            initial_w,
            initial_h,
            WindowOptions {
                resize: true,
                ..WindowOptions::default()
            },
        ).expect("Failed to create UI window");

        window.limit_update_rate(Some(Duration::from_micros(16600))); // ~60 FPS

        while window.is_open() && !window.is_key_down(Key::Escape) {
            let frame_opt = {
                shared_frame.read().unwrap().clone()
            };
            
            let detections = {
                shared_detections.read().unwrap().clone()
            };

            if let Some(frame) = frame_opt {
                let mut rgb_image = frame.into_rgb8();
                draw_detections(&mut rgb_image, &detections);

                let (width, height) = (rgb_image.width() as usize, rgb_image.height() as usize);
                let mut buffer: Vec<u32> = Vec::with_capacity(width * height);

                for pixel in rgb_image.pixels() {
                    let (r, g, b) = (pixel[0] as u32, pixel[1] as u32, pixel[2] as u32);
                    let argb = (r << 16) | (g << 8) | b;
                    buffer.push(argb);
                }

                window.update_with_buffer(&buffer, width, height).unwrap();
            } else {
                window.update();
            }
        }

        // Signal threads to stop
        *is_running.write().unwrap() = false;
    });

    Ok(())
}

// ──────────────────────────────────────────────
//  Single Image Mode
// ──────────────────────────────────────────────

fn run_single_image(detector: &YoloDetector, image_path: &str) -> Result<()> {
    eprintln!("  ⏳ Running inference on: {}", image_path);
    let detections = detector
        .detect(image_path)
        .with_context(|| "Detection failed")?;
    eprintln!("  ✔ Inference complete — {} detection(s).\n", detections.len());

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

    anyhow::bail!("Unrecognised device '{s}'. Expected one of: cpu, cuda:<N>, tensorrt:<N>");
}

fn print_results(detections: &[ai_vision::types::Detection]) {
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

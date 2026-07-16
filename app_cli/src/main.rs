//! CLI thin-client for the YOLOv9e mosquito species detector.
//!
//! This binary contains **zero** inference logic. It is responsible
//! only for:
//!
//! 1. Parsing command-line arguments.
//! 2. Mapping the `--device` string to [`ComputeDevice`].
//! 3. Constructing a [`YoloDetector`] and calling [`YoloDetector::detect`].
//! 4. Handling Real-time DroidCam streaming or Single Image inference.
//! 5. Hosting the HTTP API server for the Web UI.

mod iot;
mod hardware;
mod api_layer3;
mod web_server;

use std::process;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use std::io::{self, Write};

use anyhow::{Context, Result};
use clap::Parser;
use minifb::{Key, Window, WindowOptions};

use ai_vision::draw::draw_detections;
use ai_vision::{ComputeDevice, YoloDetector};

use crate::iot::{get_species_color, CLASS_ID_NONE};
use crate::hardware::HardwareBackend;

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

    /// Số thứ tự cổng USB Camera cắm dây (Microscope Camera)
    #[arg(long)]
    usb_cam: Option<u32>,

    /// ESP32 S3 IP Address or Hostname (e.g., mosquito-sorter.local)
    #[arg(long)]
    esp_ip: Option<String>,

    /// Chạy chế độ điều khiển bằng PLC công nghiệp thay vì ESP32 S3
    #[arg(long)]
    plc: bool,

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

    // ── Parse device string ────────────────────────────────────────
    let device = parse_device(&cli.device)
        .with_context(|| format!("Invalid --device value '{}'", cli.device))?;

    // ── Initialise detector ────────────────────────────────────────
    eprintln!("  ⏳ Loading model: {}", cli.model);
    let detector = YoloDetector::new(&cli.model, device, Some(cli.conf), Some(cli.iou))
        .with_context(|| "Failed to initialise YoloDetector")?;
    eprintln!("  ✔ Model loaded successfully.\n");

    // ── Lựa chọn nguồn Video (DroidCam vs USB Camera) ──────────────
    let mut camera_url = cli.camera.clone();
    let mut usb_cam_index = cli.usb_cam;

    if cli.image.is_none() && camera_url.is_none() && usb_cam_index.is_none() {
        println!("=====================================================");
        println!("          CHON NGUON CAMERA CUA BAN");
        println!("=====================================================");
        println!("  [1] Su dung DroidCam IP Camera (Ket noi qua WiFi)");
        println!("  [2] Su dung Kinh hien vi / Camera USB cam day");
        println!("-----------------------------------------------------");
        print!("Moi nhap lua chon (1 hoac 2, mac dinh la 1): ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        let choice = choice.trim();

        if choice == "2" {
            print!("Nhap chi so cong USB Camera (Mac dinh: 0): ");
            io::stdout().flush().unwrap();
            let mut cam_idx_str = String::new();
            io::stdin().read_line(&mut cam_idx_str).unwrap();
            let cam_idx: u32 = cam_idx_str.trim().parse().unwrap_or(0);
            usb_cam_index = Some(cam_idx);
            println!("  -> Da chon USB Camera (Index {})", cam_idx);
        } else {
            print!("Nhap IP cua DroidCam (Mac dinh: 192.168.1.5): ");
            io::stdout().flush().unwrap();
            let mut ip = String::new();
            io::stdin().read_line(&mut ip).unwrap();
            let ip = ip.trim();
            let final_ip = if ip.is_empty() { "192.168.1.5" } else { ip };
            camera_url = Some(format!("http://{}:4747/video", final_ip));
            println!("  -> Da chon DroidCam: {}", camera_url.as_ref().unwrap());
        }
    }

    // ── Khởi tạo Hardware Backend (Tầng 2) ────────────────────────
    let backend: Arc<dyn HardwareBackend> = if cli.plc {
        Arc::new(crate::hardware::PlcBackend::new())
    } else {
        let ip_or_host = cli.esp_ip.as_deref().unwrap_or("mosquito-sorter.local");
        
        let resolved_ip = if ip_or_host == "mosquito-sorter.local" {
            // Thử mDNS trước
            if let Some(ip) = try_resolve_mdns(ip_or_host, 8888) {
                Some(ip)
            } else {
                // Nếu mDNS thất bại, thử UDP Broadcast dò tìm IP tự động
                discover_esp32_ip(Duration::from_secs(2))
            }
        } else {
            // Dùng IP do người dùng chủ động điền
            Some(ip_or_host.to_string())
        };

        if let Some(ip) = resolved_ip {
            eprintln!("  ⏳ Khai bao backend ESP32 tai {}:8888...", ip);
            match crate::hardware::Esp32Backend::new(&ip, 8888) {
                Ok(b) => {
                    eprintln!("  ✔ ESP32 IoT Backend: ACTIVE");
                    Arc::new(b)
                }
                Err(e) => {
                    eprintln!("  ✖ Loi khoi tao ket noi ESP32: {}", e);
                    Arc::new(crate::hardware::DisconnectedBackend::new())
                }
            }
        } else {
            Arc::new(crate::hardware::DisconnectedBackend::new())
        }
    };

    let shared_frame = Arc::new(RwLock::new(None::<image::DynamicImage>));
    let shared_detections = Arc::new(RwLock::new(Vec::<ai_vision::types::Detection>::new()));
    let detection_log = Arc::new(RwLock::new(Vec::<crate::web_server::DetectionLogEntry>::new()));
    let is_simulation_mode = Arc::new(AtomicBool::new(false));

    // ── Khởi chạy HTTP API Server cho Web UI (Tầng 3) ──────────────
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = rt.enter();

    crate::web_server::start_server(
        Arc::clone(&backend),
        Arc::clone(&is_simulation_mode),
        Arc::clone(&shared_frame),
        Arc::clone(&shared_detections),
        Arc::clone(&detection_log),
        3000,
    );

    if let Some(image_path) = &cli.image {
        run_single_image(&detector, image_path)?;
    } else {
        run_realtime(
            &detector,
            camera_url,
            usb_cam_index,
            backend,
            is_simulation_mode,
            shared_frame,
            shared_detections,
            detection_log,
        )?;
    }

    Ok(())
}

// ──────────────────────────────────────────────
//  Real-time Mode (DroidCam / USB Camera)
// ──────────────────────────────────────────────

fn run_realtime(
    detector: &YoloDetector,
    camera_url: Option<String>,
    usb_cam_index: Option<u32>,
    backend: Arc<dyn HardwareBackend>,
    is_simulation_mode: Arc<AtomicBool>,
    shared_frame: Arc<RwLock<Option<image::DynamicImage>>>,
    shared_detections: Arc<RwLock<Vec<ai_vision::types::Detection>>>,
    detection_log: Arc<RwLock<Vec<crate::web_server::DetectionLogEntry>>>,
) -> Result<()> {
    eprintln!("  ✔ Dang khoi dong luong Camera. Nhan ESC tren cua so de thoat.");

    let is_running = Arc::new(RwLock::new(true));

    // ── Thread 1: Camera (Fetcher Thread) ──
    let frame_clone = Arc::clone(&shared_frame);
    let running_clone = Arc::clone(&is_running);
    
    // Nokhwa Camera không phai type Send, do do ta khoi tao truc tiep ben trong thread 
    // de khong can phai truyen object qua bien gioi thread
    thread::spawn(move || {
        let mut stream = if let Some(url) = &camera_url {
            match ai_vision::stream::MjpegStream::new(url) {
                Ok(mjpeg) => ai_vision::stream::VideoStream::Mjpeg(mjpeg),
                Err(e) => {
                    eprintln!("  ✖ Loi ket noi DroidCam: {:?}", e);
                    return;
                }
            }
        } else if let Some(index) = usb_cam_index {
            match ai_vision::stream::new_webcam(index) {
                Ok(cam) => ai_vision::stream::VideoStream::Webcam(Box::new(cam)),
                Err(e) => {
                    eprintln!("  ✖ Loi mo USB Camera: {:?}", e);
                    return;
                }
            }
        } else {
            return;
        };

        while *running_clone.read().unwrap() {
            if let Ok(frame) = stream.next_frame() {
                // Ghi de khung hinh moi nhat vao buffer chia se
                *frame_clone.write().unwrap() = Some(frame);
            }
        }
    });

    // ── Thread 2: AI Inference ──
    std::thread::scope(|s| {
        let frame_clone = Arc::clone(&shared_frame);
        let det_clone = Arc::clone(&shared_detections);
        let running_clone = Arc::clone(&is_running);
        let backend_clone = Arc::clone(&backend);
        let sim_mode_clone = Arc::clone(&is_simulation_mode);
        let log_clone = Arc::clone(&detection_log);

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
                            if !detections.is_empty() {
                                let species_list: Vec<String> = detections
                                    .iter()
                                    .map(|d| format!("{} ({:.0}%)", d.class_name, d.confidence * 100.0))
                                    .collect();
                                println!("🦟 Da phat hien: {}", species_list.join(", "));

                                // Chỉ gửi lệnh tự động nếu KHÔNG ở chế độ mô phỏng
                                if !sim_mode_clone.load(Ordering::SeqCst) {
                                    if last_action_time.elapsed() >= action_cooldown {
                                        last_action_time = Instant::now();
                                        let best_match = &detections[0];
                                        let class_id = best_match.class_id as u8;
                                        let (r, g, b) = get_species_color(class_id);
                                        backend_clone.send_mosquito_command(r, g, b, class_id);

                                        // Thêm bản ghi vào lịch sử nhận diện tự động
                                        let mut log = log_clone.write().unwrap();
                                        log.push(crate::web_server::DetectionLogEntry {
                                            timestamp_ms: std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_millis() as u64,
                                            class_id,
                                            species_name: best_match.class_name.to_string(),
                                            confidence: best_match.confidence,
                                            rgb: (r, g, b),
                                            slot: crate::api_layer3::slot_name(class_id as usize),
                                            angle: crate::api_layer3::slot_angle(class_id as usize),
                                        });
                                        if log.len() > 100 {
                                            log.remove(0);
                                        }
                                    }
                                }
                            } else {
                                // Tắt đèn, giữ nguyên servo nếu không mô phỏng
                                if !sim_mode_clone.load(Ordering::SeqCst) {
                                    if last_action_time.elapsed() >= action_cooldown {
                                        backend_clone.send_mosquito_command(0, 0, 0, CLASS_ID_NONE);
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
        // Đợi khung hình đầu tiên để tự động khớp độ phân giải
        let mut initial_w = 640;
        let mut initial_h = 480;
        eprintln!("  ⏳ Dang lay do phan giai camera...");
        while *is_running.read().unwrap() {
            if let Some(frame) = shared_frame.read().unwrap().as_ref() {
                initial_w = frame.width() as usize;
                initial_h = frame.height() as usize;
                eprintln!("  ✔ Do phan giai camera: {}x{}", initial_w, initial_h);
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

// ──────────────────────────────────────────────
//  Auto-Discovery Helpers
// ──────────────────────────────────────────────

fn try_resolve_mdns(hostname: &str, port: u16) -> Option<String> {
    use std::net::ToSocketAddrs;
    let addr = format!("{}:{}", hostname, port);
    if let Ok(mut addrs) = addr.to_socket_addrs() {
        if let Some(socket_addr) = addrs.next() {
            let ip = socket_addr.ip().to_string();
            eprintln!("  ✔ Da phan giai mDNS '{}' thanh IP: {}", hostname, ip);
            return Some(ip);
        }
    }
    None
}

fn discover_esp32_ip(timeout: Duration) -> Option<String> {
    use std::net::UdpSocket;
    eprintln!("  ⏳ Dang do tim thiet bi ESP32 S3 trong mang LAN qua UDP Broadcast (Port 8889)...");
    let socket = UdpSocket::bind("0.0.0.0:8889").ok()?;
    socket.set_read_timeout(Some(timeout)).ok()?;

    let mut buf = [0u8; 64];
    match socket.recv_from(&mut buf) {
        Ok((size, src_addr)) => {
            let msg = String::from_utf8_lossy(&buf[..size]);
            if msg == "mosquito-sorter-beacon" {
                let ip = src_addr.ip().to_string();
                eprintln!("  ✔ Da tu dong phat hien ESP32 tai IP: {}", ip);
                return Some(ip);
            }
        }
        Err(_) => {
            eprintln!("  ⚠ Het thoi gian cho (Timeout) - Khong nhan duoc tin hieu UDP Broadcast tu ESP32.");
        }
    }
    None
}


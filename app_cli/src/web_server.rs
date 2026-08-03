//! Tầng 3 — HTTP API REST Server
//!
//! Khởi tạo Web Server phục vụ giao tiếp giữa Local Web UI và Laptop Backend.

use axum::{
    extract::{State, Json},
    http::StatusCode,
    routing::{get, post},
    response::IntoResponse,
    Router,
};
use async_stream::stream;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::hardware::HardwareBackend;
use crate::api_layer3::{self, slot_index, slot_name, slot_angle};
use crate::iot::get_species_color;

use std::sync::RwLock;

#[derive(Clone, Serialize)]
pub struct DetectionLogEntry {
    pub timestamp_ms: u64,
    pub class_id: u8,
    pub species_name: String,
    pub confidence: f32,
    pub rgb: (u8, u8, u8),
    pub slot: String,
    pub angle: i32,
}

/// State chia sẻ giữa các handler của Axum.
#[derive(Clone)]
pub struct WebState {
    pub backend: Arc<dyn HardwareBackend>,
    pub is_simulation_mode: Arc<AtomicBool>,
    pub shared_frame: Arc<RwLock<Option<image::DynamicImage>>>,
    pub shared_detections: Arc<RwLock<Vec<ai_vision::types::Detection>>>,
    pub detection_log: Arc<RwLock<Vec<DetectionLogEntry>>>,
    pub last_jpeg_cache: Arc<RwLock<Option<Vec<u8>>>>,
    pub reconnect_camera: Arc<AtomicBool>,
    pub frozen_capture_frame: Arc<RwLock<Option<image::DynamicImage>>>,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub mode: String,
    pub current_angle: i32,
    pub backend: String,
}

#[derive(Deserialize)]
pub struct RotateRequest {
    pub degrees: i32,
}

#[derive(Deserialize)]
pub struct RotateSlotRequest {
    pub slot: String,
    pub angle: i32,
}

#[derive(Deserialize)]
pub struct MoveSlotRequest {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize)]
pub struct SimulateRequest {
    pub class_id: u8,
}

#[derive(Serialize)]
pub struct SpeciesInfo {
    pub class_id: u8,
    pub name: String,
    pub slot: String,
    pub angle: i32,
    pub rgb: (u8, u8, u8),
}

#[derive(Deserialize)]
pub struct CaptureSampleRequest {
    pub class_id: u8,
    pub species_name: String,
    pub slot: String,
    pub angle: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AnnotationEntry {
    pub timestamp_ms: u64,
    pub filename: String,
    pub class_id: u8,
    pub species_name: String,
    pub slot: String,
    pub angle: i32,
    pub bbox: [f32; 4],
}

#[derive(Serialize)]
pub struct DatasetStats {
    pub total_samples: usize,
    pub last_captured_ms: u64,
}

// ──────────────────────────────────────────────
//  REST API Handlers
// ──────────────────────────────────────────────

async fn get_status(State(state): State<WebState>) -> Json<StatusResponse> {
    let mode = if state.is_simulation_mode.load(Ordering::SeqCst) {
        "simulation".to_string()
    } else {
        "auto".to_string()
    };

    Json(StatusResponse {
        mode,
        current_angle: state.backend.get_current_angle(),
        backend: state.backend.name().to_string(),
    })
}

async fn set_simulation_mode(State(state): State<WebState>) -> StatusCode {
    state.is_simulation_mode.store(true, Ordering::SeqCst);
    println!("  [Web API] Đã chuyển sang chế độ: MÔ PHỎNG");
    StatusCode::OK
}

async fn set_auto_mode(State(state): State<WebState>) -> StatusCode {
    state.is_simulation_mode.store(false, Ordering::SeqCst);
    println!("  [Web API] Đã chuyển sang chế độ: TỰ ĐỘNG");
    StatusCode::OK
}

async fn post_reset_home(State(state): State<WebState>) -> StatusCode {
    api_layer3::reset_home(&*state.backend);
    StatusCode::OK
}

async fn post_rotate_left(State(state): State<WebState>, Json(payload): Json<RotateRequest>) -> StatusCode {
    api_layer3::rotate_left(&*state.backend, payload.degrees);
    StatusCode::OK
}

async fn post_rotate_right(State(state): State<WebState>, Json(payload): Json<RotateRequest>) -> StatusCode {
    api_layer3::rotate_right(&*state.backend, payload.degrees);
    StatusCode::OK
}

async fn post_rotate_slot_to_angle(
    State(state): State<WebState>,
    Json(payload): Json<RotateSlotRequest>,
) -> StatusCode {
    if let Some(slot_idx) = slot_index(&payload.slot) {
        api_layer3::rotate_slot_to_angle(&*state.backend, slot_idx, payload.angle);
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    }
}

async fn post_move_slot_to_slot(
    State(state): State<WebState>,
    Json(payload): Json<MoveSlotRequest>,
) -> StatusCode {
    if let (Some(from_idx), Some(to_idx)) = (slot_index(&payload.from), slot_index(&payload.to)) {
        api_layer3::move_slot_to_slot(&*state.backend, from_idx, to_idx);
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    }
}

async fn post_simulate_mosquito(
    State(state): State<WebState>,
    Json(payload): Json<SimulateRequest>,
) -> StatusCode {
    if state.is_simulation_mode.load(Ordering::SeqCst) {
        api_layer3::simulate_mosquito(&*state.backend, payload.class_id);

        // Thêm bản ghi vào lịch sử nhận diện giả lập
        let class_id = payload.class_id;
        let (r, g, b) = get_species_color(class_id);
        let name = ai_vision::types::LABELS.get(class_id as usize)
            .copied()
            .unwrap_or("Unknown");

        let mut log = state.detection_log.write().unwrap();
        log.push(DetectionLogEntry {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            class_id,
            species_name: name.to_string(),
            confidence: 1.0, // Mô phỏng giả định 100% confidence
            rgb: (r, g, b),
            slot: slot_name(class_id as usize),
            angle: slot_angle(class_id as usize),
        });
        if log.len() > 100 {
            log.remove(0);
        }

        StatusCode::OK
    } else {
        // Chỉ cho phép mô phỏng khi ở chế độ Simulation Mode
        StatusCode::FORBIDDEN
    }
}

async fn get_species_list() -> Json<Vec<SpeciesInfo>> {
    let mut list = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for (i, name) in ai_vision::types::LABELS.iter().enumerate() {
        let rgb = get_species_color(i as u8);
        seen_ids.insert(i as u8);
        list.push(SpeciesInfo {
            class_id: i as u8,
            name: name.to_string(),
            slot: slot_name(i),
            angle: slot_angle(i),
            rgb,
        });
    }

    if let Ok(content) = std::fs::read_to_string("dataset_collected/annotations.jsonl") {
        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<AnnotationEntry>(line) {
                if !seen_ids.contains(&entry.class_id) {
                    seen_ids.insert(entry.class_id);
                    let rgb = get_species_color(entry.class_id);
                    list.push(SpeciesInfo {
                        class_id: entry.class_id,
                        name: entry.species_name,
                        slot: entry.slot,
                        angle: entry.angle,
                        rgb,
                    });
                }
            }
        }
    }

    Json(list)
}

async fn get_snapshot(State(state): State<WebState>) -> impl axum::response::IntoResponse {
    let frame_opt = state.shared_frame.read().unwrap().clone();
    let detections = state.shared_detections.read().unwrap().clone();
    if let Some(frame) = frame_opt {
        let mut rgb_image = frame.into_rgb8();
        ai_vision::draw::draw_detections(&mut rgb_image, &detections);

        let annotated_frame = image::DynamicImage::ImageRgb8(rgb_image);
        let mut buffer = std::io::Cursor::new(Vec::new());
        if annotated_frame.write_to(&mut buffer, image::ImageFormat::Jpeg).is_ok() {
            let bytes = buffer.into_inner();
            *state.last_jpeg_cache.write().unwrap() = Some(bytes.clone());

            let mut headers = axum::http::HeaderMap::new();
            headers.insert(axum::http::header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
            headers.insert(axum::http::header::CACHE_CONTROL, "no-cache, no-store, must-revalidate".parse().unwrap());
            return (headers, bytes).into_response();
        }
    }

    // Nếu không lấy được frame mới, sử dụng cache JPEG từ khung hình gần nhất
    if let Some(cached_bytes) = state.last_jpeg_cache.read().unwrap().clone() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(axum::http::header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
        headers.insert(axum::http::header::CACHE_CONTROL, "no-cache, no-store, must-revalidate".parse().unwrap());
        return (headers, cached_bytes).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

async fn get_stream(State(state): State<WebState>) -> impl axum::response::IntoResponse {
    let stream = stream! {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(33)); // ~30 FPS
        loop {
            interval.tick().await;
            let frame_bytes_opt = {
                let frame_opt = state.shared_frame.read().unwrap().clone();
                let detections = state.shared_detections.read().unwrap().clone();
                if let Some(frame) = frame_opt {
                    let mut rgb_image = frame.into_rgb8();
                    ai_vision::draw::draw_detections(&mut rgb_image, &detections);
                    let annotated_frame = image::DynamicImage::ImageRgb8(rgb_image);
                    let mut buffer = std::io::Cursor::new(Vec::new());
                    if annotated_frame.write_to(&mut buffer, image::ImageFormat::Jpeg).is_ok() {
                        let bytes = buffer.into_inner();
                        *state.last_jpeg_cache.write().unwrap() = Some(bytes.clone());
                        Some(bytes)
                    } else {
                        state.last_jpeg_cache.read().unwrap().clone()
                    }
                } else {
                    state.last_jpeg_cache.read().unwrap().clone()
                }
            };

            if let Some(bytes) = frame_bytes_opt {
                let header = format!("--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n", bytes.len());
                let mut chunk = Vec::with_capacity(header.len() + bytes.len() + 2);
                chunk.extend_from_slice(header.as_bytes());
                chunk.extend_from_slice(&bytes);
                chunk.extend_from_slice(b"\r\n");
                yield Ok::<_, std::io::Error>(axum::body::Bytes::from(chunk));
            }
        }
    };

    let body = axum::body::Body::from_stream(stream);
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "multipart/x-mixed-replace; boundary=frame".parse().unwrap(),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        "no-cache, no-store, must-revalidate".parse().unwrap(),
    );

    (headers, body).into_response()
}

async fn get_detection_log(State(state): State<WebState>) -> Json<Vec<DetectionLogEntry>> {
    let log = state.detection_log.read().unwrap().clone();
    Json(log)
}

async fn post_capture_sample(
    State(state): State<WebState>,
    Json(payload): Json<CaptureSampleRequest>,
) -> StatusCode {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let filename = format!("sample_{}.jpg", timestamp_ms);
    let img_dir = "dataset_collected/images";
    let _ = std::fs::create_dir_all(img_dir);
    let img_path = format!("{}/{}", img_dir, filename);

    let frame_opt = state
        .frozen_capture_frame
        .write()
        .unwrap()
        .take()
        .or_else(|| state.shared_frame.read().unwrap().clone());
    if let Some(frame) = frame_opt {
        let rgb_image = frame.into_rgb8();
        let dynamic_img = image::DynamicImage::ImageRgb8(rgb_image);
        if let Err(e) = dynamic_img.save(&img_path) {
            eprintln!("  [Active Learning] Lỗi lưu ảnh mẫu: {}", e);
        } else {
            println!("  [Active Learning] Đã lưu ảnh mẫu: {}", img_path);
        }
    } else {
        println!("  [Active Learning] Không có khung hình trong buffer, chỉ ghi metadata.");
    }

    let entry = AnnotationEntry {
        timestamp_ms,
        filename: filename.clone(),
        class_id: payload.class_id,
        species_name: payload.species_name.clone(),
        slot: payload.slot.clone(),
        angle: payload.angle,
        bbox: [0.5, 0.5, 0.6, 0.6],
    };

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("dataset_collected/annotations.jsonl")
    {
        use std::io::Write;
        if let Ok(json_str) = serde_json::to_string(&entry) {
            let _ = writeln!(file, "{}", json_str);
        }
    }

    let slot_idx = slot_index(&payload.slot).unwrap_or(payload.class_id as usize) as u8;
    let (r, g, b) = get_species_color(payload.class_id);
    state.backend.send_mosquito_command(r, g, b, slot_idx);

    let mut log = state.detection_log.write().unwrap();
    log.push(DetectionLogEntry {
        timestamp_ms,
        class_id: payload.class_id,
        species_name: format!("⭐ [RAW] {}", payload.species_name),
        confidence: 1.0,
        rgb: (r, g, b),
        slot: payload.slot.clone(),
        angle: payload.angle,
    });
    if log.len() > 100 {
        log.remove(0);
    }

    StatusCode::OK
}

async fn get_dataset_stats() -> Json<DatasetStats> {
    let mut total_samples = 0;
    let mut last_captured_ms = 0;

    if let Ok(content) = std::fs::read_to_string("dataset_collected/annotations.jsonl") {
        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<AnnotationEntry>(line) {
                total_samples += 1;
                if entry.timestamp_ms > last_captured_ms {
                    last_captured_ms = entry.timestamp_ms;
                }
            }
        }
    }

    Json(DatasetStats {
        total_samples,
        last_captured_ms,
    })
}

async fn get_dataset_export() -> impl axum::response::IntoResponse {
    use std::io::{Write, Read};
    let mut zip_buffer = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let mut all_labels: std::collections::BTreeMap<u32, String> = ai_vision::types::LABELS
            .iter()
            .enumerate()
            .map(|(i, &name)| (i as u32, name.to_string()))
            .collect();

        if let Ok(content) = std::fs::read_to_string("dataset_collected/annotations.jsonl") {
            for line in content.lines() {
                if let Ok(entry) = serde_json::from_str::<AnnotationEntry>(line) {
                    all_labels.entry(entry.class_id as u32).or_insert(entry.species_name.clone());
                }
            }
        }

        let mut yaml_content = String::from("path: ../dataset\ntrain: images\nval: images\n\nnames:\n");
        for (id, name) in &all_labels {
            yaml_content.push_str(&format!("  {}: {}\n", id, name));
        }
        let _ = zip.start_file("data.yaml", options);
        let _ = zip.write_all(yaml_content.as_bytes());

        if let Ok(content) = std::fs::read_to_string("dataset_collected/annotations.jsonl") {
            for line in content.lines() {
                if let Ok(entry) = serde_json::from_str::<AnnotationEntry>(line) {
                    let img_path = format!("dataset_collected/images/{}", entry.filename);
                    if let Ok(mut img_file) = std::fs::File::open(&img_path) {
                        let mut img_bytes = Vec::new();
                        if img_file.read_to_end(&mut img_bytes).is_ok() {
                            let zip_img_name = format!("train/images/{}", entry.filename);
                            let _ = zip.start_file(&zip_img_name, options);
                            let _ = zip.write_all(&img_bytes);
                        }
                    }

                    let txt_filename = entry.filename.replace(".jpg", ".txt").replace(".jpeg", ".txt");
                    let zip_txt_name = format!("train/labels/{}", txt_filename);
                    let [x, y, w, h] = entry.bbox;
                    let label_line = format!("{} {:.6} {:.6} {:.6} {:.6}\n", entry.class_id, x, y, w, h);
                    let _ = zip.start_file(&zip_txt_name, options);
                    let _ = zip.write_all(label_line.as_bytes());
                }
            }
        }
        let _ = zip.finish();
    }

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "application/zip".parse().unwrap(),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        "attachment; filename=\"mosquito_active_dataset.zip\"".parse().unwrap(),
    );

    (headers, zip_buffer).into_response()
}

async fn post_reconnect_camera(State(state): State<WebState>) -> StatusCode {
    println!("  🔄 [API] Yêu cầu kết nối lại Camera từ Web UI...");
    state.reconnect_camera.store(true, Ordering::SeqCst);
    StatusCode::OK
}

async fn post_freeze_sample_frame(State(state): State<WebState>) -> StatusCode {
    let current_frame = state.shared_frame.read().unwrap().clone();
    *state.frozen_capture_frame.write().unwrap() = current_frame;
    println!("  ❄️ [Active Learning] Đã khóa khung hình mẫu từ camera.");
    StatusCode::OK
}

async fn post_unfreeze_sample_frame(State(state): State<WebState>) -> StatusCode {
    *state.frozen_capture_frame.write().unwrap() = None;
    println!("  ▶️ [Active Learning] Đã hủy khóa khung hình mẫu.");
    StatusCode::OK
}

// ──────────────────────────────────────────────
//  Server Launch
// ──────────────────────────────────────────────

/// Chạy HTTP Server trên luồng nền của Tokio.
pub fn start_server(
    backend: Arc<dyn HardwareBackend>,
    is_simulation_mode: Arc<AtomicBool>,
    shared_frame: Arc<RwLock<Option<image::DynamicImage>>>,
    shared_detections: Arc<RwLock<Vec<ai_vision::types::Detection>>>,
    detection_log: Arc<RwLock<Vec<DetectionLogEntry>>>,
    reconnect_camera: Arc<AtomicBool>,
    port: u16,
) {
    let state = WebState {
        backend,
        is_simulation_mode,
        shared_frame,
        shared_detections,
        detection_log,
        last_jpeg_cache: Arc::new(RwLock::new(None)),
        reconnect_camera,
        frozen_capture_frame: Arc::new(RwLock::new(None)),
    };

    // Tạo cấu hình CORS để cho phép Local Web UI giao tiếp
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    use tower_http::services::ServeDir;

    let app = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/mode/simulation", post(set_simulation_mode))
        .route("/api/mode/auto", post(set_auto_mode))
        .route("/api/reset_home", post(post_reset_home))
        .route("/api/rotate_left", post(post_rotate_left))
        .route("/api/rotate_right", post(post_rotate_right))
        .route("/api/rotate_slot_to_angle", post(post_rotate_slot_to_angle))
        .route("/api/move_slot_to_slot", post(post_move_slot_to_slot))
        .route("/api/simulate_mosquito", post(post_simulate_mosquito))
        .route("/api/species_list", get(get_species_list))
        .route("/api/snapshot", get(get_snapshot))
        .route("/api/stream", get(get_stream))
        .route("/api/reconnect_camera", post(post_reconnect_camera))
        .route("/api/freeze_sample_frame", post(post_freeze_sample_frame))
        .route("/api/unfreeze_sample_frame", post(post_unfreeze_sample_frame))
        .route("/api/detection_log", get(get_detection_log))
        .route("/api/capture_sample", post(post_capture_sample))
        .route("/api/dataset/stats", get(get_dataset_stats))
        .route("/api/dataset/export", get(get_dataset_export))
        .fallback_service(ServeDir::new("web_ui/dist"))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    
    // Chạy Server bất đồng bộ trong Tokio Runtime
    tokio::spawn(async move {
        println!("  🌐 HTTP API Server: http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });
}

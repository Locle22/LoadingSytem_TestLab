//! Tầng 3 — HTTP API REST Server
//!
//! Khởi tạo Web Server phục vụ giao tiếp giữa Local Web UI và Laptop Backend.

use axum::{
    extract::{State, Json},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::hardware::HardwareBackend;
use crate::api_layer3::{self, slot_index, slot_name, slot_angle};
use crate::iot::get_species_color;

// ──────────────────────────────────────────────
//  Shared State & DTOs
// ──────────────────────────────────────────────

/// State chia sẻ giữa các handler của Axum.
#[derive(Clone)]
pub struct WebState {
    pub backend: Arc<dyn HardwareBackend>,
    pub is_simulation_mode: Arc<AtomicBool>,
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
        StatusCode::OK
    } else {
        // Chỉ cho phép mô phỏng khi ở chế độ Simulation Mode
        StatusCode::FORBIDDEN
    }
}

async fn get_species_list() -> Json<Vec<SpeciesInfo>> {
    let mut list = Vec::new();
    for (i, name) in ai_vision::types::LABELS.iter().enumerate() {
        let rgb = get_species_color(i as u8);
        list.push(SpeciesInfo {
            class_id: i as u8,
            name: name.to_string(),
            slot: slot_name(i),
            angle: slot_angle(i),
            rgb,
        });
    }
    Json(list)
}

// ──────────────────────────────────────────────
//  Server Launch
// ──────────────────────────────────────────────

/// Chạy HTTP Server trên luồng nền của Tokio.
pub fn start_server(backend: Arc<dyn HardwareBackend>, is_simulation_mode: Arc<AtomicBool>, port: u16) {
    let state = WebState {
        backend,
        is_simulation_mode,
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

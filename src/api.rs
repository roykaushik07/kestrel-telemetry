use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use kestrel_contracts::TelemetryFrame;

/// Per-drone ring of recent frames, trimmed by age. Backs the console live map.
pub struct HotWindow {
    window_ms: u64,
    frames: Mutex<HashMap<String, VecDeque<TelemetryFrame>>>,
}

impl HotWindow {
    pub fn new(window_secs: u64) -> Self {
        Self { window_ms: window_secs * 1000, frames: Mutex::new(HashMap::new()) }
    }

    pub fn push(&self, frame: TelemetryFrame) {
        let mut guard = self.frames.lock().unwrap();
        let ring = guard.entry(frame.drone_id.clone()).or_default();
        let newest = frame.observed_at_ms;
        ring.push_back(frame);
        while ring.front().is_some_and(|f| newest.saturating_sub(f.observed_at_ms) > self.window_ms) {
            ring.pop_front();
        }
    }

    pub fn recent(&self, drone_id: &str) -> Option<Vec<TelemetryFrame>> {
        self.frames.lock().unwrap().get(drone_id).map(|r| r.iter().cloned().collect())
    }

    pub fn latest_all(&self) -> Vec<TelemetryFrame> {
        self.frames.lock().unwrap().values().filter_map(|r| r.back().cloned()).collect()
    }
}

pub async fn serve(bind: &str, window: Arc<HotWindow>) -> std::io::Result<()> {
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/drones", get(list_latest))
        .route("/v1/drones/:id/track", get(track))
        .with_state(window);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await
}

async fn list_latest(State(w): State<Arc<HotWindow>>) -> Json<Vec<TelemetryFrame>> {
    Json(w.latest_all())
}

async fn track(
    State(w): State<Arc<HotWindow>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<TelemetryFrame>>, StatusCode> {
    w.recent(&id).map(Json).ok_or(StatusCode::NOT_FOUND)
}

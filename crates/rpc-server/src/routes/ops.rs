use crate::state::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(health_check))
        .route("/readyz", get(readiness_check))
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json("ok"))
}

async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    if state.storage.rocksdb.path().exists() {
        (StatusCode::OK, Json("ready"))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json("database_not_found"))
    }
}


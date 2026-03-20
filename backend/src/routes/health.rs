use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::types::api::HealthResponse;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health_check))
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let db_status = match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => "connected".to_string(),
        Err(e) => format!("error: {e}"),
    };

    Json(HealthResponse {
        status: "ok".to_string(),
        db: db_status,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

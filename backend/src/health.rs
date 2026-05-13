use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::AppState;

pub async fn handler(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.pg).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "status": "ok", "db": "ok" })),
        ),
        Err(e) => {
            tracing::error!("Health check DB error: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "degraded", "db": "error" })),
            )
        }
    }
}

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{auth::AuthUser, AppState};

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RecordCookBody {
    pub recipe_id: i64,
    pub recipe_name: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub recipe_id: i64,
    pub recipe_name: String,
    pub cooked_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryParams {
    pub search: Option<String>,
    /// "week" | "month" | "all" (default: all, but always last 60 days)
    pub filter: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /history — record a cook event for the authenticated user.
pub async fn record_cook(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<RecordCookBody>,
) -> impl IntoResponse {
    let result = sqlx::query!(
        r#"
        INSERT INTO cook_history (user_id, recipe_id, recipe_name)
        VALUES ($1, $2, $3)
        RETURNING id, cooked_at
        "#,
        user_id,
        body.recipe_id,
        body.recipe_name,
    )
    .fetch_one(&state.pg)
    .await;

    match result {
        Ok(row) => (
            StatusCode::CREATED,
            Json(json!({ "id": row.id, "cooked_at": row.cooked_at })),
        ),
        Err(e) => {
            tracing::error!("record_cook error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to record cook" })),
            )
        }
    }
}

/// GET /history — list cook history for the authenticated user.
/// Always scoped to last 60 days. Optional search and time-range filter.
pub async fn list_history(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Query(params): Query<HistoryParams>,
) -> impl IntoResponse {
    // Determine the time cutoff based on filter
    let interval = match params.filter.as_deref() {
        Some("week") => "7 days",
        Some("month") => "30 days",
        _ => "60 days",
    };

    let search_pattern = params
        .search
        .as_deref()
        .map(|s| format!("%{}%", s));

    // Build cutoff timestamp
    let cutoff_sql = format!(
        "SELECT NOW() - INTERVAL '{}'",
        interval.replace('\'', "") // sanitised — only our own constant values
    );
    let cutoff: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(&cutoff_sql)
        .fetch_one(&state.pg)
        .await
        .unwrap_or_else(|_| chrono::Utc::now() - chrono::Duration::days(60));

    let rows: Result<Vec<HistoryEntry>, _> = if let Some(ref pattern) = search_pattern {
        sqlx::query_as!(
            HistoryEntry,
            r#"
            SELECT id, recipe_id, recipe_name, cooked_at
            FROM cook_history
            WHERE user_id = $1
              AND cooked_at > $2
              AND recipe_name ILIKE $3
            ORDER BY cooked_at DESC
            "#,
            user_id,
            cutoff,
            pattern,
        )
        .fetch_all(&state.pg)
        .await
    } else {
        sqlx::query_as!(
            HistoryEntry,
            r#"
            SELECT id, recipe_id, recipe_name, cooked_at
            FROM cook_history
            WHERE user_id = $1
              AND cooked_at > $2
            ORDER BY cooked_at DESC
            "#,
            user_id,
            cutoff,
        )
        .fetch_all(&state.pg)
        .await
    };

    match rows {
        Ok(entries) => {
            let count = entries.len();
            (StatusCode::OK, Json(json!({ "entries": entries, "count": count })))
        }
        Err(e) => {
            tracing::error!("list_history error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to fetch history" })),
            )
        }
    }
}

/// DELETE /history/:id — delete a single history entry (must be owned by user).
pub async fn delete_history_entry(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let result = sqlx::query!(
        "DELETE FROM cook_history WHERE id = $1 AND user_id = $2",
        id,
        user_id,
    )
    .execute(&state.pg)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "entry not found" })),
        ),
        Ok(_) => (StatusCode::NO_CONTENT, Json(json!(null))),
        Err(e) => {
            tracing::error!("delete_history_entry error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to delete" })),
            )
        }
    }
}

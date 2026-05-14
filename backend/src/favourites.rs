use axum::{
    extract::{Path, State},
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
pub struct FavouriteBody {
    pub recipe_id: i64,
    pub recipe_name: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FavouriteEntry {
    pub id: Uuid,
    pub recipe_id: i64,
    pub recipe_name: String,
    pub saved_at: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /favourites — save a recipe (idempotent).
pub async fn add_favourite(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<FavouriteBody>,
) -> impl IntoResponse {
    let result = sqlx::query!(
        r#"
        INSERT INTO favourites (user_id, recipe_id, recipe_name)
        VALUES ($1, $2, $3)
        ON CONFLICT ON CONSTRAINT favourites_user_recipe_unique DO NOTHING
        RETURNING id, saved_at
        "#,
        user_id,
        body.recipe_id,
        body.recipe_name,
    )
    .fetch_optional(&state.pg)
    .await;

    match result {
        Ok(Some(row)) => (
            StatusCode::CREATED,
            Json(json!({ "id": row.id, "saved_at": row.saved_at })),
        ),
        Ok(None) => {
            // Already exists — return 200
            (
                StatusCode::OK,
                Json(json!({ "message": "already saved" })),
            )
        }
        Err(e) => {
            tracing::error!("add_favourite error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to save favourite" })),
            )
        }
    }
}

/// DELETE /favourites/:recipe_id — remove a saved recipe.
pub async fn remove_favourite(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(recipe_id): Path<i64>,
) -> impl IntoResponse {
    let result = sqlx::query!(
        "DELETE FROM favourites WHERE user_id = $1 AND recipe_id = $2",
        user_id,
        recipe_id,
    )
    .execute(&state.pg)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "favourite not found" })),
        ),
        Ok(_) => (StatusCode::NO_CONTENT, Json(json!(null))),
        Err(e) => {
            tracing::error!("remove_favourite error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to remove favourite" })),
            )
        }
    }
}

/// GET /favourites — list all favourites for the authenticated user.
pub async fn list_favourites(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> impl IntoResponse {
    let rows: Result<Vec<FavouriteEntry>, _> = sqlx::query_as!(
        FavouriteEntry,
        r#"
        SELECT id, recipe_id, recipe_name, saved_at
        FROM favourites
        WHERE user_id = $1
        ORDER BY saved_at DESC
        "#,
        user_id,
    )
    .fetch_all(&state.pg)
    .await;

    match rows {
        Ok(favourites) => {
            let count = favourites.len();
            (StatusCode::OK, Json(json!({ "favourites": favourites, "count": count })))
        }
        Err(e) => {
            tracing::error!("list_favourites error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "failed to fetch favourites" })),
            )
        }
    }
}

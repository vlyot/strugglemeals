use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use sqlx::PgPool;

use crate::AppState;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

pub enum AuthError {
    MissingToken,
    InvalidToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "missing token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "invalid token"),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

// ---------------------------------------------------------------------------
// AuthUser extractor
// Reads x-stack-refresh-token header and looks up the session in neon_auth.session.
// Returns AuthUser(user_id) where user_id is a UUID string.
// ---------------------------------------------------------------------------

pub struct AuthUser(pub String);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("x-stack-refresh-token")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        let user_id = validate_session_token(token, &state.pg)
            .await
            .map_err(|_| AuthError::InvalidToken)?;

        Ok(AuthUser(user_id))
    }
}

// ---------------------------------------------------------------------------
// Session token validation
// Looks up the token in neon_auth.session, checks expiry, returns user_id.
// ---------------------------------------------------------------------------

pub async fn validate_session_token(
    token: &str,
    pg: &PgPool,
) -> Result<String, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT "userId"::text AS user_id
        FROM neon_auth.session
        WHERE token = $1
          AND "expiresAt" > NOW()
        LIMIT 1
        "#,
        token
    )
    .fetch_one(pg)
    .await?;

    Ok(row.user_id.expect("userId is non-null in session table"))
}

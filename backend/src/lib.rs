pub mod ai;
pub mod auth;
pub mod favourites;
pub mod health;
pub mod history;
pub mod recipes;

use r2d2_sqlite::SqliteConnectionManager;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::Semaphore;

pub type SqlitePool = Arc<r2d2::Pool<SqliteConnectionManager>>;

#[derive(Clone)]
pub struct AppState {
    pub pg: sqlx::PgPool,
    pub sqlite: SqlitePool,
    pub http: reqwest::Client,
    pub gemini_api_key: String,
    pub groq_api_key: String,
    /// Set to true once the FTS5 recipes_fts table is fully populated.
    /// While false, fetch_candidates falls back to the json_each scan.
    pub fts_ready: Arc<AtomicBool>,
    /// Token-bucket rate limiter for POST /ai/identify-ingredients.
    /// Permits = GEMINI_RATE_LIMIT_RPM (default 10). Refilled every 60 s.
    pub gemini_limiter: Arc<Semaphore>,
}

// Allow axum's FromRef to extract SqlitePool from AppState (used by recipes handler)
impl axum::extract::FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.sqlite.clone()
    }
}

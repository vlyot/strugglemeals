pub mod ai;
pub mod auth;
pub mod favourites;
pub mod health;
pub mod history;
pub mod recipes;

use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

pub type SqlitePool = Arc<r2d2::Pool<SqliteConnectionManager>>;

#[derive(Clone)]
pub struct AppState {
    pub pg: sqlx::PgPool,
    pub sqlite: SqlitePool,
    pub http: reqwest::Client,
    pub gemini_api_key: String,
    pub groq_api_key: String,
}

// Allow axum's FromRef to extract SqlitePool from AppState (used by recipes handler)
impl axum::extract::FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.sqlite.clone()
    }
}

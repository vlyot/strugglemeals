mod health;
mod recipes;
mod upload;

use axum::{Router, extract::DefaultBodyLimit, routing::{get, post}};
use dotenvy::dotenv;
use r2d2_sqlite::SqliteConnectionManager;
use recipes::SqlitePool;
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub pg: sqlx::PgPool,
    pub sqlite: SqlitePool,
}

#[tokio::main]
async fn main() {
    let _ = dotenv();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "backend=debug,tower_http=debug,axum=debug".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let sqlite_path = env::var("SQLITE_PATH").unwrap_or_else(|_| "/data/recipes.db".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    tracing::info!("Connecting to Postgres...");
    let pg = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to Neon Postgres");

    tracing::info!("Opening SQLite at {sqlite_path}...");
    let manager = SqliteConnectionManager::file(&sqlite_path);
    let sqlite = Arc::new(
        r2d2::Pool::builder()
            .max_size(8)
            .build(manager)
            .expect("Failed to open SQLite pool"),
    );
    tracing::info!("SQLite ready");

    let state = AppState { pg, sqlite };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health::handler))
        .route("/recipes/search", get(recipes::search))
        .route("/recipes/{id}", get(recipes::get_one))
        .route("/internal/upload-db", post(upload::handler))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024 * 1024)) // 2 GB
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("Server listening on {addr}");
    axum::serve(listener, app).await.expect("Server error");
}

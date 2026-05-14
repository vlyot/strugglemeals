use backend::{ai, favourites, health, history, recipes, AppState};
use axum::{
    Router,
    http::{HeaderName, HeaderValue, Method},
    routing::{delete, get, post},
};
use dotenvy::dotenv;
use r2d2_sqlite::SqliteConnectionManager;
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc};
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    let gemini_api_key = env::var("GEMINI_API_KEY").unwrap_or_default();
    let groq_api_key = env::var("GROQ_API_KEY").unwrap_or_default();
    let frontend_url = env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());

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

    let http = reqwest::Client::new();

    let state = AppState { pg, sqlite, http, gemini_api_key, groq_api_key };

    // Support multiple comma-separated origins in FRONTEND_URL
    let origins: Vec<HeaderValue> = frontend_url
        .split(',')
        .map(|s| s.trim())
        .filter_map(|s| s.parse().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("x-stack-refresh-token"),
            HeaderName::from_static("x-stack-access-token"),
        ])
        .allow_credentials(true);

    let app = Router::new()
        // Public
        .route("/health", get(health::handler))
        .route("/recipes/search", get(recipes::search))
        .route("/recipes/{id}", get(recipes::get_one))
        .route("/ai/identify-ingredients", post(ai::identify_ingredients))
        .route("/ai/present-recipe", post(ai::present_recipe))
        .route("/ai/theme-shortlist", post(ai::theme_shortlist))
        // Auth-gated
        .route("/history", post(history::record_cook))
        .route("/history", get(history::list_history))
        .route("/history/{id}", delete(history::delete_history_entry))
        .route("/favourites", post(favourites::add_favourite))
        .route("/favourites/{recipe_id}", delete(favourites::remove_favourite))
        .route("/favourites", get(favourites::list_favourites))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("Server listening on {addr}");
    axum::serve(listener, app).await.expect("Server error");
}

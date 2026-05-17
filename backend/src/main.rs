use backend::{ai, favourites, health, history, recipes, AppState};
use axum::{
    Router,
    http::{HeaderName, HeaderValue, Method},
    routing::{delete, get, post},
};
use dotenvy::dotenv;
use r2d2_sqlite::SqliteConnectionManager;
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::{atomic::{AtomicBool, Ordering}, Arc}};
use tokio::sync::Semaphore;
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
    // Apply performance pragmas once at startup.
    // journal_mode returns the active mode as a result row, so pragma_update
    // (which calls execute_batch) fails with ExecuteReturnedResults.
    // pragma_update_and_check uses query_row internally and handles it correctly.
    // synchronous and cache_size do not return rows — pragma_update is fine.
    {
        let conn = sqlite.get().expect("Failed to get SQLite connection for pragmas");
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))
            .expect("Failed to set WAL mode");
        conn.pragma_update(None, "synchronous", "NORMAL")
            .expect("Failed to set synchronous=NORMAL");
        conn.pragma_update(None, "cache_size", -65536_i64)
            .expect("Failed to set cache_size");
        tracing::info!("SQLite pragmas applied (WAL mode, 64MB page cache)");
    }

    let http = reqwest::Client::new();
    let fts_ready = Arc::new(AtomicBool::new(false));

    // Spawn FTS5 migration in background so the server accepts traffic immediately.
    // Once complete, fts_ready flips to true and all subsequent recipe searches
    // use the fast FTS5 MATCH path instead of the json_each full-table scan.
    {
        let fts_sqlite = sqlite.clone();
        let fts_flag = fts_ready.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let conn = fts_sqlite.get().expect("sqlite conn for fts migration");

                let fts_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='recipes_fts'",
                    [],
                    |row| row.get(0),
                ).unwrap_or(0);
                let recipe_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM recipes", [], |row| row.get(0),
                ).unwrap_or(0);
                let fts_row_count: i64 = if fts_count > 0 {
                    conn.query_row("SELECT COUNT(*) FROM recipes_fts", [], |row| row.get(0)).unwrap_or(0)
                } else { 0 };

                if fts_count == 0 || fts_row_count < recipe_count {
                    tracing::info!("Running FTS5 migration ({recipe_count} recipes)...");
                    // Plain FTS5 table — stores its own copy of ingredients_text.
                    // Content tables (content='recipes') require a special rebuild
                    // flow and cannot be populated with a direct INSERT...SELECT.
                    conn.execute_batch(
                        "CREATE VIRTUAL TABLE IF NOT EXISTS recipes_fts
                             USING fts5(
                                 ingredients_text,
                                 tokenize='unicode61 remove_diacritics 1'
                             );",
                    ).expect("FTS5 create failed");
                    conn.execute_batch(
                        "INSERT INTO recipes_fts(rowid, ingredients_text)
                         SELECT r.id,
                                (SELECT group_concat(je.value, ' ')
                                 FROM json_each(r.ingredients_core) je)
                         FROM recipes r
                         WHERE r.id NOT IN (SELECT rowid FROM recipes_fts);",
                    ).expect("FTS5 populate failed");
                    tracing::info!("FTS5 migration complete.");
                } else {
                    tracing::info!("FTS5 table already present ({fts_row_count} rows), skipping migration.");
                }
            }).await;

            match result {
                Ok(()) => fts_flag.store(true, Ordering::Relaxed),
                Err(e) => tracing::error!("FTS5 migration task panicked: {e:?}"),
            }
        });
    }

    let rpm_limit: usize = env::var("GEMINI_RATE_LIMIT_RPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let gemini_limiter = Arc::new(Semaphore::new(rpm_limit));
    {
        let limiter = gemini_limiter.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                let current = limiter.available_permits();
                let to_add = rpm_limit.saturating_sub(current);
                if to_add > 0 {
                    limiter.add_permits(to_add);
                }
            }
        });
    }
    tracing::info!("Gemini rate limit: {rpm_limit} RPM");

    let state = AppState { pg, sqlite, http, gemini_api_key, groq_api_key, fts_ready, gemini_limiter };

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

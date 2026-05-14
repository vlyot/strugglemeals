/// Integration tests for auth, history, and favourites endpoints.
/// Run with:
///   DATABASE_URL=... cargo test --test auth_integration
///
/// These tests spin up the full Axum router against a real Neon Postgres DB.
/// A test session token is injected directly into neon_auth.session.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backend::AppState;
use r2d2_sqlite::SqliteConnectionManager;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower::ServiceExt; // for oneshot

async fn build_app() -> (axum::Router, sqlx::PgPool, String) {
    let db_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for integration tests");

    let pg = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect to Postgres");

    // Use an in-memory SQLite for tests (recipes not needed)
    let manager = SqliteConnectionManager::memory();
    let sqlite = Arc::new(r2d2::Pool::builder().max_size(2).build(manager).unwrap());

    let state = AppState {
        pg: pg.clone(),
        sqlite,
        http: reqwest::Client::new(),
        gemini_api_key: String::new(),
        groq_api_key: String::new(),
    };

    // Insert a test session into neon_auth.session
    let test_user_id = uuid::Uuid::new_v4();
    let test_token = format!("test_token_{}", uuid::Uuid::new_v4());

    sqlx::query!(
        r#"
        INSERT INTO neon_auth.session (id, token, "userId", "expiresAt", "createdAt", "updatedAt")
        VALUES (gen_random_uuid(), $1, $2, NOW() + INTERVAL '1 hour', NOW(), NOW())
        "#,
        test_token,
        test_user_id,
    )
    .execute(&pg)
    .await
    .expect("Failed to insert test session");

    use axum::routing::{delete, get, post};
    let app = axum::Router::new()
        .route("/history", post(backend::history::record_cook))
        .route("/history", get(backend::history::list_history))
        .route("/history/{id}", delete(backend::history::delete_history_entry))
        .route("/favourites", post(backend::favourites::add_favourite))
        .route("/favourites/{recipe_id}", delete(backend::favourites::remove_favourite))
        .route("/favourites", get(backend::favourites::list_favourites))
        .with_state(state);

    (app, pg, test_token)
}

async fn cleanup(pg: &sqlx::PgPool, token: &str) {
    // Remove test session and any data it created
    sqlx::query!(
        r#"DELETE FROM neon_auth.session WHERE token = $1"#,
        token
    )
    .execute(pg)
    .await
    .ok();

    // Clean up history and favourites created by test user
    sqlx::query!("DELETE FROM cook_history WHERE recipe_name LIKE 'test_%'")
        .execute(pg)
        .await
        .ok();
    sqlx::query!("DELETE FROM favourites WHERE recipe_name LIKE 'test_%'")
        .execute(pg)
        .await
        .ok();
}

#[tokio::test]
async fn test_no_token_returns_401() {
    let (app, pg, token) = build_app().await;

    let res = app
        .oneshot(Request::get("/history").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    cleanup(&pg, &token).await;
}

#[tokio::test]
async fn test_invalid_token_returns_401() {
    let (app, pg, token) = build_app().await;

    let res = app
        .oneshot(
            Request::get("/history")
                .header("x-stack-refresh-token", "invalid_token_xyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    cleanup(&pg, &token).await;
}

#[tokio::test]
async fn test_record_cook_inserts_and_returns_201() {
    let (app, pg, token) = build_app().await;

    let res = app
        .oneshot(
            Request::post("/history")
                .header("x-stack-refresh-token", &token)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"recipe_id": 999999, "recipe_name": "test_fried_rice"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    cleanup(&pg, &token).await;
}

#[tokio::test]
async fn test_list_history_returns_ok_for_valid_user() {
    let (app, pg, token) = build_app().await;

    let res = app
        .oneshot(
            Request::get("/history")
                .header("x-stack-refresh-token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    cleanup(&pg, &token).await;
}

#[tokio::test]
async fn test_add_favourite_returns_201() {
    let (app, pg, token) = build_app().await;

    let res = app
        .oneshot(
            Request::post("/favourites")
                .header("x-stack-refresh-token", &token)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"recipe_id": 888888, "recipe_name": "test_pasta"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    cleanup(&pg, &token).await;
}

#[tokio::test]
async fn test_add_favourite_idempotent() {
    let (app, pg, token) = build_app().await;

    // First insert
    let body = r#"{"recipe_id": 777777, "recipe_name": "test_idempotent_pasta"}"#;

    let (app2, _, _) = build_app().await; // second call needs fresh router

    let res1 = app
        .oneshot(
            Request::post("/favourites")
                .header("x-stack-refresh-token", &token)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let res2 = app2
        .oneshot(
            Request::post("/favourites")
                .header("x-stack-refresh-token", &token)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res1.status(), StatusCode::CREATED);
    assert_eq!(res2.status(), StatusCode::OK); // already saved
    cleanup(&pg, &token).await;
}

#[tokio::test]
async fn test_list_favourites_returns_ok() {
    let (app, pg, token) = build_app().await;

    let res = app
        .oneshot(
            Request::get("/favourites")
                .header("x-stack-refresh-token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    cleanup(&pg, &token).await;
}

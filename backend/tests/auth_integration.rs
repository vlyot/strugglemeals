/// Integration tests for auth, history, and favourites endpoints.
/// Run with:
///   DATABASE_URL=... cargo test --test auth_integration -- --test-threads=1

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{delete, get, post},
    Router,
};
use backend::{favourites, history, AppState};
use r2d2_sqlite::SqliteConnectionManager;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower::ServiceExt;

async fn build_app() -> (Router, sqlx::PgPool, String, String) {
    let db_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for integration tests");

    let pg = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect to Postgres");

    let manager = SqliteConnectionManager::memory();
    let sqlite = Arc::new(r2d2::Pool::builder().max_size(2).build(manager).unwrap());

    let state = AppState {
        pg: pg.clone(),
        sqlite,
        http: reqwest::Client::new(),
        gemini_api_key: String::new(),
        groq_api_key: String::new(),
    };

    let test_user_id = uuid::Uuid::new_v4();
    let test_email = format!("test_{}@example.com", test_user_id);
    let test_name = format!("Test User {}", test_user_id);
    sqlx::query!(
        r#"INSERT INTO neon_auth."user" (id, name, email, "emailVerified", "createdAt", "updatedAt", role) VALUES ($1, $2, $3, false, NOW(), NOW(), 'user')"#,
        test_user_id,
        test_name,
        test_email,
    )
    .execute(&pg)
    .await
    .expect("Failed to insert test user");

    let test_token = format!("test_token_{}", uuid::Uuid::new_v4());
    sqlx::query!(
        r#"INSERT INTO neon_auth.session (id, token, "userId", "expiresAt", "createdAt", "updatedAt") VALUES (gen_random_uuid(), $1, $2, NOW() + INTERVAL '1 hour', NOW(), NOW())"#,
        test_token,
        test_user_id,
    )
    .execute(&pg)
    .await
    .expect("Failed to insert test session");

    let app = Router::new()
        .route("/history", post(history::record_cook))
        .route("/history", get(history::list_history))
        .route("/history/{id}", delete(history::delete_history_entry))
        .route("/favourites", post(favourites::add_favourite))
        .route("/favourites/{recipe_id}", delete(favourites::remove_favourite))
        .route("/favourites", get(favourites::list_favourites))
        .with_state(state);

    (app, pg, test_token, test_user_id.to_string())
}

async fn cleanup(pg: &sqlx::PgPool, token: &str, user_id: &str) {
    sqlx::query!("DELETE FROM cook_history WHERE recipe_name LIKE 'test_%'").execute(pg).await.ok();
    sqlx::query!("DELETE FROM favourites WHERE recipe_name LIKE 'test_%'").execute(pg).await.ok();
    sqlx::query!("DELETE FROM neon_auth.session WHERE token = $1", token).execute(pg).await.ok();
    let uid: uuid::Uuid = user_id.parse().unwrap();
    sqlx::query!("DELETE FROM neon_auth.\"user\" WHERE id = $1", uid).execute(pg).await.ok();
}

#[tokio::test]
async fn test_no_token_returns_401() {
    let (app, pg, token, uid) = build_app().await;
    let res = app.oneshot(Request::get("/history").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    cleanup(&pg, &token, &uid).await;
}

#[tokio::test]
async fn test_invalid_token_returns_401() {
    let (app, pg, token, uid) = build_app().await;
    let res = app.oneshot(
        Request::get("/history").header("x-stack-refresh-token", "bogus_token").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    cleanup(&pg, &token, &uid).await;
}

#[tokio::test]
async fn test_record_cook_inserts_and_returns_201() {
    let (app, pg, token, uid) = build_app().await;
    let res = app.oneshot(
        Request::post("/history")
            .header("x-stack-refresh-token", &token)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"recipe_id":999999,"recipe_name":"test_fried_rice"}"#))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    cleanup(&pg, &token, &uid).await;
}

#[tokio::test]
async fn test_list_history_returns_ok_for_valid_user() {
    let (app, pg, token, uid) = build_app().await;
    let res = app.oneshot(
        Request::get("/history").header("x-stack-refresh-token", &token).body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    cleanup(&pg, &token, &uid).await;
}

#[tokio::test]
async fn test_add_favourite_returns_201() {
    let (app, pg, token, uid) = build_app().await;
    let res = app.oneshot(
        Request::post("/favourites")
            .header("x-stack-refresh-token", &token)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"recipe_id":888888,"recipe_name":"test_pasta"}"#))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    cleanup(&pg, &token, &uid).await;
}

#[tokio::test]
async fn test_add_favourite_idempotent() {
    let (app, pg, token, uid) = build_app().await;
    let body = r#"{"recipe_id":777777,"recipe_name":"test_idempotent"}"#;
    let res1 = app.clone().oneshot(
        Request::post("/favourites")
            .header("x-stack-refresh-token", &token)
            .header("content-type", "application/json")
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();
    let res2 = app.oneshot(
        Request::post("/favourites")
            .header("x-stack-refresh-token", &token)
            .header("content-type", "application/json")
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();
    assert_eq!(res1.status(), StatusCode::CREATED);
    assert_eq!(res2.status(), StatusCode::OK);
    cleanup(&pg, &token, &uid).await;
}

#[tokio::test]
async fn test_list_favourites_returns_ok() {
    let (app, pg, token, uid) = build_app().await;
    let res = app.oneshot(
        Request::get("/favourites").header("x-stack-refresh-token", &token).body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    cleanup(&pg, &token, &uid).await;
}

#[tokio::test]
async fn test_list_history_isolated_per_user() {
    let (app_a, pg, tok_a, uid_a) = build_app().await;
    let (app_b, _, tok_b, uid_b) = build_app().await;

    app_a.clone().oneshot(
        Request::post("/history")
            .header("x-stack-refresh-token", &tok_a)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"recipe_id":111111,"recipe_name":"test_user_a_secret"}"#))
            .unwrap(),
    ).await.unwrap();

    let res = app_b.oneshot(
        Request::get("/history").header("x-stack-refresh-token", &tok_b).body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let has_a = json["entries"].as_array().unwrap()
        .iter().any(|e| e["recipe_name"].as_str() == Some("test_user_a_secret"));
    assert!(!has_a, "User B must not see user A's history");

    cleanup(&pg, &tok_a, &uid_a).await;
    cleanup(&pg, &tok_b, &uid_b).await;
}

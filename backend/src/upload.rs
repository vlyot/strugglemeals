use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::env;
use tokio::{fs, io::AsyncWriteExt};

use crate::AppState;

pub async fn handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Token check
    let expected = match env::var("UPLOAD_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "UPLOAD_TOKEN not configured" })),
            );
        }
    };

    let mut token_ok = false;
    let mut bytes_written: u64 = 0;

    // We need to write to a temp file first, then atomically rename
    let sqlite_path = env::var("SQLITE_PATH").unwrap_or_else(|_| "/data/recipes.db".to_string());
    let tmp_path = format!("{sqlite_path}.tmp");

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "token" {
            let value = field.text().await.unwrap_or_default();
            if value == expected {
                token_ok = true;
            } else {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({ "error": "invalid token" })),
                );
            }
            continue;
        }

        if name == "file" {
            if !token_ok {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({ "error": "token must come before file field" })),
                );
            }

            let mut file = match fs::File::create(&tmp_path).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("Failed to create tmp file: {e}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "error": "failed to create file" })),
                    );
                }
            };

            // Stream chunks directly to disk — no full-memory buffering
            let mut field = field;
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        if let Err(e) = file.write_all(&chunk).await {
                            tracing::error!("Write error: {e}");
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({ "error": "write failed" })),
                            );
                        }
                        bytes_written += chunk.len() as u64;
                    }
                    Ok(None) => break,
                    Err(e) => {
                        tracing::error!("Multipart read error: {e}");
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({ "error": "read failed" })),
                        );
                    }
                }
            }

            if let Err(e) = file.flush().await {
                tracing::error!("Flush error: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "flush failed" })),
                );
            }
        }
    }

    if !token_ok {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "token missing" })),
        );
    }

    if bytes_written == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "no file data received" })),
        );
    }

    // Atomic rename: replace live DB only after full write succeeds
    if let Err(e) = fs::rename(&tmp_path, &sqlite_path).await {
        tracing::error!("Rename error: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "rename failed" })),
        );
    }

    // Drop all SQLite connections so the pool reopens against the new file
    // The pool will reconnect lazily on next request
    drop(state.sqlite);

    tracing::info!("DB upload complete: {bytes_written} bytes written to {sqlite_path}");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "bytes_written": bytes_written,
            "path": sqlite_path,
        })),
    )
}

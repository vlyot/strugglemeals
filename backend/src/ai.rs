use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::AppState;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// Theme assigned by Groq based on recipe content.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Theme {
    Light,
    Filling,
    Quick,
}

/// Difficulty level assigned by Groq.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

// ---------------------------------------------------------------------------
// POST /ai/identify-ingredients
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct IdentifyRequest {
    /// Base64-encoded image bytes.
    pub image_base64: String,
    /// MIME type, e.g. "image/jpeg" or "image/png".
    #[serde(default = "default_mime")]
    pub mime_type: String,
}

fn default_mime() -> String {
    "image/jpeg".to_string()
}

#[derive(Debug, Serialize)]
pub struct IdentifyResponse {
    pub ingredients: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub async fn identify_ingredients(
    State(state): State<AppState>,
    Json(body): Json<IdentifyRequest>,
) -> impl IntoResponse {
    if state.gemini_api_key.is_empty() {
        return (
            StatusCode::OK,
            Json(IdentifyResponse {
                ingredients: None,
                error: Some("fallback".into()),
                message: Some("Gemini API key not configured".into()),
            }),
        );
    }

    // Validate + size-cap the base64 payload (~4 MB decoded limit)
    let decoded_len = body.image_base64.len() * 3 / 4;
    if decoded_len > 4 * 1024 * 1024 {
        return (
            StatusCode::OK,
            Json(IdentifyResponse {
                ingredients: None,
                error: Some("fallback".into()),
                message: Some("Image too large (max 4 MB)".into()),
            }),
        );
    }
    if B64.decode(&body.image_base64).is_err() {
        return (
            StatusCode::OK,
            Json(IdentifyResponse {
                ingredients: None,
                error: Some("fallback".into()),
                message: Some("Invalid base64 encoding".into()),
            }),
        );
    }

    let result = call_gemini(&state, &body.image_base64, &body.mime_type).await;

    match result {
        Ok(ingredients) => (
            StatusCode::OK,
            Json(IdentifyResponse {
                ingredients: Some(ingredients),
                error: None,
                message: None,
            }),
        ),
        Err(e) => {
            tracing::warn!("Gemini identify failed: {e}");
            (
                StatusCode::OK,
                Json(IdentifyResponse {
                    ingredients: None,
                    error: Some("fallback".into()),
                    message: Some("Could not identify ingredients from image".into()),
                }),
            )
        }
    }
}

/// Call the Gemini Vision API. Retries once on 429 or 5xx.
async fn call_gemini(
    state: &AppState,
    image_b64: &str,
    mime_type: &str,
) -> Result<Vec<String>, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
        state.gemini_api_key
    );

    let payload = json!({
        "contents": [{
            "parts": [
                {
                    "text": "You are a kitchen assistant. List only the distinct raw food ingredients visible in this image. Return a JSON array of lowercase strings, e.g. [\"chicken\", \"broccoli\"]. No quantities, no commentary, no non-food items. Return ONLY the JSON array, nothing else."
                },
                {
                    "inline_data": {
                        "mime_type": mime_type,
                        "data": image_b64
                    }
                }
            ]
        }],
        "generationConfig": {
            "temperature": 0.1,
            "maxOutputTokens": 512
        }
    });

    for attempt in 0..2u8 {
        let resp = state
            .http
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();

        if status.is_success() {
            let body: Value = resp.json().await.map_err(|e| e.to_string())?;
            return parse_gemini_ingredients(&body);
        }

        // Retry once on rate-limit or server error
        if attempt == 0 && (status.as_u16() == 429 || status.is_server_error()) {
            tracing::warn!("Gemini returned {status}, retrying...");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }

        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini API error {status}: {err_body}"));
    }

    Err("Gemini API unavailable after retry".into())
}

fn parse_gemini_ingredients(body: &Value) -> Result<Vec<String>, String> {
    let text = body
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())
        .ok_or("Unexpected Gemini response structure")?;

    // Extract the JSON array from the text (may have surrounding whitespace/newlines)
    let trimmed = text.trim();
    let start = trimmed.find('[').ok_or("No JSON array in Gemini response")?;
    let end = trimmed.rfind(']').ok_or("No closing bracket in Gemini response")?;
    let json_str = &trimmed[start..=end];

    let arr: Vec<Value> = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse Gemini JSON: {e}"))?;

    let ingredients: Vec<String> = arr
        .into_iter()
        .filter_map(|v| v.as_str().map(|s| s.trim().to_lowercase()))
        .filter(|s| !s.is_empty())
        .collect();

    if ingredients.is_empty() {
        return Err("Gemini returned empty ingredient list".into());
    }

    Ok(ingredients)
}

// ---------------------------------------------------------------------------
// POST /ai/present-recipe
// ---------------------------------------------------------------------------

/// Minimal recipe data sent by Phase 5 (matches RecipeDetail from recipes.rs).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RecipeInput {
    pub id: i64,
    pub title: String,
    pub ingredients_raw: Vec<String>,
    pub ingredients_core: Vec<String>,
    pub directions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PresentRequest {
    pub recipe: RecipeInput,
    /// Ingredients the user has (normalised lowercase, no pantry staples).
    pub user_ingredients: Vec<String>,
}

/// Per-ingredient entry with have/don't-have flag.
#[derive(Debug, Serialize, Deserialize)]
pub struct IngredientEntry {
    pub name: String,
    pub amount: String,
    pub have: bool,
}

/// Substitution suggestion for a missing ingredient.
#[derive(Debug, Serialize, Deserialize)]
pub struct Substitution {
    pub ingredient: String,
    pub substitute: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Full presentation output returned to the frontend.
#[derive(Debug, Serialize, Deserialize)]
pub struct PresentResponse {
    pub theme: String,
    pub difficulty: String,
    pub time_minutes: u32,
    pub description: String,
    pub ingredients: Vec<IngredientEntry>,
    pub steps: Vec<String>,
    pub substitutions: Vec<Substitution>,
}

pub async fn present_recipe(
    State(state): State<AppState>,
    Json(body): Json<PresentRequest>,
) -> impl IntoResponse {
    if state.groq_api_key.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "presentation_failed", "message": "Groq API key not configured" })),
        );
    }

    match call_groq(&state, &body.recipe, &body.user_ingredients).await {
        Ok(presented) => (StatusCode::OK, Json(json!(presented))),
        Err(e) => {
            tracing::error!("Groq present-recipe failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "presentation_failed", "message": e })),
            )
        }
    }
}

async fn call_groq(
    state: &AppState,
    recipe: &RecipeInput,
    user_ingredients: &[String],
) -> Result<PresentResponse, String> {
    let user_set = user_ingredients
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<std::collections::HashSet<_>>();

    let ingredients_display = recipe.ingredients_raw.join("\n");
    let directions_display = recipe.directions.join("\n");
    let user_have = user_ingredients.join(", ");

    let system_prompt = r#"You are a recipe presentation assistant. Given a recipe and the user's available ingredients, return ONLY valid JSON — no markdown fences, no commentary, no explanation. The JSON must match this schema exactly:

{
  "theme": "Light" | "Filling" | "Quick",
  "difficulty": "Easy" | "Medium" | "Hard",
  "time_minutes": <integer>,
  "description": "<one sentence, max 10 words, casual and direct tone>",
  "ingredients": [
    { "name": "<ingredient name>", "amount": "<quantity from recipe or 'to taste'>", "have": true | false }
  ],
  "steps": ["<rewritten step as clear imperative sentence>"],
  "substitutions": [
    { "ingredient": "<name>", "substitute": "<swap>", "note": "<optional brief note>" }
  ]
}

Theme rules:
- Quick: under 20 min estimated OR 5 or fewer core ingredients
- Light: salads, soups, eggs, fish, or clearly low-carb dishes
- Filling: everything else (hearty mains, rice/pasta dishes, stews)

Substitutions: include an entry for every ingredient where have=false.
Steps: rewrite each direction as a clean imperative sentence. Preserve all detail but remove filler phrases.
Do not include pantry staples (salt, pepper, oil, butter, flour, sugar, common spices) in substitutions."#;

    let user_message = format!(
        "Recipe title: {title}\n\nIngredients:\n{ingredients}\n\nDirections:\n{directions}\n\nUser has: {have}",
        title = recipe.title,
        ingredients = ingredients_display,
        directions = directions_display,
        have = if user_have.is_empty() { "nothing specified".to_string() } else { user_have },
    );

    let payload = json!({
        "model": "llama-3.3-70b-versatile",
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_message }
        ],
        "temperature": 0.3,
        "max_tokens": 2048,
        "response_format": { "type": "json_object" }
    });

    let resp = state
        .http
        .post("https://api.groq.com/openai/v1/chat/completions")
        .bearer_auth(&state.groq_api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    if !status.is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(format!("Groq API error {status}: {err_body}"));
    }

    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .ok_or("Unexpected Groq response structure")?;

    let mut presented: PresentResponse = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse Groq JSON: {e}\nRaw: {content}"))?;

    // Ensure have flags are correct regardless of what Groq returned —
    // cross-reference against user_set using substring matching (same logic as scoring).
    for ing in &mut presented.ingredients {
        let name_lower = ing.name.to_lowercase();
        ing.have = user_set
            .iter()
            .any(|u| name_lower.contains(u.as_str()) || u.contains(name_lower.as_str()));
    }

    Ok(presented)
}

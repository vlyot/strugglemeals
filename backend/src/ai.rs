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
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
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

// ---------------------------------------------------------------------------
// POST /ai/theme-shortlist
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ShortlistRequest {
    pub ingredients: Vec<String>,
    pub vegetarian: Option<bool>,
    pub vegan: Option<bool>,
    pub gluten_free: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ShortlistEntry {
    pub id: i64,
    pub title: String,
    pub theme: Option<String>,
    pub reason: Option<String>,
    pub match_score: usize,
    pub missing_count: usize,
    pub ingredient_count: i64,
    pub vegetarian: bool,
    pub vegan: bool,
    pub gluten_free: bool,
}

#[derive(Debug, Serialize)]
pub struct ShortlistResponse {
    pub results: Vec<ShortlistEntry>,
    pub groq_used: bool,
}

pub async fn theme_shortlist(
    State(state): State<AppState>,
    Json(body): Json<ShortlistRequest>,
) -> impl IntoResponse {
    if body.ingredients.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "ingredients required" })),
        );
    }

    // Reuse SQLite scoring logic — borrow pool from AppState
    let candidates = match fetch_candidates(&state.sqlite, &body) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("theme_shortlist SQLite error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "database error" })),
            );
        }
    };

    if candidates.is_empty() {
        return (
            StatusCode::OK,
            Json(serde_json::json!(ShortlistResponse { results: vec![], groq_used: false })),
        );
    }

    // Normalize user ingredients (lowercase, strip pantry staples)
    let user_ings: Vec<String> = body
        .ingredients
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty() && !is_pantry_staple_ai(s))
        .collect();

    // Try Groq for themed selection; fall back to raw scoring on failure
    if !state.groq_api_key.is_empty() {
        match call_groq_shortlist(&state, &candidates, &user_ings).await {
            Ok(results) => {
                return (
                    StatusCode::OK,
                    Json(serde_json::json!(ShortlistResponse { results, groq_used: true })),
                );
            }
            Err(e) => {
                tracing::warn!("Groq shortlist failed, using raw fallback: {e}");
            }
        }
    }

    // Fallback: top 6 raw candidates with no theme
    let fallback: Vec<ShortlistEntry> = candidates
        .into_iter()
        .take(6)
        .map(|c| ShortlistEntry {
            missing_count: c.ingredient_count as usize - c.match_score.min(c.ingredient_count as usize),
            id: c.id,
            title: c.title,
            theme: None,
            reason: None,
            match_score: c.match_score,
            ingredient_count: c.ingredient_count,
            vegetarian: c.vegetarian,
            vegan: c.vegan,
            gluten_free: c.gluten_free,
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!(ShortlistResponse { results: fallback, groq_used: false })),
    )
}

// Pantry staples for AI module (mirrors recipes.rs list)
fn is_pantry_staple_ai(ingredient: &str) -> bool {
    const STAPLES: &[&str] = &[
        "salt", "black pepper", "white pepper", "pepper", "olive oil",
        "vegetable oil", "canola oil", "oil", "butter", "water", "sugar",
        "brown sugar", "flour", "all-purpose flour", "baking soda",
        "baking powder", "vanilla extract", "vanilla", "garlic powder",
        "onion powder", "paprika", "cumin", "oregano", "thyme", "basil",
        "cayenne", "red pepper flakes", "cinnamon", "nutmeg", "bay leaves",
        "bay leaf", "cooking spray", "nonstick cooking spray", "shortening",
    ];
    STAPLES.contains(&ingredient)
}

struct CandidateRow {
    id: i64,
    title: String,
    ingredient_count: i64,
    vegetarian: bool,
    vegan: bool,
    gluten_free: bool,
    ingredients_core: Vec<String>,
    match_score: usize,
}

fn fetch_candidates(
    pool: &crate::SqlitePool,
    body: &ShortlistRequest,
) -> Result<Vec<CandidateRow>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;

    let user_ings: Vec<String> = body
        .ingredients
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty() && !is_pantry_staple_ai(s))
        .collect();

    let candidate_limit = 2000usize;
    let mut sql = String::from(
        "SELECT id, title, ingredient_count, vegetarian, vegan, gluten_free, ingredients_core \
         FROM recipes WHERE 1=1",
    );
    if body.vegetarian == Some(true) {
        sql.push_str(" AND vegetarian = 1");
    }
    if body.vegan == Some(true) {
        sql.push_str(" AND vegan = 1");
    }
    if body.gluten_free == Some(true) {
        sql.push_str(" AND gluten_free = 1");
    }
    sql.push_str(&format!(" LIMIT {candidate_limit}"));

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let mut rows: Vec<CandidateRow> = stmt
        .query_map([], |row| {
            let core_raw: String = row.get(6)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, i64>(4)? != 0,
                row.get::<_, i64>(5)? != 0,
                core_raw,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .map(|(id, title, ingredient_count, vegetarian, vegan, gluten_free, core_raw)| {
            let ingredients_core: Vec<String> = serde_json::from_str::<Value>(&core_raw)
                .ok()
                .and_then(|v| v.as_array().cloned())
                .map(|arr| arr.into_iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let match_score = score_ingredients(&user_ings, &ingredients_core);
            CandidateRow {
                id,
                title,
                ingredient_count,
                vegetarian,
                vegan,
                gluten_free,
                ingredients_core,
                match_score,
            }
        })
        .filter(|r| r.match_score > 0)
        .collect();

    rows.sort_unstable_by(|a, b| {
        b.match_score.cmp(&a.match_score)
            .then(a.ingredient_count.cmp(&b.ingredient_count))
    });
    rows.truncate(20);

    // If we got fewer than 3 matching, relax to top 5 by score (including 0-score)
    if rows.len() < 3 {
        let conn2 = pool.get().map_err(|e| e.to_string())?;
        let mut stmt2 = conn2.prepare(&sql).map_err(|e| e.to_string())?;
        let mut relaxed: Vec<CandidateRow> = stmt2
            .query_map([], |row| {
                let core_raw: String = row.get(6)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, i64>(4)? != 0,
                    row.get::<_, i64>(5)? != 0,
                    core_raw,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .map(|(id, title, ingredient_count, vegetarian, vegan, gluten_free, core_raw)| {
                let ingredients_core: Vec<String> = serde_json::from_str::<Value>(&core_raw)
                    .ok()
                    .and_then(|v| v.as_array().cloned())
                    .map(|arr| arr.into_iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let match_score = score_ingredients(&user_ings, &ingredients_core);
                CandidateRow {
                    id, title, ingredient_count, vegetarian, vegan, gluten_free,
                    ingredients_core, match_score,
                }
            })
            .collect();
        relaxed.sort_unstable_by(|a, b| {
            b.match_score.cmp(&a.match_score)
                .then(a.ingredient_count.cmp(&b.ingredient_count))
        });
        relaxed.truncate(5);
        return Ok(relaxed);
    }

    Ok(rows)
}

fn score_ingredients(user_ings: &[String], recipe_core: &[String]) -> usize {
    let recipe_lower: Vec<String> = recipe_core.iter().map(|s| s.to_lowercase()).collect();
    user_ings
        .iter()
        .filter(|ui| recipe_lower.iter().any(|ri| ri.contains(ui.as_str()) || ui.contains(ri.as_str())))
        .count()
}

async fn call_groq_shortlist(
    state: &AppState,
    candidates: &[CandidateRow],
    user_ings: &[String],
) -> Result<Vec<ShortlistEntry>, String> {
    // Build a compact candidate list for Groq (id, title, core ingredients)
    let candidate_lines: Vec<String> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            format!(
                "{}. [id:{}] {} (ingredients: {})",
                i + 1,
                c.id,
                c.title,
                c.ingredients_core.join(", ")
            )
        })
        .collect();

    let system_prompt = r#"You are a recipe selection assistant. Given a list of candidate recipes and the user's available ingredients, select up to 2 recipes per theme (Light, Filling, Quick) — maximum 6 total.

Theme rules:
- Quick: under 20 min estimated OR 5 or fewer core ingredients
- Light: salads, soups, eggs, fish, or clearly low-carb dishes
- Filling: hearty mains, rice/pasta dishes, stews, everything else

Return ONLY valid JSON — no markdown fences, no commentary. The JSON must be an object with a single key "results" containing an array:
{
  "results": [
    {
      "id": <integer recipe id>,
      "theme": "Light" | "Filling" | "Quick",
      "reason": "<one sentence, casual, max 10 words>"
    }
  ]
}

Rules:
- Only include recipes from the provided list (use the exact id)
- At most 2 per theme
- Pick the best matches based on ingredient overlap with what the user has
- Do not repeat the same recipe in multiple themes"#;

    let user_message = format!(
        "User has: {}\n\nCandidates:\n{}",
        if user_ings.is_empty() { "various ingredients".to_string() } else { user_ings.join(", ") },
        candidate_lines.join("\n")
    );

    let payload = json!({
        "model": "llama-3.3-70b-versatile",
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_message }
        ],
        "temperature": 0.2,
        "max_tokens": 1024,
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

    let parsed: Value = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse Groq JSON: {e}\nRaw: {content}"))?;

    let groq_results = parsed["results"]
        .as_array()
        .ok_or("Groq response missing 'results' array")?;

    // Build a lookup map from candidates
    let candidate_map: std::collections::HashMap<i64, &CandidateRow> =
        candidates.iter().map(|c| (c.id, c)).collect();

    let mut theme_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut results: Vec<ShortlistEntry> = Vec::new();
    let mut seen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

    for item in groq_results {
        let id = item["id"].as_i64().unwrap_or(-1);
        let theme = item["theme"].as_str().unwrap_or("").to_string();
        let reason = item["reason"].as_str().map(String::from);

        if id < 0 || theme.is_empty() { continue; }
        if seen_ids.contains(&id) { continue; }
        let count = theme_counts.entry(theme.clone()).or_insert(0);
        if *count >= 2 { continue; }

        if let Some(c) = candidate_map.get(&id) {
            let missing_count = c.ingredient_count as usize
                - c.match_score.min(c.ingredient_count as usize);
            results.push(ShortlistEntry {
                id: c.id,
                title: c.title.clone(),
                theme: Some(theme.clone()),
                reason,
                match_score: c.match_score,
                missing_count,
                ingredient_count: c.ingredient_count,
                vegetarian: c.vegetarian,
                vegan: c.vegan,
                gluten_free: c.gluten_free,
            });
            *count += 1;
            seen_ids.insert(id);
        }
    }

    if results.is_empty() {
        return Err("Groq returned no valid results".into());
    }

    Ok(results)
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

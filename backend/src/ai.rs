use axum::{
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use futures_util::stream::{self, StreamExt};
use std::convert::Infallible;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use rayon::prelude::*;

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

/// A single detected ingredient with a vision-model confidence score.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DetectedIngredient {
    pub name: String,
    /// 0.0–10.0: how certain the model is that this ingredient is present.
    pub confidence: f32,
}

#[derive(Debug, Serialize)]
pub struct IdentifyResponse {
    /// Flattened names — kept for the frontend chip-merge path.
    pub ingredients: Option<Vec<String>>,
    /// Full detection results including per-ingredient confidence scores.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected: Option<Vec<DetectedIngredient>>,
    /// Inferred pantry staples likely present given the detected ingredients.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<String>>,
    /// Legend explaining the confidence score bands to show the user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_legend: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub async fn identify_ingredients(
    State(state): State<AppState>,
    Json(body): Json<IdentifyRequest>,
) -> impl IntoResponse {
    // Enforce per-minute rate limit before any Gemini work.
    let _permit = match state.gemini_limiter.try_acquire() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(IdentifyResponse {
                    ingredients: None,
                    detected: None,
                    suggestions: None,
                    confidence_legend: None,
                    error: Some("rate_limited".into()),
                    message: Some("Too many photo scans right now. Please try again in a minute.".into()),
                }),
            );
        }
    };

    if state.gemini_api_key.is_empty() {
        return (
            StatusCode::OK,
            Json(IdentifyResponse {
                ingredients: None,
                detected: None,
                suggestions: None,
                confidence_legend: None,
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
                detected: None,
                suggestions: None,
                confidence_legend: None,
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
                detected: None,
                suggestions: None,
                confidence_legend: None,
                error: Some("fallback".into()),
                message: Some("Invalid base64 encoding".into()),
            }),
        );
    }

    let result = call_gemini(&state, &body.image_base64, &body.mime_type).await;

    match result {
        Ok((detected, suggestions, legend)) => {
            let ingredients: Vec<String> = detected.iter().map(|d| d.name.clone()).collect();
            (
                StatusCode::OK,
                Json(IdentifyResponse {
                    ingredients: Some(ingredients),
                    detected: Some(detected),
                    suggestions: if suggestions.is_empty() { None } else { Some(suggestions) },
                    confidence_legend: Some(legend),
                    error: None,
                    message: None,
                }),
            )
        }
        Err(e) => {
            tracing::error!("Gemini identify failed: {e}");
            (
                StatusCode::OK,
                Json(IdentifyResponse {
                    ingredients: None,
                    detected: None,
                    suggestions: None,
                    confidence_legend: None,
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
) -> Result<(Vec<DetectedIngredient>, Vec<String>, Value), String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
        state.gemini_api_key
    );

    let prompt = concat!(
        "You are a kitchen assistant. Analyse this photo and return ONLY a raw JSON object (no markdown, no code fences) ",
        "with exactly three keys:\n",
        "- \"detected\": array of {\"name\":\"<lowercase>\",\"confidence\":<0.0-10.0>} for each visible food ingredient. ",
        "10=unmistakably clear, 5=partially obscured, 1=barely visible.\n",
        "- \"likely_have\": array of up to 6 lowercase pantry staples almost certainly present (e.g. salt, oil, garlic).\n",
        "- \"legend\": {\"high\":\"clearly visible\",\"mid\":\"partially visible\",\"low\":\"hard to tell\"}.\n\n",
        "Example: {\"detected\":[{\"name\":\"chicken\",\"confidence\":9.2}],\"likely_have\":[\"salt\"],",
        "\"legend\":{\"high\":\"clearly visible\",\"mid\":\"partially visible\",\"low\":\"hard to tell\"}}"
    );

    let payload = json!({
        "contents": [{
            "parts": [
                { "text": prompt },
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
            "maxOutputTokens": 2048,
            "thinkingConfig": {
                "thinkingBudget": 0
            }
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
            let raw_text = resp.text().await.map_err(|e| e.to_string())?;
            tracing::info!("Gemini raw response (attempt {attempt}): {raw_text}");
            let body: Value = serde_json::from_str(&raw_text)
                .map_err(|e| format!("Failed to parse Gemini JSON body: {e}\nRaw: {raw_text}"))?;
            return parse_gemini_response(&body);
        }

        // Retry once on rate-limit or server error
        if attempt == 0 && (status.as_u16() == 429 || status.is_server_error()) {
            let err_body = resp.text().await.unwrap_or_default();
            tracing::warn!("Gemini returned {status} (attempt {attempt}), retrying... body: {err_body}");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        }

        let err_body = resp.text().await.unwrap_or_default();
        tracing::error!("Gemini API error {status}: {err_body}");
        return Err(format!("Gemini API error {status}: {err_body}"));
    }

    Err("Gemini API unavailable after retry".into())
}

/// Parse the structured Gemini response: `{detected, likely_have, legend}`.
///
/// Falls back to treating the output as a plain `[...]` array (old format) if the
/// object parse fails, so a Gemini format regression doesn't silently break detection.
fn parse_gemini_response(body: &Value) -> Result<(Vec<DetectedIngredient>, Vec<String>, Value), String> {
    tracing::info!("Gemini parsed body: {body}");

    let text = body
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            let msg = format!("Unexpected Gemini response structure — full body: {body}");
            tracing::error!("{msg}");
            msg
        })?;

    tracing::info!("Gemini extracted text: {text}");

    let trimmed = text.trim();

    // Primary path: structured object response.
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        let json_str = &trimmed[start..=end];
        if let Ok(obj) = serde_json::from_str::<Value>(json_str) {
            // Extract detected array.
            let detected: Vec<DetectedIngredient> = obj
                .get("detected")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            let name = item.get("name")?.as_str()?.trim().to_lowercase();
                            let confidence = item.get("confidence")?.as_f64()? as f32;
                            if name.is_empty() { return None; }
                            Some(DetectedIngredient { name, confidence })
                        })
                        .collect()
                })
                .unwrap_or_default();

            if detected.is_empty() {
                tracing::error!("Gemini parsed object but 'detected' array is empty. Full text: {text}");
                return Err("Gemini returned empty detected list".into());
            }

            let detected_names: std::collections::HashSet<&str> =
                detected.iter().map(|d| d.name.as_str()).collect();

            // Extract likely_have, deduplicated against detected.
            let suggestions: Vec<String> = obj
                .get("likely_have")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            let s = v.as_str()?.trim().to_lowercase();
                            if s.is_empty() || detected_names.contains(s.as_str()) {
                                return None;
                            }
                            Some(s)
                        })
                        .collect()
                })
                .unwrap_or_default();

            let legend = obj
                .get("legend")
                .cloned()
                .unwrap_or_else(|| json!({"high": "clearly visible", "mid": "partially visible", "low": "hard to tell"}));

            return Ok((detected, suggestions, legend));
        }
    }

    // Fallback: plain array format — assign confidence 10.0 to all.
    tracing::warn!("Gemini object parse failed or not an object, trying plain array fallback. text: {trimmed}");
    let start = trimmed.find('[').ok_or_else(|| {
        let msg = format!("No JSON array in Gemini response. text: {trimmed}");
        tracing::error!("{msg}");
        msg
    })?;
    let end = trimmed.rfind(']').ok_or("No closing bracket in Gemini response")?;
    let json_str = &trimmed[start..=end];

    let arr: Vec<Value> = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse Gemini JSON: {e}"))?;

    let detected: Vec<DetectedIngredient> = arr
        .into_iter()
        .filter_map(|v| {
            let name = v.as_str()?.trim().to_lowercase();
            if name.is_empty() { return None; }
            Some(DetectedIngredient { name, confidence: 10.0 })
        })
        .collect();

    if detected.is_empty() {
        return Err("Gemini returned empty ingredient list".into());
    }

    Ok((detected, vec![], json!({"high": "clearly visible", "mid": "partially visible", "low": "hard to tell"})))
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

#[derive(Debug, Deserialize, Clone)]
pub struct IngredientQty {
    pub name: String,
    /// "1 qty" | "a little" | "plenty"
    pub qty: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ShortlistRequest {
    pub ingredients: Vec<String>,
    pub ingredients_with_qty: Option<Vec<IngredientQty>>,
    pub vegetarian: Option<bool>,
    pub vegan: Option<bool>,
    pub gluten_free: Option<bool>,
    pub cuisine: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
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
    pub matched_ingredients: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ShortlistResponse {
    pub results: Vec<ShortlistEntry>,
    pub groq_used: bool,
}

/// Converts candidates to ShortlistEntries with theme=None (used for the immediate "scores" event).
fn build_shortlist_entries(candidates: &[CandidateRow]) -> Vec<ShortlistEntry> {
    candidates
        .iter()
        .take(6)
        .map(|c| ShortlistEntry {
            missing_count: c.ingredient_count as usize - c.match_count.min(c.ingredient_count as usize),
            id: c.id,
            title: c.title.clone(),
            theme: None,
            reason: None,
            match_score: c.match_count,
            ingredient_count: c.ingredient_count,
            vegetarian: c.vegetarian,
            vegan: c.vegan,
            gluten_free: c.gluten_free,
            matched_ingredients: c.matched_ingredients.clone(),
        })
        .collect()
}

/// SSE endpoint: emits two events — "scores" immediately (pure Rust), then "themes" after Groq.
/// The frontend renders recipe cards on the first event and updates theme labels on the second.
/// X-Accel-Buffering: no tells Railway's NGINX proxy to flush each SSE event immediately
/// instead of batching them, so the two-phase UX benefit is preserved in production.
///
/// Candidates are fetched exactly once and reused for both phases, avoiding a redundant
/// SQLite scan + JSON parse + rayon scoring pass.
pub async fn theme_shortlist(
    State(state): State<AppState>,
    Json(body): Json<ShortlistRequest>,
) -> impl IntoResponse {
    let user_ings = resolve_user_ings(&body);

    // Fetch candidates once via spawn_blocking so the SQLite scan + rayon scoring
    // run on the blocking thread pool, freeing the tokio executor.
    let fts_ready = state.fts_ready.load(std::sync::atomic::Ordering::Relaxed);
    let (initial_entries, candidates) = if body.ingredients.is_empty() || user_ings.is_empty() {
        (vec![], vec![])
    } else {
        let pool = state.sqlite.clone();
        let body_c = body.clone();
        let ings_c = user_ings.clone();
        match tokio::task::spawn_blocking(move || fetch_candidates(&pool, &body_c, &ings_c, fts_ready)).await {
            Ok(Ok(candidates)) if !candidates.is_empty() => {
                let entries = build_shortlist_entries(&candidates);
                (entries, candidates)
            }
            Ok(Ok(_)) => (vec![], vec![]),
            Ok(Err(e)) => {
                tracing::error!("theme_shortlist SQLite error: {e}");
                (vec![], vec![])
            }
            Err(e) => {
                tracing::error!("theme_shortlist spawn_blocking panic: {e}");
                (vec![], vec![])
            }
        }
    };

    // Phase 1: emit scored results immediately (theme = None).
    let scores_data = serde_json::to_string(&ShortlistResponse {
        results: initial_entries.clone(),
        groq_used: false,
    })
    .unwrap_or_default();
    let scores_event = Event::default().event("scores").data(scores_data);

    // Phase 2: call Groq for theme classification using the already-fetched candidates.
    let has_groq = !state.groq_api_key.is_empty() && !candidates.is_empty();

    let themes_stream = stream::once(async move {
        if !has_groq {
            return None;
        }
        match call_groq_shortlist(&state, &candidates, &user_ings).await {
            Ok(results) => {
                let data = serde_json::to_string(&ShortlistResponse { results, groq_used: true })
                    .unwrap_or_default();
                Some(Event::default().event("themes").data(data))
            }
            Err(e) => {
                tracing::warn!("Groq shortlist failed, client keeps scored results: {e}");
                None
            }
        }
    })
    .filter_map(|e| async { e });

    let s = stream::once(async move { Ok::<_, Infallible>(scores_event) })
        .chain(themes_stream.map(Ok::<_, Infallible>));

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );

    (headers, Sse::new(s).keep_alive(KeepAlive::default()))
}

// ---------------------------------------------------------------------------
// Pantry staples
// ---------------------------------------------------------------------------

pub(crate) fn is_pantry_staple_ai(ingredient: &str) -> bool {
    const STAPLES: &[&str] = &[
        // Fats & oils
        "salt", "black pepper", "white pepper", "pepper", "olive oil",
        "vegetable oil", "canola oil", "oil", "butter", "water", "sugar",
        "brown sugar", "flour", "all-purpose flour", "baking soda",
        "baking powder", "vanilla extract", "vanilla", "garlic powder",
        "onion powder", "paprika", "cumin", "oregano", "thyme", "basil",
        "cayenne", "red pepper flakes", "cinnamon", "nutmeg", "bay leaves",
        "bay leaf", "cooking spray", "nonstick cooking spray", "shortening",
        // Condiments & sauces (very common, shouldn't drive recipe selection)
        "soy sauce", "fish sauce", "oyster sauce", "hoisin sauce",
        "worcestershire sauce", "hot sauce", "tabasco", "sriracha",
        "ketchup", "mustard", "mayonnaise", "vinegar", "rice vinegar",
        "apple cider vinegar", "white vinegar", "balsamic vinegar",
        "sesame oil", "sesame seeds", "cornstarch", "corn starch",
        "tomato paste", "tomato sauce", "chicken broth", "beef broth",
        "vegetable broth", "chicken stock", "beef stock", "vegetable stock",
        "broth", "stock", "heavy cream", "heavy whipping cream",
        "milk", "lemon juice", "lime juice",
    ];
    STAPLES.contains(&ingredient)
}

// ---------------------------------------------------------------------------
// TF-IDF / BM25-inspired scoring
// ---------------------------------------------------------------------------

/// Split a string into word tokens on whitespace, hyphens, and slashes.
/// Used for word-boundary matching to avoid false positives like "egg" → "eggplant".
pub(crate) fn word_tokens(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| c.is_whitespace() || c == '-' || c == '/')
        .filter(|t| !t.is_empty())
}

/// Returns true if any token from `a` exactly equals any token from `b`.
/// Handles cases like "chicken" matching "chicken breast", while preventing
/// "egg" from matching "eggplant" or "pea" from matching "peanut".
fn tokens_overlap(a: &str, b: &str) -> bool {
    word_tokens(a).any(|ta| word_tokens(b).any(|tb| ta == tb))
}

/// Rarity-based IDF tier. Rare/specific ingredients score higher.
/// Uses token-level matching so "eggplant" gets IDF 9.0 (niche),
/// not 4.5 (common) from a false substring match on "egg".
fn rarity_idf(ingredient: &str) -> f64 {
    const ULTRA_COMMON: &[&str] = &[
        "chicken", "beef", "pork", "lamb", "onion", "garlic",
        "carrot", "celery", "potato", "rice", "pasta", "tomato",
    ];
    const COMMON: &[&str] = &[
        "egg", "eggs", "cheese", "mushroom", "spinach", "broccoli",
        "corn", "bean", "beans", "lentil", "lentils", "shrimp",
    ];
    let toks: Vec<&str> = word_tokens(ingredient).collect();
    if ULTRA_COMMON.iter().any(|&s| toks.contains(&s)) {
        2.0
    } else if COMMON.iter().any(|&s| toks.contains(&s)) {
        4.5
    } else {
        9.0
    }
}

/// SQL ORDER BY weight based on IDF tier (integer, for use in CASE WHEN expressions).
/// Niche ingredients pull harder on the candidate pool than ultra-common ones.
fn idf_sql_weight(ingredient: &str) -> u32 {
    let idf = rarity_idf(ingredient);
    if idf >= 9.0 { 3 } else if idf >= 4.5 { 2 } else { 1 }
}

/// Quantity signal multiplier.
fn qty_weight(qty: &str) -> f64 {
    match qty {
        "plenty" => 1.3,
        "a little" => 0.7,
        _ => 1.0,
    }
}

/// Score a recipe against the user's ingredients.
/// Returns (weighted_score, matched_ingredient_names).
fn score_v2(user_ings: &[IngredientQty], recipe_core: &[String], title: &str) -> (f64, Vec<String>) {
    if recipe_core.is_empty() {
        return (0.0, vec![]);
    }
    let recipe_lower: Vec<String> = recipe_core.iter().map(|s| s.to_lowercase()).collect();
    let title_lower = title.to_lowercase();

    let mut matched_names: Vec<String> = Vec::new();
    let mut weighted_sum = 0.0f64;

    // Ingredient anchor boost: if any user ingredient is the star of the dish
    // (appears in the title OR is the first core ingredient), reward that recipe.
    // Boost scales with IDF so specific ingredients (kimchi, udon) reward more than
    // generic ones (chicken, rice). Takes the max boost across all user ingredients.
    let mut anchor_boost = 1.0f64;

    for ui in user_ings {
        let name = ui.name.to_lowercase();
        let hits = recipe_lower
            .iter()
            .any(|ri| tokens_overlap(ri, &name));
        if !hits {
            continue;
        }
        matched_names.push(ui.name.clone());
        let idf = rarity_idf(&name);
        let qty = qty_weight(&ui.qty);
        // Title bonus scaled by IDF: rare ingredients (udon→5.4, egg→2.7, chicken→1.2).
        // Also uses token matching to prevent "egg" boosting on title "Eggplant Parmesan".
        let name_toks: Vec<&str> = word_tokens(&name).collect();
        let title_toks: Vec<&str> = word_tokens(&title_lower).collect();
        let title_match = name_toks.iter().any(|nt| title_toks.iter().any(|tt| tt == nt));
        let title_bonus = if title_match { idf * 0.6 } else { 0.0 };
        weighted_sum += idf * qty + title_bonus;

        // Anchor check: in title OR first core ingredient.
        let is_first = recipe_lower.first().map(|f| tokens_overlap(f, &name)).unwrap_or(false);
        if title_match || is_first {
            // Scale: idf=9.0 → +35%; idf=4.5 → +17%; idf=2.0 → +8%
            anchor_boost = anchor_boost.max(1.0 + (idf / 9.0) * 0.35);
        }
    }

    let matched = matched_names.len();
    if matched == 0 {
        return (0.0, vec![]);
    }

    let n = recipe_core.len() as f64;
    let coverage = matched as f64 / n;
    let coverage_factor = coverage.powf(1.5);

    let missing_ratio = (n - matched as f64) / n;
    let missing_factor = 1.0 - (missing_ratio.powi(2) * 0.5);

    // Simplicity reward: short recipes where the user covers most ingredients
    // score higher than bloated ones with the same absolute match count.
    // Max bonus of 1.25× at core_len=2, tapering to 1.0× at core_len≥7.
    let simplicity_bonus = if n <= 6.0 && coverage >= 0.5 {
        1.0 + (0.25 * ((6.0 - n) / 4.0).clamp(0.0, 1.0))
    } else {
        1.0
    };

    // Pantry focus: reward recipes that draw on a large fraction of the user's
    // non-staple ingredients. Only active when the user has a diverse pantry
    // (user_n > 5) — for small/focused pantries, coverage_factor already suffices.
    // sqrt exponent: mild compression so 50% pantry use (0.707) is still respectable
    // but 20% pantry use (0.447) is meaningfully penalised.
    let user_n = user_ings.len() as f64;
    let pantry_focus = if user_n > 5.0 {
        (matched as f64 / user_n).powf(0.5)
    } else {
        1.0
    };

    (
        weighted_sum * coverage_factor * missing_factor * simplicity_bonus * anchor_boost
            * pantry_focus,
        matched_names,
    )
}

// ---------------------------------------------------------------------------
// Candidate row
// ---------------------------------------------------------------------------

struct CandidateRow {
    id: i64,
    title: String,
    #[allow(dead_code)]
    cuisine: Option<String>,
    ingredient_count: i64,
    vegetarian: bool,
    vegan: bool,
    gluten_free: bool,
    ingredients_core: Vec<String>,
    directions: Vec<String>,
    score: f64,
    match_count: usize,
    matched_ingredients: Vec<String>,
}

// ---------------------------------------------------------------------------
// Resolve user ingredients from request
// ---------------------------------------------------------------------------

fn resolve_user_ings(body: &ShortlistRequest) -> Vec<IngredientQty> {
    let raw: Vec<IngredientQty> = if let Some(ref wq) = body.ingredients_with_qty {
        wq.clone()
    } else {
        body.ingredients
            .iter()
            .map(|n| IngredientQty { name: n.clone(), qty: "1 qty".to_string() })
            .collect()
    };
    raw.into_iter()
        .map(|i| IngredientQty { name: i.name.trim().to_lowercase(), qty: i.qty })
        .filter(|i| !i.name.is_empty() && !is_pantry_staple_ai(&i.name))
        .collect()
}

// ---------------------------------------------------------------------------
// Candidate fetch — ingredient-aware SQL with json_each()
// ---------------------------------------------------------------------------

fn parse_json_str_array(raw: &str) -> Vec<String> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|arr| arr.into_iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// Generate plural/singular stem variants for a single ingredient name.
/// This is intentionally minimal — targeting the most common recipe dataset patterns:
/// plural -s/-es/-ies and their reverses. Does NOT use a full Snowball/Porter stemmer
/// to avoid mangling uncommon ingredient names (e.g. "celery" → "celer").
fn stem_variants(name: &str) -> Vec<String> {
    let mut v = vec![name.to_string()];
    if name.ends_with("ies") && name.len() > 4 {
        // berries → berry, cherries → cherry
        v.push(format!("{}y", &name[..name.len() - 3]));
    } else if name.ends_with("es") && name.len() > 3 {
        // tomatoes → tomato, potatoes → potato
        v.push(name[..name.len() - 2].to_string());
    } else if name.ends_with('s') && name.len() > 3 {
        // eggs → egg, chickens → chicken
        v.push(name[..name.len() - 1].to_string());
    }
    // Add plural form if not already ending in s
    if !name.ends_with('s') {
        v.push(format!("{name}s"));
    }
    v.dedup();
    v
}

// ---------------------------------------------------------------------------
// FTS5 fast-path candidate fetch
// ---------------------------------------------------------------------------

/// Raw column tuple returned by both json_each and FTS5 SQLite queries.
type RawRow = (i64, String, Option<String>, i64, bool, bool, bool, String, String);

/// Shared row-scoring logic: maps raw SQLite columns into a CandidateRow with
/// score_v2 applied. Used by both the primary and relaxation FTS5 queries.
fn score_raw_rows(
    raw_rows: Vec<RawRow>,
    user_ings: &[IngredientQty],
    cuisine_req: &Option<String>,
) -> Vec<CandidateRow> {
    raw_rows
        .into_par_iter()
        .map(|(id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, core_raw, dir_raw)| {
            let ingredients_core = parse_json_str_array(&core_raw);
            let directions = parse_json_str_array(&dir_raw);
            let (mut score, matched_ingredients) = score_v2(user_ings, &ingredients_core, &title);
            if let (Some(ref req), Some(ref rec)) = (cuisine_req, &cuisine) {
                let rl = rec.to_lowercase();
                let rql = req.to_lowercase();
                if rl.contains(rql.as_str()) || rql.contains(rl.as_str()) {
                    score *= 1.25;
                }
            }
            let total = ingredient_count as f64;
            let core_len = ingredients_core.len() as f64;
            if total > 0.0 {
                let pantry_ratio = ((total - core_len) / total).clamp(0.0, 1.0);
                score *= (1.0 - pantry_ratio.powi(2) * 0.7).max(0.1);
            }
            let match_count = matched_ingredients.len();
            CandidateRow { id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, ingredients_core, directions, score, match_count, matched_ingredients }
        })
        .filter(|r| r.match_count > 0)
        .collect()
}

/// Execute a FTS5 MATCH query and return raw column tuples.
fn fts_query_raw(
    conn: &rusqlite::Connection,
    match_expr: &str,
    filter_sql: &str,
    limit: usize,
) -> Result<Vec<RawRow>, String> {
    let sql = format!(
        "SELECT r.id, r.title, r.cuisine, r.ingredient_count, \
         r.vegetarian, r.vegan, r.gluten_free, r.ingredients_core, r.directions \
         FROM recipes r \
         WHERE r.id IN (SELECT rowid FROM recipes_fts WHERE ingredients_text MATCH ?1) \
         {filter_sql} \
         ORDER BY r.ingredient_count ASC \
         LIMIT {limit}",
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![match_expr], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)? != 0,
                row.get::<_, i64>(5)? != 0,
                row.get::<_, i64>(6)? != 0,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// FTS5-backed candidate fetch. Replaces the json_each EXISTS scan with an
/// inverted-index MATCH lookup, then scores results with rayon in parallel.
fn fetch_candidates_fts(
    conn: &rusqlite::Connection,
    body: &ShortlistRequest,
    user_ings: &[IngredientQty],
) -> Result<Vec<CandidateRow>, String> {
    let mut filter_sql = String::new();
    if body.vegetarian == Some(true) { filter_sql.push_str(" AND r.vegetarian = 1"); }
    if body.vegan == Some(true)      { filter_sql.push_str(" AND r.vegan = 1"); }
    if body.gluten_free == Some(true){ filter_sql.push_str(" AND r.gluten_free = 1"); }

    let match_expr = build_fts_match(user_ings);
    let raw_rows = fts_query_raw(conn, &match_expr, &filter_sql, 500)?;
    let cuisine_req = body.cuisine.clone();
    let mut rows = score_raw_rows(raw_rows, user_ings, &cuisine_req);

    rows.sort_unstable_by(|a, b| {
        b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.ingredient_count.cmp(&b.ingredient_count))
    });
    rows.truncate(20);

    if rows.len() < 3 {
        let relaxed_match = build_fts_match_relaxed(user_ings);
        if relaxed_match.is_empty() {
            return Ok(rows);
        }
        let relaxed_raw = fts_query_raw(conn, &relaxed_match, &filter_sql, 200)?;
        let mut relaxed = score_raw_rows(relaxed_raw, user_ings, &cuisine_req);
        relaxed.sort_unstable_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
                .then(a.ingredient_count.cmp(&b.ingredient_count))
        });
        relaxed.truncate(5);
        return Ok(relaxed);
    }

    Ok(rows)
}

/// Build a FTS5 MATCH expression: `"egg" OR "sour cream" OR "udon"`.
/// Each term is double-quoted to enforce exact token matching, preventing
/// "egg" from matching "eggplant" and "pea" from matching "peanut".
fn build_fts_match(user_ings: &[IngredientQty]) -> String {
    user_ings
        .iter()
        .map(|ui| format!("\"{}\"", ui.name.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Same as build_fts_match but expands each name into its stem variants
/// (plural/singular) for the relaxation path.
fn build_fts_match_relaxed(user_ings: &[IngredientQty]) -> String {
    user_ings
        .iter()
        .flat_map(|ui| stem_variants(&ui.name))
        .map(|v| format!("\"{}\"", v.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn fetch_candidates(
    pool: &crate::SqlitePool,
    body: &ShortlistRequest,
    user_ings: &[IngredientQty],
    fts_ready: bool,
) -> Result<Vec<CandidateRow>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;

    // FTS5 fast path: uses pre-built inverted index instead of json_each scan.
    if fts_ready {
        return fetch_candidates_fts(&conn, body, user_ings);
    }

    let n = user_ings.len();

    // One EXISTS clause per ingredient (for WHERE and ORDER BY).
    // We duplicate the params: first set for WHERE OR'd clauses, second set for ORDER BY sum.
    let exists_clause =
        "EXISTS (SELECT 1 FROM json_each(ingredients_core) WHERE LOWER(value) LIKE ?)";
    let where_parts: Vec<&str> = vec![exists_clause; n];
    // Weight each ingredient's match contribution by its IDF tier so rare ingredients
    // (kimchi=3, egg=2, chicken=1) pull proportionally harder on the 500-candidate pool.
    let order_parts: Vec<String> = user_ings
        .iter()
        .map(|ui| {
            let w = idf_sql_weight(&ui.name);
            format!("CASE WHEN {exists_clause} THEN {w} ELSE 0 END")
        })
        .collect();

    let mut filter_sql = String::new();
    if body.vegetarian == Some(true) { filter_sql.push_str(" AND vegetarian = 1"); }
    if body.vegan == Some(true)      { filter_sql.push_str(" AND vegan = 1"); }
    if body.gluten_free == Some(true){ filter_sql.push_str(" AND gluten_free = 1"); }

    // ORDER BY sum of matches DESC (weighted by IDF tier) so high-overlap recipes
    // with rare ingredients survive the LIMIT 500 cut.
    // Params: like_params (for WHERE) ++ like_params (for ORDER BY).
    let sql = format!(
        "SELECT id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, \
         ingredients_core, directions \
         FROM recipes WHERE ({}){} ORDER BY ({}) DESC LIMIT 500",
        where_parts.join(" OR "),
        filter_sql,
        order_parts.join(" + "),
    );

    let like_params: Vec<String> = user_ings
        .iter()
        .map(|i| format!("%{}%", i.name))
        .collect();

    // Params doubled: once for WHERE, once for ORDER BY CASE expressions.
    let all_params: Vec<&String> = like_params.iter().chain(like_params.iter()).collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    // Phase 1: collect raw column tuples from SQLite (sequential — rusqlite requires it).
    // Column indices: 0=id, 1=title, 2=cuisine, 3=ingredient_count,
    //                 4=vegetarian, 5=vegan, 6=gluten_free, 7=ingredients_core, 8=directions
    let raw_rows: Vec<RawRow> = stmt
        .query_map(rusqlite::params_from_iter(all_params.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)? != 0,
                row.get::<_, i64>(5)? != 0,
                row.get::<_, i64>(6)? != 0,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Phase 2: score candidates in parallel across rayon's thread pool.
    // score_v2 is pure (no shared mutable state); user_ings is Sync.
    let cuisine_req = body.cuisine.clone();
    let mut rows: Vec<CandidateRow> = raw_rows
        .into_par_iter()
        .map(|(id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, core_raw, dir_raw)| {
            let ingredients_core = parse_json_str_array(&core_raw);
            let directions = parse_json_str_array(&dir_raw);
            let (mut score, matched_ingredients) = score_v2(user_ings, &ingredients_core, &title);
            // Cuisine boost: 1.25× if recipe cuisine matches user preference (partial, case-insensitive)
            if let (Some(ref req), Some(ref rec)) = (&cuisine_req, &cuisine) {
                let rl = rec.to_lowercase();
                let rql = req.to_lowercase();
                if rl.contains(rql.as_str()) || rql.contains(rl.as_str()) {
                    score *= 1.25;
                }
            }
            // Pantry-heavy penalty: recipes whose ingredients_core is tiny relative to their
            // total ingredient count are likely baking recipes where pantry staples are required
            // (flour, sugar, baking powder, etc.) and cannot be improvised. Suppress them.
            // pantry_ratio=0.875 (tea cakes) → ×0.46; pantry_ratio=0.3 (stir-fry) → ×0.94.
            let total = ingredient_count as f64;
            let core_len = ingredients_core.len() as f64;
            if total > 0.0 {
                let pantry_ratio = ((total - core_len) / total).clamp(0.0, 1.0);
                score *= (1.0 - pantry_ratio.powi(2) * 0.7).max(0.1);
            }
            let match_count = matched_ingredients.len();
            CandidateRow { id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, ingredients_core, directions, score, match_count, matched_ingredients }
        })
        .filter(|r| r.match_count > 0)
        .collect();

    rows.sort_unstable_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.ingredient_count.cmp(&b.ingredient_count))
    });
    rows.truncate(20);

    // Relaxation: if fewer than 3 results, broaden with stem variants (plural/singular).
    // Always filters to match_count > 0 — never returns 0-match recipes.
    if rows.len() < 3 {
        // Expand each ingredient name into its stem variants and flatten into LIKE params.
        let relaxed_like: Vec<String> = user_ings
            .iter()
            .flat_map(|ui| stem_variants(&ui.name))
            .map(|v| format!("%{v}%"))
            .collect();

        if relaxed_like.is_empty() {
            return Ok(rows);
        }

        let rn = relaxed_like.len();
        let r_where: Vec<&str> = vec![exists_clause; rn];
        // Use equal weight for relaxed variants (we only need best 5, quality over speed)
        let r_order: Vec<String> = (0..rn)
            .map(|_| format!("CASE WHEN {exists_clause} THEN 1 ELSE 0 END"))
            .collect();
        let relaxed_sql = format!(
            "SELECT id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, \
             ingredients_core, directions \
             FROM recipes WHERE ({}){} ORDER BY ({}) DESC LIMIT 200",
            r_where.join(" OR "),
            filter_sql,
            r_order.join(" + "),
        );
        let relaxed_all: Vec<&String> = relaxed_like.iter().chain(relaxed_like.iter()).collect();

        let mut stmt2 = conn.prepare(&relaxed_sql).map_err(|e| e.to_string())?;
        // Phase 1: collect raw rows (sequential)
        let relaxed_raw: Vec<RawRow> = stmt2
            .query_map(rusqlite::params_from_iter(relaxed_all.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)? != 0,
                    row.get::<_, i64>(5)? != 0,
                    row.get::<_, i64>(6)? != 0,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        // Phase 2: score in parallel
        let mut relaxed: Vec<CandidateRow> = relaxed_raw
            .into_par_iter()
            .map(|(id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, core_raw, dir_raw)| {
                let ingredients_core = parse_json_str_array(&core_raw);
                let directions = parse_json_str_array(&dir_raw);
                let (mut score, matched_ingredients) = score_v2(user_ings, &ingredients_core, &title);
                if let (Some(ref req), Some(ref rec)) = (&cuisine_req, &cuisine) {
                    let rl = rec.to_lowercase();
                    let rql = req.to_lowercase();
                    if rl.contains(rql.as_str()) || rql.contains(rl.as_str()) {
                        score *= 1.25;
                    }
                }
                let total = ingredient_count as f64;
                let core_len = ingredients_core.len() as f64;
                if total > 0.0 {
                    let pantry_ratio = ((total - core_len) / total).clamp(0.0, 1.0);
                    score *= (1.0 - pantry_ratio.powi(2) * 0.7).max(0.1);
                }
                let match_count = matched_ingredients.len();
                CandidateRow { id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, ingredients_core, directions, score, match_count, matched_ingredients }
            })
            // Always filter — never return 0-match recipes even in relaxation
            .filter(|r| r.match_count > 0)
            .collect();

        relaxed.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.ingredient_count.cmp(&b.ingredient_count))
        });
        relaxed.truncate(5);
        return Ok(relaxed);
    }

    Ok(rows)
}

// ---------------------------------------------------------------------------
// Groq shortlist call
// ---------------------------------------------------------------------------

async fn call_groq_shortlist(
    state: &AppState,
    candidates: &[CandidateRow],
    user_ings: &[IngredientQty],
) -> Result<Vec<ShortlistEntry>, String> {
    let candidate_lines: Vec<String> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let first_step = c
                .directions
                .first()
                .map(|s| s.chars().take(70).collect::<String>())
                .unwrap_or_default();
            let step_count = c.directions.len();
            format!(
                "{}. [id:{}] {} (score:{:.1}, {} steps) — {} | First step: {}",
                i + 1,
                c.id,
                c.title,
                c.score,
                step_count,
                c.ingredients_core.join(", "),
                first_step,
            )
        })
        .collect();

    let user_ing_lines: Vec<String> = user_ings
        .iter()
        .map(|i| format!("- {} ({})", i.name, i.qty))
        .collect();

    let system_prompt = r#"You are a recipe selection assistant. Given candidate recipes and the user's ingredients (with quantity signals), select up to 2 recipes per theme (Light, Filling, Quick) — maximum 6 total.

Quantity signals:
- "plenty" = user has a lot; this ingredient should be prominent in the dish
- "1 qty" = standard amount
- "a little" = small amount; works best as a supporting or garnish ingredient

Theme rules:
- Quick: under 20 min estimated OR 5 or fewer core ingredients OR few steps with fast first action (e.g. "heat oil", "toss", "mix")
- Light: salads, soups, eggs, fish, or clearly low-carb dishes
- Filling: hearty mains, rice/pasta/noodle dishes, stews, everything else

Selection priority:
1. Prefer recipes that feature the user's RAREST or most specific ingredients (e.g. if user has udon noodles, pick udon-centred dishes over generic noodle dishes)
2. "plenty" ingredients MUST be a primary component of the dish — not just used as a garnish or sauce
3. HARD RULE: if any ingredient is marked "plenty", at least one selected recipe per theme must contain that ingredient. Do not fill a theme slot with a recipe that only matches on common/generic ingredients (eggs, cheese, butter) when a more specific ingredient (udon, kimchi, tofu) is available
4. Minimise missing ingredients — prefer recipes where the user already has most of what's needed
5. Use step count and first step wording to judge Quick vs Filling accurately

Return ONLY valid JSON — no markdown fences, no commentary:
{
  "results": [
    {
      "id": <integer recipe id>,
      "theme": "Light" | "Filling" | "Quick",
      "reason": "<one casual sentence, max 12 words, mention the key ingredient that made this a good pick>"
    }
  ]
}

Rules:
- Only include recipes from the provided list (use the exact id)
- At most 2 per theme; maximum 6 total
- Do not repeat the same recipe in multiple themes"#;

    let user_message = format!(
        "User has:\n{}\n\nCandidates (sorted by relevance, best first):\n{}",
        user_ing_lines.join("\n"),
        candidate_lines.join("\n"),
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

    let candidate_map: std::collections::HashMap<i64, &CandidateRow> =
        candidates.iter().map(|c| (c.id, c)).collect();

    let mut theme_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut results: Vec<ShortlistEntry> = Vec::new();
    let mut seen_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

    for item in groq_results {
        let id = item["id"].as_i64().unwrap_or(-1);
        let theme = item["theme"].as_str().unwrap_or("").to_string();
        let reason = item["reason"].as_str().map(String::from);

        if id < 0 || theme.is_empty() {
            continue;
        }
        if seen_ids.contains(&id) {
            continue;
        }
        let count = theme_counts.entry(theme.clone()).or_insert(0);
        if *count >= 2 {
            continue;
        }

        if let Some(c) = candidate_map.get(&id) {
            let missing_count =
                c.ingredient_count as usize - c.match_count.min(c.ingredient_count as usize);
            results.push(ShortlistEntry {
                id: c.id,
                title: c.title.clone(),
                theme: Some(theme.clone()),
                reason,
                match_score: c.match_count,
                missing_count,
                ingredient_count: c.ingredient_count,
                vegetarian: c.vegetarian,
                vegan: c.vegan,
                gluten_free: c.gluten_free,
                matched_ingredients: c.matched_ingredients.clone(),
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

#[cfg(test)]
mod identify_tests {
    use super::*;
    use serde_json::json;

    fn make_body(text: &str) -> Value {
        json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": text }]
                }
            }]
        })
    }

    #[test]
    fn parse_gemini_response_full() {
        let body = make_body(
            r#"{"detected":[{"name":"chicken","confidence":9.2},{"name":"broccoli","confidence":7.1}],"likely_have":["salt","oil","garlic"],"legend":{"high":"clearly visible","mid":"partially visible","low":"hard to tell"}}"#,
        );
        let (detected, suggestions, legend) = parse_gemini_response(&body).unwrap();
        assert_eq!(detected.len(), 2);
        assert_eq!(detected[0].name, "chicken");
        assert!((detected[0].confidence - 9.2).abs() < 0.01);
        assert_eq!(detected[1].name, "broccoli");
        assert_eq!(suggestions, vec!["salt", "oil", "garlic"]);
        assert_eq!(legend["high"], "clearly visible");
    }

    #[test]
    fn parse_gemini_response_deduplicates_suggestions() {
        // "garlic" appears in both detected and likely_have — must be removed from suggestions.
        let body = make_body(
            r#"{"detected":[{"name":"chicken","confidence":8.0},{"name":"garlic","confidence":6.0}],"likely_have":["garlic","salt","oil"],"legend":{"high":"h","mid":"m","low":"l"}}"#,
        );
        let (_, suggestions, _) = parse_gemini_response(&body).unwrap();
        assert!(!suggestions.contains(&"garlic".to_string()));
        assert!(suggestions.contains(&"salt".to_string()));
        assert!(suggestions.contains(&"oil".to_string()));
    }

    #[test]
    fn parse_gemini_response_empty_detected_errors() {
        let body = make_body(
            r#"{"detected":[],"likely_have":["salt"],"legend":{"high":"h","mid":"m","low":"l"}}"#,
        );
        assert!(parse_gemini_response(&body).is_err());
    }

    #[test]
    fn parse_gemini_response_missing_likely_have() {
        // likely_have key absent — should still succeed with empty suggestions.
        let body = make_body(
            r#"{"detected":[{"name":"egg","confidence":9.0}],"legend":{"high":"h","mid":"m","low":"l"}}"#,
        );
        let (detected, suggestions, _) = parse_gemini_response(&body).unwrap();
        assert_eq!(detected[0].name, "egg");
        assert!(suggestions.is_empty());
    }

    #[test]
    fn parse_gemini_response_fallback_plain_array() {
        // Old format: plain JSON array — falls back to confidence 10.0 for all.
        let body = make_body(r#"["chicken", "broccoli", "garlic"]"#);
        let (detected, suggestions, _) = parse_gemini_response(&body).unwrap();
        assert_eq!(detected.len(), 3);
        assert_eq!(detected[0].name, "chicken");
        assert!((detected[0].confidence - 10.0).abs() < 0.01);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn parse_gemini_response_confidence_values_accepted() {
        // Confidence values outside 0–10 are accepted as-is (clamping is UI-only).
        let body = make_body(
            r#"{"detected":[{"name":"mystery","confidence":11.5},{"name":"trace","confidence":-0.5}],"likely_have":[],"legend":{"high":"h","mid":"m","low":"l"}}"#,
        );
        let (detected, _, _) = parse_gemini_response(&body).unwrap();
        assert_eq!(detected.len(), 2);
        assert!((detected[0].confidence - 11.5).abs() < 0.01);
        assert!((detected[1].confidence - (-0.5)).abs() < 0.01);
    }

    #[test]
    fn parse_gemini_response_missing_legend_uses_default() {
        // No legend key — should still succeed with a default legend.
        let body = make_body(
            r#"{"detected":[{"name":"tomato","confidence":8.5}],"likely_have":["salt"]}"#,
        );
        let (_, _, legend) = parse_gemini_response(&body).unwrap();
        assert!(legend.get("high").is_some());
        assert!(legend.get("mid").is_some());
        assert!(legend.get("low").is_some());
    }

    #[test]
    fn parse_gemini_response_trims_and_lowercases_names() {
        let body = make_body(
            r#"{"detected":[{"name":" Chicken Breast ","confidence":9.0}],"likely_have":[],"legend":{"high":"h","mid":"m","low":"l"}}"#,
        );
        let (detected, _, _) = parse_gemini_response(&body).unwrap();
        assert_eq!(detected[0].name, "chicken breast");
    }

    #[test]
    fn parse_gemini_response_no_json_errors() {
        let body = make_body("I can see some food in your fridge.");
        assert!(parse_gemini_response(&body).is_err());
    }

    // --- Rate limiter semaphore behaviour ---

    #[test]
    fn rate_limit_try_acquire_fails_when_no_permits() {
        // Zero-permit semaphore: try_acquire must fail immediately.
        let sem = tokio::sync::Semaphore::new(0);
        assert!(sem.try_acquire().is_err());
    }

    #[test]
    fn rate_limit_try_acquire_succeeds_then_second_fails() {
        // One permit: first succeeds, second fails while first is held.
        let sem = tokio::sync::Semaphore::new(1);
        let permit = sem.try_acquire().expect("first acquire should succeed");
        assert!(sem.try_acquire().is_err(), "second acquire should fail while permit held");
        drop(permit);
    }

    #[test]
    fn rate_limit_permit_released_on_drop() {
        // After dropping the permit, the semaphore restores its count.
        let sem = tokio::sync::Semaphore::new(1);
        let permit = sem.try_acquire().expect("acquire should succeed");
        drop(permit);
        assert!(sem.try_acquire().is_ok(), "re-acquire after drop should succeed");
    }
}

#[cfg(test)]
mod scoring_tests {
    use super::*;

    fn ing(name: &str, qty: &str) -> IngredientQty {
        IngredientQty { name: name.to_string(), qty: qty.to_string() }
    }

    #[test]
    fn test_tokens_overlap_no_false_positives() {
        // egg must NOT match eggplant
        assert!(!tokens_overlap("eggplant", "egg"));
        assert!(!tokens_overlap("egg", "eggplant"));
        // pea must NOT match peanut
        assert!(!tokens_overlap("peanut", "pea"));
        assert!(!tokens_overlap("pea", "peanut"));
        // chicken MUST match "chicken breast" (user typed more specific form)
        assert!(tokens_overlap("chicken", "chicken breast"));
        assert!(tokens_overlap("chicken breast", "chicken"));
        // udon MUST match "udon noodles"
        assert!(tokens_overlap("udon noodles", "udon"));
        // exact match works
        assert!(tokens_overlap("egg", "egg"));
        // no overlap
        assert!(!tokens_overlap("kimchi", "pasta"));
    }

    #[test]
    fn test_rarity_idf_no_false_positives() {
        // eggplant is niche — must NOT inherit egg's IDF via substring
        assert_eq!(rarity_idf("eggplant"), 9.0);
        // peanut is niche — must NOT inherit pea's IDF
        assert_eq!(rarity_idf("peanut butter"), 9.0);
        // chicken breast — token "chicken" is ultra-common
        assert_eq!(rarity_idf("chicken breast"), 2.0);
        // scrambled eggs — token "eggs" is COMMON
        assert_eq!(rarity_idf("scrambled eggs"), 4.5);
    }

    #[test]
    fn test_score_v2_no_eggplant_false_positive() {
        let user = vec![ing("egg", "1 qty")];
        let core = vec!["eggplant".to_string(), "tomato".to_string(), "mozzarella".to_string()];
        let (score, matched) = score_v2(&user, &core, "Eggplant Parmesan");
        assert_eq!(matched.len(), 0, "egg must not match eggplant (word boundary fix)");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_idf_sql_weight_tiers() {
        assert_eq!(idf_sql_weight("kimchi"), 3);
        assert_eq!(idf_sql_weight("udon"), 3);
        assert_eq!(idf_sql_weight("tahini"), 3);
        assert_eq!(idf_sql_weight("egg"), 2);
        assert_eq!(idf_sql_weight("eggs"), 2);
        assert_eq!(idf_sql_weight("cheese"), 2);
        assert_eq!(idf_sql_weight("chicken"), 1);
        assert_eq!(idf_sql_weight("beef"), 1);
        assert_eq!(idf_sql_weight("potato"), 1);
    }

    #[test]
    fn test_rarity_idf_tiers() {
        assert_eq!(rarity_idf("chicken"), 2.0);
        assert_eq!(rarity_idf("chicken breast"), 2.0);
        assert_eq!(rarity_idf("egg"), 4.5);
        assert_eq!(rarity_idf("eggs"), 4.5);
        assert_eq!(rarity_idf("udon"), 9.0);
        assert_eq!(rarity_idf("kimchi"), 9.0);
        assert_eq!(rarity_idf("tahini"), 9.0);
    }

    #[test]
    fn test_qty_weight() {
        assert_eq!(qty_weight("plenty"), 1.3);
        assert_eq!(qty_weight("a little"), 0.7);
        assert_eq!(qty_weight("1 qty"), 1.0);
        assert_eq!(qty_weight("unknown"), 1.0);
    }

    #[test]
    fn test_score_v2_udon_beats_turkey_loaf() {
        let user = vec![
            ing("udon", "plenty"),
            ing("egg", "1 qty"),
            ing("vegetables", "a little"),
        ];

        // Udon Noodle Soup: 4 ingredients, matches udon + egg, title contains "udon"
        let udon_core = vec!["udon".to_string(), "dashi".to_string(), "scallion".to_string(), "egg".to_string()];
        let (udon_score, udon_matched) = score_v2(&user, &udon_core, "Udon Noodle Soup");

        // Barbecue Turkey Loaf: 8 ingredients, only egg matches, no title match
        let turkey_core = vec![
            "chicken stuffing".to_string(), "water".to_string(), "butter".to_string(),
            "barbecue sauce".to_string(), "american cheese".to_string(), "ground turkey".to_string(),
            "egg".to_string(), "breadcrumbs".to_string(),
        ];
        let (turkey_score, turkey_matched) = score_v2(&user, &turkey_core, "Barbecue Turkey Loaf");

        assert!(udon_matched.len() >= 2, "udon soup should match at least 2 ingredients");
        assert_eq!(turkey_matched.len(), 1, "turkey loaf should only match egg");
        let _udon_count = udon_matched.len();
        let _turkey_count = turkey_matched.len();
        assert!(
            udon_score > turkey_score * 10.0,
            "udon soup ({udon_score:.2}) should score at least 10x turkey loaf ({turkey_score:.2})"
        );
    }

    #[test]
    fn test_score_v2_title_bonus() {
        let user = vec![ing("udon", "1 qty")];
        let core = vec!["udon".to_string(), "broth".to_string()];

        let (with_title, _) = score_v2(&user, &core, "Udon Soup");
        let (without_title, _) = score_v2(&user, &core, "Noodle Soup");

        assert!(with_title > without_title, "title match should boost score");
        // title bonus scales with IDF: udon (IDF 9.0) → bonus 5.4 × coverage × missing ≈ 1.67
        assert!(with_title > without_title + 1.0, "title bonus should add meaningful score");
    }

    #[test]
    fn test_title_bonus_scales_with_rarity() {
        // Rare ingredient in title should get bigger absolute boost than common ingredient
        let user_rare = vec![ing("kimchi", "1 qty")];
        let user_common = vec![ing("chicken", "1 qty")];
        let core_rare = vec!["kimchi".to_string(), "rice".to_string()];
        let core_common = vec!["chicken".to_string(), "rice".to_string()];

        let (rare_with, _) = score_v2(&user_rare, &core_rare, "Kimchi Fried Rice");
        let (rare_without, _) = score_v2(&user_rare, &core_rare, "Fried Rice");
        let (common_with, _) = score_v2(&user_common, &core_common, "Chicken Fried Rice");
        let (common_without, _) = score_v2(&user_common, &core_common, "Fried Rice");

        let rare_bonus = rare_with - rare_without;
        let common_bonus = common_with - common_without;
        assert!(
            rare_bonus > common_bonus,
            "kimchi title bonus ({rare_bonus:.2}) should exceed chicken title bonus ({common_bonus:.2})"
        );
    }

    #[test]
    fn test_title_bonus_no_eggplant_false_positive() {
        let user = vec![ing("egg", "1 qty")];
        // "Eggplant Parmesan" title has no "egg" token — must get no title bonus
        let core_eggplant = vec!["eggplant".to_string(), "tomato".to_string()];
        let (score_eggplant_title, _) = score_v2(&user, &core_eggplant, "Eggplant Parmesan");
        // Score should be 0 — no match in ingredients either (thanks to Fix 1)
        assert_eq!(score_eggplant_title, 0.0, "egg must not match eggplant in title or ingredients");

        // "Scrambled Egg" title has "egg" token — must get title bonus
        let core_egg = vec!["egg".to_string(), "butter".to_string()];
        let (with_bonus, _) = score_v2(&user, &core_egg, "Scrambled Egg");
        let (without_bonus, _) = score_v2(&user, &core_egg, "Scrambled Dish");
        assert!(with_bonus > without_bonus, "egg should get title bonus when title contains 'egg' token");
    }

    #[test]
    fn test_stem_variants_plurals() {
        // -ies: berries → berry
        let v = stem_variants("berries");
        assert!(v.contains(&"berry".to_string()), "berries should stem to berry");
        // -s: eggs → egg
        let v2 = stem_variants("eggs");
        assert!(v2.contains(&"egg".to_string()), "eggs should stem to egg");
        // Add plural: egg → eggs
        let v3 = stem_variants("egg");
        assert!(v3.contains(&"eggs".to_string()), "egg should get eggs variant");
        // -es: tomatoes → tomato
        let v4 = stem_variants("tomatoes");
        assert!(v4.contains(&"tomato".to_string()), "tomatoes should stem to tomato");
        // Add plural: potato → potatoes
        let v5 = stem_variants("potato");
        assert!(v5.contains(&"potatos".to_string()) || v5.contains(&"potato".to_string()));
        // Original always included
        for name in &["udon", "kimchi", "chicken"] {
            let variants = stem_variants(name);
            assert!(variants.contains(&name.to_string()), "{name} should be in its own variants");
        }
    }

    #[test]
    fn test_score_v2_nonzero_when_matched() {
        // If any ingredient matches, score must be > 0
        let user = vec![ing("chicken", "1 qty")];
        let core = vec!["chicken".to_string(), "rice".to_string()];
        let (score, matched) = score_v2(&user, &core, "Chicken Rice");
        assert!(!matched.is_empty(), "chicken should match");
        assert!(score > 0.0, "matched recipe must have score > 0");
    }

    #[test]
    fn test_cuisine_partial_match_logic() {
        // Verify partial match string logic used in cuisine boost
        let req = "asian";
        let rec = "asian-chinese";
        // word_tokens splits on '-', so "asian" token appears in both — partial match holds
        assert!(rec.to_lowercase().contains(req) || req.contains(&rec.to_lowercase()));

        let req2 = "thai";
        let rec2 = "Thai";
        assert!(rec2.to_lowercase().contains(req2));

        // "Italian" should not match "Asian"
        let req3 = "italian";
        let rec3 = "asian";
        assert!(!rec3.contains(req3) && !req3.contains(rec3));
    }

    #[test]
    fn test_score_v2_coverage_penalty() {
        let user = vec![ing("egg", "1 qty"), ing("udon", "1 qty"), ing("spinach", "1 qty")];

        // 4-ingredient recipe, 3 match → high coverage
        let small = vec!["egg".to_string(), "udon".to_string(), "spinach".to_string(), "soy sauce".to_string()];
        // 12-ingredient recipe, 3 match → low coverage
        let large: Vec<String> = vec![
            "egg".to_string(), "udon".to_string(), "spinach".to_string(),
            "a".to_string(), "b".to_string(), "c".to_string(),
            "d".to_string(), "e".to_string(), "f".to_string(),
            "g".to_string(), "h".to_string(), "i".to_string(),
        ];

        let (small_score, _small_m) = score_v2(&user, &small, "Test");
        let (large_score, _large_m) = score_v2(&user, &large, "Test");

        assert!(small_score > large_score, "smaller recipe with same match count should score higher");
    }

    #[test]
    fn test_score_v2_no_match() {
        let user = vec![ing("udon", "1 qty")];
        let core = vec!["chicken".to_string(), "rice".to_string()];
        let (score, matched) = score_v2(&user, &core, "Chicken Rice");
        assert_eq!(matched.len(), 0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_score_v2_empty_recipe() {
        let user = vec![ing("udon", "1 qty")];
        let (score, matched) = score_v2(&user, &[], "Empty");
        assert_eq!(matched.len(), 0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_is_pantry_staple() {
        assert!(is_pantry_staple_ai("salt"));
        assert!(is_pantry_staple_ai("butter"));
        assert!(is_pantry_staple_ai("olive oil"));
        assert!(!is_pantry_staple_ai("udon"));
        assert!(!is_pantry_staple_ai("egg"));
    }

    #[test]
    fn test_resolve_user_ings_filters_pantry() {
        let body = ShortlistRequest {
            ingredients: vec!["udon".to_string(), "salt".to_string(), "butter".to_string()],
            ingredients_with_qty: None,
            vegetarian: None,
            vegan: None,
            gluten_free: None,
            cuisine: None,
        };
        let result = resolve_user_ings(&body);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "udon");
        assert_eq!(result[0].qty, "1 qty");
    }

    #[test]
    fn test_resolve_user_ings_uses_qty() {
        let body = ShortlistRequest {
            ingredients: vec!["udon".to_string()],
            ingredients_with_qty: Some(vec![
                IngredientQty { name: "udon".to_string(), qty: "plenty".to_string() },
            ]),
            vegetarian: None,
            vegan: None,
            gluten_free: None,
            cuisine: None,
        };
        let result = resolve_user_ings(&body);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].qty, "plenty");
    }

    #[test]
    fn test_parse_json_str_array() {
        let raw = r#"["udon","egg","scallion"]"#;
        let result = parse_json_str_array(raw);
        assert_eq!(result, vec!["udon", "egg", "scallion"]);
    }

    #[test]
    fn test_parse_json_str_array_empty() {
        let result = parse_json_str_array("invalid json");
        assert!(result.is_empty());
    }

    // ---------------------------------------------------------------------------
    // Pantry penalty tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_pantry_penalty_values() {
        let calc = |total: f64, core: f64| -> f64 {
            let r = ((total - core) / total).max(0.0).min(1.0);
            (1.0 - r.powi(2) * 0.7).max(0.1)
        };
        // Tea Cakes: 8 total, 1 core → pantry_ratio=0.875 → heavy penalty
        let tea_cake = calc(8.0, 1.0);
        assert!(tea_cake < 0.55, "tea cake penalty should be severe: {tea_cake:.3}");
        // Normal stir-fry: 10 total, 7 core → pantry_ratio=0.3 → barely penalised
        let normal = calc(10.0, 7.0);
        assert!(normal > 0.9, "normal recipe barely penalized: {normal:.3}");
        // Clean recipe: all core, no pantry → no penalty
        let clean = calc(8.0, 8.0);
        assert_eq!(clean, 1.0, "no-pantry recipe gets no penalty");
    }

    #[test]
    fn test_baking_recipe_suppressed_vs_real_match() {
        let user = vec![ing("egg", "1 qty"), ing("spinach", "1 qty")];

        // Tea Cakes: 8 total ingredients, only "egg" survived pantry stripping into core.
        // Pantry penalty applied externally (as fetch_candidates does it).
        let tea_core = vec!["egg".to_string()];
        let (mut tea_score, _) = score_v2(&user, &tea_core, "Old-Fashioned Tea Cakes");
        let r = ((8.0_f64 - 1.0) / 8.0).max(0.0).min(1.0);
        tea_score *= (1.0 - r.powi(2) * 0.7_f64).max(0.1);

        // Egg & spinach scramble: 3 total, 3 core (no pantry staples stripped).
        // Pantry penalty applied externally.
        let scramble_core = vec!["egg".to_string(), "spinach".to_string(), "feta".to_string()];
        let (mut scramble_score, _) = score_v2(&user, &scramble_core, "Egg and Spinach Scramble");
        let r2 = ((3.0_f64 - 3.0) / 3.0).max(0.0).min(1.0);
        scramble_score *= (1.0 - r2.powi(2) * 0.7_f64).max(0.1);

        assert!(
            scramble_score > tea_score,
            "egg+spinach scramble ({scramble_score:.3}) should rank above tea cakes ({tea_score:.3}) after pantry penalty"
        );
    }

    // ---------------------------------------------------------------------------
    // Anchor boost tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_anchor_boost_udon_title() {
        let user = vec![ing("udon", "1 qty"), ing("vegetable", "1 qty")];

        // Yaki Udon: udon in title AND first core item → full anchor boost
        let yaki_core = vec!["udon".to_string(), "vegetables".to_string()];
        let (yaki_score, _) = score_v2(&user, &yaki_core, "Yaki Udon");

        // Chicken soup with udon buried in the middle — udon not the star
        let soup_core = vec![
            "chicken".to_string(), "mushroom".to_string(),
            "bok choy".to_string(), "udon".to_string(),
        ];
        let (soup_score, _) = score_v2(&user, &soup_core, "Chicken Mushroom Soup");

        assert!(
            yaki_score > soup_score,
            "yaki udon ({yaki_score:.3}) should rank above chicken soup ({soup_score:.3})"
        );
    }

    #[test]
    fn test_anchor_boost_kimchi() {
        let user = vec![ing("kimchi", "1 qty"), ing("egg", "1 qty")];

        // Kimchi Fried Rice: kimchi in title and first core item
        let kimchi_core = vec!["kimchi".to_string(), "egg".to_string(), "rice".to_string()];
        let (kimchi_score, _) = score_v2(&user, &kimchi_core, "Kimchi Fried Rice");

        // Generic egg fried rice: kimchi absent
        let plain_core = vec![
            "egg".to_string(), "rice".to_string(),
            "carrot".to_string(), "peas".to_string(),
        ];
        let (plain_score, _) = score_v2(&user, &plain_core, "Egg Fried Rice");

        assert!(
            kimchi_score > plain_score,
            "kimchi fried rice ({kimchi_score:.3}) should beat plain egg fried rice ({plain_score:.3})"
        );
    }

    // ---------------------------------------------------------------------------
    // Simplicity reward tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_simplicity_reward() {
        let user = vec![ing("egg", "1 qty"), ing("vegetable", "1 qty")];

        // Simple 2-ingredient recipe: 2/2 matched
        let simple_core = vec!["egg".to_string(), "vegetable".to_string()];
        let (simple_score, _) = score_v2(&user, &simple_core, "Egg and Vegetable");

        // Complex 8-ingredient recipe: same 2 matched, 6 missing
        let complex_core = vec![
            "egg".to_string(), "vegetable".to_string(), "tofu".to_string(),
            "kimchi".to_string(), "sesame".to_string(), "ginger".to_string(),
            "scallion".to_string(), "chili".to_string(),
        ];
        let (complex_score, _) = score_v2(&user, &complex_core, "Kimchi Tofu Egg Bowl");

        assert!(
            simple_score > complex_score,
            "simple 2-ing recipe ({simple_score:.3}) should beat complex 8-ing recipe ({complex_score:.3})"
        );
    }

    // ---------------------------------------------------------------------------
    // Pantry focus tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_pantry_focus_diverse_vs_focused() {
        // Same recipe: requires udon + egg + scallion
        let recipe_core = vec![
            "udon".to_string(),
            "egg".to_string(),
            "scallion".to_string(),
            "dashi".to_string(),
        ];
        // Focused pantry: 3 ingredients, all relevant — user_n=3 ≤ 5, pantry_focus=1.0
        let focused_user = vec![
            ing("udon", "1 qty"),
            ing("egg", "1 qty"),
            ing("scallion", "1 qty"),
        ];
        // Diverse pantry: 10 ingredients, 3 relevant — user_n=10 > 5, pantry_focus=sqrt(0.3)≈0.548
        let diverse_user = vec![
            ing("udon", "1 qty"),
            ing("egg", "1 qty"),
            ing("scallion", "1 qty"),
            ing("salmon", "1 qty"),
            ing("chocolate", "1 qty"),
            ing("strawberry", "1 qty"),
            ing("pasta", "1 qty"),
            ing("kimchi", "1 qty"),
            ing("broccoli", "1 qty"),
            ing("tofu", "1 qty"),
        ];
        let (focused_score, focused_matched) =
            score_v2(&focused_user, &recipe_core, "Udon Noodle Soup");
        let (diverse_score, diverse_matched) =
            score_v2(&diverse_user, &recipe_core, "Udon Noodle Soup");
        assert_eq!(focused_matched.len(), 3, "focused user should match 3 ingredients");
        assert_eq!(diverse_matched.len(), 3, "diverse user should also match 3 ingredients");
        assert!(
            focused_score > diverse_score,
            "focused pantry ({focused_score:.3}) should score higher than diverse pantry ({diverse_score:.3}) for same recipe"
        );
    }

    #[test]
    fn test_pantry_focus_small_pantry_not_penalised() {
        // user_n=4 and user_n=5 must produce identical scores (guard user_n > 5 fires for both)
        let recipe_core = vec!["kimchi".to_string(), "egg".to_string(), "rice".to_string()];
        let user_5 = vec![
            ing("kimchi", "1 qty"),
            ing("egg", "1 qty"),
            ing("salmon", "1 qty"),
            ing("broccoli", "1 qty"),
            ing("udon", "1 qty"),
        ];
        let user_4 = vec![
            ing("kimchi", "1 qty"),
            ing("egg", "1 qty"),
            ing("salmon", "1 qty"),
            ing("broccoli", "1 qty"),
        ];
        let (score_5, matched_5) = score_v2(&user_5, &recipe_core, "Kimchi Egg Rice");
        let (score_4, matched_4) = score_v2(&user_4, &recipe_core, "Kimchi Egg Rice");
        assert_eq!(matched_5.len(), 2);
        assert_eq!(matched_4.len(), 2);
        assert_eq!(
            score_5, score_4,
            "small pantries (≤5 ings) must not be penalised: score_5={score_5:.4}, score_4={score_4:.4}"
        );
    }

    #[test]
    fn test_diverse_pantry_prefers_more_matches() {
        // Diverse pantry: 10 non-staple ingredients
        let user = vec![
            ing("chicken", "1 qty"),
            ing("pasta", "1 qty"),
            ing("broccoli", "1 qty"),
            ing("egg", "1 qty"),
            ing("kimchi", "1 qty"),
            ing("salmon", "1 qty"),
            ing("tofu", "1 qty"),
            ing("udon", "1 qty"),
            ing("strawberry", "1 qty"),
            ing("chocolate", "1 qty"),
        ];
        // Recipe A: uses 6 of the user's ingredients (8 total recipe ingredients)
        let recipe_a_core = vec![
            "chicken".to_string(),
            "pasta".to_string(),
            "broccoli".to_string(),
            "egg".to_string(),
            "kimchi".to_string(),
            "salmon".to_string(),
            "cream".to_string(),
            "mushroom".to_string(),
        ];
        // Recipe B: uses 2 of the user's ingredients (5 total — higher recipe-centric coverage)
        let recipe_b_core = vec![
            "udon".to_string(),
            "tofu".to_string(),
            "wakame".to_string(),
            "miso".to_string(),
            "scallion".to_string(),
        ];
        let (score_a, matched_a) = score_v2(&user, &recipe_a_core, "Chicken Pasta");
        let (score_b, matched_b) = score_v2(&user, &recipe_b_core, "Udon Tofu Soup");
        assert_eq!(matched_a.len(), 6, "recipe A should match 6 user ingredients");
        assert_eq!(matched_b.len(), 2, "recipe B should match 2 user ingredients");
        assert!(
            score_a > score_b,
            "recipe using more of the user's diverse pantry ({score_a:.3}) should beat one using fewer ({score_b:.3})"
        );
    }

    // ---------------------------------------------------------------------------
    // FTS5 helper tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_build_fts_match_single() {
        let ings = vec![ing("egg", "1 qty")];
        let expr = build_fts_match(&ings);
        assert_eq!(expr, "\"egg\"");
    }

    #[test]
    fn test_build_fts_match_multiple() {
        let ings = vec![ing("egg", "1 qty"), ing("sour cream", "a little"), ing("udon", "plenty")];
        let expr = build_fts_match(&ings);
        assert_eq!(expr, "\"egg\" OR \"sour cream\" OR \"udon\"");
    }

    #[test]
    fn test_build_fts_match_strips_quotes() {
        // Double-quotes in ingredient names must be stripped to avoid breaking MATCH syntax.
        let ings = vec![ing("\"fancy\" cheese", "1 qty")];
        let expr = build_fts_match(&ings);
        assert_eq!(expr, "\"fancy cheese\"");
    }

    #[test]
    fn test_build_fts_match_relaxed_expands_variants() {
        let ings = vec![ing("egg", "1 qty")];
        let expr = build_fts_match_relaxed(&ings);
        // stem_variants("egg") → ["egg", "eggs"]
        assert!(expr.contains("\"egg\""), "should include original: {expr}");
        assert!(expr.contains("\"eggs\""), "should include plural: {expr}");
    }

    #[test]
    fn test_build_fts_match_relaxed_berry_variant() {
        let ings = vec![ing("berries", "1 qty")];
        let expr = build_fts_match_relaxed(&ings);
        // stem_variants("berries") → ["berries", "berry"]
        assert!(expr.contains("\"berry\""), "should include singular: {expr}");
        assert!(expr.contains("\"berries\""), "should include original: {expr}");
    }
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

Substitutions: include an entry for every ingredient where have=false. Do not include pantry staples (salt, pepper, oil, butter, flour, sugar, common spices). For substitutes, always use the most general category-level description possible — never suggest a specific brand or named product. Examples: say "any neutral oil" not "vegetable oil", "any acid" not "white wine vinegar", "any crunchy pickle" not "cornichons", "any mild fresh chilli" not "serrano pepper". If an ingredient is regional or specialty (e.g. "Texas style hot pickled okra pods"), describe the category it fills in the dish ("any pickled vegetable for crunch and acidity") rather than guessing a direct swap.
Steps: rewrite each direction as a clean imperative sentence. Preserve all detail but remove filler phrases."#;

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

    // Strip substitutions for pantry staples — Groq sometimes suggests swaps for
    // flour, baking powder, sugar etc. despite being instructed not to. These are
    // either trivially available or genuinely irreplaceable (batter agents), so
    // showing them as "missing" misleads users. Filter by token-level staple check
    // to catch multi-word names like "all-purpose flour" or "baking soda".
    presented.substitutions.retain(|s| {
        let ing_lower = s.ingredient.to_lowercase();
        !is_pantry_staple_ai(&ing_lower)
            && !word_tokens(&ing_lower).any(is_pantry_staple_ai)
    });

    Ok(presented)
}

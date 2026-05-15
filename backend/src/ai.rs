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

#[derive(Debug, Deserialize, Clone)]
pub struct IngredientQty {
    pub name: String,
    /// "1 qty" | "a little" | "plenty"
    pub qty: String,
}

#[derive(Debug, Deserialize)]
pub struct ShortlistRequest {
    pub ingredients: Vec<String>,
    pub ingredients_with_qty: Option<Vec<IngredientQty>>,
    pub vegetarian: Option<bool>,
    pub vegan: Option<bool>,
    pub gluten_free: Option<bool>,
    pub cuisine: Option<String>,
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
    pub matched_ingredients: Vec<String>,
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

    // Resolve qty-enriched ingredient list, falling back to plain names with default qty
    let user_ings: Vec<IngredientQty> = resolve_user_ings(&body);

    if user_ings.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "no non-pantry ingredients provided" })),
        );
    }

    let candidates = match fetch_candidates(&state.sqlite, &body, &user_ings) {
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
            missing_count: c.ingredient_count as usize - c.match_count.min(c.ingredient_count as usize),
            id: c.id,
            title: c.title,
            theme: None,
            reason: None,
            match_score: c.match_count,
            ingredient_count: c.ingredient_count,
            vegetarian: c.vegetarian,
            vegan: c.vegan,
            gluten_free: c.gluten_free,
            matched_ingredients: c.matched_ingredients,
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!(ShortlistResponse { results: fallback, groq_used: false })),
    )
}

// ---------------------------------------------------------------------------
// Pantry staples
// ---------------------------------------------------------------------------

fn is_pantry_staple_ai(ingredient: &str) -> bool {
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

/// Rarity-based IDF tier. Rare/specific ingredients score higher.
fn rarity_idf(ingredient: &str) -> f64 {
    const ULTRA_COMMON: &[&str] = &[
        "chicken", "beef", "pork", "lamb", "onion", "garlic",
        "carrot", "celery", "potato", "rice", "pasta", "tomato",
    ];
    const COMMON: &[&str] = &[
        "egg", "eggs", "cheese", "mushroom", "spinach", "broccoli",
        "corn", "bean", "beans", "lentil", "lentils", "shrimp",
    ];
    if ULTRA_COMMON.iter().any(|&s| ingredient.contains(s)) {
        2.0
    } else if COMMON.iter().any(|&s| ingredient.contains(s)) {
        4.5
    } else {
        9.0
    }
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

    for ui in user_ings {
        let name = ui.name.to_lowercase();
        let hits = recipe_lower
            .iter()
            .any(|ri| ri.contains(name.as_str()) || name.contains(ri.as_str()));
        if !hits {
            continue;
        }
        matched_names.push(ui.name.clone());
        let idf = rarity_idf(&name);
        let qty = qty_weight(&ui.qty);
        let title_bonus = if title_lower.contains(name.as_str()) { 5.0 } else { 0.0 };
        weighted_sum += idf * qty + title_bonus;
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

    (weighted_sum * coverage_factor * missing_factor, matched_names)
}

// ---------------------------------------------------------------------------
// Candidate row
// ---------------------------------------------------------------------------

struct CandidateRow {
    id: i64,
    title: String,
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

fn fetch_candidates(
    pool: &crate::SqlitePool,
    body: &ShortlistRequest,
    user_ings: &[IngredientQty],
) -> Result<Vec<CandidateRow>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;

    let n = user_ings.len();

    // One EXISTS clause per ingredient (for WHERE and ORDER BY).
    // We duplicate the params: first set for WHERE OR'd clauses, second set for ORDER BY sum.
    let exists_clause =
        "EXISTS (SELECT 1 FROM json_each(ingredients_core) WHERE LOWER(value) LIKE ?)";
    let where_parts: Vec<&str> = vec![exists_clause; n];
    let order_parts: Vec<String> = (0..n)
        .map(|_| format!("CASE WHEN {exists_clause} THEN 1 ELSE 0 END"))
        .collect();

    let mut filter_sql = String::new();
    if body.vegetarian == Some(true) { filter_sql.push_str(" AND vegetarian = 1"); }
    if body.vegan == Some(true)      { filter_sql.push_str(" AND vegan = 1"); }
    if body.gluten_free == Some(true){ filter_sql.push_str(" AND gluten_free = 1"); }

    // ORDER BY sum of matches DESC so high-overlap recipes survive the LIMIT 500 cut.
    // Params: like_params (for WHERE) ++ like_params (for ORDER BY).
    let sql = format!(
        "SELECT id, title, ingredient_count, vegetarian, vegan, gluten_free, \
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

    let mut rows: Vec<CandidateRow> = stmt
        .query_map(rusqlite::params_from_iter(all_params.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, i64>(4)? != 0,
                row.get::<_, i64>(5)? != 0,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .map(|(id, title, ingredient_count, vegetarian, vegan, gluten_free, core_raw, dir_raw)| {
            let ingredients_core = parse_json_str_array(&core_raw);
            let directions = parse_json_str_array(&dir_raw);
            let (score, matched_ingredients) = score_v2(user_ings, &ingredients_core, &title);
            let match_count = matched_ingredients.len();
            CandidateRow { id, title, ingredient_count, vegetarian, vegan, gluten_free, ingredients_core, directions, score, match_count, matched_ingredients }
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

    // Relaxation: if fewer than 3 results, return top 5 without match_count filter
    if rows.len() < 3 {
        let all_params2: Vec<&String> = like_params.iter().chain(like_params.iter()).collect();
        let mut stmt2 = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut relaxed: Vec<CandidateRow> = stmt2
            .query_map(rusqlite::params_from_iter(all_params2.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, i64>(4)? != 0,
                    row.get::<_, i64>(5)? != 0,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .map(|(id, title, ingredient_count, vegetarian, vegan, gluten_free, core_raw, dir_raw)| {
                let ingredients_core = parse_json_str_array(&core_raw);
                let directions = parse_json_str_array(&dir_raw);
                let (score, matched_ingredients) = score_v2(user_ings, &ingredients_core, &title);
                let match_count = matched_ingredients.len();
                CandidateRow { id, title, ingredient_count, vegetarian, vegan, gluten_free, ingredients_core, directions, score, match_count, matched_ingredients }
            })
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
mod scoring_tests {
    use super::*;

    fn ing(name: &str, qty: &str) -> IngredientQty {
        IngredientQty { name: name.to_string(), qty: qty.to_string() }
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
        // title bonus (+5.0) is applied before coverage/missing factors, so
        // the final difference is 5.0 × coverage_factor × missing_factor
        assert!(with_title > without_title + 1.0, "title bonus should add meaningful score");
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

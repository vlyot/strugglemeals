use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::SqlitePool;

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct RecipeSummary {
    pub id: i64,
    pub title: String,
    pub cuisine: Option<String>,
    pub ingredient_count: i64,
    pub vegetarian: bool,
    pub vegan: bool,
    pub gluten_free: bool,
    pub ingredients_core: Vec<String>,
    pub match_score: usize,
}

#[derive(Debug, Serialize)]
pub struct RecipeDetail {
    pub id: i64,
    pub title: String,
    pub cuisine: Option<String>,
    pub ingredient_count: i64,
    pub vegetarian: bool,
    pub vegan: bool,
    pub gluten_free: bool,
    pub ingredients_raw: Vec<String>,
    pub ingredients_core: Vec<String>,
    pub directions: Vec<String>,
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    /// Comma-separated ingredient names (user's pantry)
    pub ingredients: Option<String>,
    pub vegetarian: Option<bool>,
    pub vegan: Option<bool>,
    pub gluten_free: Option<bool>,
    pub cuisine: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

// ---------------------------------------------------------------------------
// Pantry staples — same list as pipeline; excluded from match scoring
// ---------------------------------------------------------------------------

const PANTRY_STAPLES: &[&str] = &[
    "salt", "black pepper", "white pepper", "pepper", "olive oil",
    "vegetable oil", "canola oil", "oil", "butter", "water", "sugar",
    "brown sugar", "flour", "all-purpose flour", "baking soda",
    "baking powder", "vanilla extract", "vanilla", "garlic powder",
    "onion powder", "paprika", "cumin", "oregano", "thyme", "basil",
    "cayenne", "red pepper flakes", "cinnamon", "nutmeg", "bay leaves",
    "bay leaf", "cooking spray", "nonstick cooking spray", "shortening",
];

fn is_pantry_staple(ingredient: &str) -> bool {
    let lower = ingredient.to_lowercase();
    PANTRY_STAPLES.contains(&lower.as_str())
}

fn parse_user_ingredients(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty() && !is_pantry_staple(s))
        .collect()
}

fn score(user_ingredients: &[String], recipe_core: &[String]) -> usize {
    let recipe_lower: Vec<String> = recipe_core.iter().map(|s| s.to_lowercase()).collect();
    user_ingredients
        .iter()
        .filter(|ui| recipe_lower.iter().any(|ri| ri.contains(ui.as_str()) || ui.contains(ri.as_str())))
        .count()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn search(
    State(pool): State<SqlitePool>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let limit = params.limit.min(100);
    let user_ingredients: Vec<String> = params
        .ingredients
        .as_deref()
        .map(parse_user_ingredients)
        .unwrap_or_default();

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("SQLite pool error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "database unavailable" })),
            );
        }
    };

    // Build query with optional dietary / cuisine filters
    // We fetch a candidate set, then rank by match score in Rust.
    let candidate_limit = (limit * 50).max(1000);

    let mut sql = String::from(
        "SELECT id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, ingredients_core \
         FROM recipes WHERE 1=1",
    );
    if params.vegetarian == Some(true) {
        sql.push_str(" AND vegetarian = 1");
    }
    if params.vegan == Some(true) {
        sql.push_str(" AND vegan = 1");
    }
    if params.gluten_free == Some(true) {
        sql.push_str(" AND gluten_free = 1");
    }
    if params.cuisine.is_some() {
        sql.push_str(" AND cuisine = ?1");
    }
    sql.push_str(&format!(" LIMIT {candidate_limit}"));

    let cuisine_val = params.cuisine.clone().unwrap_or_default();

    let mut rows: Vec<RecipeSummary> = {
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("SQLite prepare error: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "query error" })),
                );
            }
        };

        let map_row = |row: &rusqlite::Row| {
            let core_raw: String = row.get(7)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)? != 0,
                row.get::<_, i64>(5)? != 0,
                row.get::<_, i64>(6)? != 0,
                core_raw,
            ))
        };

        let iter = if params.cuisine.is_some() {
            stmt.query_map([&cuisine_val], map_row)
        } else {
            stmt.query_map([], map_row)
        };

        match iter {
            Ok(mapped) => mapped
                .filter_map(|r| r.ok())
                .map(|(id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, core_raw)| {
                    let ingredients_core: Vec<String> =
                        serde_json::from_str::<Value>(&core_raw)
                            .ok()
                            .and_then(|v| v.as_array().cloned())
                            .map(|arr| arr.into_iter().filter_map(|x| x.as_str().map(String::from)).collect())
                            .unwrap_or_default();

                    let match_score = score(&user_ingredients, &ingredients_core);

                    RecipeSummary {
                        id,
                        title,
                        cuisine,
                        ingredient_count,
                        vegetarian,
                        vegan,
                        gluten_free,
                        ingredients_core,
                        match_score,
                    }
                })
                .collect(),
            Err(e) => {
                tracing::error!("SQLite query error: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "query failed" })),
                );
            }
        }
    };

    // Sort by match score descending, then by ingredient count ascending (simpler recipes first)
    rows.sort_unstable_by(|a, b| {
        b.match_score.cmp(&a.match_score).then(a.ingredient_count.cmp(&b.ingredient_count))
    });

    rows.truncate(limit);

    (StatusCode::OK, Json(serde_json::json!({ "results": rows, "count": rows.len() })))
}

pub async fn get_one(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("SQLite pool error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "database unavailable" })),
            );
        }
    };

    let result = conn.query_row(
        "SELECT id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, \
         ingredients_raw, ingredients_core, directions FROM recipes WHERE id = ?1",
        [id],
        |row| {
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
                row.get::<_, String>(9)?,
            ))
        },
    );

    match result {
        Ok((id, title, cuisine, ingredient_count, vegetarian, vegan, gluten_free, raw_raw, core_raw, dir_raw)) => {
            let parse_json_arr = |s: &str| -> Vec<String> {
                serde_json::from_str::<Value>(s)
                    .ok()
                    .and_then(|v| v.as_array().cloned())
                    .map(|arr| arr.into_iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default()
            };

            let detail = RecipeDetail {
                id,
                title,
                cuisine,
                ingredient_count,
                vegetarian,
                vegan,
                gluten_free,
                ingredients_raw: parse_json_arr(&raw_raw),
                ingredients_core: parse_json_arr(&core_raw),
                directions: parse_json_arr(&dir_raw),
            };

            (StatusCode::OK, Json(serde_json::json!(detail)))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "recipe not found" })),
        ),
        Err(e) => {
            tracing::error!("SQLite query error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "query failed" })),
            )
        }
    }
}

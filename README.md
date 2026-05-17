# StruggleMeals

**Tell it what's in your fridge. Get a real recipe in under two minutes.**

Live: [strugglemeal.vercel.app](https://strugglemeal.vercel.app) · Part of the [Patchwork series](https://github.com/vlyot)

No account required to get value. Sign in to save history and favourites.

---

## What it does

StruggleMeals is a recipe matching app built around a single premise: open it, enter whatever ingredients you have (or point your camera at the fridge), and get real, cookable recipes ranked to your specific pantry — grouped by mood (Light / Filling / Quick) so you choose a vibe, not just a dish.

The entire 1.94M-recipe dataset lives on a Railway volume. AI is used only at the edges: Gemini Vision reads your photo, Groq classifies and presents results. At rest, the app costs nothing to run.

---

## Features

- **Photo scan** — Gemini 2.5 Flash identifies ingredients from a photo with per-item confidence (high / medium / low) and suggests pantry staples you probably already have
- **1.94M recipes** — sourced from RecipeNLG, filtered to remove luxury items, non-English entries, sauce/dip/condiment titles, and anything with fewer than 2 real steps
- **TF-IDF / BM25-inspired scorer** — ingredients weighted by rarity (niche: 9.0, common: 4.5, ultra-common: 2.0), with quantity multipliers, coverage penalties, pantry-focus factor, and cuisine boost
- **FTS5 inverted index** — O(log R) candidate selection; ~100–300ms to first results vs the previous ~minutes-long full-table scan
- **Streaming shortlist (SSE)** — Rust scores emit immediately (~100ms), Groq themes follow (~1–3s); the UI updates twice so results appear before Groq finishes
- **Substitution hints** — missing ingredients show static lookup hints (19 entries) plus Groq-generated category-level alternatives ("any neutral oil", not "vegetable oil")
- **Quantity weights** — "a little" 0.7×, "1 qty" 1.0×, "plenty" 1.3× as IDF multipliers in scoring
- **Auth** — Neon Auth (Better Auth), Google OAuth + email/password; history (60-day window) and favourites (persistent) for signed-in users
- **Non-meal filter** — sauces, dips, dressings, glazes, stocks, and condiments removed at the scoring layer before Groq ever sees them

---

## Architecture

```
Browser (Vercel)
  └── React + TypeScript + Vite
        └── shadcn/ui · Tailwind v4

Railway (always-on, $0 variable cost)
  └── Rust + Axum (HTTP + SSE)
        ├── SQLite (1.94M recipes, FTS5 index) ── Railway volume /data/recipes.db
        ├── Neon Postgres ─── auth sessions, cook history, favourites
        ├── Gemini 2.5 Flash ─── photo ingredient identification
        └── Groq llama-3.3-70b ─── recipe theming + presentation
```

### Monorepo layout

```
strugglemeal/
├── backend/
│   ├── src/
│   │   ├── main.rs        # Axum router, AppState, FTS5 migration background task
│   │   ├── ai.rs          # /ai/* handlers, TF-IDF scorer, FTS5 query helpers, rayon scoring
│   │   ├── recipes.rs     # /recipes/* handlers, substitution hint table
│   │   ├── auth.rs        # AuthUser extractor (session token → user_id via Neon Auth)
│   │   ├── history.rs     # cook_history CRUD (60-day retention)
│   │   ├── favourites.rs  # favourites CRUD (idempotent ON CONFLICT DO NOTHING)
│   │   ├── health.rs      # GET /health
│   │   └── lib.rs         # module re-exports for integration tests
│   ├── tests/
│   │   └── auth_integration.rs  # 8 integration tests (401s, CRUD, idempotency, isolation)
│   └── Cargo.toml
├── frontend/
│   └── src/
│       ├── components/
│       │   ├── cook/           # MethodSelector, IngredientInput, ShortlistView, RecipeModal
│       │   ├── sections/       # Landing page: hero, vision, philosophy, how-it-works, examples, waitlist
│       │   ├── header.tsx      # Scroll-aware nav, mobile hamburger, auth-aware CTA
│       │   └── footer.tsx
│       ├── pages/              # CookPage, HistoryPage, FavouritesPage, AuthView
│       ├── lib/api.ts          # All fetch calls; streamShortlist() SSE async generator
│       └── router.tsx          # Protected routes, /handler/:pathParam auth view
├── pipeline/
│   └── process.py              # RecipeNLG → recipes.db (run locally, uploaded once to Railway volume)
└── .github/workflows/ci.yml
```

---

## How the matching works

### Candidate selection — FTS5

Each recipe's normalised ingredient list is stored as space-joined tokens in a `recipes_fts` FTS5 virtual table (`tokenize='unicode61 remove_diacritics 1'`). Each user ingredient is double-quoted in the MATCH expression to enforce exact token boundaries:

```sql
WHERE r.id IN (
  SELECT rowid FROM recipes_fts
  WHERE ingredients_text MATCH '"egg" OR "sour cream" OR "spinach"'
)
LIMIT 500
```

This replaced the previous `json_each(ingredients_core) LIKE ?` EXISTS approach, which was an O(R × R_avg × I) full-table scan (~580M operations per query). The FTS5 path is O(log R) per token.

The index is built in a background `tokio::spawn` + `spawn_blocking` task on startup. While it builds, requests fall back to the json_each path transparently via `fts_ready: Arc<AtomicBool>`.

Candidates are pre-sorted by ingredient rarity weight in SQL (`ORDER BY` CASE expression: niche → 3, common → 2, ultra-common → 1) so the 500-candidate pool surfaces rare-ingredient recipes first. Raw rows are collected sequentially (rusqlite requirement), then scored in parallel via `rayon::par_iter`.

### Scoring — TF-IDF / BM25-inspired

```
score = (Σ idf(ingredient) × qty_weight(qty) + title_bonus)
        × coverage^1.5
        × (1 − missing_ratio² × 0.5)
        × simplicity_bonus
        × anchor_boost
        × pantry_focus
        × pantry_penalty
        × cuisine_boost
```

**IDF tiers:**

| Tier | Examples | IDF |
|---|---|---|
| Ultra-common | chicken, rice, onion, garlic, butter | 2.0 |
| Common | egg, cheese, flour, cream, tomato | 4.5 |
| Niche | udon, kimchi, miso, tahini, dashi | 9.0 |

**Modifiers:**

| Factor | Formula | Effect |
|---|---|---|
| `qty_weight` | plenty=1.3, 1 qty=1.0, a little=0.7 | Scales IDF by how much you have |
| `coverage^1.5` | (matched / recipe_n)^1.5 | Penalises low-coverage recipe matches |
| `missing_ratio²` | 1 − (missing/recipe_n)² × 0.5 | Soft drag for many unfilled ingredients |
| `simplicity_bonus` | up to 1.25× when core_len ≤ 6, coverage ≥ 50% | Rewards struggle-meal-style short recipes |
| `anchor_boost` | 1.0 + (idf/9.0) × 0.35 | Ingredient appears in title or is first in core list |
| `pantry_focus` | sqrt(matched/user_n) when user_n > 5 | Penalises recipes that use few of a large/diverse pantry |
| `pantry_penalty` | max(0.1, 1 − pantry_ratio² × 0.7) | Suppresses baking recipes heavy with pantry-staple filler |
| `cuisine_boost` | 1.25× | Post-score if recipe.cuisine matches user's requested cuisine |

**Token matching (`tokens_overlap`):** all ingredient comparisons split on whitespace / hyphen / slash and check exact token equality. "egg" never matches "eggplant"; "pea" never matches "peanut butter". Used in both `score_v2` match check and IDF tier lookup.

**Title bonus:** `idf × 0.6` when the ingredient token appears in the recipe title (IDF-scaled, so niche ingredients get a larger bonus than common ones).

**Relaxation path:** when fewer than 3 results are found, `stem_variants` generates plural/singular/`-ies` variants for each ingredient and retries — always filtering `match_count > 0` so zero-score recipes never surface.

### Groq theme classification

The top 20 scored candidates (with step count and first-step excerpt per recipe) are sent to Groq `llama-3.3-70b-versatile`. Groq assigns each recipe to Light / Filling / Quick and returns up to 2 per theme. Falls back to raw top-6 if Groq fails — endpoint always returns HTTP 200.

**Theme rules:** Quick = under 20 min or ≤5 core ingredients; Light = salads/soups/eggs/fish; Filling = everything else.

---

## AI integrations

### Gemini Vision — `POST /ai/identify-ingredients`

Accepts `{ image_base64, mime_type }`. Returns:

```json
{
  "detected": [{ "name": "egg", "confidence": 9.2 }],
  "suggestions": ["salt", "black pepper"],
  "confidence_legend": { "high": "≥7.5", "mid": "≥4.5", "low": "<4.5" },
  "ingredients": ["egg"]
}
```

- 4 MB decoded image size cap enforced before sending to Gemini
- 1 automatic retry on 429/5xx from Gemini
- Always returns HTTP 200 — failure is soft (frontend falls back to manual entry)
- **Rate limiter:** `tokio::Semaphore` with fixed-window refill every 60s; default 10 RPM (Gemini free tier); configurable via `GEMINI_RATE_LIMIT_RPM` env var

### Groq — `POST /ai/present-recipe`

Accepts `{ recipe, user_ingredients }`. Calls `llama-3.3-70b-versatile` with `response_format: json_object`. Returns:

```json
{
  "theme": "Quick",
  "difficulty": "Easy",
  "time_minutes": 15,
  "description": "One sentence, max 10 words",
  "ingredients": [{ "name": "egg", "amount": "2 large", "have": true }],
  "steps": ["Imperative step…"],
  "substitutions": [{ "ingredient": "mirin", "substitute": "any sweet rice wine", "note": "…" }]
}
```

- `have` flags are cross-verified in Rust after Groq returns (same token matching as the scorer)
- Substitutions filtered post-parse to remove pantry staples (prevents Groq hallucinations like "flour → udon")
- Groq is prompted to give category-level substitutes ("any neutral oil", "any acid") rather than specific brands or alternatives
- Returns 502 on Groq failure — frontend shows a retry button

### `POST /ai/theme-shortlist` — SSE

Two-phase streaming response:

1. **`event: scores`** (~100ms) — Rust scoring complete, recipe cards rendered immediately
2. **`event: themes`** (~1–3s) — Groq theming complete, themed tabs updated

`X-Accel-Buffering: no` response header instructs Railway's NGINX to flush SSE chunks immediately. Falls back to plain JSON for environments that buffer SSE.

---

## API reference

| Method | Path | Auth | Description |
|---|---|---|---|
| `GET` | `/health` | — | Health check |
| `GET` | `/recipes/search` | — | Search by ingredients + filters (`vegetarian`, `vegan`, `gluten_free`, `cuisine`) |
| `GET` | `/recipes/:id` | — | Full recipe with raw ingredients, substitution hints, optional flags |
| `POST` | `/ai/identify-ingredients` | — | Gemini Vision ingredient ID from photo |
| `POST` | `/ai/theme-shortlist` | — | SSE: score candidates, then theme with Groq |
| `POST` | `/ai/present-recipe` | — | Groq full recipe presentation |
| `POST` | `/history` | Required | Record a cook session |
| `GET` | `/history` | Required | List history (60-day window, search/filter params) |
| `DELETE` | `/history/:id` | Required | Remove a history entry |
| `POST` | `/favourites` | Required | Save a recipe (idempotent: 201 new, 200 exists) |
| `GET` | `/favourites` | Required | List saved recipes |
| `DELETE` | `/favourites/:recipe_id` | Required | Remove a saved recipe |

Auth: `x-stack-refresh-token` header containing the Neon Auth session token.

---

## Local development

### Prerequisites

- Rust stable (1.75+)
- Node.js 20+
- A `recipes.db` file (see [Dataset](#dataset) — or request the pre-built 2.1 GB file)

### Backend

```bash
cd backend

# .env
DATABASE_URL=postgresql://...          # Neon Postgres connection string
SQLITE_PATH=/absolute/path/recipes.db
GEMINI_API_KEY=...
GROQ_API_KEY=...
FRONTEND_URL=http://localhost:5173
RUST_LOG=info

cargo run
# Server on :8080
```

### Frontend

```bash
cd frontend

# .env.local
VITE_API_URL=http://localhost:8080
VITE_NEON_AUTH_URL=https://<neon-host>/neondb/auth

npm install
npm run dev
# Dev server on :5173
```

### Tests

```bash
cd backend

# Unit tests — no database required
SQLX_OFFLINE=true cargo test --lib

# Integration tests — requires DATABASE_URL in .env
cargo test
```

49 unit tests covering: `score_v2`, `rarity_idf`, `tokens_overlap`, `qty_weight`, `stem_variants`, FTS5 match builders, Gemini rate limiter, non-meal title filter, pantry focus/penalty, anchor boost, simplicity reward, cuisine boost.

---

## Deployment

### Backend — Railway

| Variable | Value |
|---|---|
| `DATABASE_URL` | Neon Postgres connection string (pooled) |
| `SQLITE_PATH` | `/data/recipes.db` |
| `GEMINI_API_KEY` | Gemini API key |
| `GROQ_API_KEY` | Groq API key |
| `FRONTEND_URL` | `https://strugglemeal.vercel.app,http://localhost:5173` |
| `RUST_LOG` | `info` |
| `PORT` | `8080` |
| `GEMINI_RATE_LIMIT_RPM` | `10` (free tier); set `50` on paid Gemini |

Railway volume at `/data` stores `recipes.db` (~2.1 GB). On first boot after mounting, the FTS5 index rebuilds in the background (~2–5 min). Traffic is served on the json_each fallback path during this window; subsequent restarts skip the rebuild via row-count check.

### Frontend — Vercel

| Variable | Value |
|---|---|
| `VITE_API_URL` | `https://backend-production-e383.up.railway.app` |
| `VITE_NEON_AUTH_URL` | `https://<neon-host>/neondb/auth` |

`vercel.json` includes an SPA rewrite (`/(.*) → /index.html`) for client-side routing.

---

## Dataset

Source: [RecipeNLG](https://recipenlg.cs.put.poznan.pl/) (~2.23M rows). Pipeline: `pipeline/process.py` — run locally, output uploaded once to Railway volume.

**Filtering criteria (applied in order):**

1. Non-English — drop if >15% non-ASCII chars across title + NER tokens
2. Luxury blocklist — truffle, wagyu, foie gras, lobster, saffron, caviar, etc.
3. Core ingredient count — separate pantry staples from NER; drop if core count < 2 or > 13
4. Minimum steps — drop if directions array has < 2 entries

**Results:** 2,231,142 in → 1,940,275 kept. Rejected: 20 non-English, 10,516 luxury, 110,057 core-count, 170,274 insufficient steps.

**Schema:** `recipes(id, title, cuisine, ingredients_raw, ingredients_core, directions, ingredient_count, vegetarian, vegan, gluten_free)`. Pantry staples (salt, pepper, oil, butter, common spices) are excluded from `ingredients_core` — assumed always present and never scored. Dietary tags (`vegetarian`, `vegan`, `gluten_free`) computed once at processing time from NER keywords.

---

## Tech stack

| Layer | Technology |
|---|---|
| Frontend | React 19, TypeScript, Vite, shadcn/ui, Tailwind v4 |
| Backend | Rust, Axum 0.8, Tokio |
| Recipe DB | SQLite via rusqlite (bundled-full, FTS5 enabled), r2d2 pool |
| User DB | Neon Postgres (sqlx 0.8) |
| Auth | Neon Auth (Better Auth), Google OAuth + email/password |
| AI — Vision | Google Gemini 2.5 Flash |
| AI — Text | Groq llama-3.3-70b-versatile |
| Parallel scoring | Rayon |
| Streaming | Axum SSE (futures-util) |
| Frontend hosting | Vercel |
| Backend hosting | Railway (always-on, volume storage) |
| CI | GitHub Actions |

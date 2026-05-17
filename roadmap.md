# StruggleMeals — Implementation Roadmap

---

## Final Outcome

A deployed, publicly accessible web application where a user can open it on any device, enter what they have in their fridge, and receive real recipe suggestions they can actually cook — in under two minutes from landing to recipe. No installation, no account required to get value. Signed-in users have persistent history and favourites. The app runs continuously on Railway at no variable cost under normal usage, sourcing recipes from a local SQLite dataset and using Gemini Vision and Groq only at the edges. The codebase is clean, well-structured, and presentable as a portfolio artefact on GitHub.

---

## Phases

---

### Phase 1 — Project Foundation

_Establishes the skeleton everything else builds on. Must be completed before any other phase._

- Monorepo structure with React (TypeScript) frontend and Rust + Axum backend
- Frontend and backend able to communicate locally
- Environment variable handling and secrets management in place
- Railway deployment configured — both services deploying from GitHub on push
- Basic CI pipeline (build passes, nothing broken on push)
- Neon database provisioned and connected to the backend

**Exit condition:** Both services deploy successfully to Railway and the frontend can reach the backend.

---

### Phase 2 — Recipe Dataset Pipeline

_Fully independent of all other phases except Phase 1. Can run in parallel with Phases 3 and 4._

**Implementation notes:**

- **Source:** RecipeNLG (~2.2M rows). CSV columns used: `title`, `ingredients` (raw strings), `directions`, `source`, `NER` (normalised ingredient names). `NER` is used for all filtering, tagging, and match scoring; `ingredients` is used for display only.
- **Pipeline:** Python script at `pipeline/process.py`. Run locally, outputs `data/recipes.db`. Upload DB to Railway volume manually.
- **Trimming criteria applied in order:**
  1. Non-English: drop if >15% non-ASCII chars across title + NER tokens
  2. Luxury blocklist: drop if NER contains truffle, wagyu, foie gras, lobster, saffron, caviar, etc.
  3. Core ingredient count: separate pantry staples (salt, pepper, oil, butter, sugar, common spices, etc.) from NER tokens — drop if core count < 2 or > 13
  4. Minimum steps: drop if directions array has < 2 entries
- **Pantry staples** are excluded from both the cap count and the match scoring — assumed always present
- **Dietary heuristic tagging** (stored as boolean columns, computed once at processing time):
  - `vegetarian`: no meat/poultry/seafood keywords in NER
  - `vegan`: vegetarian + no dairy/eggs/honey in NER
  - `gluten_free`: no wheat/flour/pasta/bread keywords in NER
- **SQLite schema:** `recipes(id, title, cuisine, ingredients_raw, ingredients_core, directions, ingredient_count, vegetarian, vegan, gluten_free)` — indexes on `ingredient_count`, `(vegetarian, vegan, gluten_free)`, `cuisine`
- **Backend:** `rusqlite` (bundled, compiles SQLite statically) + `r2d2`/`r2d2_sqlite` connection pool. `AppState` holds both Postgres pool and SQLite pool. Axum sub-state extraction via `FromRef<AppState>`.
- **API endpoints implemented:**
  - `GET /recipes/search?ingredients=a,b,c&vegetarian=true&vegan=true&gluten_free=true&cuisine=italian&limit=20` — fetches candidate set, scores by ingredient overlap in Rust (substring bidirectional match), sorts by score desc then ingredient_count asc, returns top N
  - `GET /recipes/:id` — full recipe detail including raw ingredients and directions

**Actual results (pipeline run May 2026):**
- Rows read: 2,231,142 — Rows inserted: 1,940,275
- Rejected: 20 non-English, 10,516 luxury, 110,057 core count violations, 170,274 insufficient steps
- Output DB size: ~2GB at `data/recipes.db`, uploaded to Railway volume at `/data/recipes.db`
- Upload method: one-time multipart HTTP endpoint (removed post-upload)

**Status: COMPLETE** — live at `https://backend-production-e383.up.railway.app/recipes/search`

**Exit condition:** Given a list of ingredients and optional filters, the backend returns a ranked list of real recipe matches from the SQLite dataset.

---

### Phase 3 — Authentication & User Data

_Independent of Phases 2, 4, and 5. Can run in parallel once Phase 1 is done._

- Neon Auth configured with Google OAuth and email/password
- Auth integrated into the Axum backend — protected routes working
- User table and session management in Neon
- Recipe history schema — stores recipe reference, user ID, timestamp
- Favourites schema — stores recipe reference, user ID, no expiry
- 60-day archive logic for history
- All endpoints return appropriate responses for signed-in vs anonymous users

**Implementation notes:**

- `backend/src/auth.rs` — `AuthUser` extractor reads `x-stack-refresh-token`, validates against `neon_auth.session`, returns `user_id` as text
- `backend/src/history.rs` — `POST /history` (201), `GET /history` (60-day window, search, filter params), `DELETE /history/:id` (404 if not owned)
- `backend/src/favourites.rs` — `POST /favourites` (idempotent: 201 new, 200 exists), `DELETE /favourites/:recipe_id`, `GET /favourites`
- DB: `cook_history` and `favourites` tables in `public` schema; `favourites_user_recipe_unique` constraint enforces idempotency
- Integration test suite at `backend/tests/auth_integration.rs` covers 401s, CRUD, idempotency, per-user isolation
- Frontend: `@neondatabase/neon-js` wired in `main.tsx` + `stack/client.ts`; `router.tsx` has `ProtectedRoute` + `/handler/:pathParam` auth view; `HistoryPage` + `FavouritesPage` pages; `lib/api.ts` sends `x-stack-refresh-token` on all auth calls
- Header shows History/Favourites links + `UserButton` when signed in; Sign in CTA when not
- Vercel env vars required: `VITE_NEON_AUTH_URL=https://ep-purple-mouse-aod4wg5j.c-2.ap-southeast-1.aws.neon.tech/neondb/auth`; Railway `FRONTEND_URL=https://strugglemeal.vercel.app` (set)

**Status: COMPLETE** — backend live, frontend wired, DB schema provisioned, Vercel env vars pending manual deploy

**Exit condition:** A user can sign in, and their session is recognised by the backend with correct anonymous/authenticated behaviour on all routes.

---

### Phase 4 — AI Integrations

_Independent of Phases 2, 3, and 5. Can run in parallel once Phase 1 is done._

**Implementation notes:**

- **New module:** `backend/src/ai.rs` — two thin adapter handlers, no business logic. Phase 5 owns orchestration.
- **New deps:** `reqwest 0.12` (rustls-tls, consistent with existing TLS approach), `base64 0.22`
- **AppState additions:** `http: reqwest::Client` (shared, one per process), `gemini_api_key: String`, `groq_api_key: String`
- **New env vars:** `GEMINI_API_KEY`, `GROQ_API_KEY`

**Endpoints:**

- `POST /ai/identify-ingredients` — accepts `{ image_base64, mime_type }`, calls Gemini Vision (`gemini-2.5-flash`). Returns `{ ingredients: string[] }` on success or `{ ingredients: null, error: "fallback", message: "..." }` on any failure (always 200 — frontend decides what to show). 1 automatic retry on 429/5xx. 4 MB decoded size cap enforced before sending.
- `POST /ai/present-recipe` — accepts `{ recipe: RecipeInput, user_ingredients: string[] }`, calls Groq (`llama-3.3-70b-versatile`). Returns full presentation JSON on success or `{ error: "presentation_failed" }` with 502 on failure.

**Groq output schema** (enforced via `response_format: json_object` + system prompt):
```json
{
  "theme": "Light" | "Filling" | "Quick",
  "difficulty": "Easy" | "Medium" | "Hard",
  "time_minutes": 15,
  "description": "One sentence, max 10 words",
  "ingredients": [{ "name": "...", "amount": "...", "have": true }],
  "steps": ["Imperative step..."],
  "substitutions": [{ "ingredient": "...", "substitute": "...", "note": "..." }]
}
```
Theme rules: Quick = under 20 min or ≤5 core ingredients; Light = salads/soups/eggs/fish; Filling = everything else.
Substitutions: all missing ingredients (have=false). `have` flags are cross-verified in Rust after Groq returns, using the same substring matching as Phase 2 scoring.

**Failure behaviour:** Gemini failure is soft (returns fallback 200 — frontend shows manual entry). Groq failure is hard (returns 502 — frontend shows retry button). Both log via `tracing::warn!` / `tracing::error!`.

**Status: COMPLETE** — builds clean, endpoints live at `/ai/identify-ingredients` and `/ai/present-recipe`

**Exit condition:** Given an image, Gemini returns an ingredient list. Given recipe data, Groq returns a formatted, themed recipe. Both handle failure gracefully.

---

### Phase 5 — Core Application Logic

_Depends on Phase 2 (dataset query), Phase 3 (auth), and Phase 4 (AI integrations). Ties them together into the full user flow._

**Backend — `POST /ai/theme-shortlist`** (new endpoint):
- Accepts `{ ingredients[], ingredients_with_qty?: [{name, qty}][], vegetarian?, vegan?, gluten_free?, cuisine? }`
- **Candidate selection:** SQLite `json_each()` EXISTS clauses filter to only recipes containing at least one user ingredient (LIMIT 500) — replaces unfiltered LIMIT 2000
- **Scoring (TF-IDF / BM25-inspired):** each candidate scored as `(Σ idf × qty_weight + title_bonus) × coverage^1.5 × missing_penalty × simplicity_bonus × anchor_boost × pantry_focus × pantry_penalty`
  - IDF tiers: ultra-common (chicken, onion…) → 2.0; common (egg, cheese…) → 4.5; specific/niche (udon, kimchi…) → 9.0
  - Qty weights: "plenty" → 1.3×, "1 qty" → 1.0×, "a little" → 0.7×
  - Anchor boost: ingredient in title or first in core → `1.0 + (idf/9.0) × 0.35` (niche: +35%, common: +8%)
  - Simplicity reward: short recipes (core_len ≤ 6, coverage ≥ 50%) → up to 1.25× bonus
  - Pantry penalty: `max(0.1, 1 − pantry_ratio² × 0.7)` where `pantry_ratio = (total − core) / total`
  - Title bonus: +5.0 if ingredient appears in recipe title
  - Coverage penalty: (matched/total)^1.5 — penalises low-coverage matches
  - Missing penalty: 1 − (missing_ratio² × 0.5) — soft drag for many unfilled ingredients
- Passes top 20 ranked candidates to Groq `llama-3.3-70b-versatile` with step count + first-step excerpt per candidate
- Groq picks up to 2 recipes per theme (Light/Filling/Quick) — uses step wording to judge Quick vs Filling accurately
- Returns `{ results: ShortlistEntry[], groq_used: bool }` — falls back to raw top-6 if Groq fails (always 200)
- SQLite WAL mode + 64MB page cache applied at startup via PRAGMAs

**Backend — existing endpoints reused as-is:**
- `GET /recipes/:id` — fetch full recipe detail for modal
- `POST /ai/present-recipe` — Groq formats full recipe from Phase 4
- `POST /history`, `POST /favourites` — write endpoints from Phase 3

**Frontend — `/cook` route** (new page, no auth required):
- Step 1 (input): tag-style ingredient chips, Text/Photo tabs, filter toggles (Vegetarian/Vegan/Gluten-Free), Voice tab disabled with tooltip
- Photo tab calls `POST /ai/identify-ingredients` and pre-populates chips
- Step 2 (shortlist): themed tabs (Light/Filling/Quick) with match % bar, Best Match badge on featured card, "Cook this →" CTA
- Recipe modal: full `PresentResponse` — ingredients with ✓/? markers, numbered steps, substitutions section, save-to-favourites button (auth-aware)
- History silently recorded for signed-in users after opening a recipe (`recordCookSilent` swallows auth errors)
- Landing hero CTA updated from `#get-started` → `/cook`

**Infrastructure:**
- `FRONTEND_URL` env var on Railway now supports comma-separated origins for multi-domain CORS
- Neon Auth `trusted_origins` updated to include `strugglemeals.vercel.app`
- Vercel `vercel.json` SPA rewrite rule added (`/(.*) → /index.html`) for client-side routing

**Quantity classification:** Implemented in Phase 7. `ingredients_with_qty` sent from frontend, used as IDF multipliers in scoring.

**Status: COMPLETE** — full flow live at `https://strugglemeals.vercel.app/cook`

**Exit condition:** ✓ Ingredients in → themed shortlist out → full recipe on selection → history written for signed-in users.

---

### Phase 6 — Frontend

**Status: COMPLETE** (delivered across Phases 5 & 6)

- Screen 1: Landing page — hero, vision, philosophy, how-it-works, examples, waitlist sections; scroll-aware header; mobile hamburger
- Screen 2: `/cook` — ingredient input with Text/Photo tabs, filter toggles, Voice stub
- Screen 3: `/cook` shortlist — themed tabs (Light/Filling/Quick), match bars, best-match badge
- Screen 4: Recipe modal — ingredient ✓/? markers, numbered steps, substitutions, save-to-favourites
- History and Favourites pages for signed-in users (protected routes)
- Design system: Plus Jakarta Sans, warm oklch palette, shadcn/ui components throughout

**Exit condition:** ✓ All screens functional with real backend data. Auth state reflected correctly throughout.

---

### Phase 7 — Polish & Validation Prep

_Depends on Phase 6. Final phase before real-world validation._

**Completed:**
- Recipe matching algorithm replaced with TF-IDF / BM25-inspired scorer (`score_v2`):
  - IDF rarity tiers + quantity weight multipliers + title match bonus + coverage/missing penalties
  - SQL candidate filter changed from unfiltered LIMIT 2000 → `json_each()` EXISTS clauses (finds relevant recipes rather than first 2000 rows by insertion order)
  - Groq now receives step count + first-step excerpt per candidate for better Quick/Filling classification
  - `ingredients_with_qty` passed from frontend to backend
  - SQLite WAL mode + 64MB page cache via startup PRAGMAs
- 12 unit tests covering all scoring functions (`score_v2`, `rarity_idf`, `qty_weight`, `resolve_user_ings`, etc.)
- **Algorithm quality pass — 5 improvements (commit `a3b72b3`):**
  1. **Word-boundary token matching** (`tokens_overlap`): replaces bidirectional substring check — "egg" no longer falsely matches "eggplant", "pea" no longer matches "peanut butter". Both `score_v2` match check and `rarity_idf` IDF tier lookup now use token equality rather than `contains`.
  2. **IDF-scaled title bonus**: flat +5.0 replaced with `idf × 0.6` — common ingredients (chicken → +1.2) get a smaller title bonus than niche ones (udon/kimchi → +5.4). Title check also uses token matching to prevent "Eggplant Parmesan" from boosting an "egg" query.
  3. **Relaxation path hardening** (`stem_variants`): when < 3 results, the relaxation retry now generates plural/singular/`-ies` variants for each ingredient, broadens LIKE params accordingly, and always filters `match_count > 0` — no 0-score recipes ever leak into results.
  4. **Cuisine boost**: `cuisine` column fetched in SQL SELECT; if the user's requested cuisine matches the recipe's cuisine (case-insensitive contains), the post-score is multiplied by 1.25×. Zero-score recipes are unaffected (0 × 1.25 = 0).
  5. **IDF-weighted SQL ORDER BY** (`idf_sql_weight`): CASE expressions for ORDER BY use weight 3 for niche ingredients, 2 for common, 1 for ultra-common — so the 500-candidate pool is pre-sorted to surface rare-ingredient recipes before common ones.
- **UI fix** (commit `a3b72b3`): shortlist matched-ingredient chips now correctly show only the ingredients that actually matched (not all user ingredients), sourced from the `matched_ingredients[]` array in the API response.
- 21 unit tests total (up from 12) — new tests cover `tokens_overlap`, `rarity_idf` no-false-positives, eggplant false-positive regression, `idf_sql_weight` tiers, scaled title bonus, cuisine match logic, `stem_variants` plurals.
- E2E tested with Playwright: egg+eggplant+tomato scenario confirmed eggplant recipes surface with correct chips; udon+tofu+miso+Asian scenario confirmed relevant noodle/miso results with no irrelevant recipes.

- **Algorithm quality pass 2 — 3 improvements:**
  1. **Pantry-heavy penalty** (applied in `fetch_candidates()` after `score_v2()`): baking recipes whose `ingredients_core` is tiny relative to `ingredient_count` are suppressed. Formula: `score × max(0.1, 1 − pantry_ratio² × 0.7)` where `pantry_ratio = (ingredient_count − core_len) / ingredient_count`. A recipe with 8 total ingredients but only 1 in core (e.g., "Old-Fashioned Tea Cakes") gets ×0.46; a stir-fry with 10 total, 7 core gets ×0.94.
  2. **Ingredient anchor boost** (inside `score_v2()`): when any matched user ingredient appears in the recipe title OR is the first item in `ingredients_core`, score is boosted by `1.0 + (idf / 9.0) × 0.35`. Scales with rarity: niche ingredients (kimchi, udon, tofu) → up to +35%; common (egg, cheese) → +17%; ultra-common (chicken, rice) → +8%. No hardcoded ingredient lists — IDF tier drives the magnitude.
  3. **Simplicity reward** (inside `score_v2()`): short recipes where the user covers most ingredients score higher. Bonus of `1.0 + 0.25 × ((6 − n) / 4).clamp(0,1)` when `core_len ≤ 6` and `coverage ≥ 50%` — up to 1.25× at core_len=2, tapering to 1.0 at core_len=6. Rewards struggle-meal-style recipes (yaki udon, egg scramble) over bloated ones with the same absolute match count.
- 26 unit tests total (up from 21) — new tests: `test_pantry_penalty_values`, `test_baking_recipe_suppressed_vs_real_match`, `test_anchor_boost_udon_title`, `test_anchor_boost_kimchi`, `test_simplicity_reward`.
- E2E tested with Playwright (May 2026): egg+vegetable → no baking recipes; udon+vegetable+egg → "Japanese Style Curry Udon Noodles" as best match; kimchi+egg+tofu → kimchi/tofu-centred dishes surfacing correctly.

- **Algorithm quality pass 3 — diverse pantry focus:**
  - **Pantry focus multiplier** (inside `score_v2()`): when a user has more than 5 non-staple ingredients, a `pantry_focus` factor rewards recipes that use a large fraction of the user's specific pantry. Formula: `pantry_focus = sqrt(matched / user_n)` when `user_n > 5`, else `1.0`. The `sqrt` exponent provides mild compression — 50% pantry coverage → ×0.71, 20% coverage → ×0.45. A recipe using 3 of 10 diverse ingredients is penalised ~55% relative to a focused-pantry user seeing the same recipe with 3/3 ingredients.
  - **Design rationale:** existing `coverage_factor` measures matched/recipe_n (recipe-centric); `pantry_focus` measures matched/user_n (user-centric). The two are orthogonal — `coverage_factor` rewards recipes whose ingredients you know; `pantry_focus` rewards recipes that actually use what you brought. No double-counting with IDF (count-based, not IDF-weighted).
  - **Guard threshold `user_n > 5`:** at 5 or fewer ingredients, the pantry is focused enough that `coverage_factor` already selects correctly. The guard ensures zero behaviour change for typical small-pantry queries.
  - **Updated scoring formula:** `score = weighted_sum × coverage^1.5 × missing_factor × simplicity_bonus × anchor_boost × pantry_focus`
- 29 unit tests total (up from 26) — new tests: `test_pantry_focus_diverse_vs_focused`, `test_pantry_focus_small_pantry_not_penalised`, `test_diverse_pantry_prefers_more_matches`.

- **Substitution hints on missing ingredients (commit `d1e7afe`):**
  - `GET /recipes/:id` now returns `ingredients_raw: [{raw, hint}]` instead of `string[]`
  - `hint_for(raw)` in `recipes.rs` uses `SUBSTITUTION_HINTS` static table (19 entries) + `word_tokens` from `ai.rs` for word-boundary matching
  - Covers: broths/stock, soy sauce, sesame oil, cornstarch, soup bases/mixes, mirin, dashi, coconut milk, cream, fish/oyster sauce, worcestershire, dijon, anchovies, capers, rice wine, white wine, curry sauce, scallions
  - `ShortlistView` missing panel renders each raw ingredient with a dim italic hint below it
  - `presentRecipe` unwraps `.raw` before sending to backend (backward-compatible)
- **Streaming shortlist (commit `d1e7afe`):**
  - `POST /ai/theme-shortlist` converted to SSE using `axum::response::sse`
  - Emits `event: scores` immediately (~50ms, pure Rust) then `event: themes` after Groq (~1–3s)
  - Frontend `streamShortlist()` async generator reads the SSE stream, updating recipe cards twice
  - Handles buffering proxies (Railway Nixpacks nginx) via plain JSON fallback when `data:` lines are absent
  - `build_shortlist_entries()` helper extracted from handler; `ShortlistEntry` derives `Clone`
  - Added `futures-util = "0.3"` to Cargo.toml

- **Substitution quality + UX fixes:**
  - **Groq sub filter** (`backend/src/ai.rs`, `call_groq()`): after parsing Groq's `PresentResponse`, `substitutions` are filtered with `.retain()` to remove any entry where the ingredient is a pantry staple — checked both as a full string and token-by-token via `is_pantry_staple_ai()`. Eliminates hallucinations like "flour → udon" that Groq produces when the user happens to have a noodle ingredient.
  - **Auto-open missing panel** (`frontend/src/components/cook/ShortlistView.tsx`): featured card now initialises `missingOpen = true` and fires a `useEffect` on mount to immediately fetch `/recipes/:id` for the missing ingredients + static hints. Users see the missing list and substitution hints without any click. Non-featured cards retain their on-click lazy-fetch behaviour.
  - **Uniform card design** (`frontend/src/components/cook/ShortlistView.tsx`): all shortlist cards use the same compact collapsed layout. First card starts expanded; all others start collapsed. Clicking a card header toggles it open/closed with a CSS `max-height` animation (0→600px). Chevron rotates 180° when expanded. Matched chips, missing ingredient list, match bar, and "Cook this →" button are revealed only on expand — no visual distinction between featured and non-featured cards.

- **Missing ingredient UX — optional flag + category-level substitutes:**
  - **`optional` field on `RawIngredient`** (`backend/src/recipes.rs`): each missing ingredient now carries `optional: bool`. Set by running `is_pantry_staple_ai()` (token-level check) on the raw ingredient string — covers condiments, oils, acids, spices, and common pantry items. `is_pantry_staple_ai` promoted from `fn` to `pub(crate)` to be callable from `recipes.rs`.
  - **Optional badge in missing panel** (`frontend/src/components/cook/ShortlistView.tsx`): missing ingredients with `optional: true` render a small "optional" pill badge inline — visually distinguishes hard-to-find required ingredients from pantry staples the user almost certainly has or can skip.
  - **Category-level Groq substitutes** (`backend/src/ai.rs`, `call_groq()` system prompt): Groq is now instructed to give the most general category-level substitution possible — "any neutral oil" not "vegetable oil", "any acid" not "white wine vinegar", "any crunchy pickle" not "cornichons". For regional/specialty ingredients (e.g. "Texas style hot pickled okra pods"), Groq describes the functional role in the dish rather than naming an alternative ("any pickled vegetable for crunch and acidity").

- **Search latency — FTS5 inverted index + rayon parallel scoring:**

  **Problem:** every recipe search was a full-table scan on 1.94M rows via `json_each(ingredients_core) LIKE ?` with no usable index — O(R × R_avg × I) per request (~580M operations for a typical query). Railway's NGINX proxy was also batching both SSE events, collapsing the two-phase UX.

  **What was built:**
  - `recipes_fts` FTS5 virtual table (`backend/src/main.rs`): plain (non-content) table storing one row per recipe with `ingredients_text` as space-joined ingredient tokens. `tokenize='unicode61 remove_diacritics 1'`.
  - Background migration at startup (`tokio::spawn` + `spawn_blocking`): server accepts traffic on the json_each fallback path while the index builds, then `fts_ready: Arc<AtomicBool>` flips to `true` transparently.
  - `fetch_candidates_fts()` (`backend/src/ai.rs`): replaces the EXISTS/json_each WHERE clauses with `WHERE r.id IN (SELECT rowid FROM recipes_fts WHERE ingredients_text MATCH ?)`. Each ingredient double-quoted in the MATCH expression (`"egg"`) to enforce exact token boundaries — consistent with `tokens_overlap`.
  - `build_fts_match()` / `build_fts_match_relaxed()`: construct MATCH expressions; relaxed variant expands `stem_variants` for the <3 results fallback path.
  - `rusqlite` feature upgraded from `["bundled"]` → `["bundled", "bundled-full"]` to compile in `SQLITE_ENABLE_FTS5`.
  - `rayon = "1"` added; `fetch_candidates` restructured so raw SQLite rows are collected sequentially (rusqlite requirement) then scored via `into_par_iter()` across CPU cores.
  - `score_raw_rows()` and `fts_query_raw()` helpers extracted for shared use between primary and relaxation paths. `RawRow` type alias hoisted to module level.
  - `X-Accel-Buffering: no` response header added to `theme_shortlist` — instructs Railway's NGINX to flush each SSE chunk immediately.
  - 5 new unit tests for FTS5 match builders. Total: 43 unit tests (up from 38).

  **Actual complexity (FTS5 fast path):** O(log R) per ingredient token for candidate selection, O(C × I × R_avg / cores) for parallel scoring with C ≤ 500. Phase 1 (scores SSE, no Groq) ~100–300ms. Phase 2 (Groq themes) ~500ms–2s network-bound.

- **Critical bug — FTS5 migration panic silently disabled FTS5 on every deploy:**

  **Root cause (confirmed in Railway logs):** a stale FTS5 table from an earlier `content='recipes'` schema existed on the Railway volume. `CREATE VIRTUAL TABLE IF NOT EXISTS` silently skipped recreation, leaving the old shadow table layout intact. The `INSERT INTO recipes_fts` then panicked immediately with `no such column: T.ingredients_text`. Because the migration task panicked, `fts_ready` never flipped to `true` — every request on every deploy fell back to the O(580M) json_each scan, causing minutes-long latency in production while local testing (no stale volume) worked fine.

  **Fix (`backend/src/main.rs`):** migration now runs `DROP TABLE IF EXISTS recipes_fts` before `CREATE VIRTUAL TABLE recipes_fts`. Dropping a FTS5 virtual table removes all shadow tables atomically. The incremental `WHERE r.id NOT IN (SELECT rowid FROM recipes_fts)` guard was removed — table is always rebuilt clean. One-time rebuild cost: ~2–5 min on Railway; table then persists across deploys and the row-count check skips the migration on subsequent restarts.

- **Performance fix — fetch_candidates called twice per request:**

  **Root cause:** `theme_shortlist` called `fetch_candidates()` independently for Phase 1 (scores SSE) and Phase 2 (Groq), running the full FTS5 query + 500-row fetch + 1000 JSON parses + rayon scoring twice per request. Phase 2 re-fetched because `initial_entries` only held `ShortlistEntry` structs (no `directions` field needed by Groq).

  **Fix (`backend/src/ai.rs`):** `fetch_candidates` called once. `Vec<CandidateRow>` kept in scope and moved into the Phase 2 `stream::once` closure directly. Halves SQLite I/O and scoring work per request. 46/46 unit tests pass unchanged.

- **UX fix — recipe count subtitle showed total across all tabs, not active tab:**

  **Bug:** "N recipes matched" subtitle in `ShortlistView` used `results.length` (total across all themes). With 0 Light / 3 Filling / 3 Quick, switching to Light showed "6 recipes matched" alongside "No light recipes found for your ingredients."

  **Fix (`frontend/src/components/cook/ShortlistView.tsx`):** active tab lifted into `useState<Theme>`. `<Tabs>` converted from uncontrolled (`defaultValue`) to controlled (`value` + `onValueChange`). Subtitle counts `byTheme(activeTheme).length` — updates immediately on tab switch.

- **Gemini Vision rate limiting:**

  **Problem:** `POST /ai/identify-ingredients` had no throttle. Gemini 2.5 Flash free tier caps at 10 RPM — concurrent traffic exhausted quota and returned hard 429s to users.

  **Implementation (`backend/src/lib.rs`, `backend/src/main.rs`, `backend/src/ai.rs`):**
  - `gemini_limiter: Arc<tokio::sync::Semaphore>` in `AppState`. Permit count from `GEMINI_RATE_LIMIT_RPM` env var (default: 10).
  - Background task refills semaphore to full every 60 seconds (fixed-window matching Gemini's RPM window).
  - `identify_ingredients` calls `try_acquire()` before any validation or API call. No permit → HTTP 429 with `{ error: "rate_limited", message: "Too many photo scans right now. Please try again in a minute." }`. Frontend surfaces via existing `photoError` path.
  - Set `GEMINI_RATE_LIMIT_RPM` Railway env var to raise limit when upgrading to a paid Gemini tier.
  - 3 new unit tests. Total: 46 unit tests, zero clippy warnings.

- **Quality fix — non-meal recipes filtered from shortlist:**
  - **Bug:** recipes that are sauces, dips, dressings, marinades, glazes, condiments, spreads, stocks, frostings, jams, etc. could surface as meal suggestions (e.g. "Cheese Sauce" classified as Filling).
  - **Fix (`backend/src/ai.rs`):** `is_non_meal_title(title)` function checks the recipe title's last meaningful token against a suffix blocklist (sauce, dip, dressing, marinade, glaze, rub, seasoning, topping, spread, butter, condiment, relish, chutney, salsa, vinaigrette, syrup, jam, jelly, frosting, icing, ganache, stock, broth) and a standalone term list (gravy, aioli, hummus, guacamole, pesto, tapenade). Applied in `score_raw_rows()` alongside the existing `match_count > 0` filter — non-meals never reach Phase 1 display or Groq.
  - Groq prompt also updated with an explicit exclusion rule as a secondary defence.
  - 3 new unit tests (`test_non_meal_title_sauces_and_dips`, `test_non_meal_title_standalone_terms`, `test_non_meal_title_allows_real_meals`). Total: 49 unit tests.

**Remaining:**
- Basic accessibility pass
- Copy and microcopy review — tone consistent, helper text clear
- README and GitHub repository cleaned up for portfolio presentation
- App shared with at least one real target user for initial feedback
- Any critical fixes from initial feedback applied

**Exit condition:** App is publicly accessible, stable, and in the hands of at least one real user.

---

## Parallelisation Summary

| Phase                | Depends On              | Can Run In Parallel With   |
| -------------------- | ----------------------- | -------------------------- |
| 1 — Foundation       | Nothing                 | —                          |
| 2 — Dataset Pipeline | Phase 1                 | Phases 3, 4                |
| 3 — Auth & User Data | Phase 1                 | Phases 2, 4                |
| 4 — AI Integrations  | Phase 1                 | Phases 2, 3                |
| 5 — Core Logic       | Phases 2, 4             | Phase 6 (scaffolding only) |
| 6 — Frontend         | Phase 5 (for real data) | —                          |
| 7 — Polish           | Phase 6                 | —                          |

The critical path is: **1 → 2 + 4 → 5 → 6 → 7**. Phases 3 runs entirely off the critical path and can be slotted in at any point after Phase 1.

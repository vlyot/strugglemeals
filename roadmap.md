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
- **Scoring (TF-IDF / BM25-inspired):** each candidate scored as `(Σ idf(ingredient) × qty_weight(qty) + title_bonus) × coverage_ratio^1.5 × missing_penalty`
  - IDF tiers: ultra-common (chicken, onion…) → 2.0; common (egg, cheese…) → 4.5; specific/niche (udon, kimchi…) → 9.0
  - Qty weights: "plenty" → 1.3×, "1 qty" → 1.0×, "a little" → 0.7×
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

**Remaining:**
- End-to-end testing of the full happy path and key error paths
- Performance check — ingredient query response time acceptable, no obvious bottlenecks
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

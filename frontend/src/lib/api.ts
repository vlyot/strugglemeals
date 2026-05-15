import { authClient } from "@/stack/client";

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:8080";

async function authHeaders(): Promise<Record<string, string>> {
  // getSession() fetches the current session including the token
  const { data } = await authClient.getSession();
  const token = data?.session?.token;
  if (!token) throw new Error("Not authenticated");
  return {
    "x-stack-refresh-token": token,
    "Content-Type": "application/json",
  };
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

export interface HistoryEntry {
  id: string;
  recipe_id: number;
  recipe_name: string;
  cooked_at: string;
}

export interface HistoryResponse {
  entries: HistoryEntry[];
  count: number;
}

export async function recordCook(recipe_id: number, recipe_name: string): Promise<{ id: string; cooked_at: string }> {
  const headers = await authHeaders();
  const res = await fetch(`${API_URL}/history`, {
    method: "POST",
    headers,
    body: JSON.stringify({ recipe_id, recipe_name }),
    credentials: "include",
  });
  if (!res.ok) throw new Error(`recordCook failed: ${res.status}`);
  return res.json();
}

export async function fetchHistory(options?: { search?: string; filter?: "all" | "week" | "month" }): Promise<HistoryResponse> {
  const headers = await authHeaders();
  const params = new URLSearchParams();
  if (options?.search) params.set("search", options.search);
  if (options?.filter && options.filter !== "all") params.set("filter", options.filter);
  const qs = params.toString();
  const res = await fetch(`${API_URL}/history${qs ? `?${qs}` : ""}`, {
    headers,
    credentials: "include",
  });
  if (!res.ok) throw new Error(`fetchHistory failed: ${res.status}`);
  return res.json();
}

export async function deleteHistoryEntry(id: string): Promise<void> {
  const headers = await authHeaders();
  await fetch(`${API_URL}/history/${id}`, {
    method: "DELETE",
    headers,
    credentials: "include",
  });
}

// ---------------------------------------------------------------------------
// Favourites
// ---------------------------------------------------------------------------

export interface FavouriteEntry {
  id: string;
  recipe_id: number;
  recipe_name: string;
  saved_at: string;
}

export interface FavouritesResponse {
  favourites: FavouriteEntry[];
  count: number;
}

export async function addFavourite(recipe_id: number, recipe_name: string): Promise<void> {
  const headers = await authHeaders();
  await fetch(`${API_URL}/favourites`, {
    method: "POST",
    headers,
    body: JSON.stringify({ recipe_id, recipe_name }),
    credentials: "include",
  });
}

export async function removeFavourite(recipe_id: number): Promise<void> {
  const headers = await authHeaders();
  await fetch(`${API_URL}/favourites/${recipe_id}`, {
    method: "DELETE",
    headers,
    credentials: "include",
  });
}

export async function fetchFavourites(): Promise<FavouritesResponse> {
  const headers = await authHeaders();
  const res = await fetch(`${API_URL}/favourites`, { headers, credentials: "include" });
  if (!res.ok) throw new Error(`fetchFavourites failed: ${res.status}`);
  return res.json();
}

// ---------------------------------------------------------------------------
// Cook flow (Phase 5)
// ---------------------------------------------------------------------------

export interface ShortlistEntry {
  id: number;
  title: string;
  theme: string | null;
  reason: string | null;
  match_score: number;
  missing_count: number;
  ingredient_count: number;
  vegetarian: boolean;
  vegan: boolean;
  gluten_free: boolean;
  matched_ingredients: string[];
}

export interface ShortlistResponse {
  results: ShortlistEntry[];
  groq_used: boolean;
}

export interface IngredientWithQtyRequest {
  name: string;
  qty: "1 qty" | "a little" | "plenty";
}

export interface ShortlistRequest {
  ingredients: string[];
  ingredients_with_qty?: IngredientWithQtyRequest[];
  vegetarian?: boolean;
  vegan?: boolean;
  gluten_free?: boolean;
  cuisine?: string;
}

/**
 * Streams the shortlist via SSE — yields once immediately (scored results, theme=null)
 * then again after Groq responds (themed results). Falls back gracefully if the proxy
 * buffers SSE into a single delivery or returns plain JSON.
 */
export async function* streamShortlist(
  payload: ShortlistRequest
): AsyncGenerator<ShortlistResponse> {
  const res = await fetch(`${API_URL}/ai/theme-shortlist`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
    credentials: "include",
  });
  if (!res.ok || !res.body) throw new Error(`streamShortlist failed: ${res.status}`);

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = "";
  let yieldedAny = false;

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    const lines = buf.split("\n");
    buf = lines.pop() ?? "";
    for (const line of lines) {
      if (line.startsWith("data:")) {
        const text = line.slice(5).trim();
        if (text) { yield JSON.parse(text) as ShortlistResponse; yieldedAny = true; }
      }
    }
  }

  // Flush any remaining buffer (handles single-chunk delivery from buffering proxies)
  if (buf.trim()) {
    if (buf.startsWith("data:")) {
      const text = buf.slice(5).trim();
      if (text) { yield JSON.parse(text) as ShortlistResponse; yieldedAny = true; }
    } else if (!yieldedAny) {
      // Plain JSON fallback — proxy collapsed SSE into a single JSON body
      try {
        yield JSON.parse(buf) as ShortlistResponse;
      } catch { /* ignore malformed */ }
    }
  }
}

export interface RawIngredient {
  raw: string;
  hint: string | null;
  optional: boolean;
}

export interface RecipeDetail {
  id: number;
  title: string;
  cuisine: string | null;
  ingredient_count: number;
  vegetarian: boolean;
  vegan: boolean;
  gluten_free: boolean;
  ingredients_raw: RawIngredient[];
  ingredients_core: string[];
  directions: string[];
}

export async function fetchRecipeDetail(id: number): Promise<RecipeDetail> {
  const res = await fetch(`${API_URL}/recipes/${id}`, { credentials: "include" });
  if (!res.ok) throw new Error(`fetchRecipeDetail failed: ${res.status}`);
  return res.json();
}

export interface PresentResponse {
  theme: string;
  difficulty: string;
  time_minutes: number;
  description: string;
  ingredients: { name: string; amount: string; have: boolean }[];
  steps: string[];
  substitutions: { ingredient: string; substitute: string; note?: string }[];
}

export async function presentRecipe(
  recipe: RecipeDetail,
  userIngredients: string[],
): Promise<PresentResponse> {
  const res = await fetch(`${API_URL}/ai/present-recipe`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      recipe: {
        id: recipe.id,
        title: recipe.title,
        ingredients_raw: recipe.ingredients_raw.map((i) => i.raw),
        ingredients_core: recipe.ingredients_core,
        directions: recipe.directions,
      },
      user_ingredients: userIngredients,
    }),
    credentials: "include",
  });
  if (!res.ok) throw new Error(`presentRecipe failed: ${res.status}`);
  return res.json();
}

// Silent history record — swallows errors so a missing auth token doesn't break the cook flow
export async function recordCookSilent(recipe_id: number, recipe_name: string): Promise<void> {
  try {
    await recordCook(recipe_id, recipe_name);
  } catch {
    // Not authenticated or network error — silently skip
  }
}

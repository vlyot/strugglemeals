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

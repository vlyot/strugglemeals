import { createAuthClient } from "@neondatabase/neon-js/auth";
import { BetterAuthReactAdapter } from "@neondatabase/neon-js/auth/react";

export const authClient = createAuthClient(
  import.meta.env.VITE_NEON_AUTH_URL as string,
  { adapter: BetterAuthReactAdapter() }
);

export async function getSessionToken(): Promise<string | null> {
  // Bypass the adapter's JWT injection — fetch the raw session directly so we
  // get the opaque DB token that the backend validates against neon_auth.session.
  const authUrl = import.meta.env.VITE_NEON_AUTH_URL as string;
  const res = await fetch(`${authUrl}/get-session`, { credentials: "include" });
  if (!res.ok) return null;
  const data = await res.json();
  return data?.session?.token ?? null;
}

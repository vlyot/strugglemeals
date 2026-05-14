import { Link } from "react-router-dom";
import { authClient } from "@/stack/client";

export function AuthNudgeBanner() {
  const { data: session } = authClient.useSession();
  const user = session?.user ?? null;

  if (user) {
    return (
      <div
        className="inline-flex items-center gap-2 px-4 py-2 rounded-full text-sm font-medium"
        style={{ backgroundColor: "#16a34a", color: "#fff" }}
      >
        <span>✓</span>
        <span>Recipes you cook are saved to History automatically</span>
      </div>
    );
  }

  return (
    <p className="text-sm text-muted-foreground">
      <Link to="/handler/sign-in" className="text-primary hover:underline font-medium">
        Sign in →
      </Link>{" "}
      to save recipes to History
    </p>
  );
}

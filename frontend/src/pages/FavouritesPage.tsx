import { useEffect, useState } from "react";
import { usePageTitle } from "@/hooks/usePageTitle";
import { Link } from "react-router-dom";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { ThemeBadge } from "@/components/cook/IngredientInput";
import { fetchFavourites, removeFavourite, type FavouriteEntry } from "@/lib/api";

function formatSavedDate(iso: string): string {
  return new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" }).format(new Date(iso));
}

function deriveTheme(name: string): string {
  const lower = name.toLowerCase();
  if (lower.includes("salad") || lower.includes("soup") || lower.includes("light")) return "Light";
  if (lower.includes("quick") || lower.includes("easy") || lower.includes("simple")) return "Quick";
  return "Filling";
}

interface FavouriteCardProps {
  entry: FavouriteEntry;
  onRemove: (id: number) => Promise<void>;
}

function FavouriteCard({ entry, onRemove }: FavouriteCardProps) {
  const [removing, setRemoving] = useState(false);

  async function handleRemove() {
    setRemoving(true);
    await onRemove(entry.recipe_id);
    setRemoving(false);
  }

  return (
    <Card className="relative">
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-2 mb-2">
          <ThemeBadge theme={deriveTheme(entry.recipe_name)} />
          <button
            type="button"
            onClick={handleRemove}
            disabled={removing}
            className="text-muted-foreground hover:text-destructive transition-colors shrink-0"
            title="Remove from favourites"
          >
            ♥
          </button>
        </div>
        <h3 className="font-semibold text-foreground text-sm leading-snug mb-1">{entry.recipe_name}</h3>
        <p className="text-xs text-muted-foreground">Saved {formatSavedDate(entry.saved_at)}</p>
        <Link
          to="/cook"
          state={{ recipeId: entry.recipe_id, recipeTitle: entry.recipe_name }}
          className="mt-3 block w-full text-center text-sm font-medium text-primary-foreground bg-primary rounded-lg py-2 hover:bg-primary/90 transition-colors"
        >
          Cook →
        </Link>
      </CardContent>
    </Card>
  );
}

export default function FavouritesPage() {
  usePageTitle("Favourites")
  const [favourites, setFavourites] = useState<FavouriteEntry[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchFavourites()
      .then((data) => setFavourites(data.favourites))
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  async function handleRemove(recipe_id: number) {
    await removeFavourite(recipe_id);
    setFavourites((prev) => prev.filter((f) => f.recipe_id !== recipe_id));
  }

  return (
    <div className="min-h-screen bg-background">
      {/* Nav */}
      <header className="sticky top-0 z-40 border-b border-border bg-background/95 backdrop-blur-sm">
        <div className="max-w-2xl mx-auto px-4 h-14 flex items-center justify-between">
          <Link to="/" className="font-serif text-lg font-light text-foreground tracking-tight">
            StruggleMeals
          </Link>
          <span className="text-sm font-medium text-foreground">Favourites</span>
          <Link to="/cook" className="text-sm text-muted-foreground hover:text-foreground transition-colors">
            ← Cook
          </Link>
        </div>
      </header>

      <div className="max-w-2xl mx-auto px-4 py-8">
        <div className="mb-6">
          <h1 className="text-2xl font-semibold text-foreground">Favourites</h1>
          {!loading && (
            <p className="text-sm text-muted-foreground mt-1">
              Kept indefinitely · {favourites.length} recipe{favourites.length !== 1 ? "s" : ""}
            </p>
          )}
        </div>

        {loading ? (
          <div className="grid grid-cols-2 gap-4">
            {[...Array(4)].map((_, i) => (
              <Skeleton key={i} className="h-36 w-full rounded" />
            ))}
          </div>
        ) : favourites.length === 0 ? (
          <div className="text-center py-16 text-muted-foreground">
            <p>No favourites yet. Save a recipe to see it here.</p>
          </div>
        ) : (
          <div className="grid grid-cols-2 lg:grid-cols-3 gap-4">
            {favourites.map((entry) => (
              <FavouriteCard key={entry.id} entry={entry} onRemove={handleRemove} />
            ))}
          </div>
        )}

        <div className="mt-10">
          <Link
            to="/cook"
            className="block w-full text-center px-6 py-3 text-sm font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
          >
            Find more recipes →
          </Link>
        </div>
      </div>
    </div>
  );
}

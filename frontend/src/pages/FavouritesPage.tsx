import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { fetchFavourites, removeFavourite, type FavouriteEntry } from "@/lib/api";

function formatDate(iso: string): string {
  return new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", year: "numeric" }).format(new Date(iso));
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
        <div className="flex items-start justify-between gap-2">
          <div className="flex-1 min-w-0">
            <span
              className="text-xs px-2 py-0.5 rounded-full border mb-2 inline-block"
              style={{ color: "var(--color-primary)", borderColor: "var(--color-primary)" }}
            >
              Saved
            </span>
            <h3 className="font-semibold text-foreground text-sm leading-snug mb-1">{entry.recipe_name}</h3>
            <p className="text-xs text-muted-foreground">Saved {formatDate(entry.saved_at)}</p>
          </div>
          <button
            type="button"
            onClick={handleRemove}
            disabled={removing}
            className="text-muted-foreground hover:text-destructive transition-colors shrink-0 mt-1"
            title="Remove from favourites"
          >
            ♥
          </button>
        </div>
        <Link
          to={`/?recipe=${entry.recipe_id}`}
          className="mt-3 block w-full text-center text-sm font-medium text-primary hover:underline"
        >
          Cook →
        </Link>
      </CardContent>
    </Card>
  );
}

export default function FavouritesPage() {
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
      <div className="max-w-2xl mx-auto px-4 py-12">
        <div className="flex items-center justify-between mb-8">
          <h1 className="text-2xl font-semibold text-foreground">Favourites</h1>
          <Link to="/" className="text-sm text-muted-foreground hover:text-foreground">← Back</Link>
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
            to="/#get-started"
            className="block w-full text-center px-6 py-3 text-sm font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
          >
            Find more recipes →
          </Link>
        </div>
      </div>
    </div>
  );
}

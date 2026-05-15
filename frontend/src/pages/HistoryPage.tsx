import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { ThemeBadge } from "@/components/cook/IngredientInput";
import { fetchHistory, deleteHistoryEntry, addFavourite, type HistoryEntry } from "@/lib/api";

type Filter = "all" | "week" | "month";

function tagForEntry(entry: HistoryEntry): string {
  const name = entry.recipe_name.toLowerCase();
  if (name.includes("salad") || name.includes("soup") || name.includes("light")) return "Light";
  if (name.includes("quick") || name.includes("easy") || name.includes("simple")) return "Quick";
  return "Filling";
}

function formatRelativeDate(iso: string): string {
  const diffDays = Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);
  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  return new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" }).format(new Date(iso));
}

interface HistoryRowProps {
  entry: HistoryEntry;
  isSaved: boolean;
  onSave: (entry: HistoryEntry) => void;
  onDelete: (id: string) => void;
}

function HistoryRow({ entry, isSaved, onSave, onDelete }: HistoryRowProps) {
  const tag = tagForEntry(entry);
  return (
    <div className="flex items-center justify-between gap-3 py-4 border-b border-border last:border-0">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="font-medium text-foreground truncate">{entry.recipe_name}</span>
          <ThemeBadge theme={tag} />
        </div>
        <p className="text-sm text-muted-foreground mt-0.5">{formatRelativeDate(entry.cooked_at)}</p>
      </div>
      <div className="flex items-center gap-3 shrink-0">
        <Link to="/cook" className="text-sm font-medium text-primary hover:underline">
          Cook again →
        </Link>
        <button
          type="button"
          onClick={() => onSave(entry)}
          disabled={isSaved}
          className={[
            "text-sm transition-colors",
            isSaved
              ? "text-green-700 font-medium cursor-default"
              : "text-muted-foreground hover:text-primary",
          ].join(" ")}
          title={isSaved ? "Saved" : "Save to favourites"}
        >
          {isSaved ? "✓ Saved" : "♡ Save"}
        </button>
        <button
          type="button"
          onClick={() => onDelete(entry.id)}
          className="text-sm text-muted-foreground hover:text-destructive transition-colors"
          title="Delete entry"
        >
          ✕
        </button>
      </div>
    </div>
  );
}

export default function HistoryPage() {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [loadedKey, setLoadedKey] = useState<string | null>(null);
  const [savedIds, setSavedIds] = useState<Set<string>>(new Set());

  const currentKey = `${search}:${filter}`;

  useEffect(() => {
    let cancelled = false;
    fetchHistory({ search: search || undefined, filter })
      .then((data) => {
        if (!cancelled) {
          setEntries(data.entries);
          setLoadedKey(`${search}:${filter}`);
        }
      })
      .catch(console.error);
    return () => { cancelled = true; };
  }, [search, filter]);

  const loading = loadedKey !== currentKey;

  async function handleDelete(id: string) {
    await deleteHistoryEntry(id);
    setEntries((prev) => prev.filter((e) => e.id !== id));
    setSavedIds((prev) => { const next = new Set(prev); next.delete(id); return next; });
  }

  async function handleSave(entry: HistoryEntry) {
    await addFavourite(entry.recipe_id, entry.recipe_name);
    setSavedIds((prev) => new Set(prev).add(entry.id));
  }

  const filterLabels: { key: Filter; label: string }[] = [
    { key: "all", label: "All" },
    { key: "week", label: "This week" },
    { key: "month", label: "Last 30 days" },
  ];

  return (
    <div className="min-h-screen bg-background">
      {/* Nav */}
      <header className="sticky top-0 z-40 border-b border-border bg-background/95 backdrop-blur-sm">
        <div className="max-w-2xl mx-auto px-4 h-14 flex items-center justify-between">
          <Link to="/" className="font-serif text-lg font-light text-foreground tracking-tight">
            StruggleMeals
          </Link>
          <span className="text-sm font-medium text-foreground">History</span>
          <Link to="/cook" className="text-sm text-muted-foreground hover:text-foreground transition-colors">
            ← Cook
          </Link>
        </div>
      </header>

      <div className="max-w-2xl mx-auto px-4 py-8">
        <Input
          placeholder="Search history..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="mb-4"
        />

        <div className="flex gap-2 mb-6">
          {filterLabels.map(({ key, label }) => (
            <Button
              key={key}
              variant={filter === key ? "default" : "outline"}
              size="sm"
              onClick={() => setFilter(key)}
            >
              {label}
            </Button>
          ))}
        </div>

        {loading ? (
          <div className="space-y-4">
            {[...Array(4)].map((_, i) => (
              <Skeleton key={i} className="h-16 w-full rounded" />
            ))}
          </div>
        ) : entries.length === 0 ? (
          <div className="text-center py-16 text-muted-foreground">
            <p>No history yet. Start cooking!</p>
          </div>
        ) : (
          <div>
            {entries.map((entry) => (
              <HistoryRow
                key={entry.id}
                entry={entry}
                isSaved={savedIds.has(entry.id)}
                onSave={handleSave}
                onDelete={handleDelete}
              />
            ))}
          </div>
        )}

        <p className="mt-8 text-xs text-muted-foreground text-center">
          History auto-deletes after 60 days · Favourite to keep indefinitely
        </p>
      </div>
    </div>
  );
}

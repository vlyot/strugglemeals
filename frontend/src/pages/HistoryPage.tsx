import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { fetchHistory, deleteHistoryEntry, addFavourite, type HistoryEntry } from "@/lib/api";

type Filter = "all" | "week" | "month";

function tagForEntry(entry: HistoryEntry): string {
  const name = entry.recipe_name.toLowerCase();
  if (name.includes("salad") || name.includes("soup") || name.includes("light")) return "Light";
  if (name.includes("quick") || name.includes("easy") || name.includes("simple")) return "Quick";
  return "Filling";
}

function formatDate(iso: string): string {
  return new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" }).format(new Date(iso));
}

interface HistoryRowProps {
  entry: HistoryEntry;
  onSave: (entry: HistoryEntry) => void;
  onDelete: (id: string) => void;
}

function HistoryRow({ entry, onSave, onDelete }: HistoryRowProps) {
  const tag = tagForEntry(entry);
  return (
    <div className="flex items-center justify-between gap-3 py-4 border-b border-border last:border-0">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="font-medium text-foreground truncate">{entry.recipe_name}</span>
          <span
            className="shrink-0 text-xs px-2 py-0.5 rounded-full border"
            style={{ color: "var(--color-primary)", borderColor: "var(--color-primary)" }}
          >
            {tag}
          </span>
        </div>
        <p className="text-sm text-muted-foreground mt-0.5">{formatDate(entry.cooked_at)}</p>
      </div>
      <div className="flex items-center gap-3 shrink-0">
        <Link to={`/?recipe=${entry.recipe_id}`} className="text-sm font-medium text-primary hover:underline">
          Cook again →
        </Link>
        <button
          type="button"
          onClick={() => onSave(entry)}
          className="text-sm text-muted-foreground hover:text-primary transition-colors"
          title="Save to favourites"
        >
          ♡ Save
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
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    fetchHistory({ search: search || undefined, filter })
      .then((data) => setEntries(data.entries))
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [search, filter]);

  async function handleDelete(id: string) {
    await deleteHistoryEntry(id);
    setEntries((prev) => prev.filter((e) => e.id !== id));
  }

  async function handleSave(entry: HistoryEntry) {
    await addFavourite(entry.recipe_id, entry.recipe_name);
  }

  const filterLabels: { key: Filter; label: string }[] = [
    { key: "all", label: "All" },
    { key: "week", label: "This week" },
    { key: "month", label: "Last 30 days" },
  ];

  return (
    <div className="min-h-screen bg-background">
      <div className="max-w-2xl mx-auto px-4 py-12">
        <div className="flex items-center justify-between mb-8">
          <h1 className="text-2xl font-semibold text-foreground">History</h1>
          <Link to="/" className="text-sm text-muted-foreground hover:text-foreground">← Back</Link>
        </div>

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
              <HistoryRow key={entry.id} entry={entry} onSave={handleSave} onDelete={handleDelete} />
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

import { useState, useRef } from "react"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { ThemeBadge } from "./IngredientInput"
import { fetchRecipeDetail, type ShortlistEntry } from "@/lib/api"

interface Props {
  results: ShortlistEntry[]
  loading: boolean
  presenting: number | null
  userIngredients: string[]
  onBack: () => void
  onCook: (entry: ShortlistEntry) => void
}

const THEMES = ["Light", "Filling", "Quick"] as const
type Theme = (typeof THEMES)[number]

const THEME_META: Record<Theme, { icon: string; subtitle: string }> = {
  Light: { icon: "🥗", subtitle: "Under 400 cal, light" },
  Filling: { icon: "🍳", subtitle: "Satisfying · complete meal" },
  Quick: { icon: "⚡", subtitle: "Under 12 min · minimal prep" },
}

function MatchBar({ score, total }: { score: number; total: number }) {
  const pct = total > 0 ? Math.round((score / total) * 100) : 0
  return (
    <div className="flex items-center gap-2">
      <span className="text-xs text-muted-foreground uppercase tracking-wider">Match</span>
      <div className="flex-1 h-1.5 bg-border rounded-full overflow-hidden">
        <div
          className="h-full rounded-full transition-all"
          style={{
            width: `${pct}%`,
            background: pct >= 80 ? "var(--color-primary)" : "var(--color-accent)",
          }}
        />
      </div>
      <span className="text-xs font-medium text-muted-foreground">
        {pct}% · {score}/{total}
      </span>
    </div>
  )
}

function RecipeCard({
  entry,
  featured,
  presenting,
  userIngredients,
  onCook,
}: {
  entry: ShortlistEntry
  featured: boolean
  presenting: boolean
  userIngredients: string[]
  onCook: () => void
}) {
  const matchRatio = entry.ingredient_count > 0 ? entry.match_score / entry.ingredient_count : 1
  const lowMatch = !featured && matchRatio < 0.4

  const [expanded, setExpanded] = useState(false)
  const [missingOpen, setMissingOpen] = useState(false)
  const [missingIngredients, setMissingIngredients] = useState<string[] | null>(null)
  const [missingLoading, setMissingLoading] = useState(false)
  const fetchedRef = useRef(false)

  const showPanel = featured || expanded

  const handleMissingClick = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (missingOpen) {
      setMissingOpen(false)
      return
    }
    setMissingOpen(true)
    if (fetchedRef.current) return
    fetchedRef.current = true
    setMissingLoading(true)
    const userLower = userIngredients.map((u) => u.toLowerCase())
    fetchRecipeDetail(entry.id)
      .then((detail) => {
        const missing = detail.ingredients_raw.filter((raw) => {
          const rawLower = raw.toLowerCase()
          return !userLower.some((u) => rawLower.includes(u) || u.includes(rawLower.split(/\s+/).slice(-1)[0]))
        })
        setMissingIngredients(missing)
      })
      .catch(() => setMissingIngredients([]))
      .finally(() => setMissingLoading(false))
  }

  return (
    <div
      className={[
        "rounded-2xl border p-5 flex flex-col gap-4 transition-colors cursor-pointer",
        featured
          ? "border-primary/30 bg-card shadow-sm"
          : lowMatch
            ? "border-border/40 bg-card/40 opacity-75"
            : "border-border bg-card/60",
      ].join(" ")}
      onPointerEnter={() => !featured && setExpanded(true)}
      onPointerLeave={() => !featured && setExpanded(false)}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex flex-col gap-1 flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            {featured && (
              <span className="text-xs font-semibold tracking-wider uppercase text-primary bg-primary/10 px-2 py-0.5 rounded-full">
                Best match
              </span>
            )}
            <ThemeBadge theme={entry.theme} />
          </div>
          <h3
            className={[
              "font-serif font-light leading-tight text-foreground",
              featured ? "text-2xl" : "text-xl",
            ].join(" ")}
          >
            {entry.title}
          </h3>
          {entry.reason && (
            <p className="text-sm text-muted-foreground">{entry.reason}</p>
          )}
        </div>
        {!featured && (
          <button
            type="button"
            onClick={onCook}
            disabled={presenting}
            className="text-muted-foreground hover:text-foreground transition-colors shrink-0 mt-1"
            aria-label={`Cook ${entry.title}`}
          >
            <svg width="18" height="18" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M17 8l4 4m0 0l-4 4m4-4H3" />
            </svg>
          </button>
        )}
      </div>

      {/* Ingredient panel */}
      {showPanel && (
        <div className="flex flex-col gap-2">
          <div className="h-px bg-border/50" />
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Your ingredients</p>
          <div className="flex flex-wrap gap-1.5">
            {entry.matched_ingredients.map((ing) => (
              <span
                key={ing}
                className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-green-50 text-green-700 border border-green-200"
              >
                <span>✓</span> {ing}
              </span>
            ))}
            {entry.missing_count > 0 && (
              <button
                type="button"
                onClick={handleMissingClick}
                className="inline-flex items-center text-xs px-2 py-0.5 rounded-full bg-muted text-muted-foreground border border-border hover:bg-muted/80 transition-colors"
              >
                {missingOpen ? "▲" : "+"}{entry.missing_count} more needed
              </button>
            )}
          </div>
          {missingOpen && (
            <div className="flex flex-col gap-1 pl-1">
              {missingLoading ? (
                <div className="flex flex-col gap-1">
                  {[...Array(entry.missing_count)].map((_, i) => (
                    <Skeleton key={i} className="h-3 w-32 rounded" />
                  ))}
                </div>
              ) : (
                missingIngredients?.map((ing, i) => (
                  <span key={i} className="text-xs text-muted-foreground">· {ing}</span>
                ))
              )}
            </div>
          )}
          <p className="text-xs text-muted-foreground">
            {entry.match_score} of {entry.ingredient_count} matched
          </p>
        </div>
      )}

      {featured && <MatchBar score={entry.match_score} total={entry.ingredient_count} />}

      {featured && (
        <Button
          onClick={onCook}
          disabled={presenting}
          className="w-full bg-primary text-primary-foreground hover:bg-primary/90 h-11 rounded-xl"
        >
          {presenting ? "Preparing recipe..." : "Cook this →"}
        </Button>
      )}

      {lowMatch && !expanded && (
        <p className="text-xs text-muted-foreground/70">
          Missing {entry.missing_count} of {entry.ingredient_count} ingredients
        </p>
      )}
    </div>
  )
}

function ThemeSection({
  theme,
  entries,
  presenting,
  userIngredients,
  onCook,
}: {
  theme: Theme
  entries: ShortlistEntry[]
  presenting: number | null
  userIngredients: string[]
  onCook: (entry: ShortlistEntry) => void
}) {
  const meta = THEME_META[theme]
  if (entries.length === 0) {
    return (
      <div className="py-12 text-center text-muted-foreground text-sm">
        No {theme.toLowerCase()} recipes found for your ingredients.
      </div>
    )
  }

  const [featured, ...rest] = entries

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        <span className="text-lg">{meta.icon}</span>
        <div>
          <p className="font-medium text-foreground">
            {theme} recipes
          </p>
          <p className="text-xs text-muted-foreground">
            {entries.length} option{entries.length !== 1 ? "s" : ""} · {meta.subtitle}
          </p>
        </div>
      </div>

      <RecipeCard
        entry={featured}
        featured
        presenting={presenting === featured.id}
        userIngredients={userIngredients}
        onCook={() => onCook(featured)}
      />
      {rest.map((entry) => (
        <RecipeCard
          key={entry.id}
          entry={entry}
          featured={false}
          presenting={presenting === entry.id}
          userIngredients={userIngredients}
          onCook={() => onCook(entry)}
        />
      ))}
    </div>
  )
}

export function ShortlistView({ results, loading, presenting, userIngredients, onBack, onCook }: Props) {
  // Determine default tab (first theme that has results)
  const byTheme = (theme: Theme) =>
    results.filter((r) => r.theme === theme)

  const defaultTab =
    THEMES.find((t) => byTheme(t).length > 0) ?? "Filling"

  if (loading) {
    return (
      <div className="flex flex-col gap-4 pt-2">
        <Skeleton className="h-6 w-40 rounded-lg" />
        <Skeleton className="h-36 rounded-2xl" />
        <Skeleton className="h-36 rounded-2xl" />
        <Skeleton className="h-28 rounded-2xl opacity-60" />
      </div>
    )
  }

  if (results.length === 0) {
    return (
      <div className="py-16 text-center flex flex-col gap-4 items-center">
        <p className="text-lg font-medium text-foreground">No recipes found</p>
        <p className="text-sm text-muted-foreground max-w-xs">
          Try adding more ingredients or removing dietary filters.
        </p>
        <Button variant="outline" onClick={onBack}>
          Try again
        </Button>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-5">
      {/* Back + title */}
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={onBack}
          className="text-muted-foreground hover:text-foreground transition-colors"
          aria-label="Back to ingredients"
        >
          <svg width="20" height="20" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M19 12H5M5 12l7-7M5 12l7 7" />
          </svg>
        </button>
        <div>
          <p className="font-medium text-foreground">Ingredients</p>
          <p className="text-xs text-muted-foreground">
            {results.length} recipe{results.length !== 1 ? "s" : ""} matched
          </p>
        </div>
      </div>

      {/* Theme tabs */}
      <Tabs defaultValue={defaultTab} className="flex-col">
        <TabsList className="w-full h-10">
          {THEMES.map((t) => (
            <TabsTrigger key={t} value={t} className="flex-1 h-full">
              {t}
              {byTheme(t).length > 0 && (
                <span className="ml-1.5 text-xs opacity-60">({byTheme(t).length})</span>
              )}
            </TabsTrigger>
          ))}
        </TabsList>

        {THEMES.map((t) => (
          <TabsContent key={t} value={t} className="mt-4">
            <ThemeSection
              theme={t}
              entries={byTheme(t)}
              presenting={presenting}
              userIngredients={userIngredients}
              onCook={onCook}
            />
          </TabsContent>
        ))}
      </Tabs>
    </div>
  )
}

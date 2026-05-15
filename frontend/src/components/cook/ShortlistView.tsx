import { useState, useRef, useEffect } from "react"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Progress } from "@/components/ui/progress"
import { ThemeBadge } from "./IngredientInput"
import { fetchRecipeDetail, type ShortlistEntry, type RawIngredient } from "@/lib/api"

interface Props {
  results: ShortlistEntry[]
  loading: boolean
  progress: number
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
  open,
  presenting,
  userIngredients,
  onToggle,
  onCook,
}: {
  entry: ShortlistEntry
  open: boolean
  presenting: boolean
  userIngredients: string[]
  onToggle: () => void
  onCook: () => void
}) {
  const [missingOpen, setMissingOpen] = useState(false)
  const [missingIngredients, setMissingIngredients] = useState<RawIngredient[] | null>(null)
  const [missingLoading, setMissingLoading] = useState(false)
  const fetchedRef = useRef(false)

  const fetchMissing = () => {
    if (fetchedRef.current) return
    fetchedRef.current = true
    setMissingLoading(true)
    const userLower = userIngredients.map((u) => u.toLowerCase())
    fetchRecipeDetail(entry.id)
      .then((detail) => {
        const missing = detail.ingredients_raw.filter((item) => {
          const rawLower = item.raw.toLowerCase()
          return !userLower.some((u) => rawLower.includes(u) || u.includes(rawLower.split(/\s+/).slice(-1)[0]))
        })
        setMissingIngredients(missing)
      })
      .catch(() => setMissingIngredients([]))
      .finally(() => setMissingLoading(false))
  }

  useEffect(() => {
    if (open && entry.missing_count > 0) fetchMissing()
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  const handleMissingClick = (e: React.MouseEvent) => {
    e.stopPropagation()
    setMissingOpen((v) => !v)
  }

  return (
    <div
      className={[
        "rounded-2xl border bg-card transition-colors duration-200 overflow-hidden",
        open ? "border-primary/30 shadow-sm" : "border-border cursor-pointer hover:border-primary/20",
      ].join(" ")}
      onClick={!open ? onToggle : undefined}
    >
      {/* Always-visible header */}
      <div
        className="flex items-center gap-3 px-5 py-4"
        style={{ cursor: "pointer" }}
        onClick={open ? onToggle : undefined}
      >
        <div className="flex flex-col gap-0.5 flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <ThemeBadge theme={entry.theme} />
          </div>
          <h3 className="font-serif font-light text-xl leading-tight text-foreground truncate">
            {entry.title}
          </h3>
          {entry.reason && (
            <p className="text-sm text-muted-foreground truncate">{entry.reason}</p>
          )}
        </div>
        <svg
          width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor"
          className={["shrink-0 text-muted-foreground transition-transform duration-300", open ? "rotate-180" : ""].join(" ")}
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </div>

      {/* Expandable panel */}
      <div
        className="overflow-hidden"
        style={{
          maxHeight: open ? "600px" : "0px",
          transition: open
            ? "max-height 0.45s cubic-bezier(0.4, 0, 0.2, 1)"
            : "max-height 0.3s cubic-bezier(0.4, 0, 0.2, 1)",
        }}
      >
        <div className="flex flex-col gap-3 px-5 pb-5">
          <div className="h-px bg-border/50" />

          {/* Matched + missing chips */}
          <div className="flex flex-col gap-2">
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
                  {missingOpen ? "▲ " : "+ "}{entry.missing_count} more needed
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
                  missingIngredients?.map((item, i) => (
                    <div key={i} className="flex flex-col">
                      <div className="flex items-baseline gap-1.5">
                        <span className="text-xs text-muted-foreground">· {item.raw}</span>
                        {item.optional && (
                          <span className="text-[10px] px-1.5 py-px rounded-full bg-muted text-muted-foreground/60 border border-border leading-tight shrink-0">
                            optional
                          </span>
                        )}
                      </div>
                      {item.hint && (
                        <span className="text-xs text-muted-foreground/50 pl-3 italic">{item.hint}</span>
                      )}
                    </div>
                  ))
                )}
              </div>
            )}

            <p className="text-xs text-muted-foreground">
              {entry.match_score} of {entry.ingredient_count} matched
            </p>
          </div>

          <MatchBar score={entry.match_score} total={entry.ingredient_count} />

          <Button
            onClick={(e) => { e.stopPropagation(); onCook() }}
            disabled={presenting}
            className="w-full bg-primary text-primary-foreground hover:bg-primary/90 h-11 rounded-xl"
          >
            {presenting ? "Preparing recipe..." : "Cook this →"}
          </Button>
        </div>
      </div>
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
  const [openId, setOpenId] = useState<number | null>(entries[0]?.id ?? null)

  if (entries.length === 0) {
    return (
      <div className="py-12 text-center text-muted-foreground text-sm">
        No {theme.toLowerCase()} recipes found for your ingredients.
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-3">
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

      {entries.map((entry) => (
        <RecipeCard
          key={entry.id}
          entry={entry}
          open={openId === entry.id}
          presenting={presenting === entry.id}
          userIngredients={userIngredients}
          onToggle={() => setOpenId(openId === entry.id ? null : entry.id)}
          onCook={() => onCook(entry)}
        />
      ))}
    </div>
  )
}

export function ShortlistView({ results, loading, progress, presenting, userIngredients, onBack, onCook }: Props) {
  // Determine default tab (first theme that has results)
  const byTheme = (theme: Theme) =>
    results.filter((r) => r.theme === theme)

  const defaultTab =
    THEMES.find((t) => byTheme(t).length > 0) ?? "Filling"

  const showProgress = progress > 0 && progress < 100

  if (loading) {
    return (
      <div className="flex flex-col gap-4 pt-2">
        {showProgress && (
          <div className="flex flex-col gap-1.5">
            <div className="flex items-center justify-between">
              <p className="text-xs text-muted-foreground">
                {progress < 70 ? "Searching recipes…" : "Picking the best matches…"}
              </p>
              <p className="text-xs text-muted-foreground tabular-nums">{progress}%</p>
            </div>
            <Progress value={progress} className="h-1.5" />
          </div>
        )}
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
      {/* Progress bar — visible while themes are still loading after scores arrived */}
      {showProgress && (
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between">
            <p className="text-xs text-muted-foreground">Picking the best matches…</p>
            <p className="text-xs text-muted-foreground tabular-nums">{progress}%</p>
          </div>
          <Progress value={progress} className="h-1.5" />
        </div>
      )}

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

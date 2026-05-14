import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { ThemeBadge } from "./IngredientInput"
import { authClient } from "@/stack/client"
import { addFavourite } from "@/lib/api"
import { useState } from "react"
import type { PresentResponse } from "@/lib/api"

interface Props {
  open: boolean
  onClose: () => void
  recipeId: number | null
  recipeTitle: string
  response: PresentResponse | null
  loading: boolean
  error: string | null
}

function DifficultyBadge({ difficulty }: { difficulty: string }) {
  const styles: Record<string, string> = {
    Easy: "text-green-700 bg-green-50 border-green-200",
    Medium: "text-amber-700 bg-amber-50 border-amber-200",
    Hard: "text-red-700 bg-red-50 border-red-200",
  }
  return (
    <span
      className={`text-xs font-medium px-2 py-0.5 rounded-full border ${styles[difficulty] ?? "bg-secondary"}`}
    >
      {difficulty}
    </span>
  )
}

export function RecipeModal({
  open,
  onClose,
  recipeId,
  recipeTitle,
  response,
  loading,
  error,
}: Props) {
  const { data: session } = authClient.useSession()
  const isSignedIn = !!session?.user
  const [saved, setSaved] = useState(false)
  const [saving, setSaving] = useState(false)

  const handleSave = async () => {
    if (!recipeId || saving || saved) return
    setSaving(true)
    try {
      await addFavourite(recipeId, recipeTitle)
      setSaved(true)
    } catch {
      // silently fail — show sign in nudge
    } finally {
      setSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-lg max-h-[90vh] overflow-y-auto p-0">
        {loading ? (
          <div className="p-6 flex flex-col gap-4">
            <Skeleton className="h-7 w-3/4 rounded-lg" />
            <div className="flex gap-2">
              <Skeleton className="h-5 w-16 rounded-full" />
              <Skeleton className="h-5 w-16 rounded-full" />
              <Skeleton className="h-5 w-20 rounded-full" />
            </div>
            <Skeleton className="h-4 w-full rounded" />
            <Skeleton className="h-4 w-5/6 rounded" />
            <div className="flex flex-col gap-2 mt-2">
              {[...Array(5)].map((_, i) => (
                <Skeleton key={i} className="h-3 w-full rounded" />
              ))}
            </div>
          </div>
        ) : error ? (
          <div className="p-6 flex flex-col gap-4 items-center text-center">
            <p className="text-sm text-muted-foreground">{error}</p>
            <Button variant="outline" size="sm" onClick={onClose}>
              Close
            </Button>
          </div>
        ) : response ? (
          <>
            {/* Header */}
            <div className="p-6 pb-4 border-b border-border">
              <DialogHeader>
                <DialogTitle className="font-serif text-2xl font-light leading-tight pr-4">
                  {recipeTitle}
                </DialogTitle>
              </DialogHeader>
              <div className="flex items-center gap-2 mt-3 flex-wrap">
                <ThemeBadge theme={response.theme} />
                <DifficultyBadge difficulty={response.difficulty} />
                <span className="text-xs text-muted-foreground">
                  {response.time_minutes} min
                </span>
                <span className="flex-1" />
              </div>
              {response.description && (
                <p className="text-sm text-muted-foreground mt-2 italic">
                  {response.description}
                </p>
              )}
            </div>

            <div className="p-6 flex flex-col gap-6">
              {/* Ingredients */}
              <section>
                <h4 className="text-xs font-semibold tracking-widest uppercase text-muted-foreground mb-3">
                  Ingredients
                </h4>
                <ul className="flex flex-col gap-2">
                  {response.ingredients.map((ing, i) => (
                    <li key={i} className="flex items-start gap-2 text-sm">
                      <span
                        className={`mt-0.5 shrink-0 w-4 h-4 rounded-full flex items-center justify-center text-[10px] font-bold ${
                          ing.have
                            ? "bg-green-100 text-green-700"
                            : "bg-muted text-muted-foreground"
                        }`}
                      >
                        {ing.have ? "✓" : "?"}
                      </span>
                      <span className={ing.have ? "text-foreground" : "text-muted-foreground"}>
                        <span className="font-medium">{ing.amount}</span> {ing.name}
                      </span>
                    </li>
                  ))}
                </ul>
              </section>

              {/* Steps */}
              <section>
                <h4 className="text-xs font-semibold tracking-widest uppercase text-muted-foreground mb-3">
                  Steps
                </h4>
                <ol className="flex flex-col gap-3">
                  {response.steps.map((step, i) => (
                    <li key={i} className="flex gap-3 text-sm">
                      <span className="shrink-0 w-5 h-5 rounded-full bg-primary/10 text-primary text-[10px] font-bold flex items-center justify-center mt-0.5">
                        {i + 1}
                      </span>
                      <span className="text-foreground/80 leading-relaxed">{step}</span>
                    </li>
                  ))}
                </ol>
              </section>

              {/* Substitutions */}
              {response.substitutions.length > 0 && (
                <section>
                  <h4 className="text-xs font-semibold tracking-widest uppercase text-muted-foreground mb-3">
                    Substitutions
                  </h4>
                  <ul className="flex flex-col gap-2">
                    {response.substitutions.map((s, i) => (
                      <li key={i} className="text-sm text-muted-foreground">
                        <span className="font-medium text-foreground">{s.ingredient}</span>
                        {" → "}
                        <span>{s.substitute}</span>
                        {s.note && (
                          <span className="text-muted-foreground/70"> ({s.note})</span>
                        )}
                      </li>
                    ))}
                  </ul>
                </section>
              )}
            </div>

            {/* Footer */}
            <div className="px-6 pb-6 flex flex-col gap-3 border-t border-border pt-4">
              {isSignedIn ? (
                <Button
                  onClick={handleSave}
                  disabled={saving || saved}
                  variant={saved ? "secondary" : "outline"}
                  className="w-full"
                >
                  {saved ? "✓ Saved to favourites" : saving ? "Saving..." : "Save to favourites"}
                </Button>
              ) : (
                <p className="text-sm text-muted-foreground text-center">
                  <a
                    href="/handler/sign-in"
                    className="text-primary hover:underline font-medium"
                  >
                    Sign in
                  </a>{" "}
                  to save recipes to your favourites
                </p>
              )}
            </div>
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

import { useState } from "react"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { authClient } from "@/stack/client"
import { addFavourite } from "@/lib/api"
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

function SaveFooter({
  recipeId,
  recipeTitle,
  isSignedIn,
}: {
  recipeId: number | null
  recipeTitle: string
  isSignedIn: boolean
}) {
  const [saved, setSaved] = useState(false)
  const [saving, setSaving] = useState(false)

  const handleSave = async () => {
    if (!recipeId || saving || saved) return
    setSaving(true)
    try {
      await addFavourite(recipeId, recipeTitle)
      setSaved(true)
    } catch {
      // silently fail
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="px-6 pb-6 pt-4 flex flex-col gap-3 border-t border-border mt-2">
      {isSignedIn ? (
        <Button
          onClick={handleSave}
          disabled={saving || saved}
          variant={saved ? "secondary" : "outline"}
          className="w-full"
        >
          {saved ? "✓ Saved to favourites" : saving ? "Saving..." : "♡ Save to Favourites"}
        </Button>
      ) : (
        <p className="text-sm text-muted-foreground text-center">
          <a href="/handler/sign-in" className="text-primary hover:underline font-medium">
            Sign in
          </a>{" "}
          to save recipes to your favourites
        </p>
      )}
    </div>
  )
}

function stepCircleClass(i: number): string {
  if (i === 0) return "bg-primary text-primary-foreground"
  if (i === 1) return "bg-foreground text-background"
  return "bg-muted text-muted-foreground"
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
            {/* Dark header */}
            <div className="p-6 pb-5 bg-foreground rounded-t-lg">
              <DialogHeader>
                <div className="flex items-center gap-2 flex-wrap mb-2">
                  {response.theme && (
                    <span className="text-xs font-medium px-2 py-0.5 rounded-full border border-background/20 text-background/70">
                      {response.theme}
                    </span>
                  )}
                  <span className="text-xs font-medium px-2 py-0.5 rounded-full border border-background/20 text-background/70">
                    {response.difficulty}
                  </span>
                  <span className="text-xs text-background/60">
                    ⏱ {response.time_minutes} min
                  </span>
                </div>
                <DialogTitle className="font-serif text-2xl font-light text-background leading-tight pr-4">
                  {recipeTitle}
                </DialogTitle>
              </DialogHeader>
              {response.description && (
                <p className="text-sm text-background/70 mt-2 leading-relaxed">
                  {response.description}
                </p>
              )}
            </div>

            {/* Tabbed content */}
            <Tabs defaultValue="steps" className="flex-col">
              <div className="px-6 pt-4">
                <TabsList className="w-full h-10">
                  <TabsTrigger value="ingredients" className="flex-1 h-full">
                    Ingredients
                  </TabsTrigger>
                  <TabsTrigger value="steps" className="flex-1 h-full">
                    Steps
                  </TabsTrigger>
                  <TabsTrigger value="subs" className="flex-1 h-full">
                    Subs
                  </TabsTrigger>
                </TabsList>
              </div>

              <TabsContent value="ingredients" className="px-6 pb-2 mt-4">
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
              </TabsContent>

              <TabsContent value="steps" className="px-6 pb-2 mt-4">
                <ol className="flex flex-col gap-4">
                  {response.steps.map((step, i) => (
                    <li key={i} className="flex gap-3 text-sm">
                      <span
                        className={`shrink-0 w-6 h-6 rounded-full text-[11px] font-bold flex items-center justify-center mt-0.5 ${stepCircleClass(i)}`}
                      >
                        {i + 1}
                      </span>
                      <span className="text-foreground/80 leading-relaxed">{step}</span>
                    </li>
                  ))}
                </ol>
              </TabsContent>

              <TabsContent value="subs" className="px-6 pb-2 mt-4">
                {response.substitutions.length === 0 ? (
                  <p className="text-sm text-muted-foreground text-center py-8">
                    No substitutions needed.
                  </p>
                ) : (
                  <ul className="flex flex-col gap-3">
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
                )}
              </TabsContent>
            </Tabs>

            {/* Footer — keyed by recipeId so save state resets on new recipe */}
            <SaveFooter key={recipeId ?? 0} recipeId={recipeId} recipeTitle={recipeTitle} isSignedIn={isSignedIn} />
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

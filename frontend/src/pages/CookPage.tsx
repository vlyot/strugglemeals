import { useState } from "react"
import {
  fetchShortlist,
  fetchRecipeDetail,
  presentRecipe,
  recordCookSilent,
  type ShortlistEntry,
  type PresentResponse,
} from "@/lib/api"
import { IngredientInput, type Filters } from "@/components/cook/IngredientInput"
import { ShortlistView } from "@/components/cook/ShortlistView"
import { RecipeModal } from "@/components/cook/RecipeModal"
import { Link } from "react-router-dom"
import { authClient } from "@/stack/client"

type Step = "input" | "shortlist"

export default function CookPage() {
  const { data: session } = authClient.useSession()

  // Ingredient state
  const [ingredients, setIngredients] = useState<string[]>([])
  const [filters, setFilters] = useState<Filters>({
    vegetarian: false,
    vegan: false,
    gluten_free: false,
  })

  // Flow state
  const [step, setStep] = useState<Step>("input")
  const [shortlistLoading, setShortlistLoading] = useState(false)
  const [shortlistResults, setShortlistResults] = useState<ShortlistEntry[]>([])

  // Modal state
  const [modalOpen, setModalOpen] = useState(false)
  const [presenting, setPresenting] = useState<number | null>(null)
  const [modalRecipeId, setModalRecipeId] = useState<number | null>(null)
  const [modalRecipeTitle, setModalRecipeTitle] = useState("")
  const [modalResponse, setModalResponse] = useState<PresentResponse | null>(null)
  const [modalLoading, setModalLoading] = useState(false)
  const [modalError, setModalError] = useState<string | null>(null)

  const addIngredient = (name: string) => {
    setIngredients((prev) => (prev.includes(name) ? prev : [...prev, name]))
  }

  const removeIngredient = (name: string) => {
    setIngredients((prev) => prev.filter((i) => i !== name))
  }

  const toggleFilter = (key: keyof Filters) => {
    setFilters((prev) => ({ ...prev, [key]: !prev[key] }))
  }

  const handlePhotoIngredients = (names: string[]) => {
    setIngredients((prev) => {
      const merged = [...prev]
      for (const n of names) {
        if (!merged.includes(n)) merged.push(n)
      }
      return merged
    })
  }

  const handleFindRecipes = async () => {
    if (ingredients.length === 0) return
    setShortlistLoading(true)
    try {
      const data = await fetchShortlist({
        ingredients,
        vegetarian: filters.vegetarian || undefined,
        vegan: filters.vegan || undefined,
        gluten_free: filters.gluten_free || undefined,
      })
      setShortlistResults(data.results)
      setStep("shortlist")
    } catch (e) {
      console.error("fetchShortlist error:", e)
      // Stay on input step; ideally show a toast — for now just log
    } finally {
      setShortlistLoading(false)
    }
  }

  const handleCook = async (entry: ShortlistEntry) => {
    setPresenting(entry.id)
    setModalRecipeId(entry.id)
    setModalRecipeTitle(entry.title)
    setModalResponse(null)
    setModalError(null)
    setModalLoading(true)
    setModalOpen(true)

    try {
      const detail = await fetchRecipeDetail(entry.id)
      const presented = await presentRecipe(detail, ingredients)
      setModalResponse(presented)

      // Fire-and-forget history record for signed-in users
      if (session?.user) {
        recordCookSilent(entry.id, entry.title)
      }
    } catch (e) {
      setModalError(
        "Couldn't prepare this recipe right now. Please try again.",
      )
      console.error("presentRecipe error:", e)
    } finally {
      setModalLoading(false)
      setPresenting(null)
    }
  }

  const handleBack = () => {
    setStep("input")
    setShortlistResults([])
  }

  const handleModalClose = () => {
    setModalOpen(false)
    setModalResponse(null)
    setModalError(null)
  }

  return (
    <div className="min-h-screen bg-background">
      {/* Nav */}
      <header className="sticky top-0 z-40 border-b border-border bg-background/95 backdrop-blur-sm">
        <div className="max-w-lg mx-auto px-4 h-14 flex items-center justify-between">
          <Link to="/" className="font-serif text-lg font-light text-foreground tracking-tight">
            StruggleMeals
          </Link>
          <div className="flex items-center gap-3 text-sm">
            {session?.user ? (
              <div className="flex items-center gap-3">
                <Link
                  to="/history"
                  className="text-muted-foreground hover:text-foreground transition-colors"
                >
                  History
                </Link>
                <Link
                  to="/favourites"
                  className="text-muted-foreground hover:text-foreground transition-colors"
                >
                  Saved
                </Link>
              </div>
            ) : (
              <Link
                to="/handler/sign-in"
                className="text-muted-foreground hover:text-foreground transition-colors"
              >
                Sign in
              </Link>
            )}
          </div>
        </div>
      </header>

      {/* Main */}
      <main className="max-w-lg mx-auto px-4 py-8">
        {step === "input" && (
          <div className="flex flex-col gap-6">
            <div>
              <h1 className="font-serif text-3xl font-light text-foreground">
                What's in your kitchen?
              </h1>
              <p className="text-muted-foreground text-sm mt-1">
                Add your ingredients and we'll find the best recipes.
              </p>
            </div>
            <IngredientInput
              ingredients={ingredients}
              filters={filters}
              loading={shortlistLoading}
              onAddIngredient={addIngredient}
              onRemoveIngredient={removeIngredient}
              onToggleFilter={toggleFilter}
              onPhotoIngredients={handlePhotoIngredients}
              onSubmit={handleFindRecipes}
            />
          </div>
        )}

        {step === "shortlist" && (
          <ShortlistView
            results={shortlistResults}
            loading={shortlistLoading}
            presenting={presenting}
            onBack={handleBack}
            onCook={handleCook}
          />
        )}
      </main>

      <RecipeModal
        open={modalOpen}
        onClose={handleModalClose}
        recipeId={modalRecipeId}
        recipeTitle={modalRecipeTitle}
        response={modalResponse}
        loading={modalLoading}
        error={modalError}
      />
    </div>
  )
}

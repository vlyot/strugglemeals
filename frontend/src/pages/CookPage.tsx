import { Fragment, useState, useEffect, useRef } from "react"
import {
  streamShortlist,
  fetchRecipeDetail,
  presentRecipe,
  recordCookSilent,
  type ShortlistEntry,
  type PresentResponse,
} from "@/lib/api"
import {
  IngredientInput,
  type Filters,
  type IngredientWithQty,
  type Quantity,
} from "@/components/cook/IngredientInput"
import { MethodSelector } from "@/components/cook/MethodSelector"
import { ShortlistView } from "@/components/cook/ShortlistView"
import { RecipeModal } from "@/components/cook/RecipeModal"
import { AuthNudgeBanner } from "@/components/sections/results"
import { Link } from "react-router-dom"
import { authClient } from "@/stack/client"

type Step = "method" | "input" | "shortlist"

export default function CookPage() {
  const { data: session } = authClient.useSession()

  // Ingredient state
  const [ingredients, setIngredients] = useState<IngredientWithQty[]>([])
  const [filters, setFilters] = useState<Filters>({
    vegetarian: false,
    vegan: false,
    gluten_free: false,
  })
  const [cuisine, setCuisine] = useState("")

  // Flow state
  const [step, setStep] = useState<Step>("method")
  const [defaultTab, setDefaultTab] = useState<"text" | "photo">("text")
  const [shortlistLoading, setShortlistLoading] = useState(false)
  const [shortlistProgress, setShortlistProgress] = useState(0)
  const [shortlistResults, setShortlistResults] = useState<ShortlistEntry[]>([])
  const [inputError, setInputError] = useState<string | null>(null)
  const progressTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Smoothly tick progress toward a target value, never exceeding it
  const tickProgressTo = (target: number) => {
    if (progressTimerRef.current) clearInterval(progressTimerRef.current)
    progressTimerRef.current = setInterval(() => {
      setShortlistProgress((prev) => {
        if (prev >= target) {
          clearInterval(progressTimerRef.current!)
          return prev
        }
        return prev + Math.max(1, Math.floor((target - prev) * 0.12))
      })
    }, 80)
  }

  useEffect(() => () => { if (progressTimerRef.current) clearInterval(progressTimerRef.current) }, [])

  // Modal state
  const [modalOpen, setModalOpen] = useState(false)
  const [presenting, setPresenting] = useState<number | null>(null)
  const [modalRecipeId, setModalRecipeId] = useState<number | null>(null)
  const [modalRecipeTitle, setModalRecipeTitle] = useState("")
  const [modalResponse, setModalResponse] = useState<PresentResponse | null>(null)
  const [modalLoading, setModalLoading] = useState(false)
  const [modalError, setModalError] = useState<string | null>(null)

  const handleMethodSelect = (method: "text" | "photo") => {
    setDefaultTab(method)
    setStep("input")
  }

  const addIngredient = (name: string) => {
    setIngredients((prev) =>
      prev.some((i) => i.name === name) ? prev : [...prev, { name, qty: "1 qty" as Quantity }]
    )
  }

  const removeIngredient = (name: string) => {
    setIngredients((prev) => prev.filter((i) => i.name !== name))
  }

  const updateIngredientQty = (name: string, qty: Quantity) => {
    setIngredients((prev) => prev.map((i) => (i.name === name ? { ...i, qty } : i)))
  }

  const toggleFilter = (key: keyof Filters) => {
    setFilters((prev) => ({ ...prev, [key]: !prev[key] }))
  }

  const handlePhotoIngredients = (names: string[]) => {
    setIngredients((prev) => {
      const merged = [...prev]
      for (const n of names) {
        if (!merged.some((i) => i.name === n)) {
          merged.push({ name: n, qty: "1 qty" as Quantity })
        }
      }
      return merged
    })
  }

  const handleFindRecipes = async () => {
    if (ingredients.length === 0) return
    setInputError(null)
    setShortlistLoading(true)
    setShortlistProgress(5)
    setShortlistResults([])
    setStep("shortlist")
    tickProgressTo(65)
    try {
      let first = true
      for await (const chunk of streamShortlist({
        ingredients: ingredients.map((i) => i.name),
        ingredients_with_qty: ingredients.map((i) => ({ name: i.name, qty: i.qty })),
        vegetarian: filters.vegetarian || undefined,
        vegan: filters.vegan || undefined,
        gluten_free: filters.gluten_free || undefined,
        cuisine: cuisine || undefined,
      })) {
        setShortlistResults(chunk.results)
        if (first) {
          // Scores arrived — snap to 70, then crawl toward 90 while Groq runs
          if (progressTimerRef.current) clearInterval(progressTimerRef.current)
          setShortlistProgress(70)
          tickProgressTo(90)
          setShortlistLoading(false)
          first = false
        } else {
          // Themes arrived — complete
          if (progressTimerRef.current) clearInterval(progressTimerRef.current)
          setShortlistProgress(100)
        }
      }
    } catch (e) {
      console.error("streamShortlist error:", e)
      setInputError("Something went wrong finding recipes. Please try again.")
      setStep("input")
    } finally {
      if (progressTimerRef.current) clearInterval(progressTimerRef.current)
      setShortlistProgress(100)
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
      const presented = await presentRecipe(detail, ingredients.map((i) => i.name))
      setModalResponse(presented)

      if (session?.user) {
        recordCookSilent(entry.id, entry.title)
      }
    } catch (e) {
      setModalError("Couldn't prepare this recipe right now. Please try again.")
      console.error("presentRecipe error:", e)
    } finally {
      setModalLoading(false)
      setPresenting(null)
    }
  }

  const handleBack = () => {
    setStep("input")
    setShortlistResults([])
    setShortlistProgress(0)
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
        {step === "method" && (
          <div className="flex flex-col gap-6">
            <div>
              <p className="text-xs font-medium tracking-widest uppercase text-muted-foreground mb-1">
                Step 1 of 2
              </p>
              <h1 className="font-serif text-3xl font-light text-foreground">
                How do you want to add ingredients?
              </h1>
            </div>
            <MethodSelector onSelect={handleMethodSelect} />
          </div>
        )}

        {step === "input" && (
          <div className="flex flex-col gap-6">
            {/* Step indicator */}
            <div className="flex items-center gap-2">
              {(["01", "02", "03"] as const).map((label, i) => (
                <Fragment key={label}>
                  {i > 0 && <div className="flex-1 h-px bg-border" />}
                  <div
                    className={[
                      "w-7 h-7 rounded-full flex items-center justify-center text-xs font-bold shrink-0",
                      i === 1
                        ? "bg-primary text-primary-foreground"
                        : "border border-border text-muted-foreground",
                    ].join(" ")}
                  >
                    {label}
                  </div>
                </Fragment>
              ))}
            </div>

            <IngredientInput
              ingredients={ingredients}
              filters={filters}
              loading={shortlistLoading}
              cuisine={cuisine}
              defaultTab={defaultTab}
              onAddIngredient={addIngredient}
              onRemoveIngredient={removeIngredient}
              onUpdateQty={updateIngredientQty}
              onToggleFilter={toggleFilter}
              onCuisineChange={setCuisine}
              onPhotoIngredients={handlePhotoIngredients}
              onSubmit={handleFindRecipes}
            />

            {inputError && (
              <p className="text-sm text-destructive text-center">{inputError}</p>
            )}
          </div>
        )}

        {step === "shortlist" && (
          <div className="flex flex-col gap-4">
            <ShortlistView
              results={shortlistResults}
              loading={shortlistLoading}
              progress={shortlistProgress}
              presenting={presenting}
              userIngredients={ingredients.map((i) => i.name)}
              onBack={handleBack}
              onCook={handleCook}
            />
            <div className="flex justify-center pt-2">
              <AuthNudgeBanner />
            </div>
          </div>
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

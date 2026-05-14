import { useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"

export interface Filters {
  vegetarian: boolean
  vegan: boolean
  gluten_free: boolean
}

interface Props {
  ingredients: string[]
  filters: Filters
  loading: boolean
  onAddIngredient: (name: string) => void
  onRemoveIngredient: (name: string) => void
  onToggleFilter: (key: keyof Filters) => void
  onPhotoIngredients: (names: string[]) => void
  onSubmit: () => void
}

type TabId = "text" | "photo"

const FILTER_LABELS: { key: keyof Filters; label: string }[] = [
  { key: "vegetarian", label: "Vegetarian" },
  { key: "vegan", label: "Vegan" },
  { key: "gluten_free", label: "Gluten-Free" },
]

export function IngredientInput({
  ingredients,
  filters,
  loading,
  onAddIngredient,
  onRemoveIngredient,
  onToggleFilter,
  onPhotoIngredients,
  onSubmit,
}: Props) {
  const [tab, setTab] = useState<TabId>("text")
  const [inputVal, setInputVal] = useState("")
  const [photoLoading, setPhotoLoading] = useState(false)
  const [photoError, setPhotoError] = useState<string | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)

  const commitInput = () => {
    const name = inputVal.trim().toLowerCase()
    if (name && !ingredients.includes(name)) {
      onAddIngredient(name)
    }
    setInputVal("")
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault()
      commitInput()
    }
  }

  const handlePhotoUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    setPhotoError(null)
    setPhotoLoading(true)

    try {
      const buffer = await file.arrayBuffer()
      const bytes = new Uint8Array(buffer)
      let binary = ""
      for (let i = 0; i < bytes.byteLength; i++) {
        binary += String.fromCharCode(bytes[i])
      }
      const image_base64 = btoa(binary)
      const mime_type = file.type || "image/jpeg"

      const apiUrl = import.meta.env.VITE_API_URL ?? "http://localhost:8080"
      const res = await fetch(`${apiUrl}/ai/identify-ingredients`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ image_base64, mime_type }),
        credentials: "include",
      })
      const data = await res.json()
      if (data.ingredients && data.ingredients.length > 0) {
        onPhotoIngredients(data.ingredients)
        setTab("text")
      } else {
        setPhotoError("Couldn't identify ingredients. Try a clearer photo.")
      }
    } catch {
      setPhotoError("Upload failed. Please try again.")
    } finally {
      setPhotoLoading(false)
      if (fileRef.current) fileRef.current.value = ""
    }
  }

  return (
    <div className="flex flex-col gap-6">
      {/* Method tabs */}
      <div className="flex gap-1 border-b border-border">
        {(["text", "photo"] as TabId[]).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={[
              "px-4 py-2.5 text-sm font-medium capitalize transition-colors",
              tab === t
                ? "border-b-2 border-primary text-foreground -mb-px"
                : "text-muted-foreground hover:text-foreground",
            ].join(" ")}
          >
            {t === "text" ? "✏ Text" : "📷 Photo"}
          </button>
        ))}
        <button
          type="button"
          disabled
          className="px-4 py-2.5 text-sm font-medium text-muted-foreground/40 cursor-not-allowed"
          title="Coming soon"
        >
          🎙 Voice
        </button>
      </div>

      {/* Input area */}
      {tab === "text" ? (
        <div className="flex gap-2">
          <Input
            placeholder="What else do you have?"
            value={inputVal}
            onChange={(e) => setInputVal(e.target.value)}
            onKeyDown={handleKeyDown}
            className="flex-1"
            autoFocus
          />
          <Button
            onClick={commitInput}
            disabled={!inputVal.trim()}
            className="bg-primary text-primary-foreground hover:bg-primary/90 px-5"
          >
            Add
          </Button>
        </div>
      ) : (
        <div className="flex flex-col gap-3 items-start">
          <p className="text-sm text-muted-foreground">
            Upload a photo of your ingredients — we'll identify them automatically.
          </p>
          <Button
            type="button"
            variant="outline"
            onClick={() => fileRef.current?.click()}
            disabled={photoLoading}
          >
            {photoLoading ? "Identifying..." : "Choose photo"}
          </Button>
          <input
            ref={fileRef}
            type="file"
            accept="image/*"
            className="hidden"
            onChange={handlePhotoUpload}
          />
          {photoError && (
            <p className="text-sm text-destructive">{photoError}</p>
          )}
        </div>
      )}

      {/* Ingredient chips */}
      {ingredients.length > 0 && (
        <div className="rounded-xl border border-border p-4 flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium tracking-widest uppercase text-muted-foreground">
              Your Ingredients
            </span>
            <span className="text-xs font-medium text-primary">
              {ingredients.length} added
            </span>
          </div>
          <div className="flex flex-wrap gap-2">
            {ingredients.map((name) => (
              <span
                key={name}
                className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full border border-border bg-secondary text-sm text-foreground"
              >
                {name}
                <button
                  type="button"
                  onClick={() => onRemoveIngredient(name)}
                  className="text-muted-foreground hover:text-foreground transition-colors leading-none"
                  aria-label={`Remove ${name}`}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Filters */}
      <div className="flex flex-col gap-2">
        <span className="text-xs font-medium tracking-widest uppercase text-muted-foreground">
          Filters
        </span>
        <div className="flex flex-wrap gap-2">
          {FILTER_LABELS.map(({ key, label }) => (
            <button
              key={key}
              type="button"
              onClick={() => onToggleFilter(key)}
              className={[
                "px-4 py-1.5 rounded-full text-sm border transition-colors",
                filters[key]
                  ? "bg-primary text-primary-foreground border-primary"
                  : "bg-background text-foreground border-border hover:border-foreground/40",
              ].join(" ")}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* CTA */}
      <Button
        onClick={onSubmit}
        disabled={ingredients.length === 0 || loading}
        className="w-full py-4 text-base bg-primary text-primary-foreground hover:bg-primary/90 h-auto rounded-xl"
      >
        {loading ? "Finding recipes..." : "Find my recipes →"}
      </Button>
    </div>
  )
}

// Compact badge used in ShortlistView
export function ThemeBadge({ theme }: { theme: string | null }) {
  if (!theme) return null
  const styles: Record<string, string> = {
    Light: "bg-green-50 text-green-700 border-green-200",
    Filling: "bg-amber-50 text-amber-700 border-amber-200",
    Quick: "bg-blue-50 text-blue-700 border-blue-200",
  }
  return (
    <Badge
      variant="outline"
      className={`text-xs font-medium ${styles[theme] ?? "bg-secondary text-foreground"}`}
    >
      {theme}
    </Badge>
  )
}

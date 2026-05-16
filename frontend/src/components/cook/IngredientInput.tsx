import { useRef, useState, useEffect, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"

export type Quantity = "1 qty" | "a little" | "plenty"

export interface IngredientWithQty {
  name: string
  qty: Quantity
}

export interface Filters {
  vegetarian: boolean
  vegan: boolean
  gluten_free: boolean
}

interface Props {
  ingredients: IngredientWithQty[]
  filters: Filters
  loading: boolean
  cuisine: string
  defaultTab?: "text" | "photo"
  onAddIngredient: (name: string) => void
  onRemoveIngredient: (name: string) => void
  onUpdateQty: (name: string, qty: Quantity) => void
  onToggleFilter: (key: keyof Filters) => void
  onCuisineChange: (val: string) => void
  onPhotoIngredients: (
    names: string[],
    detected: Array<{ name: string; confidence: number }>,
    suggestions: string[],
    legend: { high: string; mid: string; low: string } | null,
  ) => void
  onSubmit: () => void
  // Photo scan enrichment
  suggestions?: string[]
  detectedWithConf?: Array<{ name: string; confidence: number }>
  confidenceLegend?: { high: string; mid: string; low: string } | null
  hasDonePhotoScan?: boolean
  photoScanCount?: number
  onAcceptSuggestion?: (name: string) => void
  onAcceptAllSuggestions?: () => void
  onDismissSuggestion?: (name: string) => void
}

type TabId = "text" | "photo"

const FILTER_LABELS: { key: keyof Filters; label: string }[] = [
  { key: "vegetarian", label: "Vegetarian" },
  { key: "vegan", label: "Vegan" },
  { key: "gluten_free", label: "Gluten-Free" },
]

const CUISINE_OPTIONS = [
  "Any",
  "Italian",
  "Asian",
  "Mexican",
  "Indian",
  "Mediterranean",
  "American",
  "French",
]

const QTY_OPTIONS: Quantity[] = ["1 qty", "a little", "plenty"]

// Resize an image file to maxPx on its longest side, compress as JPEG,
// and return the base64 string (no data-URI prefix).
function resizeToBase64(file: File, maxPx: number, quality: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file)
    const img = new Image()
    img.onload = () => {
      URL.revokeObjectURL(url)
      const { width, height } = img
      const scale = Math.min(1, maxPx / Math.max(width, height))
      const w = Math.round(width * scale)
      const h = Math.round(height * scale)
      const canvas = document.createElement("canvas")
      canvas.width = w
      canvas.height = h
      canvas.getContext("2d")!.drawImage(img, 0, 0, w, h)
      const dataUrl = canvas.toDataURL("image/jpeg", quality)
      resolve(dataUrl.split(",")[1])
    }
    img.onerror = () => { URL.revokeObjectURL(url); reject(new Error("Failed to load image")) }
    img.src = url
  })
}

// Maps a 0–10 confidence score to a Tailwind text-colour class.
function confidenceColor(score: number): string {
  if (score >= 7.5) return "text-green-600"
  if (score >= 4.5) return "text-amber-500"
  return "text-rose-400"
}

const NUDGE_PAIRS: [string[], string][] = [
  [
    ["chicken", "beef", "pork", "lamb", "fish", "salmon", "tuna"],
    "Got any garlic, herbs, or a marinade sauce hiding somewhere?",
  ],
  [["egg", "eggs"], "Any cheese, butter, or cream in the door?"],
  [
    ["pasta", "noodles", "rice", "spaghetti"],
    "Don't forget sauces or stock at the back.",
  ],
  [
    ["tomato", "carrot", "onion", "pepper", "capsicum"],
    "Any leafy greens or protein hiding at the back?",
  ],
]

// Returns contextual nudge copy based on detected ingredient names.
function getNudgeText(ingredientNames: string[]): string {
  for (const [triggers, msg] of NUDGE_PAIRS) {
    if (triggers.some((t) => ingredientNames.includes(t))) return msg
  }
  return "Don't forget items at the back of the fridge, in the door, or in your pantry."
}

export function IngredientInput({
  ingredients,
  filters,
  loading,
  cuisine,
  defaultTab = "text",
  onAddIngredient,
  onRemoveIngredient,
  onUpdateQty,
  onToggleFilter,
  onCuisineChange,
  onPhotoIngredients,
  onSubmit,
  suggestions,
  detectedWithConf,
  confidenceLegend,
  hasDonePhotoScan,
  photoScanCount,
  onAcceptSuggestion,
  onAcceptAllSuggestions,
  onDismissSuggestion,
}: Props) {
  const [tab, setTab] = useState<TabId>(defaultTab)
  const [inputVal, setInputVal] = useState("")
  const [photoLoading, setPhotoLoading] = useState(false)
  const [photoError, setPhotoError] = useState<string | null>(null)
  const [nudgeDismissed, setNudgeDismissed] = useState(false)
  const [duplicateFlash, setDuplicateFlash] = useState<string | null>(null)
  const [dragOver, setDragOver] = useState(false)
  const fileRef = useRef<HTMLInputElement>(null)
  const dropZoneRef = useRef<HTMLDivElement>(null)

  const commitInput = () => {
    const name = inputVal.trim().toLowerCase()
    if (!name) return
    if (ingredients.some((i) => i.name === name)) {
      setDuplicateFlash(name)
      setTimeout(() => setDuplicateFlash(null), 1500)
      setInputVal("")
      return
    }
    onAddIngredient(name)
    setNudgeDismissed(true)
    setInputVal("")
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault()
      commitInput()
    }
  }

  const processImageFile = useCallback(async (file: File) => {
    setPhotoError(null)
    setPhotoLoading(true)
    try {
      const image_base64 = await resizeToBase64(file, 1920, 0.85)
      const mime_type = "image/jpeg"

      const apiUrl = import.meta.env.VITE_API_URL ?? "http://localhost:8080"
      const res = await fetch(`${apiUrl}/ai/identify-ingredients`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ image_base64, mime_type }),
        credentials: "include",
      })
      const data = await res.json()
      if (data.ingredients && data.ingredients.length > 0) {
        onPhotoIngredients(
          data.ingredients,
          data.detected ?? [],
          data.suggestions ?? [],
          data.confidence_legend ?? null,
        )
        setNudgeDismissed(false)
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
  }, [onPhotoIngredients])

  const handlePhotoUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) processImageFile(file)
  }

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(false)
    const file = Array.from(e.dataTransfer.files).find((f) =>
      f.type.startsWith("image/"),
    )
    if (file) processImageFile(file)
    else setPhotoError("Please drop an image file.")
  }

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(true)
  }

  const handleDragLeave = (e: React.DragEvent) => {
    if (!dropZoneRef.current?.contains(e.relatedTarget as Node)) {
      setDragOver(false)
    }
  }

  // Ctrl+V / Cmd+V clipboard paste — only active while on the photo tab
  useEffect(() => {
    if (tab !== "photo") return
    const handler = (e: ClipboardEvent) => {
      const item = Array.from(e.clipboardData?.items ?? []).find((i) =>
        i.type.startsWith("image/"),
      )
      if (!item) return
      const file = item.getAsFile()
      if (file) processImageFile(file)
    }
    window.addEventListener("paste", handler)
    return () => window.removeEventListener("paste", handler)
  }, [tab, processImageFile])

  const isReady = ingredients.length >= 3

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
              "px-4 py-2.5 text-sm font-medium transition-colors",
              tab === t
                ? "border-b-2 border-primary text-foreground -mb-px"
                : "text-muted-foreground hover:text-foreground",
            ].join(" ")}
          >
            {t === "text"
              ? "✏ Text"
              : photoScanCount && photoScanCount > 0
                ? `📷 Photo · ${photoScanCount} found`
                : "📷 Photo"}
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
        <div className="flex flex-col gap-2">
          {/* Contextual nudge after photo scan */}
          {hasDonePhotoScan && !nudgeDismissed && (
            <div className="flex items-start justify-between gap-2 rounded-lg bg-muted/60 px-3 py-2 text-xs text-muted-foreground">
              <span>{getNudgeText(ingredients.map((i) => i.name))}</span>
              <button
                type="button"
                onClick={() => setNudgeDismissed(true)}
                className="shrink-0 leading-none hover:text-foreground transition-colors"
                aria-label="Dismiss hint"
              >
                ×
              </button>
            </div>
          )}
          <div className="flex gap-2">
            <Input
              placeholder="Type an ingredient..."
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
          {/* Duplicate entry feedback */}
          {duplicateFlash && (
            <p className="text-xs text-muted-foreground italic">
              "{duplicateFlash}" is already in your list.
            </p>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-3 w-full">
          {/* Drop zone */}
          <div
            ref={dropZoneRef}
            onDrop={handleDrop}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onClick={() => !photoLoading && fileRef.current?.click()}
            className={[
              "relative flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed px-6 py-10 transition-colors cursor-pointer select-none",
              dragOver
                ? "border-primary bg-primary/5"
                : "border-border bg-muted/30 hover:border-primary/50 hover:bg-muted/50",
              photoLoading ? "pointer-events-none opacity-60" : "",
            ].join(" ")}
          >
            {photoLoading ? (
              <>
                <span className="text-3xl animate-pulse">🔍</span>
                <p className="text-sm font-medium text-foreground">Identifying ingredients…</p>
              </>
            ) : (
              <>
                <span className="text-3xl">{dragOver ? "📂" : "📷"}</span>
                <div className="text-center">
                  <p className="text-sm font-medium text-foreground">
                    Drop a photo here, or{" "}
                    <span className="text-primary underline underline-offset-2">choose file</span>
                  </p>
                  <p className="text-xs text-muted-foreground mt-1">
                    Also works with <kbd className="px-1 py-0.5 rounded bg-muted border border-border text-[10px] font-mono">Ctrl+V</kbd> to paste from clipboard
                  </p>
                </div>
              </>
            )}
          </div>
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
              Added · {ingredients.length}
            </span>
            {isReady && (
              <span className="flex items-center gap-1.5 text-xs font-medium text-green-700">
                <span className="w-1.5 h-1.5 rounded-full bg-green-500 inline-block" />
                Ready
              </span>
            )}
          </div>
          <div className="flex flex-wrap gap-2">
            {ingredients.map(({ name, qty }) => (
              <span
                key={name}
                className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full border border-border bg-secondary text-sm text-foreground"
              >
                <span className="font-medium">{name}</span>
                {(() => {
                  const conf = detectedWithConf?.find((d) => d.name === name)
                  if (!conf) return null
                  const legendLabel = confidenceLegend
                    ? conf.confidence >= 7.5
                      ? confidenceLegend.high
                      : conf.confidence >= 4.5
                        ? confidenceLegend.mid
                        : confidenceLegend.low
                    : null
                  return (
                    <span
                      className={`text-[9px] font-bold ml-0.5 tabular-nums ${confidenceColor(conf.confidence)}`}
                      title={legendLabel ? `${conf.confidence.toFixed(1)} — ${legendLabel}` : `Confidence: ${conf.confidence.toFixed(1)}/10`}
                    >
                      {conf.confidence.toFixed(1)}
                    </span>
                  )
                })()}
                <span className="flex items-center gap-0.5 ml-1">
                  {QTY_OPTIONS.map((q) => (
                    <button
                      key={q}
                      type="button"
                      onClick={() => onUpdateQty(name, q)}
                      className={[
                        "px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
                        qty === q
                          ? "bg-primary/10 text-primary"
                          : "text-muted-foreground hover:text-foreground",
                      ].join(" ")}
                      aria-pressed={qty === q}
                      aria-label={`Set ${name} to ${q}`}
                    >
                      {q}
                    </button>
                  ))}
                </span>
                <button
                  type="button"
                  onClick={() => onRemoveIngredient(name)}
                  className="ml-1 text-muted-foreground hover:text-foreground transition-colors leading-none"
                  aria-label={`Remove ${name}`}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
          <p className="text-xs text-muted-foreground italic">
            Tap any chip to adjust quantity. 'One serving' means whatever that is to you.
          </p>
        </div>
      )}

      {/* Confidence legend key */}
      {hasDonePhotoScan && confidenceLegend && (
        <p className="text-xs text-muted-foreground">
          Confidence:{" "}
          <span className="text-green-600 font-medium">7.5–10</span>{" "}
          {confidenceLegend.high} ·{" "}
          <span className="text-amber-500 font-medium">4.5–7.4</span>{" "}
          {confidenceLegend.mid} ·{" "}
          <span className="text-rose-400 font-medium">0–4.4</span>{" "}
          {confidenceLegend.low}
        </p>
      )}

      {/* "You probably have" suggestions panel */}
      {suggestions && suggestions.length > 0 && (
        <div className="rounded-xl border border-dashed border-border p-4 flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium tracking-widest uppercase text-muted-foreground">
              You probably have · {suggestions.length}
            </span>
            <button
              type="button"
              onClick={onAcceptAllSuggestions}
              className="text-xs text-primary hover:text-primary/80 font-medium transition-colors"
            >
              Add all
            </button>
          </div>
          <div className="flex flex-wrap gap-2">
            {suggestions.map((name) => (
              <span
                key={name}
                className="inline-flex items-center gap-1 px-2.5 py-1 rounded-full border border-dashed border-border bg-background text-sm text-muted-foreground"
              >
                {name}
                <button
                  type="button"
                  onClick={() => onAcceptSuggestion?.(name)}
                  className="ml-1 text-primary hover:text-primary/80 font-medium leading-none transition-colors"
                  aria-label={`Add ${name}`}
                >
                  +
                </button>
                <button
                  type="button"
                  onClick={() => onDismissSuggestion?.(name)}
                  className="text-muted-foreground hover:text-foreground leading-none transition-colors"
                  aria-label={`Dismiss ${name}`}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
          <p className="text-xs text-muted-foreground italic">
            Tap + to confirm you have it.
          </p>
        </div>
      )}

      {/* Dietary filters */}
      <div className="flex flex-col gap-2">
        <span className="text-xs font-medium tracking-widest uppercase text-muted-foreground">
          Dietary Filters
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

      {/* Cuisine preference */}
      <div className="flex flex-col gap-2">
        <span className="text-xs font-medium tracking-widest uppercase text-muted-foreground">
          Cuisine Preference
        </span>
        <div className="relative">
          <select
            value={cuisine}
            onChange={(e) => onCuisineChange(e.target.value)}
            className="w-full border border-border rounded-lg px-3 py-2 text-sm bg-background text-foreground appearance-none cursor-pointer focus:outline-none focus:ring-1 focus:ring-primary"
          >
            {CUISINE_OPTIONS.map((opt) => (
              <option key={opt} value={opt === "Any" ? "" : opt}>
                {opt}
              </option>
            ))}
          </select>
          <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground text-xs">
            ▼
          </span>
        </div>
      </div>

      {/* CTA */}
      <div className="flex flex-col gap-1">
        <Button
          onClick={onSubmit}
          disabled={ingredients.length === 0 || loading}
          className="w-full py-4 text-base bg-primary text-primary-foreground hover:bg-primary/90 h-auto rounded-xl"
        >
          {loading ? "Finding recipes..." : "Find Recipes →"}
        </Button>
        <p className="text-center text-xs text-muted-foreground">
          Searching 1.94M recipes
        </p>
      </div>
    </div>
  )
}

// Compact badge used across cook flow, history, favourites
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

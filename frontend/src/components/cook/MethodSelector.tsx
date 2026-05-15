import { Camera, CheckSquare, Pencil } from "lucide-react"
import { Badge } from "@/components/ui/badge"

interface Props {
  onSelect: (method: "text" | "photo") => void
}

const OPTIONS = [
  {
    id: "text" as const,
    icon: Pencil,
    label: "Type it in",
    description: "Enter each ingredient one at a time.",
    recommended: false,
    disabled: false,
  },
  {
    id: "photo" as const,
    icon: Camera,
    label: "Snap a photo",
    description: "Photo your fridge or counter. AI identifies your ingredients.",
    recommended: true,
    disabled: false,
  },
  {
    id: null,
    icon: CheckSquare,
    label: "Pick from a list",
    description: "Browse common ingredients and tap to select.",
    recommended: false,
    disabled: true,
  },
]

export function MethodSelector({ onSelect }: Props) {
  return (
    <div className="flex flex-col gap-3">
      {OPTIONS.map(({ id, icon: Icon, label, description, recommended, disabled }) => (
        <button
          key={label}
          type="button"
          disabled={disabled}
          onClick={() => id && onSelect(id)}
          className={[
            "rounded-xl border p-5 flex items-start gap-4 text-left transition-colors w-full",
            disabled
              ? "border-border opacity-50 cursor-not-allowed"
              : "border-border hover:border-primary/50 hover:bg-primary/5 cursor-pointer",
            recommended ? "border-primary/30 bg-primary/5" : "",
          ].join(" ")}
        >
          <span
            className={[
              "mt-0.5 shrink-0 w-9 h-9 rounded-lg flex items-center justify-center",
              recommended
                ? "bg-primary text-primary-foreground"
                : "bg-secondary text-muted-foreground",
            ].join(" ")}
          >
            <Icon size={18} />
          </span>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="font-medium text-foreground text-sm">{label}</span>
              {recommended && (
                <Badge
                  variant="outline"
                  className="text-[10px] px-1.5 py-0 font-semibold border-accent/40 bg-accent/10 text-amber-700"
                >
                  Recommended
                </Badge>
              )}
            </div>
            <p className="text-xs text-muted-foreground mt-0.5">{description}</p>
          </div>
        </button>
      ))}
    </div>
  )
}

import { useEffect } from "react"

export function usePageTitle(title: string) {
  useEffect(() => {
    document.title = title ? `${title} — StruggleMeals` : "StruggleMeals"
    return () => {
      document.title = "StruggleMeals"
    }
  }, [title])
}

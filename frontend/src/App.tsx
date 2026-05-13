import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'

const API_URL = import.meta.env.VITE_API_URL ?? ''

function App() {
  return (
    <main className="min-h-screen bg-background flex items-center justify-center p-4">
      <Card className="w-full max-w-md shadow-lg border-border">
        <CardHeader className="text-center">
          <CardTitle className="text-3xl font-semibold text-primary tracking-tight">
            StruggleMeals
          </CardTitle>
          <p className="text-muted-foreground text-sm mt-1">
            Real recipes from whatever you have.
          </p>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 items-center">
          <p className="text-center text-foreground/80 text-sm">
            Coming soon — Phase 1 foundation deployed.
          </p>
          {API_URL && (
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                fetch(`${API_URL}/health`)
                  .then((r) => r.json())
                  .then((d) => alert(JSON.stringify(d)))
                  .catch(() => alert('Backend unreachable'))
              }
            >
              Ping backend
            </Button>
          )}
        </CardContent>
      </Card>
    </main>
  )
}

export default App

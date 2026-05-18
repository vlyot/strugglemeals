import { usePageTitle } from "@/hooks/usePageTitle"
import { Header } from "@/components/header"
import { Footer } from "@/components/footer"
import { HeroSection } from "@/components/sections/hero"
import { VisionSection } from "@/components/sections/vision"
import { PhilosophySection } from "@/components/sections/philosophy"
import { HowItWorksSection } from "@/components/sections/how-it-works"
import { ExamplesSection } from "@/components/sections/examples"

export default function App() {
  usePageTitle("")
  return (
    <main className="min-h-screen bg-background">
      <Header />
      <HeroSection />
      <VisionSection />
      <PhilosophySection />
      <HowItWorksSection />
      <ExamplesSection />
<Footer />
    </main>
  )
}

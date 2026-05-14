import { useEffect, useRef, useState } from "react"

const steps = [
  {
    step: "Scan",
    timing: "Step 1",
    title: "Open Your Fridge",
    description: "Take a photo of your ingredients or type what you have. Our AI identifies everything automatically — even the half-empty condiment jars.",
    details: ["Photo recognition", "Text entry", "Pantry staples auto-added", "Dietary preferences saved"],
  },
  {
    step: "Match",
    timing: "Step 2",
    title: "We Find Your Recipes",
    description: "Our engine searches hundreds of recipes and ranks them by how well they match your exact ingredients. No forced substitutions, no missing items.",
    details: ["Ingredient match score", "Cuisine type filter", "Cook time filter", "Skill level filter"],
  },
  {
    step: "Choose",
    timing: "Step 3",
    title: "Pick What Sounds Good",
    description: "Browse suggestions with photos, ratings, and a clear ingredient match breakdown. Filter by time, cuisine, or whatever you're craving tonight.",
    details: ["Recipe photos", "Match percentage", "Missing ingredients listed", "Community ratings"],
  },
  {
    step: "Cook",
    timing: "Step 4",
    title: "Step-by-Step Instructions",
    description: "Follow along with clear, structured instructions. Built-in timers keep you on track. Rate when you're done to sharpen future recommendations.",
    details: ["Structured steps", "Built-in timers", "Serving size scaler", "Save favourites"],
  },
]

export function HowItWorksSection() {
  const [isVisible, setIsVisible] = useState(false)
  const [activeStep, setActiveStep] = useState(0)
  const sectionRef = useRef<HTMLElement>(null)

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting) setIsVisible(true) },
      { threshold: 0.1 }
    )
    if (sectionRef.current) observer.observe(sectionRef.current)
    return () => observer.disconnect()
  }, [])

  return (
    <section ref={sectionRef} id="how-it-works" className="relative py-20 md:py-36 lg:py-48 overflow-hidden">
      <div className="max-w-[1800px] mx-auto px-6 md:px-12 lg:px-20">
        <div className="grid lg:grid-cols-12 lg:gap-20">
          {/* Label */}
          <div className="lg:col-span-2">
            <div
              className="flex items-center gap-4 lg:sticky lg:top-32"
              style={{
                opacity: isVisible ? 1 : 0,
                transform: isVisible ? "translateX(0)" : "translateX(-20px)",
                transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1)",
              }}
            >
              <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">(03)</span>
              <div className="w-8 h-px bg-border" />
              <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">How It Works</span>
            </div>
          </div>

          {/* Content */}
          <div className="lg:col-span-10">
            <div className="mb-20 md:mb-32 max-w-3xl">
              <h2
                className="font-serif text-3xl sm:text-4xl md:text-5xl lg:text-6xl font-light leading-[1.1] tracking-[-0.01em] text-foreground mb-8 text-pretty"
                style={{
                  opacity: isVisible ? 1 : 0,
                  transform: isVisible ? "translateY(0)" : "translateY(40px)",
                  transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.1s",
                }}
              >
                From empty fridge to dinner in four steps
              </h2>
              <p
                className="text-lg text-muted-foreground leading-relaxed"
                style={{
                  opacity: isVisible ? 1 : 0,
                  transform: isVisible ? "translateY(0)" : "translateY(30px)",
                  transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.2s",
                }}
              >
                No planning required. No grocery run. Just open StruggleMeals and start.
              </p>
            </div>

            {/* Accordion steps */}
            <div className="space-y-0">
              {steps.map((step, index) => (
                <div
                  key={step.step}
                  className="group border-t border-border"
                  style={{
                    opacity: isVisible ? 1 : 0,
                    transform: isVisible ? "translateY(0)" : "translateY(30px)",
                    transition: `all 0.8s cubic-bezier(0.16, 1, 0.3, 1) ${0.3 + index * 0.1}s`,
                  }}
                >
                  <button
                    type="button"
                    onClick={() => setActiveStep(activeStep === index ? -1 : index)}
                    className="w-full py-8 md:py-12 text-left"
                    aria-expanded={activeStep === index}
                  >
                    {/* Mobile: step + toggle on one row, title below. Desktop: 12-col grid */}
                    <div className="flex flex-col gap-2 md:grid md:grid-cols-12 md:gap-8 md:items-start">
                      <div className="flex items-center justify-between md:col-span-3">
                        <div className="flex items-center gap-3">
                          <span className="font-mono text-xs tracking-wider text-muted-foreground">0{index + 1}</span>
                          <span className="font-sans text-xl md:text-2xl text-foreground group-hover:text-accent transition-colors duration-300">
                            {step.step}
                          </span>
                        </div>
                        {/* Toggle visible on mobile inline, hidden in its own col on md+ */}
                        <div className="md:hidden w-6 h-6 flex items-center justify-center">
                          <span
                            className="text-lg text-muted-foreground"
                            style={{ transform: activeStep === index ? "rotate(45deg)" : "rotate(0)", transition: "transform 0.3s" }}
                          >
                            +
                          </span>
                        </div>
                      </div>
                      <div className="md:col-span-6">
                        <h3 className="text-base md:text-xl text-foreground/80 group-hover:text-foreground transition-colors duration-300">
                          {step.title}
                        </h3>
                      </div>
                      <div className="hidden md:flex md:col-span-3 items-center justify-between">
                        <span className="text-sm text-muted-foreground">{step.timing}</span>
                        <div className="w-6 h-6 flex items-center justify-center">
                          <span
                            className="text-lg text-muted-foreground"
                            style={{ transform: activeStep === index ? "rotate(45deg)" : "rotate(0)", transition: "transform 0.3s" }}
                          >
                            +
                          </span>
                        </div>
                      </div>
                    </div>
                  </button>

                  <div
                    className="overflow-hidden"
                    style={{
                      maxHeight: activeStep === index ? "400px" : "0",
                      opacity: activeStep === index ? 1 : 0,
                      transition: "max-height 0.5s ease-out, opacity 0.5s ease-out",
                    }}
                  >
                    <div className="pb-8 md:pb-16 grid md:grid-cols-12 gap-8">
                      <div className="hidden md:block md:col-span-3" />
                      <div className="md:col-span-6 space-y-6">
                        <p className="text-muted-foreground leading-relaxed">{step.description}</p>
                        <ul className="grid grid-cols-2 gap-3">
                          {step.details.map((detail) => (
                            <li key={detail} className="flex items-center gap-2 text-sm text-foreground/70">
                              <span className="w-1 h-1 bg-accent shrink-0" />
                              {detail}
                            </li>
                          ))}
                        </ul>
                      </div>
                    </div>
                  </div>
                </div>
              ))}
              <div className="border-t border-border" />
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

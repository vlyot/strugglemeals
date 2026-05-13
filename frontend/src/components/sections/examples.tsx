import { useEffect, useRef, useState } from "react"
import Autoplay from "embla-carousel-autoplay"
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "@/components/ui/carousel"

const beforeSlides = [
  {
    src: "/struggle1.jpg",
    alt: "A chaotic fridge meal attempt",
    caption: "The Fridge Scramble",
    position: "object-center",
  },
  {
    src: "/struggle2.jpg",
    alt: "Random ingredients thrown together with no plan",
    caption: "Whatever Was Left",
    position: "object-contain",
  },
  {
    src: "/Ice Sandwich.jpeg",
    alt: "An improvised ice cream sandwich",
    caption: "The Ice Sandwich",
    position: "object-center",
  },
]

const recipes = [
  {
    id: "scramble",
    name: "The Classic Scramble",
    subtitle: "Eggs, leftover rice, wilting spinach",
    tag: "15 min · Easy",
    ingredientCount: "3 ingredients",
    description: "What starts as fridge despair becomes a protein-packed stir-fry scramble. Endlessly riffable, always satisfying — and it takes 15 minutes.",
    whyItWorks: "Eggs bind everything together. High heat does the rest.",
  },
  {
    id: "pasta",
    name: "Pantry Pasta",
    subtitle: "Canned tomatoes, pasta, garlic, olive oil",
    tag: "20 min · Easy",
    ingredientCount: "4 ingredients",
    description: "Four pantry staples that 90% of people have. Zero trips to the store needed. A proper sauce in the time it takes to boil water.",
    whyItWorks: "Garlic bloomed in oil is the foundation of every great pasta sauce.",
  },
  {
    id: "fried-rice",
    name: "Leftover Fried Rice",
    subtitle: "Day-old rice, eggs, soy sauce, any veg",
    tag: "10 min · Easy",
    ingredientCount: "4 ingredients",
    description: "The original struggle meal, elevated. Works with almost anything you have. Day-old rice is actually better — less moisture means more crunch.",
    whyItWorks: "High heat + dry rice = restaurant-quality wok char at home.",
  },
]

export function ExamplesSection() {
  const [isVisible, setIsVisible] = useState(false)
  const [activeRecipe, setActiveRecipe] = useState(0)
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
    <section ref={sectionRef} id="examples" className="relative py-32 md:py-48 overflow-hidden bg-secondary">
      <div className="max-w-[1800px] mx-auto px-6 md:px-12 lg:px-20">

        {/* ── BEFORE ── */}
        <div
          className="flex items-center gap-4 mb-16 md:mb-24"
          style={{
            opacity: isVisible ? 1 : 0,
            transform: isVisible ? "translateX(0)" : "translateX(-20px)",
            transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1)",
          }}
        >
          <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">(04)</span>
          <div className="w-8 h-px bg-border" />
          <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">Before &amp; After</span>
        </div>

        <div className="grid lg:grid-cols-2 gap-12 lg:gap-20 items-center mb-32 md:mb-48">
          {/* Carousel */}
          <div
            style={{
              opacity: isVisible ? 1 : 0,
              transform: isVisible ? "translateY(0)" : "translateY(40px)",
              transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.1s",
            }}
          >
            <Carousel className="w-full" opts={{ loop: true }} plugins={[Autoplay({ delay: 3000, stopOnInteraction: true })]}>
              <CarouselContent>
                {beforeSlides.map((slide) => (
                  <CarouselItem key={slide.src}>
                    <div className="relative aspect-[4/3] overflow-hidden rounded-xl">
                      <img
                        src={slide.src}
                        alt={slide.alt}
                        className={`w-full h-full ${slide.position === "object-contain" ? "object-contain" : `object-cover ${slide.position}`}`}
                      />
                      <div className="absolute bottom-0 left-0 right-0 px-5 py-4 bg-gradient-to-t from-foreground/60 to-transparent">
                        <span className="text-xs tracking-[0.2em] uppercase text-background/80">{slide.caption}</span>
                      </div>
                    </div>
                  </CarouselItem>
                ))}
              </CarouselContent>
              <CarouselPrevious className="left-3" />
              <CarouselNext className="right-3" />
            </Carousel>
          </div>

          {/* Before copy */}
          <div
            className="space-y-6"
            style={{
              opacity: isVisible ? 1 : 0,
              transform: isVisible ? "translateY(0)" : "translateY(40px)",
              transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.2s",
            }}
          >
            <h2 className="font-serif text-3xl sm:text-4xl md:text-5xl font-light leading-[1.1] tracking-[-0.01em] text-foreground text-pretty">
              This is what most people end up with
            </h2>
            <p className="text-lg leading-relaxed text-muted-foreground">
              You open the fridge, grab whatever looks usable, and throw it all together with no plan. No structure, no technique — just hope. It works sometimes. Mostly it doesn't.
            </p>
            <p className="text-lg leading-relaxed text-muted-foreground">
              StruggleMeals takes those same random ingredients and turns them into something intentional.
            </p>
          </div>
        </div>

        {/* ── AFTER ── */}
        <div
          className="flex items-center gap-4 mb-16"
          style={{
            opacity: isVisible ? 1 : 0,
            transform: isVisible ? "translateY(0)" : "translateY(20px)",
            transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.3s",
          }}
        >
          <div className="w-8 h-px bg-border" />
          <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">What it becomes instead</span>
        </div>

        <div className="grid lg:grid-cols-12 gap-8 lg:gap-12">
          {/* Garbage plate — the after */}
          <div className="lg:col-span-8 relative"
            style={{
              opacity: isVisible ? 1 : 0,
              transform: isVisible ? "translateY(0)" : "translateY(30px)",
              transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.35s",
            }}
          >
            <div className="relative aspect-[4/3] overflow-hidden rounded-xl">
              <img
                src="/Garbage-Plate-720x405.webp"
                alt="The Garbage Plate — Rochester NY's iconic dish made from whatever's on hand"
                className="w-full h-full object-cover object-center"
              />
              <div className="absolute bottom-0 left-0 right-0 px-6 py-5 bg-gradient-to-t from-foreground/70 to-transparent">
                <p className="text-xs tracking-[0.2em] uppercase text-background/60 mb-1">Inspired by</p>
                <p className="font-sans text-xl text-background font-semibold">The Garbage Plate</p>
                <p className="text-sm text-background/60">Rochester, NY — a beloved local institution serving filling but cheap foods</p>
              </div>
            </div>
          </div>

          {/* Recipe cards */}
          <div className="lg:col-span-4 space-y-4">
            <p className="text-xs tracking-[0.3em] uppercase text-muted-foreground mb-2">Sample Recipes</p>
            {recipes.map((recipe, index) => (
              <button
                key={recipe.id}
                type="button"
                onClick={() => setActiveRecipe(index)}
                className={`w-full text-left p-6 md:p-8 transition-colors duration-500 rounded-xl ${
                  activeRecipe === index ? "bg-background" : "bg-background/50 hover:bg-background/70"
                }`}
                style={{
                  opacity: isVisible ? 1 : 0,
                  transform: isVisible ? "translateY(0)" : "translateY(30px)",
                  transition: `opacity 0.8s cubic-bezier(0.16, 1, 0.3, 1) ${0.4 + index * 0.1}s, transform 0.8s cubic-bezier(0.16, 1, 0.3, 1) ${0.4 + index * 0.1}s, background-color 0.5s`,
                }}
              >
                <div className="space-y-4">
                  <div>
                    <h3 className="font-serif text-2xl md:text-3xl font-light text-foreground mb-1">{recipe.name}</h3>
                    <p className="text-sm text-muted-foreground italic">{recipe.subtitle}</p>
                  </div>
                  <div
                    className="overflow-hidden"
                    style={{
                      maxHeight: activeRecipe === index ? "300px" : "0",
                      opacity: activeRecipe === index ? 1 : 0,
                      transition: "max-height 0.5s ease-out, opacity 0.5s ease-out",
                    }}
                  >
                    <div className="space-y-4 pt-2">
                      <p className="text-sm leading-relaxed text-foreground/70">{recipe.description}</p>
                      <div className="flex items-center gap-2 text-xs">
                        <span className="text-muted-foreground uppercase tracking-wider">Why it works:</span>
                        <span className="text-foreground/60">{recipe.whyItWorks}</span>
                      </div>
                      <div className="flex items-center gap-2 pt-2 border-t border-border">
                        <span className="text-xs tracking-wider text-accent font-medium">{recipe.ingredientCount}</span>
                        <span className="w-1 h-1 bg-muted-foreground rounded-full" />
                        <span className="text-xs text-muted-foreground">{recipe.tag}</span>
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <div className="flex-1 h-px bg-border" />
                    <div className={`w-2 h-2 transition-all duration-500 ${activeRecipe === index ? "bg-accent scale-125" : "bg-border"}`} />
                  </div>
                </div>
              </button>
            ))}
          </div>
        </div>
      </div>
    </section>
  )
}

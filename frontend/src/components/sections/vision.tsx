import { useEffect, useRef, useState } from "react"
import Autoplay from "embla-carousel-autoplay"
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "@/components/ui/carousel"

const stats = [
  { value: "30 min", label: "Avg cook time" },
  { value: "90%", label: "Less food waste" },
  { value: "500+", label: "Recipe variations" },
  { value: "Free", label: "To start" },
]

// object-position crops the reacting dude out of hq720 and fridge
const slides = [
  {
    src: "/hq720.jpg",
    alt: "A fully stocked fridge with a dog sitting inside",
    caption: "What's actually in there",
  },
  {
    src: "/fridge.jpg",
    alt: "An empty fridge being rated",
    caption: "The empty fridge problem",
  },
  {
    src: "/case.jpg",
    alt: "Someone staring into a full but overwhelming fridge",
    caption: "Too much, still nothing to eat",
  },
]

export function VisionSection() {
  const [isVisible, setIsVisible] = useState(false)
  const sectionRef = useRef<HTMLElement>(null)

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting) setIsVisible(true) },
      { threshold: 0.1 }
    )
    if (sectionRef.current) observer.observe(sectionRef.current)
    return () => observer.disconnect()
  }, [])

  const reveal = (delay: string, fromY = "40px") => ({
    opacity: isVisible ? 1 : 0,
    transform: isVisible ? "translateY(0)" : `translateY(${fromY})`,
    transition: `all 0.8s cubic-bezier(0.16, 1, 0.3, 1) ${delay}`,
  })

  return (
    <section ref={sectionRef} id="why-it-works" className="relative py-32 md:py-48 lg:py-64 overflow-hidden">
      {/* Background grid */}
      <div className="absolute inset-0 pointer-events-none">
        <svg className="absolute top-0 left-0 w-full h-full opacity-[0.03]" viewBox="0 0 100 100" preserveAspectRatio="none">
          <defs>
            <pattern id="visionGrid" width="10" height="10" patternUnits="userSpaceOnUse">
              <path d="M 10 0 L 0 0 0 10" fill="none" stroke="currentColor" strokeWidth="0.5" />
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#visionGrid)" />
        </svg>
      </div>

      <div className="max-w-[1800px] mx-auto px-6 md:px-12 lg:px-20">
        {/* Label */}
        <div
          className="flex items-center gap-4 mb-16 md:mb-24"
          style={{
            opacity: isVisible ? 1 : 0,
            transform: isVisible ? "translateX(0)" : "translateX(-20px)",
            transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1)",
          }}
        >
          <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">(01)</span>
          <div className="w-8 h-px bg-border" />
          <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">The Problem</span>
        </div>

        {/* Big text left + carousel right */}
        <div className="grid lg:grid-cols-2 gap-16 lg:gap-24 items-center mb-24 md:mb-40">
          <div>
            <h2
              className="font-serif text-3xl sm:text-4xl md:text-5xl lg:text-6xl font-light leading-[1.1] tracking-[-0.01em] text-foreground text-pretty"
              style={{ opacity: isVisible ? 1 : 0, transform: isVisible ? "translateY(0)" : "translateY(40px)", transition: "all 1s cubic-bezier(0.16, 1, 0.3, 1) 0.1s" }}
            >
              The average household throws away $1,500 worth of food every year — not because they don't want to cook, but because they don't know what to make with what they have.
            </h2>
          </div>

          <div style={reveal("0.2s")}>
            <Carousel className="w-full" opts={{ loop: true }} plugins={[Autoplay({ delay: 3500, stopOnInteraction: true })]}>
              <CarouselContent>
                {slides.map((slide) => (
                  <CarouselItem key={slide.src}>
                    <div className="relative aspect-[4/3] overflow-hidden rounded-xl">
                      <img
                        src={slide.src}
                        alt={slide.alt}
                        className="w-full h-full object-contain"
                      />
                      <div className="absolute bottom-0 left-0 right-0 px-5 py-4 bg-gradient-to-t from-foreground/60 to-transparent rounded-b-xl">
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
        </div>

        {/* Two paragraphs */}
        <div className="grid md:grid-cols-2 gap-12 md:gap-20 mb-24 md:mb-32">
          <div style={reveal("0.3s")}>
            <p className="text-lg md:text-xl leading-relaxed text-foreground/80">
              Opening the fridge and staring blankly at leftovers, half-used vegetables, and random proteins is a universal experience. Eventually, you give up and order takeout.
            </p>
          </div>
          <div style={reveal("0.4s")}>
            <p className="text-lg md:text-xl leading-relaxed text-foreground/80">
              StruggleMeals turns that daily frustration into a moment of creativity. Snap a photo or type what you have — and get a real, personalized recipe that actually works.
            </p>
          </div>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-8 md:gap-12 pt-8 border-t border-border" style={reveal("0.5s", "30px")}>
          {stats.map((stat) => (
            <div key={stat.label} className="space-y-1 leading-5">
              <span className="text-4xl md:text-5xl lg:text-6xl font-light text-foreground font-sans">
                {stat.value}
              </span>
              <p className="text-xs tracking-[0.15em] uppercase text-muted-foreground">{stat.label}</p>
            </div>
          ))}
        </div>
      </div>

      {/* Decorative line */}
      <div
        className="absolute bottom-0 left-6 md:left-12 lg:left-20 right-6 md:right-12 lg:right-20 h-px bg-border"
        style={{
          transform: isVisible ? "scaleX(1)" : "scaleX(0)",
          transformOrigin: "left",
          transition: "transform 1.5s cubic-bezier(0.16, 1, 0.3, 1) 0.6s",
        }}
      />
    </section>
  )
}

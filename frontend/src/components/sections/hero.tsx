import { useEffect, useState } from "react"

export function HeroSection() {
  const [isLoaded, setIsLoaded] = useState(false)
  const [scrollY, setScrollY] = useState(0)

  useEffect(() => {
    setIsLoaded(true)
    const onScroll = () => setScrollY(window.scrollY)
    window.addEventListener("scroll", onScroll, { passive: true })
    return () => window.removeEventListener("scroll", onScroll)
  }, [])

  const reveal = (delay: string) => ({
    opacity: isLoaded ? 1 : 0,
    transform: isLoaded ? "translateY(0)" : "translateY(24px)",
    transition: `all 0.9s cubic-bezier(0.16, 1, 0.3, 1) ${delay}`,
  })

  return (
    <section className="relative h-screen min-h-[600px] overflow-hidden">
      {/* Full-bleed parallax image */}
      <div
        className="absolute inset-0"
        style={{ transform: `translateY(${scrollY * 0.3}px)`, willChange: "transform" }}
      >
        <img
          src="/pexels-photo-12674077.webp"
          alt="A person cooking with everyday ingredients"
          className="w-full h-full object-cover object-center"
          style={{ transform: "scale(1.1)" }}
        />
      </div>
      {/* Warm dark tint over image */}
      <div className="absolute inset-0" style={{ background: "rgba(30, 18, 6, 0.55)" }} />
      {/* Diagonal left-heavy vignette */}
      <div className="absolute inset-0" style={{ background: "linear-gradient(105deg, rgba(11,6,3,0.82) 0%, rgba(11,6,3,0.5) 38%, rgba(11,6,3,0.05) 68%, transparent 100%)" }} />
      {/* Bottom fade */}
      <div className="absolute inset-0" style={{ background: "linear-gradient(to top, rgba(11,6,3,0.6) 0%, rgba(11,6,3,0.1) 44%, transparent 100%)" }} />

      {/* Bottom layout: headline left + copy/CTA right */}
      <div className="absolute bottom-0 left-0 right-0 pb-20 md:pb-28 px-6 md:px-12 lg:px-20">
        <div className="max-w-[1800px] mx-auto grid lg:grid-cols-2 gap-12 lg:gap-20 items-end">

          {/* Left: large serif headline */}
          <div>
            <h1
              className="font-serif text-5xl sm:text-6xl md:text-7xl lg:text-8xl font-light leading-[1.0] tracking-[-0.03em] text-background"
              style={reveal("0.2s")}
            >
              <span className="block">What's in your fridge</span>
              <span className="block">becomes</span>
              <span className="block italic font-light text-accent">your next meal</span>
            </h1>
          </div>

          {/* Right: sub-copy + CTA */}
          <div className="space-y-8" style={reveal("0.5s")}>
            <div className="flex items-center gap-4">
              <div className="w-8 h-px bg-accent" />
              <span className="text-xs tracking-[0.3em] uppercase text-background/70">
                Feel like you've only got scraps?
              </span>
            </div>
            <p className="text-lg md:text-xl leading-relaxed text-background/80 max-w-md">
              Snap what you have. Get a real recipe in seconds. No wasted groceries, no takeout guilt — just meals from whatever's already in your kitchen.
            </p>
            <a
              href="#get-started"
              className="inline-flex items-center gap-3 bg-background text-foreground px-8 py-4 text-sm font-medium hover:bg-accent hover:text-foreground transition-colors duration-300 rounded-xl"
            >
              Start Cooking
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M17 8l4 4m0 0l-4 4m4-4H3" />
              </svg>
            </a>
          </div>
        </div>
      </div>

      {/* Bottom-center: scroll indicator */}
      <div
        className="absolute bottom-8 left-1/2 -translate-x-1/2 flex flex-col items-center gap-3"
        style={reveal("0.8s")}
      >
        <span className="text-xs tracking-[0.3em] uppercase text-background/50">Scroll</span>
        <div className="w-px h-10 overflow-hidden">
          <div
            className="w-full bg-background/50"
            style={{ height: "100%", animation: "scrollPulse 1.8s ease-in-out infinite" }}
          />
        </div>
      </div>
    </section>
  )
}

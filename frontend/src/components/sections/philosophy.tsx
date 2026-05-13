import { useEffect, useRef, useState } from "react";
import { ImageReveal } from "@/components/ui/image-reveal";

const principles = [
  {
    number: "01",
    title: "No Inspiration",
    description:
      "You open the fridge to find scraps of food items here and there, and nothing clicks. We surface unexpected recipe ideas from exactly what you have — no imagination required.",
  },
  {
    number: "02",
    title: "Fear of Waste",
    description:
      "Half a zucchini. Three eggs. Some leftover rice. We make that work — so nothing ends up in the trash and every ingredient earns its place.",
  },
  {
    number: "03",
    title: "Too Many Steps",
    description:
      "Complex recipes feel overwhelming after a long day. We match recipes to your skill level and time available so dinner stays achievable.",
  },
  {
    number: "04",
    title: "Missing Ingredients",
    description:
      "Most recipes require a store run. Ours start with what's already in your kitchen — substitutions built in, no extra errands needed.",
  },
];

export function PhilosophySection() {
  const [isVisible, setIsVisible] = useState(false);
  const sectionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) setIsVisible(true);
      },
      { threshold: 0.15 },
    );
    if (sectionRef.current) observer.observe(sectionRef.current);
    return () => observer.disconnect();
  }, []);

  return (
    <section
      ref={sectionRef}
      id="philosophy"
      className="relative py-32 md:py-48 bg-background overflow-hidden"
    >
      <div className="max-w-[1800px] mx-auto px-6 md:px-12 lg:px-20 relative">
        {/* Label */}
        <div
          className="flex items-center gap-4 mb-16 md:mb-24"
          style={{
            opacity: isVisible ? 1 : 0,
            transform: isVisible ? "translateX(0)" : "translateX(-20px)",
            transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1)",
          }}
        >
          <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">
            (02)
          </span>
          <div className="w-8 h-px bg-border" />
          <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">
            Why People Give Up
          </span>
        </div>

        {/* Headline */}
        <h2
          className="font-serif text-3xl sm:text-4xl md:text-5xl lg:text-6xl font-light leading-[1.1] tracking-[-0.01em] text-foreground max-w-3xl text-pretty mb-20 md:mb-32"
          style={{
            opacity: isVisible ? 1 : 0,
            transform: isVisible ? "translateY(0)" : "translateY(40px)",
            transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.1s",
          }}
        >
          Four reasons dinner doesn't happen — and how we fix each one
        </h2>

        {/* Image + principles side by side */}
        <div className="grid lg:grid-cols-2 gap-16 lg:gap-24 items-start">
          {/* Image */}
          <div
            className="relative"
            style={{
              opacity: isVisible ? 1 : 0,
              transform: isVisible ? "translateY(0)" : "translateY(40px)",
              transition: "all 1s cubic-bezier(0.16, 1, 0.3, 1) 0.2s",
            }}
          >
            <div className="relative aspect-[3/4] overflow-hidden rounded-xl">
              <ImageReveal
                src="/shutterstock_1706275177-1.webp"
                alt="Chef preparing salmon — the gap between having ingredients and knowing what to cook"
                className="object-cover object-center"
                delay={300}
              />
            </div>
            <p className="mt-4 text-xs tracking-[0.2em] uppercase text-muted-foreground">
              The gap between having ingredients and knowing what to cook
            </p>
          </div>

          {/* Principles */}
          <div className="space-y-14">
            {principles.map((principle, index) => (
              <div
                key={principle.number}
                className="group flex items-start gap-6"
                style={{
                  opacity: isVisible ? 1 : 0,
                  transform: isVisible ? "translateY(0)" : "translateY(40px)",
                  transition: `all 0.8s cubic-bezier(0.16, 1, 0.3, 1) ${0.3 + index * 0.1}s`,
                }}
              >
                <span className="font-mono text-xs tracking-wider text-muted-foreground pt-1 shrink-0">
                  {principle.number}
                </span>
                <div className="space-y-3">
                  <h3 className="font-serif text-2xl md:text-3xl font-light text-foreground group-hover:text-primary transition-colors duration-500">
                    {principle.title}
                  </h3>
                  <p className="text-base leading-relaxed text-muted-foreground">
                    {principle.description}
                  </p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

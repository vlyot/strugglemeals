import React, { useEffect, useRef, useState } from "react"

const infoItems = [
  {
    label: "Early Access",
    description: "We're onboarding users in batches to keep quality high. You'll get notified when your spot is ready.",
  },
  {
    label: "Free Forever",
    description: "Core features will always be free. No credit card required. No tricks.",
  },
  {
    label: "Your Privacy",
    description: "We don't sell your data. Your fridge contents stay yours.",
  },
]

const socialLinks = ["Instagram", "GitHub", "Twitter / X"]

export function WaitlistSection() {
  const [isVisible, setIsVisible] = useState(false)
  const [formState, setFormState] = useState({ name: "", email: "", struggle: "" })
  const [submitted, setSubmitted] = useState(false)
  const sectionRef = useRef<HTMLElement>(null)

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting) setIsVisible(true) },
      { threshold: 0.1 }
    )
    if (sectionRef.current) observer.observe(sectionRef.current)
    return () => observer.disconnect()
  }, [])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
setSubmitted(true)
  }

  const reveal = (delay: string, fromY = "40px") => ({
    opacity: isVisible ? 1 : 0,
    transform: isVisible ? "translateY(0)" : `translateY(${fromY})`,
    transition: `all 0.8s cubic-bezier(0.16, 1, 0.3, 1) ${delay}`,
  })

  return (
    <section ref={sectionRef} id="get-started" className="relative py-32 md:py-48 overflow-hidden">
      {/* Background dot pattern */}
      <div className="absolute inset-0 pointer-events-none">
        <svg className="absolute top-0 left-0 w-full h-full opacity-[0.02]" viewBox="0 0 100 100" preserveAspectRatio="none">
          <defs>
            <pattern id="waitlistDots" width="5" height="5" patternUnits="userSpaceOnUse">
              <circle cx="0.5" cy="0.5" r="0.5" fill="currentColor" />
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#waitlistDots)" />
        </svg>
      </div>

      <div className="max-w-[1800px] mx-auto px-6 md:px-12 lg:px-20">
        <div className="grid lg:grid-cols-12 gap-16 lg:gap-20">
          {/* Label */}
          <div className="lg:col-span-2">
            <div
              className="flex items-center gap-4"
              style={{
                opacity: isVisible ? 1 : 0,
                transform: isVisible ? "translateX(0)" : "translateX(-20px)",
                transition: "all 0.8s cubic-bezier(0.16, 1, 0.3, 1)",
              }}
            >
              <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">(05)</span>
              <div className="w-8 h-px bg-border" />
              <span className="text-xs tracking-[0.3em] uppercase text-muted-foreground">Get Started</span>
            </div>
          </div>

          {/* Content */}
          <div className="lg:col-span-10">
            <div className="grid lg:grid-cols-2 gap-16 lg:gap-24">
              {/* Left */}
              <div className="space-y-8">
                <h2
                  className="font-serif text-3xl sm:text-4xl md:text-5xl lg:text-6xl font-light leading-[1.1] tracking-[-0.01em] text-foreground text-pretty"
                  style={reveal("0.1s")}
                >
                  Join the waitlist — try it free
                </h2>

                <p className="text-lg text-muted-foreground leading-relaxed max-w-md" style={reveal("0.2s", "30px")}>
                  StruggleMeals is in early access. Drop your email and we'll let you know when your spot is ready.
                </p>

                <div className="space-y-6 pt-8" style={reveal("0.3s", "20px")}>
                  {infoItems.map((item) => (
                    <div key={item.label} className="space-y-1">
                      <span className="text-xs tracking-[0.2em] uppercase text-muted-foreground">{item.label}</span>
                      <p className="text-foreground/80 text-sm leading-relaxed">{item.description}</p>
                    </div>
                  ))}
                  <div className="space-y-1 pt-2">
                    <span className="text-xs tracking-[0.2em] uppercase text-muted-foreground">Contact</span>
                    <p className="text-foreground">
                      <a href="mailto:contact@strugglemeal.app" className="hover:text-accent transition-colors duration-300">
                        contact@strugglemeal.app
                      </a>
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-6 pt-8" style={reveal("0.4s", "20px")}>
                  {socialLinks.map((social) => (
                    <a
                      key={social}
                      href="#"
                      className="text-sm tracking-[0.1em] uppercase text-muted-foreground hover:text-foreground transition-colors duration-300"
                    >
                      {social}
                    </a>
                  ))}
                </div>
              </div>

              {/* Right — form or success */}
              {submitted ? (
                <div className="flex flex-col justify-center space-y-4" style={{ opacity: isVisible ? 1 : 0, transition: "opacity 0.5s ease" }}>
                  <div className="w-12 h-px bg-accent" />
                  <h3 className="font-serif text-3xl md:text-4xl font-light text-foreground">You're on the list.</h3>
                  <p className="text-muted-foreground leading-relaxed">
                    We'll email you when your spot opens up. In the meantime, start thinking about what's in that fridge.
                  </p>
                </div>
              ) : (
                <form onSubmit={handleSubmit} className="space-y-8" style={reveal("0.3s")}>
                  <div className="space-y-2">
                    <label htmlFor="name" className="text-xs tracking-[0.2em] uppercase text-muted-foreground">Name</label>
                    <input
                      type="text" id="name"
                      value={formState.name}
                      onChange={(e) => setFormState({ ...formState, name: e.target.value })}
                      className="w-full px-0 py-3 bg-transparent border-0 border-b border-border text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:border-foreground transition-colors duration-300"
                      placeholder="Your name" required
                    />
                  </div>
                  <div className="space-y-2">
                    <label htmlFor="email" className="text-xs tracking-[0.2em] uppercase text-muted-foreground">Email</label>
                    <input
                      type="email" id="email"
                      value={formState.email}
                      onChange={(e) => setFormState({ ...formState, email: e.target.value })}
                      className="w-full px-0 py-3 bg-transparent border-0 border-b border-border text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:border-foreground transition-colors duration-300"
                      placeholder="your@email.com" required
                    />
                  </div>
                  <div className="space-y-2">
                    <label htmlFor="struggle" className="text-xs tracking-[0.2em] uppercase text-muted-foreground">
                      What's your biggest kitchen struggle? <span className="normal-case opacity-60">(optional)</span>
                    </label>
                    <textarea
                      id="struggle"
                      value={formState.struggle}
                      onChange={(e) => setFormState({ ...formState, struggle: e.target.value })}
                      rows={3}
                      className="w-full px-0 py-3 bg-transparent border-0 border-b border-border text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:border-foreground transition-colors duration-300 resize-none"
                      placeholder="I always have random vegetables and no idea what to do with them..."
                    />
                  </div>
                  <div className="pt-4">
                    <button
                      type="submit"
                      className="group inline-flex items-center gap-4 px-8 py-4 bg-primary text-primary-foreground hover:bg-primary/90 transition-colors duration-300"
                    >
                      <span className="text-sm tracking-[0.15em] uppercase">Join Waitlist</span>
                      <svg className="w-5 h-5 transition-transform duration-300 group-hover:translate-x-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M17 8l4 4m0 0l-4 4m4-4H3" />
                      </svg>
                    </button>
                  </div>
                </form>
              )}
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

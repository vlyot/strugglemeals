export function Footer() {
  const currentYear = new Date().getFullYear()
  const navLinks = ["How it Works", "Why It Works", "Examples", "Get Started"]

  return (
    <footer className="relative py-16 md:py-24 border-t border-border">
      <div className="max-w-[1800px] mx-auto px-6 md:px-12 lg:px-20">
        <div className="grid lg:grid-cols-12 gap-12 lg:gap-20">
          {/* Logo & Tagline */}
          <div className="lg:col-span-6 space-y-6">
            <a href="/" className="inline-block">
              <span className="text-lg md:text-xl font-medium tracking-[-0.01em] text-foreground">
                StruggleMeals
              </span>
            </a>
            <p className="text-sm text-muted-foreground leading-relaxed max-w-sm">
              Turn what's in your fridge into a meal. No grocery run required.
            </p>
          </div>

          {/* Navigation */}
          <div className="lg:col-span-3">
            <h4 className="text-xs tracking-[0.2em] uppercase text-muted-foreground mb-6">
              Product
            </h4>
            <nav className="space-y-4">
              {navLinks.map((item) => (
                <a
                  key={item}
                  href={`#${item.toLowerCase().replace(/\s+/g, "-")}`}
                  className="block text-sm text-foreground/70 hover:text-foreground transition-colors duration-300"
                >
                  {item}
                </a>
              ))}
            </nav>
          </div>
        </div>

        {/* Bottom Bar */}
        <div className="mt-16 pt-8 border-t border-border flex flex-col md:flex-row items-center justify-between gap-6">
          <p className="text-xs text-muted-foreground">
            © {currentYear} StruggleMeals. All rights reserved.
          </p>
          <button
            type="button"
            onClick={() => window.scrollTo({ top: 0, behavior: "smooth" })}
            className="group flex items-center gap-2 text-xs tracking-[0.1em] uppercase text-muted-foreground hover:text-foreground transition-colors duration-300"
          >
            <span>Back to top</span>
            <svg
              className="w-4 h-4 transition-transform duration-300 group-hover:-translate-y-1"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5 10l7-7m0 0l7 7m-7-7v18" />
            </svg>
          </button>
        </div>

        {/* Decorative Large Text */}
        <div className="mt-16 md:mt-24 overflow-hidden">
          <p className="font-sans text-[8vw] md:text-[6vw] lg:text-[5vw] font-semibold tracking-[-0.02em] text-foreground/[0.03] leading-none whitespace-nowrap">
            Great meals start with whatever you've got.
          </p>
        </div>
      </div>
    </footer>
  )
}

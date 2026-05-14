import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import { UserButton } from "@neondatabase/neon-js/auth/react"
import { authClient } from "@/stack/client"
import { cn } from "@/lib/utils"

const navItems = [
  { label: "How it Works", href: "#how-it-works" },
  { label: "Why It Works", href: "#why-it-works" },
  { label: "Examples", href: "#examples" },
  { label: "Get Started", href: "#get-started" },
]

export function Header() {
  const { data: session } = authClient.useSession()
  const user = session?.user ?? null
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false)
  const [isScrolled, setIsScrolled] = useState(false)

  useEffect(() => {
    const onScroll = () => setIsScrolled(window.scrollY > 60)
    window.addEventListener("scroll", onScroll, { passive: true })
    return () => window.removeEventListener("scroll", onScroll)
  }, [])

  const scrolled = isScrolled || isMobileMenuOpen

  return (
    <header
      className={cn(
        "fixed top-0 left-0 right-0 z-50 transition-all duration-500",
        scrolled
          ? "bg-background border-b border-border"
          : "bg-transparent border-b border-transparent"
      )}
    >
      <div className="max-w-[1800px] mx-auto px-6 md:px-12 lg:px-20">
        <div className="flex items-center justify-between h-20 md:h-24">
          {/* Logo */}
          <a href="/" className="relative group">
            <span className={cn(
              "text-lg md:text-xl font-medium tracking-[-0.01em] transition-colors duration-500",
              scrolled ? "text-foreground" : "text-background"
            )}>
              StruggleMeals
            </span>
            <span className="absolute -bottom-1 left-0 w-0 h-px bg-accent transition-all duration-500 group-hover:w-full" />
          </a>

          {/* Desktop Nav */}
          <nav className="hidden lg:flex items-center gap-10">
            {navItems.map((item) => (
              <a
                key={item.label}
                href={item.href}
                className={cn(
                  "relative group text-sm tracking-normal transition-colors duration-300",
                  scrolled
                    ? "text-muted-foreground hover:text-foreground"
                    : "text-background/70 hover:text-background"
                )}
              >
                {item.label}
                <span className="absolute -bottom-1 left-0 w-0 h-px bg-accent transition-all duration-300 group-hover:w-full" />
              </a>
            ))}
          </nav>

          {/* CTA */}
          <div className="hidden lg:flex items-center gap-4">
            {user ? (
              <>
                <Link
                  to="/history"
                  className={cn(
                    "text-sm tracking-normal transition-colors duration-300",
                    scrolled ? "text-muted-foreground hover:text-foreground" : "text-background/70 hover:text-background"
                  )}
                >
                  History
                </Link>
                <Link
                  to="/favourites"
                  className={cn(
                    "text-sm tracking-normal transition-colors duration-300",
                    scrolled ? "text-muted-foreground hover:text-foreground" : "text-background/70 hover:text-background"
                  )}
                >
                  Favourites
                </Link>
                <UserButton />
              </>
            ) : (
              <Link
                to="/handler/sign-in"
                className={cn(
                  "inline-flex items-center gap-2 px-6 py-3 text-sm tracking-normal transition-all duration-300",
                  scrolled
                    ? "text-primary-foreground bg-primary hover:bg-primary/90"
                    : "text-foreground bg-background hover:bg-accent"
                )}
                style={{ borderRadius: "0.75rem" }}
              >
                Sign in →
              </Link>
            )}
          </div>

          {/* Mobile hamburger */}
          <button
            type="button"
            onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
            className="lg:hidden relative w-10 h-10 flex items-center justify-center"
            aria-label="Toggle menu"
          >
            <div className="flex flex-col gap-1.5">
              <span
                className={cn(
                  "w-6 h-px transition-all duration-300",
                  scrolled ? "bg-foreground" : "bg-background",
                  isMobileMenuOpen && "rotate-45 translate-y-[4px]"
                )}
              />
              <span
                className={cn(
                  "w-6 h-px transition-all duration-300",
                  scrolled ? "bg-foreground" : "bg-background",
                  isMobileMenuOpen && "-rotate-45 -translate-y-[3px]"
                )}
              />
            </div>
          </button>
        </div>
      </div>

      {/* Mobile Menu */}
      <div
        className={cn(
          "lg:hidden fixed inset-0 bg-background transition-all duration-500 ease-out",
          isMobileMenuOpen ? "opacity-100 pointer-events-auto" : "opacity-0 pointer-events-none"
        )}
        style={{ top: "80px" }}
      >
        <nav className="flex flex-col items-center justify-center h-full gap-8 pb-20">
          {navItems.map((item, index) => (
            <a
              key={item.label}
              href={item.href}
              onClick={() => setIsMobileMenuOpen(false)}
              className="text-2xl font-sans tracking-[0.05em] text-foreground hover:text-accent"
              style={{
                transform: isMobileMenuOpen ? "translateY(0)" : "translateY(20px)",
                opacity: isMobileMenuOpen ? 1 : 0,
                transition: `all 0.5s ease-out ${isMobileMenuOpen ? index * 50 : 0}ms`,
              }}
            >
              {item.label}
            </a>
          ))}
          {user ? (
            <>
              <Link
                to="/history"
                onClick={() => setIsMobileMenuOpen(false)}
                className="text-2xl font-sans tracking-[0.05em] text-foreground hover:text-accent"
                style={{
                  transform: isMobileMenuOpen ? "translateY(0)" : "translateY(20px)",
                  opacity: isMobileMenuOpen ? 1 : 0,
                  transition: `all 0.5s ease-out ${isMobileMenuOpen ? navItems.length * 50 : 0}ms`,
                }}
              >
                History
              </Link>
              <Link
                to="/favourites"
                onClick={() => setIsMobileMenuOpen(false)}
                className="text-2xl font-sans tracking-[0.05em] text-foreground hover:text-accent"
                style={{
                  transform: isMobileMenuOpen ? "translateY(0)" : "translateY(20px)",
                  opacity: isMobileMenuOpen ? 1 : 0,
                  transition: `all 0.5s ease-out ${isMobileMenuOpen ? (navItems.length + 1) * 50 : 0}ms`,
                }}
              >
                Favourites
              </Link>
              <div className="mt-4">
                <UserButton />
              </div>
            </>
          ) : (
            <Link
              to="/handler/sign-in"
              onClick={() => setIsMobileMenuOpen(false)}
              className="mt-8 px-8 py-4 text-sm tracking-normal text-primary-foreground bg-primary"
              style={{
                transform: isMobileMenuOpen ? "translateY(0)" : "translateY(20px)",
                opacity: isMobileMenuOpen ? 1 : 0,
                transition: `all 0.5s ease-out ${isMobileMenuOpen ? navItems.length * 50 : 0}ms`,
              }}
            >
              Sign in
            </Link>
          )}
        </nav>
      </div>
    </header>
  )
}

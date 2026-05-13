import { useEffect, useRef, useState } from "react"

interface ImageRevealProps {
  src: string
  alt: string
  className?: string
  sizes?: string
  delay?: number
  priority?: boolean
}

export function ImageReveal({
  src,
  alt,
  className = "",
  delay = 0,
}: ImageRevealProps) {
  const [isVisible, setIsVisible] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setIsVisible(true)
          observer.disconnect()
        }
      },
      { threshold: 0.1 }
    )
    if (containerRef.current) observer.observe(containerRef.current)
    return () => observer.disconnect()
  }, [])

  return (
    <div ref={containerRef} className="relative overflow-hidden w-full h-full">
      <img
        src={src}
        alt={alt}
        className={`absolute inset-0 w-full h-full ${className}`}
        style={{ objectFit: "cover" }}
      />
      {/* Primary mask — slides up to reveal */}
      <div
        className="absolute inset-0 bg-background"
        style={{
          transform: isVisible ? "translateY(-100%)" : "translateY(0)",
          transition: `transform 1.2s cubic-bezier(0.77, 0, 0.175, 1) ${delay}ms`,
          zIndex: 10,
        }}
      />
      {/* Secondary mask for depth */}
      <div
        className="absolute inset-0 bg-secondary/50"
        style={{
          transform: isVisible ? "translateY(-100%)" : "translateY(0)",
          transition: `transform 1s cubic-bezier(0.77, 0, 0.175, 1) ${delay + 100}ms`,
          zIndex: 9,
        }}
      />
    </div>
  )
}

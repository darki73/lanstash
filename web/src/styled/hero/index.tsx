/**
 * Hero: a game's art as a wide band across the top of its page: a wide image whole, a portrait
 * poster cut to its lower part (where its logo is), fading into the page at the bottom, with an
 * optional badge in its corner. It fades in once loaded; an image that fails leaves nothing.
 */
import { type ReactNode, useState } from "react"
import { classes } from "@/lib/classes"

export interface HeroProps {
  readonly image: string
  readonly cover?: "wide" | "poster"
  readonly badge?: ReactNode
}

export function Hero({ image, cover = "wide", badge }: HeroProps) {
  const [state, setState] = useState<{ src: string; loaded: boolean; failed: boolean }>({
    src: image,
    loaded: false,
    failed: false,
  })
  const now = state.src === image ? state : { src: image, loaded: false, failed: false }
  if (now.failed) return null
  return (
    <div className="relative aspect-[21/6] overflow-hidden rounded-xl border bg-sunken shadow-panel">
      <img
        src={image}
        alt=""
        decoding="async"
        onLoad={() => setState({ src: image, loaded: true, failed: false })}
        onError={() => setState({ src: image, loaded: false, failed: true })}
        className={classes(
          "size-full object-cover transition-opacity duration-500",
          cover === "poster" && "object-[center_80%]",
          now.loaded ? "opacity-100" : "opacity-0",
        )}
      />
      <div aria-hidden className="absolute inset-x-0 bottom-0 h-1/2 bg-linear-to-t from-canvas/90 to-transparent" />
      {badge !== undefined && <span className="absolute top-3 right-3 flex shadow-panel">{badge}</span>}
    </div>
  )
}

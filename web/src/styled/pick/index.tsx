/**
 * Pick: games to choose from, as covers in a grid (as many to a row as fit, at least two on a phone).
 * A cover: the game's art (a wide banner whole, a portrait poster by its lower part; a band in a hue
 * of its own with its initials while it loads or when it has none), a box in its corner to choose it,
 * a badge in the other corner (such as how much lancache holds), its name (a link to its page) and a
 * line under it (its size). A chosen cover is ringed in the cache's amber and its art lifts a little.
 */
import { type ReactNode, useState } from "react"
import { useT } from "@/i18n"
import { classes } from "@/lib/classes"
import { hueOf, initialsOf } from "@/styled/cards"

export function PickGrid({ label, children }: { readonly label: string; readonly children?: ReactNode }) {
  return (
    <ul aria-label={label} className="grid grid-cols-2 gap-3 sm:grid-cols-[repeat(auto-fill,minmax(13rem,1fr))]">
      {children}
    </ul>
  )
}

export interface PickCardProps {
  readonly name: string
  readonly href?: string
  readonly image?: string
  readonly cover?: "wide" | "poster"
  readonly detail?: string
  readonly badge?: ReactNode
  readonly chosen: boolean
  /** Choosing: none, no box. */
  readonly onChoose?: (chosen: boolean) => void
}

export function PickCard({ name, href, image, cover = "wide", detail, badge, chosen, onChoose }: PickCardProps) {
  const t = useT("common")
  const [state, setState] = useState<{ src?: string; loaded: boolean; failed: boolean }>({
    loaded: false,
    failed: false,
  })
  const now = state.src === image ? state : { src: image, loaded: false, failed: false }
  const hue = hueOf(name)
  return (
    <li
      aria-label={name}
      className={classes(
        "group relative flex flex-col overflow-hidden rounded-xl border bg-raised shadow-panel transition-[box-shadow,border-color]",
        chosen ? "border-cache ring-2 ring-cache/60" : "hover:border-faint/40",
      )}
    >
      <div
        className="relative aspect-[460/215] overflow-hidden bg-sunken"
        style={{
          backgroundImage: `repeating-linear-gradient(135deg, hsl(${hue} 40% 50% / 0.06) 0 2px, transparent 2px 14px), linear-gradient(135deg, hsl(${hue} 45% 45% / 0.32), hsl(${(hue + 50) % 360} 40% 30% / 0.12))`,
        }}
      >
        {!now.loaded && (
          <span
            aria-hidden
            className="absolute inset-0 flex items-center justify-center font-semibold text-2xl text-text/25"
          >
            {initialsOf(name)}
          </span>
        )}
        {image !== undefined && !now.failed && (
          <img
            src={image}
            alt=""
            loading="lazy"
            decoding="async"
            onLoad={() => setState({ src: image, loaded: true, failed: false })}
            onError={() => setState({ src: image, loaded: false, failed: true })}
            className={classes(
              "relative size-full object-cover transition-[opacity,scale] duration-500",
              cover === "poster" && "object-[center_82%]",
              now.loaded ? "opacity-100" : "opacity-0",
              chosen ? "scale-[1.04]" : "group-hover:scale-[1.03]",
            )}
          />
        )}
        {onChoose !== undefined && (
          <label className="absolute top-2 left-2 flex cursor-pointer items-center rounded-md bg-canvas/70 p-1.5 shadow-panel backdrop-blur-md">
            <input
              type="checkbox"
              aria-label={t("ui.choose", { name })}
              checked={chosen}
              onChange={(e) => onChoose(e.target.checked)}
              className="size-4 cursor-pointer accent-cache"
            />
          </label>
        )}
        {badge !== undefined && <span className="absolute top-2 right-2 flex shadow-panel">{badge}</span>}
      </div>
      <div className="flex flex-col gap-0.5 p-3">
        {href === undefined ? (
          <span className="truncate font-medium text-sm">{name}</span>
        ) : (
          <a
            href={href}
            className="truncate rounded-sm font-medium text-sm outline-none hover:underline focus-visible:ring-2 focus-visible:ring-cache"
          >
            {name}
          </a>
        )}
        {detail !== undefined && <span className="truncate font-mono text-faint text-xs tabular-nums">{detail}</span>}
      </div>
    </li>
  )
}

/**
 * Builds: a game's builds (its own, a pre-release one, the other products of its directory) as
 * compact rows in a list. A row: a wide thumbnail of its own art where it has some (a tile in a hue of
 * its own, worked out from its name, while it loads or when it fails), the build's name with a tag after it (such as its product code), its
 * version under it in the monospace face, a badge (current, a percentage, checking), a slim bar of
 * how much of it is stashed with the amounts under it, and its actions at the end; a note under the
 * row when there is one (why it could not be worked out); its details folded under a Details line,
 * opened on a click.
 */
import { type ReactNode, useState } from "react"
import { useT } from "@/i18n"
import { fraction as clamp } from "@/lib/clamp"
import { hueOf } from "@/styled/cards"

export function BuildList({ label, children }: { readonly label: string; readonly children?: ReactNode }) {
  return (
    <ul aria-label={label} className="flex flex-col gap-2">
      {children}
    </ul>
  )
}

export interface BuildRowProps {
  readonly name: string
  /** Its own art (a wide image). */
  readonly image?: string
  readonly tag?: string
  readonly version: string
  readonly badge?: ReactNode
  /** 0 to 1; no bar without it. */
  readonly fraction?: number
  /** Under the bar ("133.28 of 133.28 GB"). */
  readonly amount?: string
  readonly note?: ReactNode
  readonly actions?: ReactNode
  /** Folded under a Details line. */
  readonly details?: ReactNode
}

function Thumb({ name, image }: { name: string; image: string }) {
  const [state, setState] = useState<{ src: string; loaded: boolean; failed: boolean }>({
    src: image,
    loaded: false,
    failed: false,
  })
  const now = state.src === image ? state : { src: image, loaded: false, failed: false }
  const hue = hueOf(name)
  return (
    <span
      aria-hidden
      className="relative hidden aspect-[16/9] w-28 shrink-0 overflow-hidden rounded-lg border sm:block"
      style={{
        backgroundImage: `linear-gradient(135deg, hsl(${hue} 45% 45% / 0.35), hsl(${(hue + 50) % 360} 40% 30% / 0.15))`,
      }}
    >
      {!now.failed && (
        <img
          src={image}
          alt=""
          loading="lazy"
          decoding="async"
          onLoad={() => setState({ src: image, loaded: true, failed: false })}
          onError={() => setState({ src: image, loaded: false, failed: true })}
          className={`size-full object-cover transition-opacity duration-500 ${now.loaded ? "opacity-100" : "opacity-0"}`}
        />
      )}
    </span>
  )
}

export function BuildRow({
  name,
  image,
  tag,
  version,
  badge,
  fraction,
  amount,
  note,
  actions,
  details,
}: BuildRowProps) {
  const pct = fraction === undefined ? 0 : 100 * clamp(fraction)
  const t = useT("common")
  return (
    <li aria-label={name} className="flex flex-col gap-3 rounded-xl border bg-raised p-4 shadow-panel">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-3 lg:flex-nowrap">
        {image !== undefined && <Thumb name={name} image={image} />}
        <div className="flex min-w-40 flex-1 flex-col gap-0.5">
          <span className="flex items-baseline gap-2">
            <span className="line-clamp-2 font-medium leading-snug">{name}</span>
            {tag !== undefined && <span className="shrink-0 font-mono text-[11px] text-faint">{tag}</span>}
          </span>
          <span className="font-mono text-muted text-xs tabular-nums">{version}</span>
        </div>
        {badge !== undefined && <span className="flex shrink-0">{badge}</span>}
        {fraction !== undefined && (
          <div className="flex w-full flex-col gap-1 sm:w-44 sm:shrink-0">
            <meter
              className="sr-only"
              min={0}
              max={100}
              value={Math.round(pct)}
              aria-label={`${name}: ${amount ?? ""}`}
            />
            <div aria-hidden className="h-1.5 overflow-hidden rounded-full bg-sunken">
              <div
                className="h-full rounded-full bg-cache transition-[width] duration-700"
                style={{ width: `${pct}%` }}
              />
            </div>
            {amount !== undefined && (
              <span className="text-end font-mono text-faint text-xs tabular-nums">{amount}</span>
            )}
          </div>
        )}
        {actions !== undefined && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
      </div>
      {note !== undefined && <div className="text-sm">{note}</div>}
      {details !== undefined && (
        <details className="group border-t pt-2">
          <summary className="flex cursor-pointer list-none items-center gap-1.5 text-faint text-xs hover:text-text">
            <span aria-hidden className="transition-transform group-open:rotate-90">
              ›
            </span>
            {t("ui.details")}
          </summary>
          <div className="pt-2">{details}</div>
        </details>
      )}
    </li>
  )
}

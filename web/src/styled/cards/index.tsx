/**
 * Cards: a grid of game cards, four to a row where the space is wide (64rem), fewer as it narrows,
 * down to one; the cards always fill the row. A card with an image shows it as a banner
 * across its top: a wide image whole, a portrait poster cut to its lower part (where its logo is),
 * fading into the card, with a badge (such as whether its build is current) in its corner. Where the
 * grid's games have covers (a cover shape is given), every card keeps the banner's place: a game
 * without an image, or whose image fails, gets a skeleton cover instead (a gradient in a hue of its
 * own, worked out from its name, a faint diagonal weave and its initials), so the cards of a row line
 * up; the skeleton also stands in while an image loads. Without a banner the badge sits beside the
 * name. An
 * icon sits on a tile beside the name; without one, the tile shows the game's initial (but only
 * when there is no banner). Then its name (a link to the game's page when given; with a muted note when the name is only a code), its
 * codes in the monospace face (when given), a muted line under them (such as its version), its
 * size large, a line of detail, a bar for its share of the
 * largest card's figure (0..1, clamped; above 0 always a sliver), lines under it, a label and its
 * value each (a game's variants and how much of each is stashed), and a footnote at the card's foot:
 * short facts side by side, split by thin rules (as a launcher's "Region │ Version" line), and its
 * actions at the end (on a line of their own when they do not fit beside the facts).
 */
import { type ReactNode, useState } from "react"
import { fraction as clamp } from "@/lib/clamp"
import { classes } from "@/lib/classes"
import { type Tone, toneBg } from "@/styled/tone"

export function CardGrid({ children }: { readonly children?: ReactNode }) {
  return (
    <div className="@container">
      <ul className="grid grid-cols-1 gap-3 @xl:grid-cols-2 @3xl:grid-cols-3 @5xl:grid-cols-4">{children}</ul>
    </div>
  )
}

export type CoverShape = "wide" | "poster"

/** A line of a card: a label (cut short when long) and its value at the end; on hover, its hint (else the label). */
export interface CardNote {
  readonly label: string
  readonly value: string
  readonly hint?: string
}

export interface GameCardProps {
  readonly name: string
  /** Where the name leads: the game's own page. */
  readonly href?: string
  /** Shown after the name, muted: such as "no name yet". */
  readonly note?: string
  readonly codes?: string
  /** Address of the game's cover image. */
  readonly image?: string
  /** The cover's shape: a wide banner (Steam's 460×215 headers) or a portrait poster. */
  readonly cover?: CoverShape
  /** Address of the game's square icon. */
  readonly icon?: string
  /** A badge, such as a Signal: on the banner's corner, else beside the name. */
  readonly badge?: ReactNode
  readonly size: string
  readonly detail: string
  /** A muted line under the name, such as the game's version. */
  readonly sub?: string
  /** An action at the end of the footnote, such as a Button. */
  readonly action?: ReactNode
  /** Facts for the muted line under the bar, such as when it was last used. */
  readonly foot?: readonly string[]
  /** Lines between the bar and the footnote, a label and its value each (a variant and how much of it is stashed). */
  readonly notes?: readonly CardNote[]
  readonly fraction: number
  readonly tone: Tone
}

/** Whether an image at an address failed: remembered per address, so a new one is tried. */
function useFailed() {
  const [failed, setFailed] = useState<readonly string[]>([])
  return {
    ok: (src: string | undefined): src is string => src !== undefined && !failed.includes(src),
    fail: (src: string) => setFailed((f) => [...f, src]),
  }
}

/** A hue of a name's own (0..359): the same name, the same hue. */
export function hueOf(name: string): number {
  let h = 0
  for (const c of name) h = (h * 31 + (c.codePointAt(0) ?? 0)) % 360
  return h
}

/** A name's initials: the first letters of its first two words ("Grand Theft Auto V": GT). */
export function initialsOf(name: string): string {
  const words = name.split(/[\s:_-]+/).filter((w) => /^[\p{L}\p{N}]/u.test(w))
  return words
    .slice(0, 2)
    .map((w) => [...w][0] ?? "")
    .join("")
    .toUpperCase()
}

function Skeleton({ name }: { name: string }) {
  const hue = hueOf(name)
  return (
    <span
      aria-hidden
      data-skeleton
      className="absolute inset-0 flex items-center justify-center"
      style={{
        backgroundImage: `repeating-linear-gradient(135deg, hsl(${hue} 40% 50% / 0.06) 0 2px, transparent 2px 14px), linear-gradient(135deg, hsl(${hue} 45% 45% / 0.32), hsl(${(hue + 50) % 360} 40% 30% / 0.12))`,
      }}
    >
      <span className="font-semibold text-4xl text-text/25 tracking-tight">{initialsOf(name)}</span>
    </span>
  )
}

function Banner(props: {
  image: string | undefined
  cover: CoverShape
  name: string
  badge: ReactNode
  onFail: () => void
}) {
  const [loaded, setLoaded] = useState(false)
  return (
    <div className="relative -mx-4 -mt-4 aspect-[460/215] overflow-hidden rounded-t-xl border-b bg-sunken">
      {!loaded && <Skeleton name={props.name} />}
      {props.image !== undefined && (
        <img
          src={props.image}
          alt=""
          loading="lazy"
          decoding="async"
          onLoad={() => setLoaded(true)}
          onError={props.onFail}
          className={classes(
            "relative size-full object-cover transition-[opacity,scale] duration-500 group-hover:scale-[1.03]",
            props.cover === "poster" && "object-[center_82%]",
            loaded ? "opacity-100" : "opacity-0",
          )}
        />
      )}
      {props.cover === "poster" && (
        <div aria-hidden className="absolute inset-x-0 bottom-0 h-1/3 bg-linear-to-t from-raised/80 to-transparent" />
      )}
      {props.badge !== undefined && <span className="absolute top-2 right-2 flex shadow-panel">{props.badge}</span>}
    </div>
  )
}

function Tile({ icon, initial, onFail }: { icon: string | undefined; initial: string; onFail: (src: string) => void }) {
  return (
    <span
      aria-hidden
      className="inline-flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-lg border bg-sunken font-semibold text-muted"
    >
      {icon === undefined ? (
        initial
      ) : (
        <img src={icon} alt="" loading="lazy" decoding="async" onError={() => onFail(icon)} className="size-full" />
      )}
    </span>
  )
}

export function GameCard(props: GameCardProps) {
  const {
    name,
    href,
    note,
    codes,
    image,
    cover,
    icon,
    badge,
    sub,
    size,
    detail,
    foot = [],
    notes = [],
    action,
    fraction,
    tone,
  } = props
  const pct = 100 * clamp(fraction)
  const failed = useFailed()
  const initial = name.slice(0, 1).toUpperCase()
  const banner = failed.ok(image) ? image : undefined
  // The grid's games have covers: this card keeps the banner's place, with a skeleton when it has no image.
  const covered = cover !== undefined || image !== undefined
  const tileIcon = failed.ok(icon) ? icon : undefined
  return (
    <li className="group flex flex-col gap-3 overflow-hidden rounded-xl border bg-raised p-4 shadow-panel transition-colors hover:border-faint/40">
      {covered && (
        <Banner
          key={banner ?? "none"}
          image={banner}
          cover={cover ?? "wide"}
          name={name}
          badge={badge}
          onFail={() => banner !== undefined && failed.fail(banner)}
        />
      )}
      <div className="flex items-start gap-3">
        {(!covered || tileIcon !== undefined) && <Tile icon={tileIcon} initial={initial} onFail={failed.fail} />}
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">
            {href === undefined ? (
              name
            ) : (
              <a
                href={href}
                className="rounded-sm underline-offset-4 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-cache"
              >
                {name}
              </a>
            )}
            {note !== undefined && <span className="ms-2 font-normal text-faint text-xs">{note}</span>}
          </span>
          {codes !== undefined && <span className="truncate font-mono text-faint text-xs">{codes}</span>}
          {sub !== undefined && <span className="truncate text-faint text-xs">{sub}</span>}
        </div>
        {!covered && badge !== undefined && <span className="flex shrink-0">{badge}</span>}
      </div>
      <div className="flex items-baseline justify-between gap-3">
        <span className="shrink-0 whitespace-nowrap font-mono text-xl tabular-nums">{size}</span>
        <span className="truncate text-muted text-xs">{detail}</span>
      </div>
      <div className="h-1 overflow-hidden rounded-full bg-sunken">
        <div
          className={classes("h-full rounded-full", toneBg[tone])}
          style={{ width: `${pct > 0 ? Math.max(pct, 1.5) : 0}%` }}
        />
      </div>
      {notes.length > 0 && (
        <ul className="-mt-1 flex flex-col gap-1 border-t border-dashed pt-2 text-xs">
          {notes.map((n) => (
            <li key={n.label} title={n.hint ?? n.label} className="flex items-baseline justify-between gap-3">
              <span className="truncate text-muted">{n.label}</span>
              <span className="shrink-0 font-mono text-faint tabular-nums">{n.value}</span>
            </li>
          ))}
        </ul>
      )}
      {(foot.length > 0 || action !== undefined) && (
        <div className="mt-auto flex min-h-6 flex-wrap items-center justify-between gap-x-3 gap-y-2">
          <span className="flex min-w-0 items-center text-faint text-xs">
            {foot.map((f, i) => (
              <span key={f} className={classes("truncate", i > 0 && "ms-2 shrink-0 border-s ps-2")}>
                {f}
              </span>
            ))}
          </span>
          {action !== undefined && <span className="ms-auto flex shrink-0 gap-2">{action}</span>}
        </div>
      )}
    </li>
  )
}

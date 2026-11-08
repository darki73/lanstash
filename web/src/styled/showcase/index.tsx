/**
 * Showcase: a game page's own frame. At the top a tall band: the game's art sharp on the right,
 * fading to the left into the same art blurred across the whole band: the first of the images given
 * that loads (a wide background, else a cover; a band in a hue of the game's own, worked out from
 * its name, when none does); a badge (such as whether its build is current) in its top right corner;
 * at the bottom left its icon (its initials on a tile without one), a line saying where it is from,
 * its name large (its logo instead, where it has one that loads) and a row of facts (a label in small
 * capitals over a figure); the main actions at the bottom right. Its parts follow under the band, rising into place when it opens.
 */
import { type ReactNode, useState } from "react"
import { hueOf, initialsOf } from "@/styled/cards"

export interface Fact {
  readonly label: string
  readonly value: string
}

export interface Art {
  readonly src: string
  readonly cover: "wide" | "poster"
}

export interface ShowcaseProps {
  readonly title: string
  /** Where it is from ("Battle.net · wow"). */
  readonly eyebrow: string
  /** Images to show, the first that loads: a wide one is shown centred, a poster by its upper part. */
  readonly art?: readonly Art[]
  /** The game's logo, shown instead of its name when it loads. */
  readonly logo?: string
  readonly icon?: string
  readonly badge?: ReactNode
  readonly facts?: readonly Fact[]
  readonly actions?: ReactNode
  readonly children?: ReactNode
}

function Picture({ src, className, onFail }: { src: string; className: string; onFail?: () => void }) {
  return <img src={src} alt="" aria-hidden decoding="async" onError={onFail} className={className} />
}

export function Showcase({
  title,
  eyebrow,
  art: images = [],
  logo,
  icon,
  badge,
  facts = [],
  actions,
  children,
}: ShowcaseProps) {
  const [failed, setFailed] = useState<readonly string[]>([])
  const shown = images.find((a) => !failed.includes(a.src))
  const art = shown?.src
  const cover = shown?.cover ?? "wide"
  const sign = logo !== undefined && !failed.includes(logo) ? logo : undefined
  const mark = icon !== undefined && !failed.includes(icon) ? icon : undefined
  const fail = (src: string) => () => setFailed((f) => [...f, src])
  const hue = hueOf(title)
  return (
    <section aria-label={title} className="flex animate-rise flex-col gap-6">
      <header className="relative isolate flex min-h-72 flex-col justify-end overflow-hidden rounded-2xl border bg-sunken shadow-panel lg:min-h-80">
        {art === undefined ? (
          <div
            aria-hidden
            className="-z-20 absolute inset-0"
            style={{
              backgroundImage: `repeating-linear-gradient(135deg, hsl(${hue} 40% 50% / 0.06) 0 2px, transparent 2px 16px), radial-gradient(120% 140% at 85% 10%, hsl(${hue} 50% 45% / 0.38), transparent 60%)`,
            }}
          />
        ) : (
          <>
            <Picture
              src={art}
              className="-z-20 absolute inset-0 size-full scale-110 object-cover opacity-45 blur-3xl"
            />
            <Picture
              src={art}
              onFail={fail(art)}
              className={`-z-10 absolute inset-y-0 right-0 h-full w-full object-cover ${cover === "poster" ? "object-[center_30%]" : "object-center"} [mask-image:linear-gradient(to_left,#000_45%,transparent_95%)] md:w-3/4`}
            />
          </>
        )}
        <div
          aria-hidden
          className="-z-10 absolute inset-0 bg-linear-to-t from-canvas/95 via-canvas/45 to-transparent"
        />
        {badge !== undefined && <span className="absolute top-4 right-4 flex shadow-panel">{badge}</span>}
        <div className="flex flex-col gap-5 p-6 md:flex-row md:items-end md:justify-between">
          <div className="flex min-w-0 items-end gap-4">
            <span
              aria-hidden
              className="flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-xl border bg-raised/80 font-semibold text-text/60 text-xl shadow-panel backdrop-blur-md"
            >
              {mark === undefined ? (
                initialsOf(title)
              ) : (
                <img src={mark} alt="" decoding="async" onError={fail(mark)} className="size-full" />
              )}
            </span>
            <div className="flex min-w-0 flex-col gap-1 [text-shadow:0_1px_14px_rgb(0_0_0/0.5)]">
              <span className="font-mono text-[11px] text-text/75 uppercase tracking-[0.16em]">{eyebrow}</span>
              {sign === undefined ? (
                <h2 className="truncate font-semibold text-3xl tracking-tight lg:text-4xl">{title}</h2>
              ) : (
                <h2 className="py-1">
                  <img
                    src={sign}
                    alt={title}
                    decoding="async"
                    onError={fail(sign)}
                    className="max-h-28 max-w-[min(26rem,70vw)] object-contain object-left drop-shadow-[0_2px_12px_rgb(0_0_0/0.5)] lg:max-h-40"
                  />
                </h2>
              )}
              {facts.length > 0 && (
                <dl className="mt-1 flex flex-wrap gap-x-6 gap-y-2">
                  {facts.map((f) => (
                    <div key={f.label} className="flex flex-col">
                      <dt className="font-mono text-[10px] text-text/60 uppercase tracking-[0.14em]">{f.label}</dt>
                      <dd className="font-mono text-sm tabular-nums">{f.value}</dd>
                    </div>
                  ))}
                </dl>
              )}
            </div>
          </div>
          {actions !== undefined && <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>}
        </div>
      </header>
      {children}
    </section>
  )
}

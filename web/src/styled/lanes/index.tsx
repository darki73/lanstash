/**
 * Lanes: the ways a download can go, one card each, one under the other to compare. A card has
 * its label in small spaced capitals, then its stops (the PC, lanstash, the CDN), each an icon in
 * a rounded square over its name, joined by lines that stretch to fill the card with what travels
 * on them written over them, and at the end of the row what comes of it, in a pill. The lane's
 * tone colours its card's edge, its lines (a dash flowing along them, from one stop to the next),
 * its pill and a marked stop (where the bytes are kept, which glows); a stop the bytes only pass
 * through is dashed. On a narrow screen the card scrolls sideways rather than break the row.
 */
import { Fragment } from "react"
import { classes } from "@/lib/classes"
import type { Tone } from "@/styled/tone"

export type StopIcon = "pc" | "stash" | "cloud"

export interface LaneStop {
  readonly name: string
  readonly icon: StopIcon
  /** What travels from this stop to the next ("HTTPS"); none on the last. */
  readonly via?: string
  /** Where the bytes are kept: marked in the lane's tone. */
  readonly marked?: boolean
  /** The bytes only pass through: dashed. */
  readonly passing?: boolean
}

export interface Lane {
  readonly label: string
  readonly tone: Exclude<Tone, "plain">
  readonly stops: readonly LaneStop[]
  /** What comes of it ("nothing kept"). */
  readonly verdict: string
}

const paths: Readonly<Record<StopIcon, string>> = {
  pc: "M4 5h16v10H4zM9 19h6M12 15v4",
  stash:
    "M5 6c0-1.1 3.1-2 7-2s7 .9 7 2-3.1 2-7 2-7-.9-7-2zM5 6v6c0 1.1 3.1 2 7 2s7-.9 7-2V6M5 12v6c0 1.1 3.1 2 7 2s7-.9 7-2v-6",
  cloud: "M7 18h10a4 4 0 0 0 .6-7.95A6 6 0 0 0 6.1 9.4 4.3 4.3 0 0 0 7 18z",
}

const tones = {
  cache: {
    card: "border-cache/25 bg-cache/[0.04]",
    edge: "bg-cache",
    text: "text-cache",
    pill: "border-cache/45 bg-cache/15 text-cache",
    mark: "border-cache/60 bg-cache/15 text-cache shadow-[0_0_18px_-4px] shadow-cache/50",
  },
  origin: {
    card: "border-origin/25 bg-origin/[0.04]",
    edge: "bg-origin",
    text: "text-origin",
    pill: "border-origin/45 bg-origin/12 text-origin",
    mark: "border-origin/60 bg-origin/15 text-origin",
  },
} as const

function Stop({ stop, tone }: { readonly stop: LaneStop; readonly tone: Lane["tone"] }) {
  return (
    <span className="flex w-24 shrink-0 flex-col items-center gap-1.5">
      <span
        className={classes(
          "flex size-11 items-center justify-center rounded-xl border",
          stop.marked ? tones[tone].mark : "bg-sunken text-text",
          stop.passing && "border-dashed bg-transparent text-muted",
        )}
      >
        <svg
          aria-hidden
          viewBox="0 0 24 24"
          className="size-5"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinejoin="round"
          strokeLinecap="round"
        >
          <path d={paths[stop.icon]} />
        </svg>
      </span>
      <span className="whitespace-nowrap text-center font-medium text-text text-xs">{stop.name}</span>
    </span>
  )
}

function Line({ via, tone }: { readonly via: string; readonly tone: Lane["tone"] }) {
  return (
    <span className="relative flex min-w-16 flex-1 -translate-y-2.5 items-center">
      <span
        className={classes(
          "-translate-x-1/2 absolute start-1/2 bottom-full mb-1 whitespace-nowrap font-mono text-[11px]",
          tones[tone].text,
        )}
      >
        {via}
      </span>
      <span className={classes("flex w-full items-center", tones[tone].text)}>
        <svg aria-hidden className="h-2 min-w-0 flex-1" preserveAspectRatio="none" viewBox="0 0 100 8">
          <line
            x1="0"
            y1="4"
            x2="100"
            y2="4"
            stroke="currentColor"
            strokeOpacity="0.3"
            strokeWidth="2"
            vectorEffect="non-scaling-stroke"
          />
          <line
            x1="0"
            y1="4"
            x2="100"
            y2="4"
            stroke="currentColor"
            strokeWidth="2"
            strokeDasharray="6 10"
            vectorEffect="non-scaling-stroke"
            className="animate-flow"
          />
        </svg>
        <svg aria-hidden viewBox="0 0 8 10" className="-ms-px h-2.5 w-2 shrink-0">
          <path d="M0 0 8 5 0 10z" fill="currentColor" />
        </svg>
      </span>
    </span>
  )
}

export function Lanes({ label, lanes }: { readonly label: string; readonly lanes: readonly Lane[] }) {
  return (
    <ul aria-label={label} className="flex flex-col gap-3">
      {lanes.map((l) => (
        <li
          key={l.label}
          aria-label={l.label}
          className={classes("relative overflow-x-auto rounded-xl border py-3.5 ps-5 pe-4", tones[l.tone].card)}
        >
          <span aria-hidden className={classes("absolute inset-y-3 start-0 w-1 rounded-e-full", tones[l.tone].edge)} />
          <span className="mb-3 block font-mono text-[11px] text-muted uppercase tracking-[0.14em]">{l.label}</span>
          <span className="flex min-w-[36rem] items-center gap-2">
            {l.stops.map((s, i) => (
              <Fragment key={s.name}>
                <Stop stop={s} tone={l.tone} />
                {i < l.stops.length - 1 && <Line via={s.via ?? ""} tone={l.tone} />}
              </Fragment>
            ))}
            <span
              className={classes(
                "ms-3 w-36 shrink-0 -translate-y-2.5 whitespace-nowrap rounded-full border py-1 text-center font-mono text-xs",
                tones[l.tone].pill,
              )}
            >
              {l.verdict}
            </span>
          </span>
        </li>
      ))}
    </ul>
  )
}

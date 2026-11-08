/**
 * Chart: traces over a fixed window, the newest value at the right edge, the way a system monitor
 * scrolls. A trace is an area (filled under the line), a line, or a dashed line, in a tone. The
 * scale runs from 0 to the highest value shown (at least 1), so a quiet window lies flat at the
 * bottom; given a lower top (see capped), the values above it are drawn cut at the top edge, each
 * with a mark there, so one burst does not flatten the rest. The large chart draws a square grid
 * behind the traces and its scale and window in the corners; the medium one (a live strip) the
 * same, lower; the small one (a tile's) only the traces.
 */
import { classes } from "@/lib/classes"
import { type Tone, toneText } from "@/styled/tone"

export interface Trace {
  readonly values: readonly number[]
  readonly tone: Tone
  readonly style?: "area" | "line" | "dashed"
}

export interface ChartProps {
  readonly traces: readonly Trace[]
  /** How many values the window holds; fewer values start part way along. */
  readonly window: number
  /** What the chart shows, for assistive technology. */
  readonly label: string
  readonly size?: "large" | "medium" | "small"
  /** The top of the scale; the highest value shown when none is given. */
  readonly top?: number
  /** Words for the top of the scale (large and medium): what the highest value is. */
  readonly scale?: string
  /** Words for the window (large and medium), such as "2 minutes". */
  readonly span?: string
}

const width = 1000
const heights = { large: 280, medium: 120, small: 100 } as const
const plotSizes = { large: "h-56 sm:h-72", medium: "h-24 sm:h-28", small: "" } as const
const columns = 20

/** The share of the height kept under 0, so a quiet trace shows as a line along the bottom. */
const floor = 0.03

/** The points of a trace: x from the right edge, y from the bottom, in the drawing's units. */
export function points(values: readonly number[], window: number, top: number, height: number): [number, number][] {
  const step = width / Math.max(1, window - 1)
  const shown = values.slice(-window)
  const usable = height * (1 - floor)
  return shown.map((v, i) => [
    width - (shown.length - 1 - i) * step,
    usable - (Math.min(top, Math.max(0, v)) / top) * usable,
  ])
}

/**
 * A top for values one burst would flatten: when the highest is more than six times the 75th
 * percentile of the values above 0 (with at least eight of them; a burst spans several buckets,
 * so a higher percentile lands in it), a top of three times that percentile, but never under a
 * quarter of the highest; otherwise undefined (the highest value is the top).
 */
export function capped(values: readonly number[]): number | undefined {
  const above = values.filter((v) => v > 0).sort((a, b) => a - b)
  if (above.length < 8) return undefined
  const p75 = above[Math.floor(0.75 * (above.length - 1))] ?? 0
  const highest = above[above.length - 1] ?? 0
  return p75 > 0 && highest > 6 * p75 ? Math.max(3 * p75, highest / 4) : undefined
}

function line(p: readonly [number, number][]): string {
  return p.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`).join("")
}

export function Chart({ traces, window, label, size = "large", top, scale, span }: ChartProps) {
  const height = heights[size]
  const highest = Math.max(1, ...traces.flatMap((t) => t.values.slice(-window)))
  const cut = top !== undefined && top < highest
  const ceiling = cut ? top : highest * 1.08
  const framed = size !== "small"
  const rows = size === "large" ? 6 : 3
  const plot = (
    <div
      className={classes(
        "relative overflow-hidden",
        framed && "rounded-lg border bg-sunken/60",
        plotSizes[size],
        !framed && "h-full w-full",
      )}
    >
      <svg
        role="img"
        aria-label={label}
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        className="absolute inset-0 h-full w-full"
      >
        {framed && (
          <g className="text-faint" stroke="currentColor" strokeOpacity={0.18}>
            {Array.from({ length: columns - 1 }, (_, i) => (i + 1) * (width / columns)).map((x) => (
              <line key={`x${x}`} x1={x} x2={x} y1={0} y2={height} vectorEffect="non-scaling-stroke" />
            ))}
            {Array.from({ length: rows - 1 }, (_, i) => (i + 1) * (height / rows)).map((y) => (
              <line key={`y${y}`} x1={0} x2={width} y1={y} y2={y} vectorEffect="non-scaling-stroke" />
            ))}
          </g>
        )}
        {traces.map((t, i) => {
          const p = points(t.values, window, ceiling, height)
          if (p.length === 0) return null
          const first = p[0]
          const last = p[p.length - 1]
          const d = line(p.length === 1 && first ? [[first[0] - 1, first[1]], first] : p)
          const style = t.style ?? "area"
          const over = cut ? t.values.slice(-window).flatMap((v, j) => (v > ceiling ? [p[j]?.[0] ?? 0] : [])) : []
          return (
            // Traces are fixed per chart, so their place is their identity.
            // biome-ignore lint/suspicious/noArrayIndexKey: see above
            <g key={i} className={toneText[t.tone]}>
              {style === "area" && first && last && (
                <path d={`${d}L${last[0]},${height}L${first[0]},${height}Z`} fill="currentColor" fillOpacity={0.14} />
              )}
              <path
                d={d}
                fill="none"
                stroke="currentColor"
                strokeWidth={framed ? 1.75 : 1.5}
                strokeDasharray={style === "dashed" ? "5 4" : undefined}
                strokeLinejoin="round"
                vectorEffect="non-scaling-stroke"
              />
              {over.map((x) => (
                <line
                  key={`c${x}`}
                  data-cut
                  x1={x}
                  x2={x}
                  y1={0}
                  y2={height * 0.06}
                  stroke="currentColor"
                  strokeWidth={4}
                  vectorEffect="non-scaling-stroke"
                />
              ))}
            </g>
          )
        })}
      </svg>
    </div>
  )
  if (!framed) return plot
  // The scale above the plot and the window under it, outside the traces' way.
  const edge = "flex justify-between gap-4 font-mono text-[11px] text-faint tabular-nums"
  return (
    <figure className="flex flex-col gap-1.5">
      <div className={edge}>
        <span>{label}</span>
        <span>{scale ?? ""}</span>
      </div>
      {plot}
      <div className={edge}>
        <span>{span === undefined ? "" : `${span} ago`}</span>
        <span>{span === undefined ? "" : "now"}</span>
      </div>
    </figure>
  )
}

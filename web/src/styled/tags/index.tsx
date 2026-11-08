/**
 * Tags: short names (host names) as small monospace pills that wrap, each in a tone: alert (cyan, the
 * internet's colour: what goes around the cache), warn (amber outline: there twice) or quiet (muted:
 * left as it is). A tag may carry a title for hover.
 */
import { classes } from "@/lib/classes"

export type TagTone = "alert" | "warn" | "quiet"

export interface Tag {
  readonly text: string
  readonly tone: TagTone
  readonly title?: string
}

const tones: Readonly<Record<TagTone, string>> = {
  alert: "border-origin/40 bg-origin/10 text-origin",
  warn: "border-cache/50 text-cache",
  quiet: "bg-sunken text-muted",
}

export function Tags({ items }: { readonly items: readonly Tag[] }) {
  return (
    <ul className="flex flex-wrap gap-1.5">
      {items.map((t) => (
        <li
          key={`${t.tone}:${t.text}`}
          title={t.title}
          className={classes("rounded-md border px-2 py-0.5 font-mono text-[11px] leading-5", tones[t.tone])}
        >
          {t.text}
        </li>
      ))}
    </ul>
  )
}

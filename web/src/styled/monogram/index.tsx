/**
 * Monogram: a small rounded square standing for something: its icon when it has one (a game's), or
 * one or two letters for something with no icon of its own here (a platform), and the letters again
 * when the icon fails to load. It is decoration: the name is always written beside it.
 */
import { useState } from "react"

export interface MonogramProps {
  readonly letters: string
  /** Address of its square icon. */
  readonly image?: string
}

export function Monogram({ letters, image }: MonogramProps) {
  const [failed, setFailed] = useState<string | undefined>(undefined)
  if (image !== undefined && failed !== image) {
    return (
      <span aria-hidden className="inline-flex size-8 shrink-0 overflow-hidden rounded-lg border bg-sunken">
        <img src={image} alt="" decoding="async" onError={() => setFailed(image)} className="size-full" />
      </span>
    )
  }
  return (
    <span
      aria-hidden
      className="inline-flex size-6 shrink-0 items-center justify-center rounded-md border bg-sunken font-mono font-semibold text-[11px] text-muted"
    >
      {letters}
    </span>
  )
}

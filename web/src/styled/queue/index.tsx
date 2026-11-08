/**
 * Queue: what waits its turn, as a launcher lists it: a heading with the count after it and a rule
 * running to the end of the line (a note may sit at its end), then a row per item: a wide thumbnail
 * of its art (its initials on a plain tile without art), its name (a link when it has a page) with a
 * detail under it in small monospace (its size), and its actions on the right. Without items a quiet
 * line says so.
 */
import { type ReactNode, useState } from "react"

export interface QueueProps {
  readonly title: string
  readonly count: number
  /** At the end of the heading's rule. */
  readonly note?: string
  /** What to say without items. */
  readonly empty: string
  readonly children?: ReactNode
}

export function Queue({ title, count, note, empty, children }: QueueProps) {
  return (
    <section aria-label={title} className="flex flex-col gap-3">
      <div className="flex items-center gap-3">
        <h3 className="shrink-0 font-semibold text-lg tracking-tight">
          {title} <span className="font-normal text-faint">({count})</span>
        </h3>
        <div aria-hidden className="h-px flex-1 bg-line" />
        {note !== undefined && <span className="shrink-0 text-faint text-xs">{note}</span>}
      </div>
      {count === 0 ? (
        <p className="px-1 text-muted text-sm">{empty}</p>
      ) : (
        <ul className="flex flex-col gap-2">{children}</ul>
      )}
    </section>
  )
}

export interface QueueItemProps {
  readonly name: string
  readonly href?: string
  readonly image?: string
  readonly detail?: string
  readonly actions?: ReactNode
}

export function QueueItem({ name, href, image, detail, actions }: QueueItemProps) {
  const [failed, setFailed] = useState<string>()
  const art = image !== undefined && failed !== image ? image : undefined
  return (
    <li className="flex items-center gap-4 rounded-lg border bg-raised p-2 pr-3 shadow-panel">
      <div className="flex aspect-[46/21] w-32 shrink-0 items-center justify-center overflow-hidden rounded-md bg-sunken">
        {art === undefined ? (
          <span aria-hidden className="font-semibold text-faint text-lg">
            {name.slice(0, 2)}
          </span>
        ) : (
          <img src={art} alt="" decoding="async" onError={() => setFailed(art)} className="size-full object-cover" />
        )}
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        {href === undefined ? (
          <span className="truncate font-medium">{name}</span>
        ) : (
          <a
            href={href}
            className="truncate font-medium hover:text-cache focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cache"
          >
            {name}
          </a>
        )}
        {detail !== undefined && <span className="font-mono text-faint text-xs tabular-nums">{detail}</span>}
      </div>
      {actions !== undefined && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </li>
  )
}

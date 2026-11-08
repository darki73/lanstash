/**
 * Palette: a box to go anywhere in the app by typing, opened from anywhere. Its button (a search
 * field's look, with the keys that open it; on a narrow screen only its magnifier) sits in the header; Ctrl+K (⌘K on a Mac) opens and
 * closes it, and so does / where nothing else takes typing. It opens over the whole app, near the top:
 * a field, and under it what answers what was typed, in groups (pages, games, …), each a row with
 * an icon or a thumbnail, its title, a muted word after it and a figure at its end. ↑ and ↓ move
 * through the rows (round from the last to the first), the pointer too; Enter or a click goes
 * where the row leads, as its link (the app follows its own links); Escape, a click beside the box
 * or going somewhere closes it, and the focus goes back where it was. What answers is the caller's
 * to say: the palette asks for it as one types.
 */
import { type ReactNode, useEffect, useId, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { useT } from "@/i18n"
import { classes } from "@/lib/classes"

export interface PaletteItem {
  /** Unique within the palette. */
  readonly key: string
  readonly title: string
  /** A muted word after the title, such as its platform. */
  readonly sub?: string
  /** A figure at the end of the row, such as its size. */
  readonly meta?: string
  /** Where the row leads: a path of the app. */
  readonly href: string
  /** An icon for the row, when it has no image. */
  readonly icon?: ReactNode
  /** A thumbnail for the row: a wide banner, or a square icon. */
  readonly image?: string
  readonly square?: boolean
}

export interface PaletteGroup {
  readonly title: string
  readonly items: readonly PaletteItem[]
}

export interface PaletteProps {
  /** What the palette is, for assistive technology and its button ("Go to"). */
  readonly label: string
  readonly placeholder: string
  /** What answers what was typed ("" when nothing is), in groups. */
  readonly find: (typed: string) => readonly PaletteGroup[]
  /** Said when nothing answers. */
  readonly none?: string
}

/** Whether typing goes to the element: a field, an editable one, a dropdown. */
function takesTyping(el: Element | null): boolean {
  if (el === null) return false
  const tag = el.tagName
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || (el as HTMLElement).isContentEditable === true
}

function Keys({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded border bg-canvas px-1.5 py-px font-mono text-[10px] text-faint leading-4">{children}</kbd>
  )
}

function Thumb({ item }: { item: PaletteItem }) {
  const [failed, setFailed] = useState(false)
  if (item.image !== undefined && !failed) {
    return (
      <span aria-hidden className="flex h-7 w-12 shrink-0 overflow-hidden rounded border bg-sunken">
        <img
          src={item.image}
          alt=""
          loading="lazy"
          decoding="async"
          onError={() => setFailed(true)}
          className={classes("size-full", item.square ? "object-contain p-0.5" : "object-cover")}
        />
      </span>
    )
  }
  return (
    <span aria-hidden className="flex h-7 w-12 shrink-0 items-center justify-center">
      {item.icon}
    </span>
  )
}

const mac = () => typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform)

export function Palette({ label, placeholder, find, none }: PaletteProps) {
  const t = useT("common")
  const [open, setOpen] = useState(false)
  const [typed, setTyped] = useState("")
  const [active, setActive] = useState(0)
  const back = useRef<Element | null>(null)
  const list = useRef<HTMLDivElement>(null)
  const id = useId()
  const show = () => {
    back.current = document.activeElement
    setTyped("")
    setActive(0)
    setOpen(true)
  }
  const close = () => {
    setOpen(false)
    const el = back.current
    if (el instanceof HTMLElement && el.isConnected) el.focus()
  }
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "k") {
        e.preventDefault()
        if (open) close()
        else show()
      } else if (!open && e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey && !takesTyping(e.target as Element)) {
        e.preventDefault()
        show()
      }
    }
    document.addEventListener("keydown", key)
    return () => document.removeEventListener("keydown", key)
  })
  const groups = open ? find(typed).filter((g) => g.items.length > 0) : []
  const rows = groups.flatMap((g) => g.items)
  const at = rows.length === 0 ? -1 : Math.min(active, rows.length - 1)
  const optionId = (i: number) => `${id}-${i}`
  useEffect(() => {
    if (at < 0) return
    const el = list.current?.querySelector<HTMLElement>(`[id="${optionId(at)}"]`)
    el?.scrollIntoView?.({ block: "nearest" })
  })
  const move = (by: number) => setActive(rows.length === 0 ? 0 : (at + by + rows.length) % rows.length)
  const go = () => {
    if (at < 0) return
    list.current?.querySelector<HTMLElement>(`[id="${optionId(at)}"]`)?.click()
  }
  let n = -1
  return (
    <>
      <button
        type="button"
        onClick={show}
        aria-label={label}
        aria-keyshortcuts="Control+K /"
        className="flex cursor-pointer items-center gap-2 rounded-md border bg-sunken py-1 ps-2.5 pe-1.5 text-faint text-xs transition-colors hover:text-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cache"
      >
        <svg aria-hidden viewBox="0 0 24 24" className="size-3.5">
          <path
            d="M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13ZM15.5 15.5 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </svg>
        <span className="hidden items-center gap-2 sm:flex">
          {placeholder}
          <Keys>{mac() ? "⌘K" : "Ctrl K"}</Keys>
        </span>
      </button>
      {open &&
        createPortal(
          <div className="fixed inset-0 z-50 flex items-start justify-center px-4 pt-[12vh]">
            <button
              type="button"
              tabIndex={-1}
              aria-label={t("ui.close")}
              onClick={close}
              className="absolute inset-0 cursor-default bg-canvas/70 backdrop-blur-sm"
            />
            <div
              role="dialog"
              aria-modal="true"
              aria-label={label}
              className="relative flex max-h-[70vh] w-full max-w-xl animate-rise flex-col overflow-hidden rounded-xl border bg-raised shadow-panel"
            >
              <div className="flex items-center gap-3 border-b px-4">
                <svg aria-hidden viewBox="0 0 24 24" className="size-4 shrink-0 text-faint">
                  <path
                    d="M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13ZM15.5 15.5 20 20"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                  />
                </svg>
                <input
                  // biome-ignore lint/a11y/noAutofocus: the palette opens to be typed in
                  autoFocus
                  role="combobox"
                  aria-expanded="true"
                  aria-controls={`${id}-list`}
                  aria-activedescendant={at < 0 ? undefined : optionId(at)}
                  aria-label={label}
                  value={typed}
                  placeholder={placeholder}
                  spellCheck={false}
                  onChange={(e) => {
                    setTyped(e.target.value)
                    setActive(0)
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "ArrowDown") move(1)
                    else if (e.key === "ArrowUp") move(-1)
                    else if (e.key === "Enter") go()
                    else if (e.key === "Escape") close()
                    else return
                    e.preventDefault()
                  }}
                  className="h-12 flex-1 bg-transparent text-sm text-text outline-none placeholder:text-faint"
                />
                <Keys>{t("ui.escape")}</Keys>
              </div>
              <div ref={list} id={`${id}-list`} role="listbox" aria-label={label} className="overflow-y-auto p-2">
                {rows.length === 0 && (
                  <p className="px-3 py-6 text-center text-faint text-sm">{none ?? t("ui.nothing")}</p>
                )}
                {groups.map((g) => (
                  // biome-ignore lint/a11y/useSemanticElements: a listbox's options are grouped by role="group"; a fieldset is for form fields
                  <div key={g.title} role="group" aria-label={g.title} className="pb-1">
                    <div className="px-3 pt-2 pb-1 font-mono text-[10px] text-faint uppercase tracking-[0.14em]">
                      {g.title}
                    </div>
                    {g.items.map((item) => {
                      n += 1
                      const i = n
                      return (
                        <a
                          key={item.key}
                          id={optionId(i)}
                          href={item.href}
                          role="option"
                          aria-selected={i === at}
                          tabIndex={-1}
                          onMouseMove={() => i !== at && setActive(i)}
                          onClick={() => setOpen(false)}
                          className={classes(
                            "flex items-center gap-3 rounded-md px-3 py-1.5 text-sm",
                            i === at ? "bg-hover text-text" : "text-muted",
                          )}
                        >
                          <Thumb item={item} />
                          <span className="min-w-0 flex-1 truncate">
                            {item.title}
                            {item.sub !== undefined && <span className="ms-2 text-faint text-xs">{item.sub}</span>}
                          </span>
                          {item.meta !== undefined && (
                            <span className="shrink-0 font-mono text-faint text-xs tabular-nums">{item.meta}</span>
                          )}
                          {i === at && <span className="shrink-0 text-cache text-xs">↵</span>}
                        </a>
                      )
                    })}
                  </div>
                ))}
              </div>
              <div className="flex items-center gap-4 border-t px-4 py-2 text-[11px] text-faint">
                <span className="flex items-center gap-1.5">
                  <Keys>↑</Keys>
                  <Keys>↓</Keys> {t("ui.move")}
                </span>
                <span className="flex items-center gap-1.5">
                  <Keys>↵</Keys> {t("ui.open")}
                </span>
                <span className="flex items-center gap-1.5">
                  <Keys>/</Keys> {t("ui.or")} <Keys>{mac() ? "⌘K" : "Ctrl K"}</Keys> {t("ui.fromAnywhere")}
                </span>
              </div>
            </div>
          </div>,
          document.body,
        )}
    </>
  )
}

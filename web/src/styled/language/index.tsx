/**
 * Language: the choice of language at the sidebar's foot, a menu however many languages there are.
 * A button shows a globe and the language in use by its own name; it opens, upwards, the list of
 * languages, each by its own name with its code beside it, the one in use checked and every one
 * marked as that language. Arrow keys move through the list, Enter or a click chooses, Escape or a
 * click elsewhere closes it.
 */
import { useEffect, useId, useRef, useState } from "react"
import { classes } from "@/lib/classes"

export interface LanguageOption {
  readonly code: string
  /** Its own name for itself ("Русский"). */
  readonly name: string
}

export interface LanguageProps {
  /** The word for "language", in the language in use. */
  readonly label: string
  readonly options: readonly LanguageOption[]
  readonly value: string
  readonly onChange: (code: string) => void
}

function Globe() {
  return (
    <svg viewBox="0 0 16 16" aria-hidden="true" className="size-3.5 shrink-0" fill="none" stroke="currentColor">
      <circle cx="8" cy="8" r="6.25" strokeWidth="1.25" />
      <path
        d="M1.75 8h12.5M8 1.75c1.9 1.7 2.85 3.8 2.85 6.25S9.9 12.55 8 14.25C6.1 12.55 5.15 10.45 5.15 8S6.1 3.45 8 1.75Z"
        strokeWidth="1.25"
      />
    </svg>
  )
}

function Chevron({ open }: { readonly open: boolean }) {
  return (
    <svg
      viewBox="0 0 16 16"
      aria-hidden="true"
      className={classes("size-3 shrink-0 transition-transform", open && "rotate-180")}
      fill="none"
      stroke="currentColor"
    >
      <path d="M4 10l4-4 4 4" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

function Check() {
  return (
    <svg viewBox="0 0 16 16" aria-hidden="true" className="size-3.5 shrink-0" fill="none" stroke="currentColor">
      <path d="M3.5 8.5l3 3 6-7" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

export function Language({ label, options, value, onChange }: LanguageProps) {
  const [open, setOpen] = useState(false)
  const [at, setAt] = useState(0)
  const box = useRef<HTMLDivElement>(null)
  const list = useId()
  const current = options.find((o) => o.code === value) ?? options[0]
  // A click elsewhere closes the list.
  useEffect(() => {
    if (!open) return
    const away = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener("mousedown", away)
    return () => document.removeEventListener("mousedown", away)
  }, [open])
  const show = () => {
    setAt(
      Math.max(
        0,
        options.findIndex((o) => o.code === value),
      ),
    )
    setOpen(true)
  }
  const choose = (code: string) => {
    setOpen(false)
    if (code !== value) onChange(code)
  }
  const keys = (e: React.KeyboardEvent) => {
    if (!open) {
      if (["ArrowUp", "ArrowDown", "Enter", " "].includes(e.key)) {
        e.preventDefault()
        show()
      }
      return
    }
    if (e.key === "Escape") setOpen(false)
    else if (e.key === "ArrowDown") setAt((i) => Math.min(options.length - 1, i + 1))
    else if (e.key === "ArrowUp") setAt((i) => Math.max(0, i - 1))
    else if (e.key === "Enter" || e.key === " ") {
      const o = options[at]
      if (o) choose(o.code)
    } else return
    e.preventDefault()
  }
  return (
    <div ref={box} className="relative">
      <button
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? list : undefined}
        aria-activedescendant={open ? `${list}-${at}` : undefined}
        onKeyDown={keys}
        aria-label={`${label}: ${current?.name ?? value}`}
        onClick={() => (open ? setOpen(false) : show())}
        className={classes(
          "flex w-full cursor-pointer items-center gap-2 rounded-md border bg-sunken px-2.5 py-1.5 text-xs transition-colors",
          open ? "border-accent text-text" : "text-muted hover:text-text",
        )}
      >
        <Globe />
        <span className="flex-1 text-left" lang={current?.code}>
          {current?.name ?? value}
        </span>
        <Chevron open={open} />
      </button>
      {open && (
        <div
          id={list}
          role="listbox"
          aria-label={label}
          className="absolute bottom-full left-0 z-20 mb-1.5 max-h-72 w-full overflow-y-auto rounded-md border bg-raised p-1 shadow-panel"
        >
          {options.map((o, i) => (
            <div
              key={o.code}
              id={`${list}-${i}`}
              role="option"
              tabIndex={-1}
              aria-selected={o.code === value}
              lang={o.code}
              onMouseEnter={() => setAt(i)}
              onClick={() => choose(o.code)}
              onKeyDown={keys}
              className={classes(
                "flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-xs",
                i === at ? "bg-sunken text-text" : "text-muted",
              )}
            >
              <span className="flex-1">{o.name}</span>
              <span className="font-mono text-[10px] uppercase tracking-wide opacity-70">{o.code}</span>
              <span className={classes("text-origin", o.code !== value && "invisible")}>
                <Check />
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

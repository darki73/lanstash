/**
 * Field: one setting in a form. On the left its label, a badge after it (where its value comes
 * from) and a line of help under it; on the right its controls. On a narrow screen the controls go
 * under the words. TextInput is the box a value is typed in (monospace; Enter submits, Escape gives
 * up the edit; values can be offered as one types; a secret one hides what is typed); Select is a
 * dropdown of values; Many picks
 * several values of a list (pills to drop, a dropdown to add).
 */
import { type ReactNode, useId } from "react"
import { useT } from "@/i18n"

export interface FieldProps {
  readonly label: string
  readonly help?: string
  readonly badge?: ReactNode
  readonly children?: ReactNode
}

export function Field({ label, help, badge, children }: FieldProps) {
  return (
    <div className="flex flex-col gap-3 border-b py-4 last:border-b-0 md:flex-row md:items-center md:justify-between md:gap-8">
      <div className="flex min-w-0 flex-col gap-1">
        <span className="flex items-center gap-2 font-medium text-sm">
          {label}
          {badge}
        </span>
        {help !== undefined && <span className="text-faint text-xs leading-relaxed">{help}</span>}
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-2">{children}</div>
    </div>
  )
}

export interface TextInputProps {
  readonly label: string
  readonly value: string
  readonly onChange: (value: string) => void
  readonly onEnter?: () => void
  readonly onEscape?: () => void
  readonly placeholder?: string
  /** Values offered as one types. */
  readonly options?: readonly string[]
  /** What is typed is hidden (a password), and never offered back by the browser as one types. */
  readonly secret?: boolean
}

export function TextInput({ label, value, onChange, onEnter, onEscape, placeholder, options, secret }: TextInputProps) {
  const list = useId()
  return (
    <>
      {options !== undefined && (
        <datalist id={list}>
          {options.map((o) => (
            <option key={o} value={o} />
          ))}
        </datalist>
      )}
      <Input
        label={label}
        value={value}
        onChange={onChange}
        onEnter={onEnter}
        onEscape={onEscape}
        placeholder={placeholder}
        list={options === undefined ? undefined : list}
        secret={secret === true}
      />
    </>
  )
}

export interface SelectProps {
  readonly label: string
  readonly value: string
  readonly options: readonly string[]
  readonly onChange: (value: string) => void
  /** The words each value is shown as (the value itself when it has none). */
  readonly names?: Readonly<Record<string, string>>
}

/** A dropdown of values. */
export function Select({ label, value, options, onChange, names }: SelectProps) {
  return (
    <select
      aria-label={label}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="rounded-md border bg-sunken px-2.5 py-1.5 font-mono text-sm text-text outline-none focus:border-accent"
    >
      {options.map((o) => (
        <option key={o} value={o}>
          {names?.[o] ?? o}
        </option>
      ))}
    </select>
  )
}

export interface ManyProps {
  readonly label: string
  /** The values picked, in order. */
  readonly values: readonly string[]
  readonly options: readonly string[]
  readonly onChange: (values: readonly string[]) => void
}

/**
 * Several values of a list: each picked one a pill with × to drop it (not the last one), then a
 * dropdown that adds one of the others.
 */
export function Many({ label, values, options, onChange }: ManyProps) {
  const rest = options.filter((o) => !values.includes(o))
  const t = useT("common")
  return (
    <>
      {values.map((v) => (
        <span
          key={v}
          className="inline-flex items-center gap-1 rounded-full border bg-sunken py-0.5 ps-2.5 pe-1 font-mono text-sm"
        >
          {v}
          {values.length > 1 && (
            <button
              type="button"
              aria-label={t("ui.dropFrom", { value: v, list: label })}
              onClick={() => onChange(values.filter((x) => x !== v))}
              className="cursor-pointer rounded-full px-1 text-faint hover:bg-hover hover:text-text"
            >
              ×
            </button>
          )}
        </span>
      ))}
      {rest.length > 0 && (
        <select
          aria-label={t("ui.addTo", { list: label })}
          value=""
          onChange={(e) => e.target.value !== "" && onChange([...values, e.target.value])}
          className="rounded-md border bg-sunken px-2.5 py-1.5 text-faint text-sm outline-none focus:border-accent"
        >
          <option value="">{t("ui.add")}</option>
          {rest.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
      )}
    </>
  )
}

function Input({
  label,
  value,
  onChange,
  onEnter,
  onEscape,
  placeholder,
  list,
  secret,
}: Omit<TextInputProps, "options" | "secret"> & { readonly list: string | undefined; readonly secret: boolean }) {
  return (
    <input
      type={secret ? "password" : "text"}
      autoComplete={secret ? "new-password" : undefined}
      aria-label={label}
      value={value}
      placeholder={placeholder}
      list={list}
      spellCheck={false}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") onEnter?.()
        if (e.key === "Escape") onEscape?.()
      }}
      className="w-56 rounded-md border bg-sunken px-3 py-1.5 font-mono text-sm text-text outline-none placeholder:text-faint focus:border-accent"
    />
  )
}

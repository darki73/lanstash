/**
 * Checkbox: a box to tick, with a label for assistive technology (the words beside it are the
 * caller's). It can be partly ticked (some of what it stands for is chosen): it shows a dash then,
 * and a click ticks it.
 */
import { useEffect, useRef } from "react"

export interface CheckboxProps {
  readonly checked: boolean
  /** Some, not all, of what it stands for is chosen. */
  readonly mixed?: boolean
  readonly label: string
  readonly onChange: (checked: boolean) => void
}

export function Checkbox({ checked, mixed = false, label, onChange }: CheckboxProps) {
  const box = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (box.current) box.current.indeterminate = mixed && !checked
  }, [mixed, checked])
  return (
    <input
      ref={box}
      type="checkbox"
      aria-label={label}
      checked={checked}
      onChange={(e) => onChange(e.target.checked)}
      className="size-4 cursor-pointer accent-cache"
    />
  )
}

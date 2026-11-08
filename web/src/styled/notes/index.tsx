/**
 * Notes: a short list of facts in plain words, each after a small marker.
 */

export function Notes({ items }: { readonly items: readonly string[] }) {
  return (
    <ul className="flex flex-col gap-2 text-muted">
      {items.map((item) => (
        <li key={item} className="flex gap-2.5">
          <span aria-hidden className="mt-2 size-1 shrink-0 rounded-full bg-faint" />
          <span>{item}</span>
        </li>
      ))}
    </ul>
  )
}

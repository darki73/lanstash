/**
 * Glyph: the small line icons of the sidebar's links, drawn in the text colour. Decoration only:
 * the link's words are always beside it.
 *   overview: a pulse over a baseline, for the monitor;
 *   settings: two sliders, for the settings;
 *   dns: a sign post with two arms pointing different ways, for the DNS server;
 *   history: a clock face whose arrow turns back, for what was updated when;
 *   health: a shield with a tick, for the checks lanstash makes of itself;
 *   clients: a screen on its stand, for the PCs that download through the cache;
 *   accounts: two heads and shoulders, for lanstash's own accounts.
 */
export type GlyphName = "overview" | "settings" | "dns" | "history" | "health" | "clients" | "accounts"

const paths: Readonly<Record<GlyphName, string>> = {
  overview: "M3 13h3.5l2.5-6 4 11 2.5-7 1.5 2H21",
  settings: "M4 8h9M17 8h3M15 5.5v5M4 16h3M11 16h9M9 13.5v5",
  dns: "M12 3v18M12 6h6l2 2-2 2h-6M12 12H6l-2 2 2 2h6",
  history: "M4 12a8 8 0 1 0 2.3-5.7M4 4v3h3M12 8v4l3 2",
  health: "M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6zM9 12l2 2 4-4",
  clients: "M4 5h16v10H4zM12 15v4M8 19h8",
  accounts:
    "M9 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM3.5 19c.5-3 2.7-5 5.5-5s5 2 5.5 5M16 11a2.5 2.5 0 1 0 0-5M17.5 14c1.8.5 3 2.2 3.3 4.5",
}

export function Glyph({ name }: { readonly name: GlyphName }) {
  return (
    <span aria-hidden className="inline-flex size-6 shrink-0 items-center justify-center rounded-md border bg-sunken">
      <svg aria-hidden viewBox="0 0 24 24" className="size-4 text-muted">
        <path
          d={paths[name]}
          fill="none"
          stroke="currentColor"
          strokeWidth="1.75"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  )
}

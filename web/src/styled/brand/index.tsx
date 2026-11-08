/**
 * Brand: a platform's mark, white on a tile of the platform's colour. The marks come from Simple
 * Icons (simple-icons, CC0-1.0; the marks themselves are their owners' trademarks, shown here to
 * say which platform a row is about). Steam's and Epic's own colours are near black, which would
 * vanish on the dark theme, so their tiles take the dark of their clients instead. It is
 * decoration: the platform's name is always written beside it.
 */
import { type SimpleIcon, siBattledotnet, siEpicgames, siRiotgames, siSteam } from "simple-icons"

export type BrandName = "battlenet" | "steam" | "epicgames" | "riotgames"

const marks: Readonly<Record<BrandName, { readonly icon: SimpleIcon; readonly tile: string }>> = {
  battlenet: { icon: siBattledotnet, tile: `#${siBattledotnet.hex}` },
  steam: { icon: siSteam, tile: "#171A21" },
  epicgames: { icon: siEpicgames, tile: "#2A2A2A" },
  riotgames: { icon: siRiotgames, tile: `#${siRiotgames.hex}` },
}

/** The tile colour of a brand, for tests and for anything drawn beside the mark. */
export const brandTile = (name: BrandName) => marks[name].tile

export function Brand({ name, size = "small" }: { readonly name: BrandName; readonly size?: "small" | "large" }) {
  const { icon, tile } = marks[name]
  return (
    <span
      aria-hidden
      className={
        size === "large"
          ? "inline-flex size-11 shrink-0 items-center justify-center rounded-xl shadow-panel"
          : "inline-flex size-6 shrink-0 items-center justify-center rounded-md"
      }
      style={{ backgroundColor: tile }}
    >
      <svg aria-hidden viewBox="0 0 24 24" className={size === "large" ? "size-6" : "size-3.5"} fill="#FFFFFF">
        <path d={icon.path} />
      </svg>
    </span>
  )
}

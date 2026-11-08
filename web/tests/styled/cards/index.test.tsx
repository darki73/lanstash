import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { CardGrid, GameCard, hueOf, initialsOf } from "@/styled/cards"

describe("styled/cards", () => {
  it("draws a card with the game's initial, name, codes, size, detail and bar", () => {
    const { container } = render(
      <CardGrid>
        <GameCard
          name="Diablo II: Resurrected"
          codes="osi"
          size="29.62 GB"
          detail="669 objects"
          fraction={1}
          tone="cache"
        />
        <GameCard name="drtl" note="no name yet" size="830.8 MB" detail="23 objects" fraction={0} tone="cache" />
      </CardGrid>,
    )
    expect(screen.getAllByRole("listitem")).toHaveLength(2)
    expect(screen.getByText("osi")).toBeTruthy()
    expect(screen.getByText("no name yet")).toBeTruthy()
    expect(screen.getAllByText("D")).toHaveLength(2)
    const bars = [...container.querySelectorAll<HTMLElement>("li > div:last-child > div")].map((b) => b.style.width)
    expect(bars).toEqual(["100%", "0%"])
  })
  it("shows the cover image as a banner, and the initial again when the image fails", () => {
    const { container } = render(
      <GameCard name="Deadlock" image="/art/1" size="6.54 GB" detail="7 depots" fraction={1} tone="cache" />,
    )
    const img = container.querySelector("img")
    expect(img?.getAttribute("src")).toBe("/art/1")
    expect(img?.getAttribute("loading")).toBe("lazy")
    expect(img?.className).toContain("opacity-0")
    if (img) fireEvent.load(img)
    expect(container.querySelector("img")?.className).toContain("opacity-100")
    if (img) fireEvent.error(img)
    expect(container.querySelector("img")).toBeNull()
    expect(screen.getByText("D")).toBeTruthy()
  })
  it("puts an icon on the tile, and the initial back when the icon fails", () => {
    const { container } = render(
      <GameCard name="Heroes" icon="/icon/h" size="15 GB" detail="1 object" fraction={1} tone="cache" />,
    )
    const icon = container.querySelector("img")
    expect(icon?.getAttribute("src")).toBe("/icon/h")
    expect(screen.queryByText("H")).toBeNull()
    if (icon) fireEvent.error(icon)
    expect(container.querySelector("img")).toBeNull()
    expect(screen.getByText("H")).toBeTruthy()
  })
  it("cuts a poster cover to its lower part and fades it into the card", () => {
    const { container } = render(
      <GameCard
        name="Diablo"
        image="/art/d"
        cover="poster"
        icon="/icon/d"
        size="1 GB"
        detail=""
        fraction={1}
        tone="cache"
      />,
    )
    const [banner, icon] = [...container.querySelectorAll("img")]
    expect(banner?.className).toContain("object-[center_82%]")
    expect(icon?.getAttribute("src")).toBe("/icon/d")
    expect(container.querySelector(".bg-linear-to-t")).toBeTruthy()
  })
  it("puts the badge on the banner's corner, else beside the name, the version under the name, and the facts in the footnote", () => {
    const { container, rerender } = render(
      <GameCard
        name="Hearthstone"
        image="/a"
        badge={<em>update</em>}
        sub="Version: 37"
        foot={["Used just now", "Region: EU"]}
        size="1 GB"
        detail=""
        fraction={1}
        tone="cache"
      />,
    )
    expect(screen.getByText("update").closest("span.absolute")).toBeTruthy()
    expect(screen.getByText("Version: 37").previousElementSibling?.textContent).toBe("Hearthstone")
    expect(screen.getByText("Used just now").nextElementSibling?.textContent).toBe("Region: EU")
    rerender(<GameCard name="Hearthstone" badge={<em>update</em>} size="1 GB" detail="" fraction={1} tone="cache" />)
    expect(screen.getByText("update").closest("span.absolute")).toBeNull()
    expect(container.querySelector("img")).toBeNull()
  })
  it("lists notes between the bar and the footnote, a label and its value each, with the hint on hover", () => {
    render(
      <GameCard
        name="World of Warcraft"
        notes={[
          { label: "Classic Era", value: "current", hint: "Classic Era 1.15.9" },
          { label: "In lancache", value: "8 %" },
        ]}
        foot={["Used just now"]}
        action={<button type="button">Prefill</button>}
        size="1 GB"
        detail=""
        fraction={1}
        tone="cache"
      />,
    )
    const era = screen.getByText("Classic Era").closest("li")
    expect(era?.lastChild?.textContent).toBe("current")
    expect(era?.getAttribute("title")).toBe("Classic Era 1.15.9")
    expect(screen.getByText("In lancache").closest("li")?.getAttribute("title")).toBe("In lancache")
    expect(screen.getByText("Used just now").closest("div")?.textContent).toBe("Used just nowPrefill")
  })
  it("keeps the banner's place with a skeleton cover where the grid has covers and the game has none", () => {
    const { container, rerender } = render(
      <GameCard name="Grand Theft Auto V" cover="wide" size="1 GB" detail="" fraction={0} tone="cache" />,
    )
    const skeleton = container.querySelector("[data-skeleton]") as HTMLElement | null
    expect(skeleton?.textContent).toBe("GT")
    expect(skeleton?.style.backgroundImage).toContain(`hsl(${hueOf("Grand Theft Auto V")} `)
    expect(skeleton?.parentElement?.className, "the banner's place").toContain("aspect-[460/215]")
    rerender(<GameCard name="Epic thing" size="1 GB" detail="" fraction={0} tone="cache" />)
    expect(container.querySelector("[data-skeleton]"), "no covers in this grid: no banner").toBeNull()
  })
  it("makes initials from the first two words, and the same hue for the same name", () => {
    expect(initialsOf("Path of Exile 2")).toBe("PO")
    expect(initialsOf("Diablo II: Resurrected")).toBe("DI")
    expect(initialsOf("wow")).toBe("W")
    expect(initialsOf("  ")).toBe("")
    expect(hueOf("Deadlock")).toBe(hueOf("Deadlock"))
    expect(hueOf("Deadlock")).not.toBe(hueOf("Dota 2"))
    expect(hueOf("x")).toBeGreaterThanOrEqual(0)
  })
})

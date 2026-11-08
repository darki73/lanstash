import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Showcase } from "@/styled/showcase"

describe("styled/showcase", () => {
  it("frames a game page: its art, name, facts, badge and actions, then its parts", () => {
    const { container } = render(
      <Showcase
        title="World of Warcraft"
        eyebrow="Battle.net · wow"
        art={[
          { src: "/art/wow/background", cover: "wide" },
          { src: "/art/wow", cover: "poster" },
        ]}
        icon="/art/wow/icon"
        badge={<span>current</span>}
        facts={[{ label: "stored", value: "224.32 GB" }]}
        actions={<button type="button">Prefill</button>}
      >
        the builds
      </Showcase>,
    )
    expect(screen.getByRole("heading", { level: 2, name: "World of Warcraft" })).toBeTruthy()
    expect(screen.getByText("Battle.net · wow")).toBeTruthy()
    expect(screen.getByText("stored").nextSibling?.textContent).toBe("224.32 GB")
    expect(screen.getByText("current")).toBeTruthy()
    expect(screen.getByRole("button", { name: "Prefill" })).toBeTruthy()
    expect(screen.getByText("the builds")).toBeTruthy()
    const srcs = () => [...container.querySelectorAll("img")].map((i) => i.getAttribute("src"))
    expect(srcs()).toEqual(["/art/wow/background", "/art/wow/background", "/art/wow/icon"])
    // The background fails: the cover next, by its upper part; then nothing but a band in its hue.
    fireEvent.error(container.querySelectorAll("img")[1] as HTMLImageElement)
    expect(srcs()).toEqual(["/art/wow", "/art/wow", "/art/wow/icon"])
    expect(container.querySelectorAll("img")[1]?.className).toContain("object-[center_30%]")
    fireEvent.error(container.querySelectorAll("img")[1] as HTMLImageElement)
    fireEvent.error(container.querySelector('img[src="/art/wow/icon"]') as HTMLImageElement)
    expect(container.querySelectorAll("img")).toHaveLength(0)
    expect(screen.getByText("WO"), "the icon's initials").toBeTruthy()
  })
  it("shows the game's logo for its name, and the name again when the logo fails", () => {
    render(<Showcase title="Destiny 2" eyebrow="Steam · 1085660" logo="/art/d2/logo" />)
    const logo = screen.getByRole("img", { name: "Destiny 2" })
    expect(screen.getByRole("heading", { level: 2, name: "Destiny 2" })).toBeTruthy()
    fireEvent.error(logo)
    expect(screen.queryByRole("img", { name: "Destiny 2" })).toBeNull()
    expect(screen.getByRole("heading", { level: 2, name: "Destiny 2" }).textContent).toBe("Destiny 2")
  })
})

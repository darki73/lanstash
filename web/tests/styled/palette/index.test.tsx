import { fireEvent, render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Palette, type PaletteGroup } from "@/styled/palette"

const groups = (typed: string): PaletteGroup[] => [
  {
    title: "Games",
    items: [
      { key: "a", title: "Diablo IV", sub: "Battle.net", meta: "172 GB", href: "/platforms/blizzard/fenris" },
      { key: "b", title: "Destiny 2", href: "/platforms/steam/1085660", image: "/art/d2" },
    ].filter((i) => i.title.toLowerCase().includes(typed.toLowerCase())),
  },
  { title: "Pages", items: typed === "" ? [{ key: "s", title: "Settings", href: "/settings" }] : [] },
]

/** Clicks reaching the document, by the link they were on, without the page going anywhere. */
function followed() {
  const went: string[] = []
  document.addEventListener("click", (e) => {
    const a = (e.target as Element).closest("a")
    if (a === null) return
    e.preventDefault()
    went.push(a.getAttribute("href") ?? "")
  })
  return went
}

describe("styled/palette", () => {
  it("opens from its button, groups what answers, moves with the arrows and goes on Enter", () => {
    const went = followed()
    render(<Palette label="Go to" placeholder="Find…" find={groups} />)
    fireEvent.click(screen.getByRole("button", { name: "Go to" }))
    const box = screen.getByRole("combobox", { name: "Go to" })
    expect(document.activeElement).toBe(box)
    const list = screen.getByRole("listbox", { name: "Go to" })
    expect(
      within(list)
        .getAllByRole("group")
        .map((g) => g.getAttribute("aria-label")),
    ).toEqual(["Games", "Pages"])
    const options = within(list).getAllByRole("option")
    expect(options.map((o) => o.getAttribute("aria-selected"))).toEqual(["true", "false", "false"])
    expect(options[0]?.textContent).toContain("Battle.net")
    expect(options[0]?.textContent).toContain("172 GB")
    expect(options[1]?.querySelector("img")?.getAttribute("src")).toBe("/art/d2")
    fireEvent.keyDown(box, { key: "ArrowUp" })
    expect(box.getAttribute("aria-activedescendant"), "round to the last").toBe(options[2]?.id)
    fireEvent.keyDown(box, { key: "ArrowDown" })
    fireEvent.keyDown(box, { key: "ArrowDown" })
    fireEvent.keyDown(box, { key: "Enter" })
    expect(went).toEqual(["/platforms/steam/1085660"])
    expect(screen.queryByRole("dialog"), "closed after going").toBeNull()
  })
  it("asks again as one types, and says when nothing answers", () => {
    render(<Palette label="Go to" placeholder="Find…" find={groups} none="Nothing here." />)
    fireEvent.click(screen.getByRole("button", { name: "Go to" }))
    const box = screen.getByRole("combobox", { name: "Go to" })
    fireEvent.change(box, { target: { value: "dest" } })
    expect(screen.getAllByRole("option").map((o) => o.getAttribute("href"))).toEqual(["/platforms/steam/1085660"])
    expect(screen.queryByRole("group", { name: "Pages" }), "an empty group is left out").toBeNull()
    fireEvent.change(box, { target: { value: "zelda" } })
    expect(screen.getByText("Nothing here.")).toBeTruthy()
  })
  it("opens on Ctrl+K and on / (but not while typing in a field), closes on Escape and gives the focus back", () => {
    render(
      <>
        <input aria-label="other" />
        <Palette label="Go to" placeholder="Find…" find={groups} />
      </>,
    )
    const other = screen.getByRole("textbox", { name: "other" })
    other.focus()
    fireEvent.keyDown(other, { key: "/" })
    expect(screen.queryByRole("dialog"), "a field takes the /").toBeNull()
    fireEvent.keyDown(other, { key: "k", ctrlKey: true })
    expect(screen.getByRole("dialog", { name: "Go to" })).toBeTruthy()
    fireEvent.keyDown(screen.getByRole("combobox", { name: "Go to" }), { key: "Escape" })
    expect(screen.queryByRole("dialog")).toBeNull()
    expect(document.activeElement).toBe(other)
    fireEvent.keyDown(document.body, { key: "/" })
    expect(screen.getByRole("dialog", { name: "Go to" })).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Close" }))
    expect(screen.queryByRole("dialog")).toBeNull()
  })
})

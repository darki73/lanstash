import { fireEvent, render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { noHiding } from "@/contracts/hidden"
import { GoTo } from "@/features/palette"
import { markHidden } from "@/features/platforms/hidden"
import { PrivacyProvider, privacyValue } from "@/features/privacy"
import { snapshot } from "../../kit"

const usage = [
  {
    service: "steam",
    game: "3240221",
    group: "3240220",
    name: "Grand Theft Auto V Enhanced",
    objects: 3,
    bytes: 99e9,
    last_used_ms: 5,
  },
  { service: "steam", game: "1085661", group: "1085660", name: "Destiny 2", objects: 3, bytes: 160e9, last_used_ms: 9 },
  { service: "blizzard", game: "fenris", name: "Diablo IV", objects: 3, bytes: 172e9, last_used_ms: 7 },
]

const catalogs = {
  steam: {
    products: [
      { code: "271590", name: "Grand Theft Auto V Legacy", game: "271590", version: "1", known: true, download: 124e9 },
      { code: "3240220", name: "Grand Theft Auto V Enhanced", game: "3240220", version: "1", known: true },
    ],
  },
}

const open = () => {
  fireEvent.click(screen.getByRole("button", { name: "Go to" }))
  return screen.getByRole("combobox", { name: "Go to" })
}

const rows = (group: string) =>
  within(screen.getByRole("group", { name: group }))
    .getAllByRole("option")
    .map((o) => `${o.querySelector("span.truncate")?.firstChild?.textContent} → ${o.getAttribute("href")}`)

describe("features/palette", () => {
  it("offers the games used last and the pages to start from before anything is typed", () => {
    render(<GoTo snapshot={snapshot({ usage })} />)
    open()
    expect(rows("Used last")).toEqual([
      "Destiny 2 → /platforms/steam/1085660",
      "Diablo IV → /platforms/blizzard/fenris",
      "Grand Theft Auto V Enhanced → /platforms/steam/3240220",
    ])
    expect(rows("Pages")).toEqual([
      "Overview → /overview",
      "Settings → /settings",
      "Battle.net → /platforms/blizzard",
      "Steam → /platforms/steam",
    ])
  })
  it("finds cached games on every platform, library games not cached, and pages", () => {
    render(<GoTo snapshot={snapshot({ usage })} catalogs={catalogs} art={(p, id, kind) => `/art/${p}/${id}/${kind}`} />)
    const box = open()
    fireEvent.change(box, { target: { value: "gta" } })
    expect(rows("Games")).toEqual(["Grand Theft Auto V Enhanced → /platforms/steam/3240220"])
    expect(rows("Library"), "the cached one is not offered again").toEqual([
      "Grand Theft Auto V Legacy → /platforms/steam/271590",
    ])
    const legacy = within(screen.getByRole("group", { name: "Library" })).getByRole("option")
    expect(legacy.textContent).toContain("Steam · not cached")
    expect(legacy.textContent).toContain("124.00 GB")
    expect(legacy.querySelector("img")?.getAttribute("src"), "Steam's square icon").toBe("/art/steam/271590/icon")
    fireEvent.change(box, { target: { value: "library" } })
    expect(rows("Pages")).toEqual([
      "Battle.net Library → /platforms/blizzard/library",
      "Steam Library → /platforms/steam/library",
      "Epic Games Library → /platforms/epicgames/library",
    ])
    fireEvent.change(box, { target: { value: "fenris" } })
    expect(rows("Games"), "by its code").toEqual(["Diablo IV → /platforms/blizzard/fenris"])
    const icon = within(screen.getByRole("group", { name: "Games" }))
      .getByRole("option")
      .querySelector("img")
    expect(icon?.getAttribute("src"), "Battle.net's square icon").toBe("/art/blizzard/fenris/icon")
  })
  it("never offers a masked game, cached or not", () => {
    const hidden = ["steam/3240220", "steam/271590"]
    render(
      <PrivacyProvider value={privacyValue(hidden, false, noHiding)}>
        <GoTo snapshot={snapshot({ usage: markHidden(usage, hidden, false) })} catalogs={catalogs} />
      </PrivacyProvider>,
    )
    const box = open()
    fireEvent.change(box, { target: { value: "grand" } })
    expect(screen.queryByRole("group", { name: "Games" })).toBeNull()
    expect(screen.queryByRole("group", { name: "Library" })).toBeNull()
    fireEvent.change(box, { target: { value: "hidden" } })
    expect(screen.queryByRole("group", { name: "Games" })).toBeNull()
  })
})

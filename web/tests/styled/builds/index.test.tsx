import { render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { BuildList, BuildRow } from "@/styled/builds"

describe("styled/builds", () => {
  it("lists builds as rows with their version, badge, bar, note, actions and folded details", () => {
    render(
      <BuildList label="Builds of World of Warcraft">
        <BuildRow
          name="Mists of Pandaria Classic"
          tag="wow_classic"
          version="version 5.5.4"
          badge={<span>current</span>}
          fraction={0.5}
          amount="1 GB of 2 GB"
          actions={<button type="button">Prefill</button>}
          details={<span>whole files</span>}
        />
        <BuildRow
          name="WoW: Forever"
          image="/art/wowf/background"
          version="version 1.60"
          note={<span>working it out</span>}
        />
      </BuildList>,
    )
    const list = screen.getByRole("list", { name: "Builds of World of Warcraft" })
    const row = within(list).getByRole("listitem", { name: "Mists of Pandaria Classic" })
    expect(row.textContent).toContain("wow_classic")
    expect(
      within(row).getByRole("meter", { name: "Mists of Pandaria Classic: 1 GB of 2 GB" }).getAttribute("value"),
    ).toBe("50")
    expect(within(row).getByText("Details")).toBeTruthy()
    expect(within(row).getByRole("button", { name: "Prefill" })).toBeTruthy()
    const other = within(list).getByRole("listitem", { name: "WoW: Forever" })
    expect(within(other).queryByRole("meter"), "no bar without a fraction").toBeNull()
    expect(within(other).getByText("working it out")).toBeTruthy()
    expect(other.querySelector("img")?.getAttribute("src"), "its own art").toBe("/art/wowf/background")
    expect(row.querySelector("img"), "no art given: no thumbnail").toBeNull()
  })
})

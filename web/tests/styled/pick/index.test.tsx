import { fireEvent, render, screen, within } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { PickCard, PickGrid } from "@/styled/pick"

describe("styled/pick", () => {
  it("shows games as covers to choose, each with its art, badge, name and size", () => {
    const chose = vi.fn()
    const { container } = render(
      <PickGrid label="Steam's games to choose from">
        <PickCard
          name="Cyberpunk 2077"
          href="/platforms/steam/1091500"
          image="/art/steam/1091500"
          detail="64.00 GB"
          badge={<span>about 80 % in lancache</span>}
          chosen={false}
          onChoose={chose}
        />
        <PickCard name="No Art Game" chosen />
      </PickGrid>,
    )
    const grid = screen.getByRole("list", { name: "Steam's games to choose from" })
    const card = within(grid).getByRole("listitem", { name: "Cyberpunk 2077" })
    expect(within(card).getByRole("link", { name: "Cyberpunk 2077" }).getAttribute("href")).toBe(
      "/platforms/steam/1091500",
    )
    expect(within(card).getByText("about 80 % in lancache")).toBeTruthy()
    expect(within(card).getByText("64.00 GB")).toBeTruthy()
    fireEvent.click(within(card).getByRole("checkbox", { name: "Choose Cyberpunk 2077" }))
    expect(chose).toHaveBeenCalledWith(true)
    const plain = within(grid).getByRole("listitem", { name: "No Art Game" })
    expect(plain.className).toContain("ring-cache")
    expect(within(plain).queryByRole("checkbox"), "nothing to choose with").toBeNull()
    expect(within(plain).getByText("NA"), "its initials").toBeTruthy()
    // An image that fails leaves the initials.
    const img = container.querySelector("img") as HTMLImageElement
    fireEvent.error(img)
    expect(within(card).getByText("C2")).toBeTruthy()
  })
})

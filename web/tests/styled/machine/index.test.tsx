import { render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Machine } from "@/styled/machine"

describe("styled/machine", () => {
  it("names the PC with its initials, its line, its action, the form and its content", () => {
    render(
      <Machine name="Game PC" detail="10.0.0.5" action={<button type="button">Rename</button>} editing={<p>form</p>}>
        <p>games</p>
      </Machine>,
    )
    const card = screen.getByRole("region", { name: "Game PC" })
    expect(within(card).getByRole("heading").textContent).toBe("Game PC")
    expect(within(card).getByText("GP").getAttribute("aria-hidden")).toBe("true")
    for (const text of ["10.0.0.5", "Rename", "form", "games"]) expect(within(card).getByText(text)).toBeTruthy()
  })
  it("puts a summary in a column beside the content when given", () => {
    render(
      <Machine name="Game PC" detail="a" side={<p>summary</p>}>
        <p>games</p>
      </Machine>,
    )
    const summary = screen.getByText("summary").parentElement
    expect(summary?.nextElementSibling?.textContent).toBe("games")
  })
  it("keeps a PC's colour and has a mark for a name without letters", () => {
    const one = render(<Machine name="Game PC" detail="a" />).container.innerHTML
    const two = render(<Machine name="Game PC" detail="a" />).container.innerHTML
    expect(one).toBe(two)
    render(<Machine name="10.0.0.9" detail="b" />)
    expect(within(screen.getByRole("region", { name: "10.0.0.9" })).getByText("1")).toBeTruthy()
  })
})

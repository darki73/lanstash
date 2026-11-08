import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Invites } from "@/features/access/invites"

describe("features/access/invites", () => {
  it("lists the invites waiting, named or not, and takes one back", () => {
    const onRevoke = vi.fn()
    render(
      <Invites
        list={[
          { id: "abc123", role: "player", links: ["steam/someone"], untilMs: 0, name: "player.two" },
          { id: "def456", role: "viewer", links: [], untilMs: 0, name: null },
        ]}
        onRevoke={onRevoke}
      />,
    )
    expect(screen.getByText("player.two")).toBeTruthy()
    expect(screen.getByText("anyone with the link")).toBeTruthy()
    expect(screen.getByText("steam/someone")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Take back invite def456" }))
    expect(onRevoke).toHaveBeenCalledWith("def456")
  })
  it("shows nothing while none waits", () => {
    const { container } = render(<Invites list={[]} onRevoke={() => {}} />)
    expect(container.textContent).toBe("")
  })
})

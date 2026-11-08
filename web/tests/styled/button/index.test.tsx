import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Button, OutLink } from "@/styled/button"

describe("styled/button", () => {
  it("acts when clicked, not when disabled", () => {
    const act = vi.fn()
    const { rerender } = render(
      <Button onClick={act} tone="accent" label="Prefill Hearthstone">
        Prefill
      </Button>,
    )
    fireEvent.click(screen.getByRole("button", { name: "Prefill Hearthstone" }))
    expect(act).toHaveBeenCalledTimes(1)
    rerender(
      <Button onClick={act} disabled>
        Prefill
      </Button>,
    )
    fireEvent.click(screen.getByRole("button", { name: "Prefill" }))
    expect(act).toHaveBeenCalledTimes(1)
  })
  it("marks a deleting action in the danger colour", () => {
    render(
      <Button onClick={() => {}} tone="danger">
        Delete 5.78 GB
      </Button>,
    )
    expect(screen.getByRole("button", { name: "Delete 5.78 GB" }).className).toContain("text-danger")
  })
  it("opens another site's page in a new tab, saying so", () => {
    render(<OutLink href="https://example.com/login">Open the sign-in page</OutLink>)
    const link = screen.getByRole("link", { name: "Open the sign-in page ↗" })
    expect(link.getAttribute("href")).toBe("https://example.com/login")
    expect(link.getAttribute("target")).toBe("_blank")
    expect(link.getAttribute("rel")).toBe("noopener noreferrer")
  })
})

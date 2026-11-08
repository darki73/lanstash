import { render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Guard } from "@/styled/guard"

function Breaks({ when }: { readonly when: boolean }): never | string {
  if (when) throw new Error("a field was missing")
  return "drawn"
}

describe("styled/guard", () => {
  it("says a view failed in its place, and draws it again on the next path", () => {
    const quiet = vi.spyOn(console, "error").mockImplementation(() => {})
    const { rerender } = render(
      <Guard reset="/a">
        <Breaks when />
      </Guard>,
    )
    expect(screen.getByText("This view could not be drawn")).toBeTruthy()
    expect(screen.getByText(/a field was missing/)).toBeTruthy()
    rerender(
      <Guard reset="/b">
        <Breaks when={false} />
      </Guard>,
    )
    expect(screen.getByText("drawn")).toBeTruthy()
    quiet.mockRestore()
  })
})

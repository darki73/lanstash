import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Stack } from "@/styled/stack"

describe("styled/stack", () => {
  it("shows its parts one under the other, in order", () => {
    render(
      <Stack>
        <span>first</span>
        <span>second</span>
      </Stack>,
    )
    const first = screen.getByText("first")
    expect(first.parentElement?.textContent).toBe("firstsecond")
    expect(first.parentElement?.className).toContain("items-start")
  })
  it("stretches every part across, wide, with more room between them", () => {
    render(
      <Stack wide>
        <p>c</p>
      </Stack>,
    )
    const box = screen.getByText("c").parentElement?.className ?? ""
    expect(box).toContain("items-stretch")
    expect(box).toContain("gap-6")
    expect(box).not.toContain("items-start")
  })
})

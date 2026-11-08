import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Search } from "@/styled/search"

describe("styled/search", () => {
  it("reports what is typed, and Escape clears it", () => {
    const seen: string[] = []
    render(<Search label="Search games" value="dead" onChange={(v) => seen.push(v)} />)
    const box = screen.getByRole("searchbox", { name: "Search games" })
    fireEvent.change(box, { target: { value: "deadl" } })
    fireEvent.keyDown(box, { key: "Escape" })
    expect(seen).toEqual(["deadl", ""])
  })
})

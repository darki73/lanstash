import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Notes } from "@/styled/notes"

describe("styled/notes", () => {
  it("lists each fact", () => {
    render(<Notes items={["one fact", "another"]} />)
    expect(screen.getAllByRole("listitem").map((li) => li.textContent)).toEqual(["one fact", "another"])
  })
})

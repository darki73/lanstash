import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Chip, Chips } from "@/styled/chips"

describe("styled/chips", () => {
  it("draws each figure with its label", () => {
    render(
      <Chips>
        <Chip value="38.10 GB" label="stored" tone="cache" />
        <Chip value="5" label="games" />
      </Chips>,
    )
    expect(screen.getAllByRole("listitem").map((li) => li.textContent)).toEqual(["38.10 GBstored", "5games"])
  })
})

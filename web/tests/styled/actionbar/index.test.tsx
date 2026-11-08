import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { ActionBar } from "@/styled/actionbar"

describe("styled/actionbar", () => {
  it("holds a summary and the actions on it", () => {
    render(
      <ActionBar label="Chosen games" actions={<button type="button">Queue</button>}>
        3 chosen
      </ActionBar>,
    )
    const bar = screen.getByRole("region", { name: "Chosen games" })
    expect(bar.textContent).toBe("3 chosenQueue")
  })
})

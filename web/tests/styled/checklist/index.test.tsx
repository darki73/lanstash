import { render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Checklist } from "@/styled/checklist"

describe("styled/checklist", () => {
  it("lists each check with its badge, name and detail, in order", () => {
    render(
      <Checklist
        label="Checks"
        items={[
          { key: "a", badge: <span>ok</span>, title: "store contents", detail: "371 games" },
          { key: "b", badge: <span>warning</span>, title: "open files", detail: "900 of 1024" },
        ]}
      />,
    )
    const rows = within(screen.getByRole("list", { name: "Checks" })).getAllByRole("listitem")
    expect(rows.map((r) => r.textContent)).toEqual(["okstore contents371 games", "warningopen files900 of 1024"])
    expect(screen.getByText("900 of 1024").className).toContain("text-muted")
  })
})

import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Panel, PanelGrid } from "@/styled/panel"

describe("styled/panel", () => {
  it("draws a titled part", () => {
    render(
      <PanelGrid>
        <Panel title="requests">inside</Panel>
      </PanelGrid>,
    )
    expect(screen.getByRole("heading", { level: 3 }).textContent).toBe("requests")
    expect(screen.getByText("inside")).toBeTruthy()
  })
})

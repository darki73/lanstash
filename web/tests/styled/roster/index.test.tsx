import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { RoleBadge, RoleSwitch, Roster, RosterRow } from "@/styled/roster"

const roles = [
  { id: "viewer", label: "Viewer", tone: "least" as const },
  { id: "admin", label: "Admin", tone: "strong" as const },
]

describe("styled/roster", () => {
  it("draws a row with its name, tag, note, middle, role and actions", () => {
    render(
      <Roster label="People">
        <RosterRow name="player.one" tag="you" note="reads" role={<RoleBadge label="viewer" tone="least" />}>
          <span>links here</span>
        </RosterRow>
      </Roster>,
    )
    expect(screen.getByRole("list", { name: "People" })).toBeTruthy()
    for (const t of ["player.one", "you", "reads", "links here", "viewer", "P"]) {
      expect(screen.getByText(t)).toBeTruthy()
    }
  })
  it("switches a role, saying nothing when the pressed one is pressed again", () => {
    const onChange = vi.fn()
    render(<RoleSwitch label="Role" options={roles} value="viewer" onChange={onChange} />)
    expect(screen.getByRole("button", { name: "Viewer" }).getAttribute("aria-pressed")).toBe("true")
    fireEvent.click(screen.getByRole("button", { name: "Viewer" }))
    expect(onChange).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole("button", { name: "Admin" }))
    expect(onChange).toHaveBeenCalledWith("admin")
  })
})

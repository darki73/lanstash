import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { noSession, type Session } from "@/contracts/session"
import { AccessProvider } from "@/features/access"
import { OwnAccount } from "@/features/access/own"
import { en } from "@/i18n/catalogue"

const permissionWords = en.access.permission

const player: Session = {
  name: "player.one",
  role: "player",
  accounts: true,
  permissions: ["prefill.own"],
  links: ["steam/someone"],
}

describe("features/access/own", () => {
  it("says what the account may do, its links, and changes its password with the old one", async () => {
    const setPassword = vi.fn(async () => undefined)
    render(
      <AccessProvider session={player} actions={{ ...noSession, setPassword }}>
        <OwnAccount />
      </AccessProvider>,
    )
    expect(screen.getByText(permissionWords.prefillOwn)).toBeTruthy()
    expect(screen.queryByText(permissionWords.settings)).toBeNull()
    expect(screen.getByText("steam/someone")).toBeTruthy()
    fireEvent.change(screen.getByLabelText("Current password"), { target: { value: "old one here" } })
    fireEvent.change(screen.getByLabelText("New password"), { target: { value: "new one here" } })
    fireEvent.click(screen.getByRole("button", { name: "Change" }))
    expect(await screen.findByText("Changed: sign in again.")).toBeTruthy()
    expect(setPassword).toHaveBeenCalledWith("player.one", "new one here", "old one here")
  })
  it("points a guest to the sign-in, and says a viewer only reads", () => {
    const { rerender } = render(
      <AccessProvider session={{ name: null, role: "guest", accounts: true }} actions={noSession}>
        <OwnAccount />
      </AccessProvider>,
    )
    expect(screen.getByRole("link", { name: "Sign in" }).getAttribute("href")).toBe("/sign-in")
    rerender(
      <AccessProvider session={{ name: "v", role: "viewer", accounts: true, permissions: [] }} actions={noSession}>
        <OwnAccount />
      </AccessProvider>,
    )
    expect(screen.getByText("Read everything; hidden games stay masked.")).toBeTruthy()
    expect(screen.queryByText("Linked platform accounts")).toBeNull()
  })
})

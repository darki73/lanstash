import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { noSession, type Session, type SessionActions } from "@/contracts/session"
import { AccessProvider } from "@/features/access"
import { UsersView } from "@/features/access/users"

const admin: Session = { name: "boss", role: "admin", accounts: true }
const viewer: Session = { name: "player.one", role: "viewer", accounts: true, permissions: [] }

const users = vi.fn<SessionActions["users"]>(async () => [
  { name: "boss", role: "admin" },
  { name: "player.one", role: "player", links: ["steam/someone"] },
])

describe("features/access/users", () => {
  it("lists the accounts, switches several platform accounts and roles, and removes one", async () => {
    const setLink = vi.fn(async () => undefined)
    const setRole = vi.fn(async () => undefined)
    const removeUser = vi.fn(async () => undefined)
    const invites = vi.fn<SessionActions["invites"]>(async () => [
      { id: "abc123", role: "viewer", links: [], untilMs: 0, name: null },
    ])
    render(
      <AccessProvider session={admin} actions={{ ...noSession, users, setLink, setRole, removeUser, invites }}>
        <UsersView linkChoices={["steam/someone", "steam/other"]} />
      </AccessProvider>,
    )
    expect(await screen.findByText("you")).toBeTruthy()
    expect(await screen.findByText("Invites waiting")).toBeTruthy()
    const mine = screen.getByRole("group", { name: "Platform accounts of player.one" })
    const other = mine.querySelector('[aria-label="steam other"]') as HTMLElement
    fireEvent.click(other)
    await waitFor(() => expect(setLink).toHaveBeenCalledWith("player.one", "steam/other", true))
    const someone = mine.querySelector('[aria-label="steam someone"]') as HTMLElement
    fireEvent.click(someone)
    await waitFor(() => expect(setLink).toHaveBeenCalledWith("player.one", "steam/someone", false))
    const roles = screen.getByRole("group", { name: "Role of player.one" })
    fireEvent.click(roles.querySelector("button") as HTMLElement)
    expect(setRole).toHaveBeenCalledWith("player.one", "viewer")
    fireEvent.click(screen.getByRole("button", { name: "Remove player.one" }))
    expect(removeUser).toHaveBeenCalledWith("player.one")
    expect(screen.getByText("Add an account")).toBeTruthy()
  })
  it("tells anyone who may not manage the accounts so", () => {
    render(
      <AccessProvider session={viewer} actions={{ ...noSession, users }}>
        <UsersView linkChoices={[]} />
      </AccessProvider>,
    )
    expect(screen.getByText("Only an admin manages the accounts")).toBeTruthy()
    expect(screen.queryByText("Add an account")).toBeNull()
  })
})

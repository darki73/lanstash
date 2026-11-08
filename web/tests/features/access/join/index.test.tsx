import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { noSession, type Session, type SessionActions } from "@/contracts/session"
import { AccessProvider } from "@/features/access"
import { Join } from "@/features/access/join"

const guest: Session = { name: null, role: "guest", accounts: true }

const fill = (label: string, value: string) => fireEvent.change(screen.getByLabelText(label), { target: { value } })

describe("features/access/join", () => {
  it("makes the account with the invite, the password typed twice", async () => {
    const join = vi.fn<SessionActions["join"]>(async () => ({ error: "player.one is taken" }))
    const invite = async () => ({ role: "player" as const, links: [], untilMs: 0, name: null })
    render(
      <AccessProvider session={guest} actions={{ ...noSession, join, invite }}>
        <Join token="f00d" done="/overview" />
      </AccessProvider>,
    )
    expect(await screen.findByText("You were invited as a player: choose a name and a password")).toBeTruthy()
    fill("Name", " player.one ")
    fill("Password", "long enough pw")
    fill("Password again", "long enough pv")
    fireEvent.click(screen.getByRole("button", { name: "Make the account" }))
    expect(await screen.findByText("The two passwords differ.")).toBeTruthy()
    expect(join).not.toHaveBeenCalled()
    fill("Password again", "long enough pw")
    fireEvent.click(screen.getByRole("button", { name: "Make the account" }))
    expect(await screen.findByText("player.one is taken")).toBeTruthy()
    expect(join).toHaveBeenCalledWith("f00d", "player.one", "long enough pw")
  })
  it("shows a name the admin chose, fixed, and says when an invite does not work", async () => {
    const invite = vi.fn<SessionActions["invite"]>(async () => ({
      role: "admin",
      links: [],
      untilMs: 0,
      name: "player.two",
    }))
    const { unmount } = render(
      <AccessProvider session={guest} actions={{ ...noSession, invite }}>
        <Join token="f00d" done="/overview" />
      </AccessProvider>,
    )
    expect(await screen.findByText("You were invited as an admin: choose a password")).toBeTruthy()
    const name = screen.getByLabelText("Name") as HTMLInputElement
    expect([name.value, name.readOnly]).toEqual(["player.two", true])
    unmount()
    render(
      <AccessProvider session={guest} actions={{ ...noSession, invite: async () => "this invite is not valid" }}>
        <Join token="f00d" done="/overview" />
      </AccessProvider>,
    )
    expect(await screen.findByText("this invite is not valid")).toBeTruthy()
    expect(screen.getByRole("link", { name: "Sign in instead" })).toBeTruthy()
  })
  it("tells a signed-in page where to go on", () => {
    render(
      <AccessProvider session={{ name: "player.one", role: "player", accounts: true }} actions={noSession}>
        <Join token="f00d" done="/overview" />
      </AccessProvider>,
    )
    expect(screen.getByText("As player.one, player.")).toBeTruthy()
    expect(screen.getByRole("link", { name: "Go on" }).getAttribute("href")).toBe("/overview")
  })
})

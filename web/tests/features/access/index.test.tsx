import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { noSession, type Session, type SessionActions } from "@/contracts/session"
import { AccessProvider, SignIn, useAccess, Who } from "@/features/access"

const admin: Session = { name: "boss", role: "admin", accounts: true }
const viewer: Session = { name: "player.one", role: "viewer", accounts: true }
const guest: Session = { name: null, role: "guest", accounts: true }

function Probe() {
  return <output>{String(useAccess().canChange)}</output>
}

const actions = (over: Partial<SessionActions> = {}): SessionActions => ({ ...noSession, ...over })

describe("features/access", () => {
  it("lets anyone change things without a provider, and only an admin with accounts", () => {
    const { rerender } = render(<Probe />)
    expect(screen.getByRole("status").textContent).toBe("true")
    rerender(
      <AccessProvider session={viewer} actions={noSession}>
        <Probe />
      </AccessProvider>,
    )
    expect(screen.getByRole("status").textContent).toBe("false")
  })
  it("says who uses the page: a link to sign in, or the account with Sign out", () => {
    const signOut = vi.fn(async () => {})
    const { rerender, container } = render(
      <AccessProvider session={guest} actions={actions({ signOut })}>
        <Who />
      </AccessProvider>,
    )
    expect(screen.getByRole("link", { name: "Sign in" }).getAttribute("href")).toBe("/sign-in")
    rerender(
      <AccessProvider session={admin} actions={actions({ signOut })}>
        <Who />
      </AccessProvider>,
    )
    expect(screen.getByRole("link", { name: "boss admin" }).getAttribute("href")).toBe("/account")
    fireEvent.click(screen.getByText("Sign out"))
    expect(signOut).toHaveBeenCalled()
    rerender(
      <AccessProvider session={{ name: null, role: "admin", accounts: false }} actions={noSession}>
        <Who />
      </AccessProvider>,
    )
    expect(container.textContent).toBe("")
  })
  it("signs in, and says why not", async () => {
    const signIn = vi.fn<SessionActions["signIn"]>(async () => ({ error: "wrong name or password" }))
    render(
      <AccessProvider session={guest} actions={actions({ signIn })}>
        <SignIn done="/overview" />
      </AccessProvider>,
    )
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "boss" } })
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "long enough pw" } })
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }))
    expect(await screen.findByText("wrong name or password")).toBeTruthy()
    expect(signIn).toHaveBeenCalledWith("boss", "long enough pw")
  })
  it("tells a signed-in page where to go on", () => {
    render(
      <AccessProvider session={admin} actions={noSession}>
        <SignIn done="/overview" />
      </AccessProvider>,
    )
    expect(screen.getByRole("link", { name: "Go on" }).getAttribute("href")).toBe("/overview")
  })
})

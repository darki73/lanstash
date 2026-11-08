import { fireEvent, render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { AccountActions, AccountState, SignedIn } from "@/contracts/account"
import { AccountPanel } from "@/features/platforms/account"

const state = (accounts: readonly SignedIn[], sign: Partial<AccountState["view"]["sign_in"]> = {}): AccountState => ({
  view: { accounts, sign_in: { state: "idle", scanned: false, error: null, ...sign } },
  qr: "/api/v1/account/steam/qr.svg?turn=1",
})

const recorder = () => {
  const did: string[] = []
  const actions: AccountActions = {
    signIn: async () => {
      did.push("in")
      return undefined
    },
    signInWithCode: async (pasted) => {
      did.push(`code ${pasted}`)
      return pasted === "used" ? "Epic: that code was used" : undefined
    },
    cancel: async () => {
      did.push("cancel")
    },
    signOut: async (account) => {
      did.push(`out ${account}`)
      return "read-only file system"
    },
    setClient: async (client) => {
      did.push(`client ${client}`)
      return client === "bad" ? "a client is <id>:<secret>" : undefined
    },
  }
  return { did, actions }
}

const byCode = (accounts: readonly SignedIn[]): AccountState => ({
  view: {
    accounts,
    sign_in: { state: "idle", scanned: false, error: null },
    method: "code",
    login_url: "https://example.com/login",
  },
  qr: "",
})

const one: SignedIn = { name: "player_one", refused: false }
const two: SignedIn = { name: "player_two", refused: true }

describe("features/platforms/account", () => {
  it("offers a sign-in when none is signed in, then shows the code to scan", () => {
    const { did, actions } = recorder()
    const { rerender } = render(<AccountPanel name="Steam" state={state([])} actions={actions} />)
    expect(screen.getByText("No Steam account signed in")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Sign lanstash in to Steam" }))
    expect(did).toEqual(["in"])
    rerender(<AccountPanel name="Steam" state={state([], { state: "waiting" })} actions={actions} />)
    const img = screen.getByRole("img", { name: "QR code to sign lanstash in to Steam" })
    expect(img.getAttribute("src")).toBe("/api/v1/account/steam/qr.svg?turn=1")
    expect(screen.getByText(/tap the Steam Guard shield/)).toBeTruthy()
    rerender(<AccountPanel name="Steam" state={state([], { state: "waiting", scanned: true })} actions={actions} />)
    expect(screen.getByText("Scanned: approve the sign-in in the Steam mobile app.")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }))
    expect(did).toEqual(["in", "cancel"])
  })
  it("lists each account, marks a refused one, adds another and signs one out", async () => {
    const { did, actions } = recorder()
    render(<AccountPanel name="Steam" state={state([one, two])} actions={actions} />)
    const rows = screen.getAllByRole("row").slice(1)
    expect(rows.map((r) => within(r).getAllByRole("cell")[0]?.textContent)).toEqual(["player_one", "player_two"])
    expect(within(rows[0] as HTMLElement).getByText("signed in")).toBeTruthy()
    expect(within(rows[1] as HTMLElement).getByText("refused: sign in again")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Add a Steam account" }))
    fireEvent.click(screen.getByRole("button", { name: "Sign player_two out of lanstash" }))
    expect(did).toEqual(["in", "out player_two"])
    expect(await screen.findByText("read-only file system")).toBeTruthy()
  })
  it("keeps the accounts listed while another is being added", () => {
    render(<AccountPanel name="Steam" state={state([one], { state: "waiting" })} actions={recorder().actions} />)
    expect(screen.getByRole("img", { name: "QR code to sign lanstash in to Steam" })).toBeTruthy()
    expect(screen.getByText("player_one")).toBeTruthy()
  })
  it("shows why a sign-in failed, and waits for the first answer", () => {
    const { rerender } = render(
      <AccountPanel name="Steam" state={state([], { state: "failed", error: "the QR code expired" })} />,
    )
    expect(screen.getByText("the QR code expired")).toBeTruthy()
    rerender(<AccountPanel name="Steam" state={undefined} />)
    expect(screen.getByText("Asking lanstash…")).toBeTruthy()
  })
  it("signs in to Epic with a code from its own page, in two steps", async () => {
    const { did, actions } = recorder()
    render(<AccountPanel name="Epic Games" state={byCode([])} actions={actions} />)
    fireEvent.click(screen.getByRole("button", { name: "Sign lanstash in to Epic Games" }))
    expect(did, "nothing asked of lanstash yet").toEqual([])
    const page = screen.getByRole("link", { name: /Open Epic Games's sign-in page/ })
    expect(page.getAttribute("target"), "a new tab").toBe("_blank")
    expect(page.getAttribute("href")).toBe("https://example.com/login")
    const button = screen.getByRole("button", { name: "Sign in to Epic Games with the code" }) as HTMLButtonElement
    expect(button.disabled, "nothing pasted").toBe(true)
    fireEvent.change(screen.getByRole("textbox", { name: "Epic Games sign-in code" }), { target: { value: "used" } })
    fireEvent.click(button)
    expect(await screen.findByText("Epic: that code was used")).toBeTruthy()
    fireEvent.change(screen.getByRole("textbox", { name: "Epic Games sign-in code" }), { target: { value: "f00d" } })
    fireEvent.click(screen.getByRole("button", { name: "Sign in to Epic Games with the code" }))
    await screen.findByText("No Epic Games account signed in")
    expect(did).toEqual(["code used", "code f00d"])
    expect(screen.queryByRole("textbox"), "the steps close once signed in").toBeNull()
  })
  it("signs an Epic account out by its id", () => {
    const { did, actions } = recorder()
    render(
      <AccountPanel
        name="Epic Games"
        state={byCode([{ name: "Player One", id: "abc", refused: false }])}
        actions={actions}
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "Sign Player One out of lanstash" }))
    expect(did).toEqual(["out abc"])
  })
  it("says when lanstash offers no sign-in for the platform", () => {
    render(<AccountPanel name="Epic Games" state={null} />)
    expect(screen.getByText("lanstash offers no Epic Games sign-in here")).toBeTruthy()
  })
  it("says which client Epic's sign-in names, takes another and goes back to the built-in one", async () => {
    const { did, actions } = recorder()
    const withClient = (client: "built-in" | "own"): AccountState => ({
      ...byCode([]),
      view: { ...byCode([]).view, client },
    })
    const { rerender } = render(<AccountPanel name="Epic Games" state={withClient("built-in")} actions={actions} />)
    const client = within(screen.getByRole("region", { name: "Sign-in client" }))
    expect(client.getByText("built in")).toBeTruthy()
    expect(
      client.getByText("The Epic Games launcher's own client, built in. Change it only if you have another."),
    ).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Use another Epic Games client" }))
    // A secret: typed hidden, not offered again by the browser's form filling.
    const box = screen.getByLabelText("Epic Games sign-in client") as HTMLInputElement
    expect(box.type).toBe("password")
    fireEvent.change(box, { target: { value: "bad" } })
    fireEvent.click(screen.getByRole("button", { name: "Use this Epic Games client" }))
    expect(await screen.findByText("a client is <id>:<secret>")).toBeTruthy()
    fireEvent.change(box, { target: { value: "ab:cd" } })
    fireEvent.click(screen.getByRole("button", { name: "Use this Epic Games client" }))
    rerender(<AccountPanel name="Epic Games" state={withClient("own")} actions={actions} />)
    expect(await screen.findByText("your own")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Use the built-in Epic Games client" }))
    expect(did).toEqual(["client bad", "client ab:cd", "client null"])
  })
  it("names no client where the platform's sign-in names none (Steam)", () => {
    render(<AccountPanel name="Steam" state={state([one])} />)
    expect(screen.queryByRole("region", { name: "Sign-in client" })).toBeNull()
  })
})

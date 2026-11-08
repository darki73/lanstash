import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { noSession, type SessionActions } from "@/contracts/session"
import { AddAccount } from "@/features/access/add"

const choices = ["steam/a", "steam/b"]

describe("features/access/add", () => {
  it("makes an invite link with the role, several platform accounts and an optional name", async () => {
    const makeInvite = vi.fn<SessionActions["makeInvite"]>(async () => ({ token: "f00d", untilMs: 0 }))
    const onDone = vi.fn()
    render(
      <AddAccount
        actions={{ ...noSession, makeInvite }}
        joinLink={(t: string) => `https://cache.example/join/${t}`}
        linkChoices={choices}
        onDone={onDone}
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "steam a" }))
    fireEvent.click(screen.getByRole("button", { name: "steam b" }))
    fireEvent.change(screen.getByLabelText("New account's name"), { target: { value: " player.two " } })
    fireEvent.click(screen.getByRole("button", { name: "Make an invite link" }))
    expect(await screen.findByText("https://cache.example/join/f00d")).toBeTruthy()
    expect(makeInvite).toHaveBeenCalledWith("player", ["steam/a", "steam/b"], "player.two")
    expect(onDone).toHaveBeenCalled()
    fireEvent.click(screen.getByRole("button", { name: "Admin" }))
    expect(screen.queryByRole("button", { name: "steam a" })).toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Make an invite link" }))
    await waitFor(() => expect(makeInvite).toHaveBeenLastCalledWith("admin", [], ""))
  })
  it("adds an account at once with a password set now, and links its platform accounts", async () => {
    const addUser = vi.fn(async () => undefined)
    const setLink = vi.fn(async () => undefined)
    render(
      <AddAccount
        actions={{ ...noSession, addUser, setLink }}
        joinLink={(t: string) => t}
        linkChoices={choices}
        onDone={() => {}}
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "Set it now" }))
    const add = screen.getByRole("button", { name: "Add the account" }) as HTMLButtonElement
    expect(add.disabled).toBe(true)
    fireEvent.change(screen.getByLabelText("New account's name"), { target: { value: "player.two" } })
    fireEvent.change(screen.getByLabelText("New account's password"), { target: { value: "long enough pw" } })
    fireEvent.click(screen.getByRole("button", { name: "steam b" }))
    fireEvent.click(add)
    expect(await screen.findByText("Added player.two.")).toBeTruthy()
    expect(addUser).toHaveBeenCalledWith("player.two", "player", "long enough pw")
    expect(setLink).toHaveBeenCalledWith("player.two", "steam/b", true)
  })
  it("says why not", async () => {
    render(
      <AddAccount
        actions={{ ...noSession, makeInvite: async () => "there is an account boss already" }}
        joinLink={(t: string) => t}
        linkChoices={[]}
        onDone={() => {}}
      />,
    )
    expect(screen.getByText("lanstash is signed in to no platform account yet")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Make an invite link" }))
    expect(await screen.findByText("there is an account boss already")).toBeTruthy()
  })
})

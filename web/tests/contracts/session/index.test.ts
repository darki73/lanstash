import { describe, expect, it } from "vitest"
import { canChange, noSession, unknownSession, withoutAccounts } from "@/contracts/session"

describe("contracts/session", () => {
  it("lets an admin, or anyone without accounts, change things", () => {
    expect(canChange({ name: "boss", role: "admin", accounts: true })).toBe(true)
    expect(canChange({ name: "player.one", role: "viewer", accounts: true })).toBe(false)
    expect(canChange({ name: null, role: "guest", accounts: true })).toBe(false)
    expect(canChange(withoutAccounts)).toBe(true)
    expect(canChange(unknownSession), "nothing changes before lanstash said who").toBe(false)
  })
  it("cannot sign in without lanstash", async () => {
    expect(await noSession.signIn("a", "b")).toEqual({ error: "lanstash keeps no accounts" })
    expect(await noSession.users()).toBe("lanstash keeps no accounts")
  })
})

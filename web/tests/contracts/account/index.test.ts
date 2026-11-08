import { describe, expect, it } from "vitest"
import { type AccountView, needsAttention, noAccount, signingIn } from "@/contracts/account"

const view = (state: AccountView["sign_in"]["state"], accounts: AccountView["accounts"] = []): AccountView => ({
  accounts,
  sign_in: { state, scanned: false, error: null },
})

describe("contracts/account", () => {
  it("counts a sign-in as under way while it starts or waits", () => {
    expect(signingIn(view("starting"))).toBe(true)
    expect(signingIn(view("waiting"))).toBe(true)
    expect(signingIn(view("idle"))).toBe(false)
    expect(signingIn(view("failed"))).toBe(false)
    expect(signingIn(undefined)).toBe(false)
  })
  it("needs attention with no account, a refused one, or a failed sign-in", () => {
    const fine = [{ name: "player_one", refused: false }]
    expect(needsAttention(view("idle", fine))).toBe(false)
    expect(needsAttention(view("idle"))).toBe(true)
    expect(needsAttention(view("idle", [...fine, { name: "player_two", refused: true }]))).toBe(true)
    expect(needsAttention(view("failed", fine))).toBe(true)
    expect(needsAttention(undefined), "not known yet").toBe(false)
  })
  it("has actions for a page that cannot sign in", async () => {
    expect(await noAccount.signIn()).toBe("signing in is not available here")
    expect(await noAccount.signOut("player_one")).toBe("signing out is not available here")
    await noAccount.cancel()
  })
})

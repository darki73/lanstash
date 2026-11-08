import { describe, expect, it } from "vitest"
import { accountActions, type Send } from "@/adapters/api/account"

const answer =
  (ok: boolean, body: unknown, seen: string[]): Send =>
  async (url) => {
    seen.push(url)
    return { ok, json: async () => body }
  }

const waiting = {
  accounts: [{ name: "player_one", refused: false }],
  account: "player_one",
  sign_in: { state: "waiting", scanned: true, error: null },
}

describe("adapters/api/account", () => {
  it("reads the accounts and drives the sign-in through the paths of the API", async () => {
    const got: string[] = []
    const posted: string[] = []
    const deleted: string[] = []
    const a = accountActions(
      answer(true, waiting, got),
      answer(true, {}, posted),
      answer(true, {}, deleted),
      "http://box",
    )
    expect(await a.read("steam")).toEqual(waiting)
    expect(got).toEqual(["http://box/api/v1/account/steam"])
    expect(await a.signIn("steam")).toBeUndefined()
    await a.cancel("steam")
    expect(await a.signOut("steam", "player one")).toBeUndefined()
    expect(posted).toEqual([
      "http://box/api/v1/account/steam/sign-in",
      "http://box/api/v1/account/steam/sign-out?account=player%20one",
    ])
    expect(deleted).toEqual(["http://box/api/v1/account/steam/sign-in"])
    expect(a.qr("steam", 3)).toBe("http://box/api/v1/account/steam/qr.svg?turn=3")
  })
  it("reads a lanstash that kept one account as a list of that one", async () => {
    const sign_in = { state: "idle", scanned: false, error: null }
    const one = accountActions(
      answer(true, { account: "player_one", sign_in }, []),
      answer(true, {}, []),
      answer(true, {}, []),
      "",
    )
    expect((await one.read("steam"))?.accounts).toEqual([{ name: "player_one", refused: false }])
    const none = accountActions(
      answer(true, { account: null, sign_in }, []),
      answer(true, {}, []),
      answer(true, {}, []),
      "",
    )
    expect((await none.read("steam"))?.accounts).toEqual([])
  })
  it("gives the API's reason, null where lanstash offers no accounts, and undefined for what is not an account", async () => {
    const seen: string[] = []
    const refused = answer(false, { error: "no account for this platform", kind: "not-found" }, seen)
    const a = accountActions(refused, refused, refused, "")
    expect(await a.read("steam"), "lanstash offers none").toBeNull()
    expect(await a.signIn("steam")).toBe("no account for this platform")
    const broken = accountActions(answer(false, { error: "boom", kind: "internal" }, seen), refused, refused, "")
    expect(await broken.read("steam"), "not known").toBeUndefined()
    const odd = accountActions(answer(true, { account: 3 }, seen), refused, refused, "")
    expect(await odd.read("steam")).toBeUndefined()
    const oddList = accountActions(
      answer(true, { accounts: [{ name: 1 }], sign_in: { state: "idle", scanned: false, error: null } }, seen),
      refused,
      refused,
      "",
    )
    expect(await oddList.read("steam")).toBeUndefined()
    const down: Send = async () => {
      throw new Error("offline")
    }
    const gone = accountActions(down, down, down, "")
    expect(await gone.read("steam")).toBeUndefined()
    expect(await gone.signOut("steam", "player_one")).toBe("lanstash did not answer")
  })
  it("sends a pasted code and a client's secret in a header, never in the address", async () => {
    const sent: { url: string; headers?: Readonly<Record<string, string>> }[] = []
    const post: Send = async (url, headers) => {
      sent.push({ url, headers })
      return { ok: true, json: async () => ({}) }
    }
    const a = accountActions(answer(true, {}, []), post, answer(true, {}, []), "")
    await a.signInWithCode("epicgames", ' {"authorizationCode":"f00d"}\n')
    await a.setClient("epicgames", "id:s3cret")
    expect(sent.map((s) => s.url)).toEqual(["/api/v1/account/epicgames/sign-in", "/api/v1/account/epicgames/client"])
    expect(sent.map((s) => s.headers?.["X-Lanstash-Secret"])).toEqual([
      encodeURIComponent('{"authorizationCode":"f00d"}'),
      "id%3As3cret",
    ])
  })
})

import { describe, expect, it } from "vitest"
import type { SendWith } from "@/adapters/api/dns"
import { readSession, sessionActions } from "@/adapters/api/session"
import type { Fetch } from "@/adapters/api/version"

type Sent = { url: string; headers?: Readonly<Record<string, string>> }

const answering =
  (ok: boolean, body: unknown, sent: Sent[] = []): SendWith =>
  async (url, headers) => {
    sent.push({ url, ...(headers === undefined ? {} : { headers }) })
    return { ok, json: async () => body }
  }

const reading =
  (body: unknown, ok = true): Fetch =>
  async () => ({ ok, json: async () => body })

const signedIn = { name: "boss", role: "admin", accounts: true }

describe("adapters/api/session", () => {
  it("reads who uses the page, and an older lanstash as one without accounts", async () => {
    expect(await readSession(reading(signedIn), "")).toEqual(signedIn)
    expect(await readSession(reading({ error: "x", kind: "not-found" }, false), "")).toEqual({
      name: null,
      role: "admin",
      accounts: false,
    })
    expect(await readSession(reading({ role: "owner" }), "")).toBeUndefined()
  })
  it("signs in with the name and password in headers, never in the address", async () => {
    const sent: Sent[] = []
    const a = sessionActions(
      reading(signedIn),
      answering(true, signedIn, sent),
      answering(true, {}),
      answering(true, {}),
      "",
    )
    expect(await a.signIn("player one", "pa ss&word")).toEqual(signedIn)
    expect(sent).toEqual([
      {
        url: "/api/v1/session",
        headers: { "X-Lanstash-User": "player%20one", "X-Lanstash-Password": "pa%20ss%26word" },
      },
    ])
  })
  it("says why a sign-in failed, and how long to wait", async () => {
    const wait = answering(false, { error: "wait 2 s", kind: "wait", retry_ms: 2000 })
    const a = sessionActions(reading(signedIn), wait, answering(true, {}), answering(true, {}), "")
    expect(await a.signIn("boss", "x")).toEqual({ error: "wait 2 s", retryMs: 2000 })
  })
  it("manages the accounts, the passwords always in headers", async () => {
    const posted: Sent[] = []
    const put: Sent[] = []
    const users = { users: [{ name: "boss", role: "admin" }] }
    const a = sessionActions(
      reading(users),
      answering(true, {}, posted),
      answering(true, {}, put),
      answering(false, { error: "boss is the last admin" }),
      "",
    )
    expect(await a.users()).toEqual(users.users)
    expect(await a.addUser("player.one", "viewer", "a long password")).toBeUndefined()
    expect(posted[0]).toEqual({
      url: "/api/v1/users/player.one?role=viewer",
      headers: { "X-Lanstash-Password": "a%20long%20password" },
    })
    expect(await a.setPassword("player.one", "new one", "old one")).toBeUndefined()
    expect(put[0]?.headers).toEqual({ "X-Lanstash-Password": "new%20one", "X-Lanstash-Old-Password": "old%20one" })
    expect(await a.removeUser("boss")).toBe("boss is the last admin")
  })
  it("makes, lists and takes back invites, and joins with one", async () => {
    const posted: Sent[] = []
    const deleted: Sent[] = []
    const listed = {
      invites: [
        { id: "abc", role: "player", links: ["steam/someone"], until_ms: 9 },
        { id: "bad", role: "owner" },
      ],
    }
    const a = sessionActions(
      reading(listed),
      answering(true, { token: "f00d", until_ms: 9 }, posted),
      answering(true, {}),
      answering(true, {}, deleted),
      "",
    )
    expect(await a.invites()).toEqual([{ id: "abc", role: "player", links: ["steam/someone"], untilMs: 9, name: null }])
    expect(await a.makeInvite("player", ["steam/some one", "steam/b"], "player.two")).toEqual({
      token: "f00d",
      untilMs: 9,
    })
    expect(posted[0]?.url).toBe("/api/v1/invites?role=player&link=steam%2Fsome%20one&link=steam%2Fb&name=player.two")
    const shown = sessionActions(
      reading({ role: "admin", links: [], until_ms: 3, name: "player.two" }),
      answering(true, {}),
      answering(true, {}),
      answering(true, {}),
      "",
    )
    expect(await shown.invite("f00d")).toEqual({ role: "admin", links: [], untilMs: 3, name: "player.two" })
    expect(await a.revokeInvite("abc")).toBeUndefined()
    expect(deleted[0]?.url).toBe("/api/v1/invites/abc")
    const joined = sessionActions(
      reading(signedIn),
      answering(true, {}, posted),
      answering(true, {}),
      answering(true, {}),
      "",
    )
    expect(await joined.join("f00d", "player.one", "long enough pw")).toEqual(signedIn)
    expect(posted[1]).toEqual({
      url: "/api/v1/invites/f00d",
      headers: { "X-Lanstash-User": "player.one", "X-Lanstash-Password": "long%20enough%20pw" },
    })
    const refused = sessionActions(
      reading({}),
      answering(false, { error: "no" }),
      answering(true, {}),
      answering(true, {}),
      "",
    )
    expect(await refused.makeInvite("viewer", [])).toBe("no")
    const older = sessionActions(
      reading({ error: "not found", kind: "not-found" }, false),
      answering(true, {}),
      answering(true, {}),
      answering(true, {}),
      "",
    )
    expect(await older.invites()).toEqual([])
  })
})

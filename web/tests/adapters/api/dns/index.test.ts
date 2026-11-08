import { describe, expect, it } from "vitest"
import { type ApiDnsPage, dnsActions, type SendWith } from "@/adapters/api/dns"

const page: ApiDnsPage = {
  setup: {
    kinds: [{ id: "adguard", name: "AdGuard Home", port: 3000, https_port: 443 }],
    config: null,
    password_set: false,
  },
  report: null,
  report_error: "no DNS server is set up yet",
  done: null,
}

describe("adapters/api/dns", () => {
  it("reads, sets up, tests, forgets and applies, the password in a header only", async () => {
    const sent: string[] = []
    const ok =
      (kind: string): SendWith =>
      async (url, headers) => {
        sent.push(`${kind} ${url}${headers === undefined ? "" : ` ${JSON.stringify(headers)}`}`)
        return { ok: true, json: async () => page }
      }
    const a = dnsActions(ok("GET"), ok("PUT"), ok("DELETE"), ok("POST"), "http://box")
    const config = {
      kind: "adguard",
      host: "192.0.2.53",
      port: 3000,
      https: false,
      pin: null,
      unchecked: false,
      user: "lan stash",
      answer: null,
    }
    expect(await a.read()).toEqual(page)
    await a.save(config, "s3cret&x", true)
    await a.save({ ...config, answer: "192.0.2.10" }, undefined, false)
    await a.save({ ...config, port: 443, https: true, pin: "AB:CD" }, undefined, true)
    await a.save({ ...config, port: 443, https: true, pin: "AB:CD", unchecked: true }, undefined, true)
    await a.forget()
    await a.apply()
    const base = "PUT http://box/api/v1/dns/setup?kind=adguard&host=192.0.2.53"
    expect(sent).toEqual([
      "GET http://box/api/v1/dns",
      `${base}&port=3000&user=lan+stash&answer=&test=1 {"X-Lanstash-Password":"s3cret&x"}`,
      `${base}&port=3000&user=lan+stash&answer=192.0.2.10`,
      `${base}&port=443&user=lan+stash&answer=&https=1&pin=AB%3ACD&test=1`,
      `${base}&port=443&user=lan+stash&answer=&https=1&unchecked=1&test=1`,
      "DELETE http://box/api/v1/dns/setup",
      "POST http://box/api/v1/dns/apply",
    ])
    expect(sent.join(" ")).not.toContain("s3cret&x&")
  })
  it("gives lanstash's reason for a refusal, and says when it did not answer", async () => {
    const refuse: SendWith = async () => ({
      ok: false,
      json: async () => ({ error: "AdGuard Home refused lanstash's user name and password" }),
    })
    const down: SendWith = async () => {
      throw new Error("offline")
    }
    const odd: SendWith = async () => ({ ok: true, json: async () => ({ nothing: 1 }) })
    const config = {
      kind: "adguard",
      host: "h",
      port: 1,
      https: false,
      pin: null,
      unchecked: false,
      user: "u",
      answer: null,
    }
    expect(await dnsActions(refuse, refuse, refuse, refuse, "").save(config, "p", false)).toBe(
      "AdGuard Home refused lanstash's user name and password",
    )
    expect(await dnsActions(down, down, down, down, "").read()).toBe("lanstash did not answer")
    expect(await dnsActions(odd, odd, odd, odd, "").read()).toBe("lanstash refused")
  })
})

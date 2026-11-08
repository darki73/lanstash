import { describe, expect, it } from "vitest"
import { noDns, untrustedIn } from "@/contracts/dns"

describe("contracts/dns", () => {
  it("has no DNS page where lanstash's DNS API cannot be reached", async () => {
    const config = {
      kind: "adguard",
      host: "h",
      port: 3000,
      https: false,
      pin: null,
      unchecked: false,
      user: "u",
      answer: null,
    }
    for (const a of [noDns.read(), noDns.save(config, "p", false), noDns.forget(), noDns.apply()]) {
      expect(await a).toBe("the DNS server's setup is not available here")
    }
  })
  it("reads the untrusted certificate a refusal names", () => {
    const words =
      "AdGuard Home at 192.0.2.53:443: 192.0.2.53: its certificate is not trusted: invalid peer certificate: UnknownIssuer (untrusted certificate, SHA-256 AB:CD:EF)"
    expect(untrustedIn(words)).toBe("AB:CD:EF")
    expect(untrustedIn("AdGuard Home refused lanstash's user name and password")).toBeUndefined()
  })
})

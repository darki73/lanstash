import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { DnsActions, DnsConfig, DnsPage } from "@/contracts/dns"
import { DnsView } from "@/features/dns"

const kinds = [{ id: "adguard", name: "AdGuard Home", port: 3000, https_port: 443 }]

const unset: DnsPage = {
  setup: { kinds, config: null, password_set: false },
  report: null,
  report_error: "no DNS server is set up yet",
  done: null,
}

const kept: DnsConfig = {
  kind: "adguard",
  host: "192.0.2.53",
  port: 3000,
  https: false,
  pin: null,
  unchecked: false,
  user: "lanstash",
  answer: "192.0.2.10",
}

const report = {
  server: "AdGuard Home",
  plan: {
    answer: "192.0.2.10",
    write_as: "cache.lan",
    services: {
      "*.steampipe.akamaized.net": "steam",
      "*.steamcontent.com": "steam",
      "lancache.steamcontent.com": "steam",
      "level3.blizzard.com": "blizzard",
    },
    answer_from: "config" as const,
    wanted: 4,
    missing: ["*.steampipe.akamaized.net"],
    duplicates: ["*.steamcontent.com"],
    extra: ["cache.lan"],
  },
  plan_error: null,
  missed: [{ name: "steampipe.akamaized.net", service: "steam", count: 3, clients: ["192.0.2.100"] }],
  lookups: 500,
  oldest_ms: 1,
  lookups_error: null,
  refused: [{ host: "other.example", count: 2 }],
  refused_others: 0,
}

const set: DnsPage = { setup: { kinds, config: kept, password_set: true }, report, report_error: null, done: null }

function fake(first: DnsPage) {
  const calls: string[] = []
  const actions: DnsActions = {
    read: async () => first,
    save: async (c, password, test) => {
      calls.push(`${test ? "test" : "save"} ${c.host}:${c.port} ${c.user} ${c.answer ?? "-"} ${password ?? "(kept)"}`)
      if (password === "wrong") return "AdGuard Home refused lanstash's user name and password"
      return test
        ? { ...first, done: { tested: true, kept: false, server: "AdGuard Home", rewrites: 3 } }
        : { ...set, done: { tested: false, kept: true, server: "AdGuard Home", rewrites: 3 } }
    },
    forget: async () => {
      calls.push("forget")
      return unset
    },
    apply: async () => {
      calls.push("apply")
      return { ...set, done: { added: 1, deduplicated: 1 } }
    },
  }
  return { actions, calls }
}

describe("features/dns", () => {
  it("sets the server up: tests it, says why it was refused, saves it", async () => {
    const { actions, calls } = fake(unset)
    render(<DnsView actions={actions} />)
    const host = await screen.findByRole("textbox", { name: "Address" })
    expect((screen.getByRole("textbox", { name: "Port" }) as HTMLInputElement).value, "the kind's port").toBe("3000")
    fireEvent.change(host, { target: { value: "192.0.2.53" } })
    fireEvent.change(screen.getByRole("textbox", { name: "User" }), { target: { value: "lanstash" } })
    const password = screen.getByLabelText("Password") as HTMLInputElement
    expect(password.type, "hidden").toBe("password")
    fireEvent.change(password, { target: { value: "wrong" } })
    fireEvent.click(screen.getByRole("button", { name: "Test the DNS server's setup" }))
    expect(await screen.findByText("AdGuard Home refused lanstash's user name and password")).toBeTruthy()
    fireEvent.change(password, { target: { value: "s3cret" } })
    fireEvent.click(screen.getByRole("button", { name: "Test the DNS server's setup" }))
    expect(
      await screen.findByText(/The test worked\. AdGuard Home answered: 3 rewrites; nothing was saved/),
    ).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Save the DNS server's setup" }))
    expect(await screen.findByText(/^Saved\. AdGuard Home answered: 3 rewrites/)).toBeTruthy()
    expect(calls).toEqual([
      "test 192.0.2.53:3000 lanstash - wrong",
      "test 192.0.2.53:3000 lanstash - s3cret",
      "save 192.0.2.53:3000 lanstash - s3cret",
    ])
  })
  it("keeps the password when none is typed, shows the rewrites and coverage, applies and forgets", async () => {
    const { actions, calls } = fake(set)
    render(<DnsView actions={actions} />)
    expect(await screen.findByText("3 of 4 names lanstash wants are rewritten to 192.0.2.10 (set here).")).toBeTruthy()
    expect(screen.getByText("3 of 4"), "the band over the page").toBeTruthy()
    expect(
      screen.getByText("192.0.2.53:3000 · HTTP · signed in as lanstash · rewrites answer cache.lan"),
      "the setup in a line",
    ).toBeTruthy()
    expect(screen.queryByLabelText("Password"), "no form until Change").toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Change the DNS server's setup" }))
    expect((screen.getByLabelText("Password") as HTMLInputElement).placeholder).toBe("leave empty to keep it")
    for (const name of ["*.steampipe.akamaized.net", "*.steamcontent.com", "other.example"]) {
      expect(screen.getByText(name), name).toBeTruthy()
    }
    expect(screen.getByText("steampipe.akamaized.net")).toBeTruthy()
    expect(screen.getByText("there twice"), "*.steamcontent.com's state").toBeTruthy()
    expect(screen.getByText("missing"), "*.steampipe.akamaized.net's state").toBeTruthy()
    expect(screen.queryByText("level3.blizzard.com"), "another tab").toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Battle.net" }))
    expect(screen.getByText("level3.blizzard.com")).toBeTruthy()
    expect(screen.getByText("to cache.lan")).toBeTruthy()
    expect(screen.queryByText("not on lanstash's lists"), "not under a platform's tab").toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Not lanstash's · 1" }))
    expect(screen.getByText("cache.lan")).toBeTruthy()
    expect(screen.getByText("not on lanstash's lists")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Steam · 1 missing" }))
    fireEvent.click(screen.getByRole("button", { name: "Add 1 rewrites to AdGuard Home" }))
    expect(await screen.findByText("Added 1 rewrite, 1 duplicate merged.")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Save the DNS server's setup" }))
    await screen.findByText(/^Saved\./)
    expect(screen.queryByLabelText("Password"), "the form closes once saved").toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Change the DNS server's setup" }))
    fireEvent.click(screen.getByRole("button", { name: "Forget the DNS server" }))
    await screen.findByText("not set up")
    expect(calls).toEqual(["apply", "save 192.0.2.53:3000 lanstash 192.0.2.10 (kept)", "forget"])
  })
  it("speaks HTTPS, and trusts an untrusted certificate with one click, or checks none", async () => {
    const tried: DnsConfig[] = []
    const fp = "AB:CD:EF"
    const actions: DnsActions = {
      ...fake(unset).actions,
      save: async (c) => {
        tried.push(c)
        return c.pin === null && !c.unchecked
          ? `AdGuard Home at h:443: h: its certificate is not trusted: UnknownIssuer (untrusted certificate, SHA-256 ${fp})`
          : { ...unset, done: { tested: true, kept: false, server: "AdGuard Home", rewrites: 0 } }
      },
    }
    render(<DnsView actions={actions} />)
    fireEvent.change(await screen.findByRole("textbox", { name: "Address" }), { target: { value: "h" } })
    fireEvent.change(screen.getByRole("textbox", { name: "User" }), { target: { value: "lanstash" } })
    fireEvent.click(screen.getByRole("button", { name: "HTTPS" }))
    expect((screen.getByRole("textbox", { name: "Port" }) as HTMLInputElement).value, "HTTPS's usual port").toBe("443")
    fireEvent.click(screen.getByRole("button", { name: "Test the DNS server's setup" }))
    expect(await screen.findByText(/Its SHA-256 is AB:CD:EF/)).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Trust this certificate and test again" }))
    expect(await screen.findByText(/The test worked/)).toBeTruthy()
    expect((screen.getByRole("textbox", { name: "Certificate SHA-256" }) as HTMLInputElement).value).toBe(fp)
    fireEvent.click(screen.getByRole("button", { name: "Don't check" }))
    expect(screen.getByText(/could pose as the server/)).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Test the DNS server's setup" }))
    await screen.findByText(/The test worked/)
    expect(tried.map((c) => [c.https, c.port, c.pin, c.unchecked])).toEqual([
      [true, 443, null, false],
      [true, 443, fp, false],
      [true, 443, null, true],
    ])
  })
  it("points the names at a name, and says when that name still needs its rewrite", async () => {
    const tried: DnsConfig[] = []
    const through = {
      ...set,
      setup: { ...set.setup, config: { ...kept, via: "cache.lan" } },
      report: { ...report, plan: { ...report.plan, via: "cache.lan", via_missing: true } },
    }
    const actions: DnsActions = {
      ...fake(through).actions,
      save: async (c) => {
        tried.push(c)
        return through
      },
    }
    render(<DnsView actions={actions} />)
    expect(await screen.findByText("cache.lan has no rewrite yet: Add creates it, to 192.0.2.10")).toBeTruthy()
    expect(
      screen.getByRole("button", { name: "Add 2 rewrites to AdGuard Home" }),
      "one missing, and the name",
    ).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Change the DNS server's setup" }))
    expect((screen.getByRole("textbox", { name: "Name" }) as HTMLInputElement).value).toBe("cache.lan")
    fireEvent.click(screen.getByRole("button", { name: "The address" }))
    expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull()
    fireEvent.click(screen.getByRole("button", { name: "Test the DNS server's setup" }))
    await screen.findByText(/cache.lan has no rewrite yet/)
    expect(tried.map((c) => c.via)).toEqual([null])
  })
  it("asks a kind that signs in with a password alone for no user", async () => {
    const pihole = { id: "pihole", name: "Pi-hole", port: 80, https_port: 443, user: false }
    const page: DnsPage = {
      ...set,
      setup: { kinds: [...kinds, pihole], config: { ...kept, kind: "pihole", port: 80, user: "" }, password_set: true },
      report: { ...report, server: "Pi-hole" },
    }
    render(<DnsView actions={fake(page).actions} />)
    expect(await screen.findByText(/^192\.0\.2\.53:80 · HTTP · signed in with its password/)).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Change the DNS server's setup" }))
    expect(screen.queryByRole("textbox", { name: "User" })).toBeNull()
    expect(screen.getByText(/Pi-hole's web password/)).toBeTruthy()
  })
  it("shows names the server's own rules send to lanstash, with the rule", async () => {
    const rule = "||steamcontent.com^$dnsrewrite=192.0.2.10"
    const ruled = {
      ...set,
      report: { ...report, plan: { ...report.plan, duplicates: [], covered: { "*.steamcontent.com": rule } } },
    }
    render(<DnsView actions={fake(ruled).actions} />)
    const by = await screen.findByText("by a rule")
    expect(by.closest("[title]")?.getAttribute("title")).toBe(rule)
    expect(screen.getByText(/1 sent to lanstash by AdGuard Home's rules \(no rewrite added\)/)).toBeTruthy()
  })
  it("says when there is no DNS page, and when lanstash is older than the page", async () => {
    const { unmount } = render(<DnsView />)
    expect(await screen.findByText("the DNS server's setup is not available here")).toBeTruthy()
    unmount()
    render(<DnsView actions={{ ...fake(unset).actions, read: async () => "not found" }} />)
    expect(await screen.findByText("This lanstash has no DNS page yet")).toBeTruthy()
  })
})

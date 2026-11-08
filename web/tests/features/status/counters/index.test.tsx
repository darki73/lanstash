import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Counters, counterNames } from "@/features/status/counters"
import { keptOf, snapshot } from "../../../kit"

describe("features/status/counters", () => {
  it("lists each group asked for, its counters over the range, since start and now", () => {
    const s = snapshot({
      counters: { requests: { hits: 3, hit_bytes: 4e9 }, eviction: { stored_bytes: 7e9 } },
      rates_mb_s: { "requests.hit_bytes": 2 },
    })
    render(
      <Counters
        snapshot={s}
        groups={["requests", "eviction", "https"]}
        kept={keptOf({ "requests.hits": [5, 6], "requests.hit_bytes": [1e9] })}
      />,
    )
    expect(screen.getByText("last 24 hours")).toBeTruthy()
    expect(screen.getByText("requests")).toBeTruthy()
    expect(screen.queryByText("https")).toBeNull()
    expect(screen.getByText("11")).toBeTruthy()
    expect(screen.getByText("1.00 GB")).toBeTruthy()
    expect(screen.getByText("4.00 GB")).toBeTruthy()
    expect(screen.getByText("2.0 MB/s")).toBeTruthy()
    // A level has no rise over a range.
    expect(screen.getByText("7.00 GB").closest("tr")?.textContent).toContain("–")
  })
  it("names the counters the groups hold", () => {
    const s = snapshot({ counters: { requests: { hits: 1, misses: 0 }, https: { active: 2 } } })
    expect(counterNames(s, ["requests", "https", "eviction"])).toEqual([
      "requests.hits",
      "requests.misses",
      "https.active",
    ])
  })
})

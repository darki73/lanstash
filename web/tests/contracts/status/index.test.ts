import { describe, expect, it } from "vitest"
import { isSnapshot, type Snapshot } from "@/contracts/status"

/** A snapshot as lanstash writes it (crates/adapters/src/status_page.rs, render_json). */
const sent = `{"now_ms":1793879185924,"started_ms":1793878672956,
"counters":{"requests":{"hits":3,"misses":8,"hit_bytes":33068},"https":{"active":2}},
"rates_mb_s":{"requests.hit_bytes":0.125},
"downloads":[{"key":"steam/depot/1/chunk/a","span":[0,16777216],"received":100,"total":null},
{"key":"blizzard/tpr/x","span":[5,null],"received":7,"total":9},{"key":"k","span":null,"received":0,"total":null}],
"usage_ms":1793879180000,
"usage":[{"service":"blizzard","game":"osi","name":"Diablo II: Resurrected","objects":1,"bytes":2},
{"service":"steam","game":"1422456","name":null,"objects":3,"bytes":4}]}`

describe("contracts/status", () => {
  it("reads what lanstash sends, field for field", () => {
    const s = JSON.parse(sent) as Snapshot
    expect(Object.keys(s).sort()).toEqual(
      ["counters", "downloads", "now_ms", "rates_mb_s", "started_ms", "usage", "usage_ms"].sort(),
    )
    expect(s.downloads.map((d) => d.span)).toEqual([[0, 16777216], [5, null], null])
    expect(Object.keys(s.downloads[0] ?? {}).sort()).toEqual(["key", "received", "span", "total"])
    expect(Object.keys(s.usage[0] ?? {}).sort()).toEqual(["bytes", "game", "name", "objects", "service"])
    expect(s.usage[1]?.name).toBeNull()
  })
  it("tells a snapshot from what the page could not draw", () => {
    expect(isSnapshot(JSON.parse(sent))).toBe(true)
    const s = JSON.parse(sent) as Record<string, unknown>
    const odd: unknown[] = [
      null,
      [],
      "status",
      { ...s, now_ms: "soon" },
      { ...s, counters: { requests: 3 } },
      { ...s, downloads: [{}] },
      { ...s, usage: [{ service: "steam" }] },
      { ...s, usage: undefined },
    ]
    for (const v of odd) expect(isSnapshot(v), JSON.stringify(v)).toBe(false)
  })
})

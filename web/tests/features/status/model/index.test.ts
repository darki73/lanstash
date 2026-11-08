import { describe, expect, it } from "vitest"
import { HIDDEN_NAME } from "@/contracts/hidden"
import {
  counter,
  counterGroups,
  groupDownloads,
  hitRates,
  isGauge,
  many,
  objectsInFlight,
  peak,
  perSecond,
  rate,
  series,
  shownName,
  storage,
  storedServices,
  throughput,
} from "@/features/status/model"
import { snapshot } from "../../../kit"

const MiB16 = 16 * 1024 * 1024

describe("features/status/model", () => {
  it("shows a masked game by the reader's word for it, and gives a plural's count and number", () => {
    expect([shownName(HIDDEN_NAME, "Un jeu caché"), shownName("Deadlock", "Un jeu caché")]).toEqual([
      "Un jeu caché",
      "Deadlock",
    ])
    expect(many(1234)).toEqual({ count: 1234, n: "1,234" })
  })
  it("sums the parallel pieces of an object into one group, pieces in order", () => {
    const groups = groupDownloads([
      { key: "steam/a", span: [MiB16, 2 * MiB16], received: MiB16 + 100, total: 3 * MiB16 },
      { key: "steam/a", span: [0, MiB16], received: MiB16, total: 3 * MiB16 },
      { key: "blizzard/b", span: [10, 20], received: 15, total: null },
    ])
    expect(groups.map((g) => [g.key, g.count])).toEqual([
      ["steam/a", 2],
      ["blizzard/b", 1],
    ])
    expect(groups[0]?.pieces.map((p) => p.start)).toEqual([0, MiB16])
    expect(groups[0]?.progress).toEqual({
      kind: "ranged",
      from: 0,
      to: 2 * MiB16,
      done: MiB16 + 100,
      length: 2 * MiB16,
    })
    expect(groups[1]?.progress).toEqual({ kind: "ranged", from: 10, to: 20, done: 5, length: 10 })
  })
  it("counts only received bytes when a range is not known", () => {
    const [g] = groupDownloads([
      { key: "k", span: null, received: 500, total: null },
      { key: "k", span: [0, null], received: 200, total: null },
    ])
    expect([g?.progress, g?.pieces]).toEqual([{ kind: "open", received: 700 }, []])
  })
  it("counts an object in pieces once in flight", () => {
    const s = snapshot({
      downloads: [
        { key: "a", span: [0, 1], received: 0, total: 1 },
        { key: "a", span: [1, 2], received: 1, total: 1 },
        { key: "b", span: null, received: 0, total: null },
      ],
    })
    expect(objectsInFlight(s)).toBe(2)
  })
  it("reads counters and rates, 0 when they are not there", () => {
    const s = snapshot({ rates_mb_s: { "requests.hit_bytes": 390 } })
    expect([counter(s, "requests", "hits"), counter(s, "https", "active")]).toEqual([3, 0])
    expect([rate(s, "requests.hit_bytes"), rate(s, "downloads.fetched_bytes")]).toEqual([390, 0])
  })
  it("works out throughput, hit rates and storage", () => {
    const s = snapshot({
      rates_mb_s: { "requests.hit_bytes": 390, "downloads.fetched_bytes": 40 },
      counters: {
        requests: { hits: 3, misses: 1, hit_bytes: 300, miss_bytes: 100 },
        eviction: { stored_bytes: 50, disk_free_bytes: 7000, evicted_bytes: 9 },
      },
    })
    expect(throughput(s)).toEqual({ fromCache: 390, downloading: 0, fromInternet: 40 })
    expect(hitRates(s)).toEqual({ objects: 0.75, bytes: 0.75 })
    expect(hitRates(snapshot({ counters: {} }))).toEqual({ objects: undefined, bytes: undefined })
    expect(storage(s)).toEqual({ stored: 50, free: 7000, evictedBytes: 9 })
  })
  it("takes the stored bytes from the usage when eviction is off", () => {
    const usage = [{ service: "steam", game: "1", name: null, objects: 1, bytes: 12 }]
    expect(storage(snapshot({ counters: {}, usage }))).toEqual({ stored: 12, free: undefined, evictedBytes: 0 })
  })
  it("traces a value per snapshot, and a counter's change a second", () => {
    const at = (now_ms: number, requests: number) => snapshot({ now_ms, counters: { requests: { requests } } })
    const recent = [at(0, 10), at(2000, 30), at(3000, 30), at(4000, 5)]
    expect(series(recent, (s) => s.now_ms)).toEqual([0, 2000, 3000, 4000])
    expect(perSecond(recent, "requests", "requests")).toEqual([10, 0, 0])
    expect(perSecond(recent.slice(0, 1), "requests", "requests")).toEqual([])
    expect([peak([1, 7, 3]), peak([])]).toEqual([7, 0])
  })
  it("lists the counter groups asked for, gauges without a rate", () => {
    const s = snapshot({
      counters: {
        requests: { hits: 3, hit_bytes: 4 },
        eviction: { stored_bytes: 5, evicted_bytes: 6 },
      },
      rates_mb_s: { "requests.hit_bytes": 12.5, "eviction.stored_bytes": 1 },
    })
    expect(counterGroups(s, ["eviction", "https", "requests"])).toEqual([
      {
        name: "eviction",
        rows: [
          { key: "stored_bytes", name: "stored bytes", value: 5, bytes: true, rate: undefined },
          { key: "evicted_bytes", name: "evicted bytes", value: 6, bytes: true, rate: 0 },
        ],
      },
      {
        name: "requests",
        rows: [
          { key: "hits", name: "hits", value: 3, bytes: false, rate: undefined },
          { key: "hit_bytes", name: "hit bytes", value: 4, bytes: true, rate: 12.5 },
        ],
      },
    ])
  })
  it("knows a level from a running total", () => {
    expect([isGauge("eviction.stored_bytes"), isGauge("https.active"), isGauge("requests.hits")]).toEqual([
      true,
      true,
      false,
    ])
  })
  it("sums the stored bytes per service, the largest first", () => {
    const row = (service: string, game: string, bytes: number) => ({ service, game, name: null, objects: 1, bytes })
    const s = storedServices([row("steam", "1", 5), row("blizzard", "d2r", 30), row("steam", "2", 40)])
    expect(s.map((x) => [x.service, x.objects, x.bytes, x.games.map((g) => g.game)])).toEqual([
      ["steam", 2, 45, ["2", "1"]],
      ["blizzard", 1, 30, ["d2r"]],
    ])
  })
})

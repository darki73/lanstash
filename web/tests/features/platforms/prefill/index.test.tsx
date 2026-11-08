import { fireEvent, render, renderHook, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import type { Prefill } from "@/contracts/prefill"
import { bitrate, JobLine, PrefillPanel, pace, timeLeft, verb } from "@/features/platforms/prefill"
import { useT } from "@/i18n"

const job = (over: Partial<Prefill>): Prefill => ({
  game: "hs",
  version: "37.0",
  state: "running",
  files_total: 40,
  files_done: 10,
  files_skipped: 4,
  files_missing: 1,
  bytes: 2_000_000_000,
  current: "/tpr/hs/data/ab/cd/abcd",
  error: null,
  started_ms: 0,
  finished_ms: null,
  ...over,
})

const reading = (label: string) => screen.getByText(label).nextSibling?.textContent

describe("features/platforms/prefill", () => {
  it("draws a running job as a band with its art, live readings, state and Stop", () => {
    const stop = vi.fn()
    const samples = [
      { at: 0, job: job({ bytes: 0, bytes_local: 0 }) },
      { at: 1000, job: job({ bytes: 3_000_000, bytes_local: 1_000_000 }) },
    ]
    render(
      <PrefillPanel
        job={job({ bytes_total: 4_000_000_000, from: "lancache" })}
        name="Hearthstone"
        artOf={(g) => `/art/${g}`}
        samples={samples}
        now={40_000}
        onStop={stop}
      />,
    )
    const band = screen.getByRole("region", { name: "Import · Hearthstone" })
    expect(band.querySelector("img")?.getAttribute("src")).toBe("/art/hs")
    expect(reading("Network")).toBe("16.0 Mbps")
    expect(reading("Lancache")).toBe("8.0 Mbps")
    expect(reading("Peak")).toBe("24.0 Mbps")
    expect(reading("Taken")).toBe("2.00 GB of 4.00 GB")
    expect(screen.getByText("2 minutes left · 10 of 40 requests")).toBeTruthy()
    expect(screen.getByRole("meter", { name: "Hearthstone: 25.0 %" })).toBeTruthy()
    expect(screen.getByText("not on the CDN").nextSibling?.textContent).toBe("1")
    expect(screen.getByText("/tpr/hs/data/ab/cd/abcd").getAttribute("title")).toBe("/tpr/hs/data/ab/cd/abcd")
    fireEvent.click(screen.getByRole("button", { name: "Stop Hearthstone" }))
    expect(stop).toHaveBeenCalledTimes(1)
  })
  it("takes what the stash lacked as its total, and says what it held already", () => {
    // An install of 91.31 GB, all but 36.7 MB of it stashed already.
    const held = { bytes_total: 91_310_000_000, bytes_skipped: 91_273_300_000, files_skipped: 50_900 }
    const { rerender } = render(
      <PrefillPanel job={job({ ...held, bytes: 10_000_000 })} name="WoW" now={1} onStop={() => {}} />,
    )
    expect(reading("Taken")).toBe("10.0 MB of 36.7 MB")
    expect(screen.getByText("already stashed").nextSibling?.textContent).toBe("91.27 GB")
    const done = job({ ...held, bytes: 36_700_000, state: "done", finished_ms: 3000 })
    rerender(<PrefillPanel job={done} name="WoW" now={3000} onStop={() => {}} />)
    expect(screen.getByText("Done in 3 seconds: 36.7 MB from the CDN, 91.27 GB already stashed")).toBeTruthy()
    // Said once: no figures repeating the line, no rates after the end, the missing file still counted.
    expect([screen.queryByText("already stashed"), screen.queryByText("took"), screen.queryByText("Network")]).toEqual([
      null,
      null,
      null,
    ])
    expect(screen.queryByText("Peak"), "no rate seen").toBeNull()
    expect(screen.getByText("not on the CDN").nextSibling?.textContent).toBe("1")
    rerender(<PrefillPanel job={{ ...done, files_missing: 0 }} name="WoW" now={3000} onStop={() => {}} />)
    expect(screen.queryByText("not on the CDN"), "a quiet zero").toBeNull()
    // From an older lanstash: the count it gave.
    rerender(<PrefillPanel job={job({ bytes_total: 4_000_000_000 })} name="WoW" now={1} onStop={() => {}} />)
    expect(reading("Taken")).toBe("2.00 GB of 4.00 GB")
    expect(screen.getByText("already stashed").nextSibling?.textContent).toBe("4")
  })
  it("lists the queue with names, art and sizes, each removable", () => {
    const removed: string[] = []
    render(
      <PrefillPanel
        job={job({ queued: ["1551360", "7"] })}
        name="Hearthstone"
        nameOf={(g) => (g === "1551360" ? "Forza Horizon 5" : g)}
        sizeOf={(g) => (g === "1551360" ? 880_000_000 : undefined)}
        linkOf={(g) => `/platforms/steam/${g}`}
        now={1}
        onStop={() => {}}
        onUnqueue={(g) => removed.push(g)}
      />,
    )
    expect(screen.getByRole("heading", { name: "Up next (2)" })).toBeTruthy()
    expect(screen.getByRole("link", { name: "Forza Horizon 5" }).getAttribute("href")).toBe("/platforms/steam/1551360")
    expect(screen.getByText("880.0 MB · 1551360")).toBeTruthy()
    expect(screen.getByRole("button", { name: "Stop Hearthstone and forget the queue" })).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "Take Forza Horizon 5 out of the queue" }))
    expect(removed).toEqual(["1551360"])
  })
  it("counts what lancache lacked and what failed its check in a lancache-only import", () => {
    const only = job({ from: "lancache-only", files_missing: 7, bytes_local: 1_500_000_000, files_rejected: 2 })
    render(<PrefillPanel job={only} name="Dota 2" now={1} onStop={() => {}} />)
    expect(screen.getByRole("region", { name: "Import from lancache · Dota 2" })).toBeTruthy()
    expect(screen.getByText("from lancache").nextSibling?.textContent).toBe("1.50 GB")
    expect(screen.getByText("from the CDN").nextSibling?.textContent).toBe("500.0 MB")
    expect(screen.getByText("not in lancache").nextSibling?.textContent).toBe("7")
    expect(screen.getByText("failed their check").nextSibling?.textContent).toBe("2")
    expect(screen.getByText("Nothing waits after this one.")).toBeTruthy()
  })
  it("says how far the planning has come, how a job ended, and how one starts", () => {
    const { rerender } = render(
      <PrefillPanel
        job={job({ files_total: 0, planning: { done: 3, total: 9 } })}
        name="x"
        now={0}
        onStop={() => {}}
      />,
    )
    expect(screen.getByText("Working out the downloads: 3/9 indexes")).toBeTruthy()
    rerender(
      <PrefillPanel
        job={job({ state: "failed", error: "no CDN", finished_ms: 60_000 })}
        name="Hearthstone"
        now={0}
        onStop={() => {}}
      />,
    )
    expect(screen.getByText("Failed after 60 seconds")).toBeTruthy()
    expect(screen.getByText("no CDN")).toBeTruthy()
    expect(screen.queryByRole("button")).toBeNull()
    rerender(<PrefillPanel job={null} name="x" now={0} onStop={() => {}} idle="Start one from Cached." />)
    expect(screen.getByText("No prefill yet")).toBeTruthy()
    expect(screen.getByText("Start one from Cached.")).toBeTruthy()
  })
  it("says what a finished job took, so one that found nothing does not read as a download", () => {
    const done = (over: Partial<Prefill>) => job({ state: "done", finished_ms: 120_000, ...over })
    const line = (j: Prefill) => {
      const { unmount } = render(<PrefillPanel job={j} name="x" now={0} onStop={() => {}} />)
      const text = screen.getByText(/^Done in/).textContent
      unmount()
      return text
    }
    expect(line(done({ from: "lancache-only", bytes: 0, bytes_local: 0 }))).toBe(
      "Done in 2 minutes: lancache held none of it",
    )
    expect(line(done({ from: "lancache-only", bytes: 3e9, bytes_local: 3e9 }))).toBe(
      "Done in 2 minutes: took 3.00 GB from lancache",
    )
    expect(line(done({ bytes: 3e9, bytes_local: 1e9 }))).toBe(
      "Done in 2 minutes: 1.00 GB from lancache, 2.00 GB from the CDN",
    )
    expect(line(done({ bytes: 0, files_done: 10, files_skipped: 10 }))).toBe(
      "Done in 2 minutes: all of it was stashed already",
    )
  })
  it("works out a job's pace from the snapshots of that job only", () => {
    const other = job({ started_ms: 5, bytes: 9e9 })
    const p = pace(
      [
        { at: 0, job: job({ bytes: 0 }) },
        { at: 500, job: other },
        { at: 2000, job: job({ bytes: 4_000_000, bytes_local: 1_000_000 }) },
        { at: 3000, job: job({ bytes: 5_000_000, bytes_local: 1_000_000 }) },
      ],
      job({}),
    )
    expect(p.moments).toEqual([
      { cache: 500_000, origin: 1_500_000 },
      { cache: 0, origin: 1_000_000 },
    ])
    // Now: the last 5 s (0 to 3000 ms here): 5 MB in 3 s, 1 MB of it from lancache.
    expect(p.local).toBeCloseTo(1_000_000 / 3)
    expect(p.origin).toBeCloseTo(4_000_000 / 3)
    expect(p.peak).toBe(2_000_000)
    // A job whose bytes grow a whole file at a time does not read 0 between two files.
    const steps = [0, 1000, 2000, 3000, 4000].map((at, i) => ({ at, job: job({ bytes: i < 3 ? 0 : 16_000_000 }) }))
    expect(pace([...steps, { at: 5000, job: job({ bytes: 16_000_000 }) }], job({})).origin).toBe(3_200_000)
    expect([bitrate(0), bitrate(1500), bitrate(3_000_000), bitrate(2e8)]).toEqual([
      "0 bps",
      "12 kbps",
      "24.0 Mbps",
      "1.60 Gbps",
    ])
  })
  it("names what a job does, and estimates the time left from its pace", () => {
    const t = renderHook(() => useT("platforms")).result.current
    expect(verb(t, job({}))).toBe("Prefill")
    expect(verb(t, job({ from: "lancache" }))).toBe("Import")
    expect(verb(t, job({ prerelease: true }))).toBe("Pre-load")
    expect(timeLeft(job({}), 40_000)).toBe(120)
    expect(timeLeft(job({ files_done: 0 }), 40_000)).toBeUndefined()
    expect(timeLeft(job({ state: "done" }), 40_000)).toBeUndefined()
  })
  it("shows a running job as a slim band, leading to the panel", () => {
    render(<JobLine job={job({ queued: ["a"] })} name="Hearthstone" now={40_000} href="/platforms/blizzard/prefill" />)
    expect(screen.getByRole("region", { name: "Prefill · 1 more queued · Hearthstone" })).toBeTruthy()
    expect(screen.getByRole("link", { name: "Details" }).getAttribute("href")).toBe("/platforms/blizzard/prefill")
    expect(screen.queryByText("Network"), "no readings in the slim band").toBeNull()
  })
})

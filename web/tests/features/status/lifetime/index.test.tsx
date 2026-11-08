import { render, renderHook, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { day, Lifetime, savings, totals } from "@/features/status/lifetime"
import { useT } from "@/i18n"
import { keptOf, snapshot } from "../../../kit"

const since = Date.UTC(2026, 10, 5, 10)
const counted = snapshot({
  counters: {
    requests: { hit_bytes: 1 },
    lifetime: { served_bytes: 3e12, fetched_bytes: 1e12, since_ms: since },
  },
})

describe("features/status/lifetime", () => {
  it("says, over the range, what clients got from the stash against the internet, and what was fetched besides", () => {
    const k = keptOf({
      "requests.hit_bytes": [3e9],
      "requests.miss_bytes": [1e9],
      "downloads.fetched_bytes": [6e9],
    })
    expect(savings(k)).toEqual({ fromStash: 3e9, downloading: 1e9, besides: 5e9 })
    render(<Lifetime snapshot={counted} kept={k} />)
    expect(
      screen.getByText("clients got from the stash in the last 24 hours, instead of from the internet"),
    ).toBeTruthy()
    expect(screen.getByText("3.00 GB · 75 %")).toBeTruthy()
    expect(screen.getByText("1.00 GB · 25 %")).toBeTruthy()
    expect(screen.getByText("5.00 GB")).toBeTruthy()
    expect(screen.getByText("From the stash since 5 Nov 2026")).toBeTruthy()
    expect(screen.getByText("3.00 TB")).toBeTruthy()
  })
  it("never counts less than nothing fetched besides", () => {
    expect(savings(keptOf({ "requests.miss_bytes": [5], "downloads.fetched_bytes": [3] })).besides).toBe(0)
  })
  it("says what was served since lanstash first counted when nothing over a range is known", () => {
    expect(totals(counted)).toEqual({ served: 3e12, fetched: 1e12, since })
    render(<Lifetime snapshot={counted} kept={null} />)
    expect(screen.getByText("clients got from the stash since 5 Nov 2026, instead of from the internet")).toBeTruthy()
    expect(screen.getByText("3.00 TB · 75 %")).toBeTruthy()
  })
  it("says the same since this start from a lanstash without lifetime totals, and nothing before anything moved", () => {
    const s = snapshot({
      counters: { requests: { hit_bytes: 300 }, downloads: { fetched_bytes: 50 }, "pass-through": { bytes: 50 } },
    })
    expect(totals(s)).toEqual({ served: 300, fetched: 100, since: undefined })
    const { container, rerender } = render(<Lifetime snapshot={s} />)
    expect(screen.getByText(/since lanstash started/)).toBeTruthy()
    rerender(<Lifetime snapshot={snapshot({ counters: {} })} />)
    expect(container.textContent).toBe("")
  })
  it("writes a day the same everywhere", () => {
    const t = renderHook(() => useT("status")).result.current
    expect(day(Date.UTC(2026, 0, 31, 23), t)).toBe("31 Jan 2026")
  })
})

import { fireEvent, render, renderHook, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import type { HealthReader } from "@/contracts/health"
import { checksWords, doing, Pulse } from "@/features/status/pulse"
import { useT } from "@/i18n"
import { KEPT_TO, keptOf, snapshot } from "../../../kit"

const words = () => renderHook(() => useT("status")).result.current

const client = { address: "192.0.2.5", label: null, host: null, service: "steam", game: "10" }
const serving = snapshot({
  clients: [
    { ...client, per_sec: 30e6, hit_bytes: 1, miss_bytes: 0, total_bytes: 1, last_ms: 0 },
    { ...client, game: "20", per_sec: 10e6, hit_bytes: 1, miss_bytes: 0, total_bytes: 1, last_ms: 0 },
  ],
})

describe("features/status/pulse", () => {
  it("says what lanstash is doing: serving, downloading, prefilling or idle", () => {
    const t = words()
    expect(doing(serving, t)).toEqual({ busy: true, words: "Serving 1 client · 40.0 MB/s" })
    expect(doing(snapshot({ downloads: [{ key: "a", span: null, received: 0, total: null }] }), t).words).toBe(
      "Downloading 1 object",
    )
    expect(doing(snapshot(), t)).toEqual({ busy: false, words: "Idle" })
  })
  it("says how the checks stand", () => {
    const t = words()
    const check = (level: "ok" | "warn" | "fail") => ({ level, what: "x", detail: "" })
    expect(checksWords([check("ok")], t)).toBe("all checks pass")
    expect(checksWords([check("warn"), check("warn")], t)).toBe("2 warnings")
    expect(checksWords([check("warn"), check("fail")], t)).toBe("1 check failing")
  })
  it("says when clients last got something, links the checks, and switches the range", async () => {
    const health: HealthReader = async () => [{ level: "fail", what: "disk", detail: "" }]
    const onRange = vi.fn()
    const k = keptOf({ "requests.hit_bytes": [5, 0, 0] })
    render(
      <Pulse
        snapshot={snapshot({ now_ms: KEPT_TO })}
        kept={k}
        health={health}
        healthHref="/health"
        range="day"
        onRange={onRange}
        every={0}
      />,
    )
    expect(screen.getByText("Idle")).toBeTruthy()
    expect(screen.getByText("clients last got something 20 min ago")).toBeTruthy()
    expect((await screen.findByRole("link", { name: "1 check failing" })).getAttribute("href")).toBe("/health")
    fireEvent.click(screen.getByRole("button", { name: "Month" }))
    expect(onRange).toHaveBeenCalledWith("month")
  })
})

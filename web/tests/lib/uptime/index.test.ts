import { describe, expect, it } from "vitest"
import { uptime } from "@/lib/uptime"

describe("lib/uptime", () => {
  it("gives hours, minutes and seconds", () => {
    expect(uptime(((27 * 60 + 7) * 60 + 9) * 1000)).toEqual({ h: 27, mm: "07", ss: "09" })
    expect(uptime(-5)).toEqual({ h: 0, mm: "00", ss: "00" })
  })
})

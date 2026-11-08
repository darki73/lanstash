import { describe, expect, it } from "vitest"
import { gigabytes, megabytes, shortSize, size } from "@/lib/bytes"

describe("lib/bytes", () => {
  it("says a size in the unit that fits", () => {
    expect([size(3_480_550_000_000), size(41_270_000_000), size(16_800_000), size(48_000), size(512)]).toEqual([
      "3.48 TB",
      "41.27 GB",
      "16.8 MB",
      "48 KB",
      "512 B",
    ])
    expect(size(0)).toBe("0 B")
  })
  it("says sizes in decimal units", () => {
    expect(gigabytes(29_610_000_000)).toBe("29.61 GB")
    expect(megabytes(16_777_216)).toBe("16.8 MB")
  })
  it("says a small size in kilobytes", () => {
    expect(shortSize(48_000)).toBe("48 KB")
    expect(shortSize(2_500_000)).toBe("2.5 MB")
  })
})

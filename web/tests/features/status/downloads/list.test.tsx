import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { Download } from "@/contracts/status"
import { DownloadList } from "@/features/status/downloads/list"

const pieces: Download[] = [
  { key: "steam/depot/1/chunk/a", span: [0, 16_000_000], received: 8_000_000, total: 32_000_000 },
  { key: "steam/depot/1/chunk/a", span: [16_000_000, 32_000_000], received: 16_000_000, total: 32_000_000 },
]

describe("features/status/downloads/list", () => {
  it("says why it is empty when nothing is in flight", () => {
    render(<DownloadList downloads={[]} />)
    expect(screen.getByText("Nothing downloading")).toBeTruthy()
    expect(screen.getByText("0 objects")).toBeTruthy()
  })
  it("draws one row per object and lists its pieces when opened", () => {
    render(<DownloadList downloads={pieces} />)
    expect(screen.getByText("1 object, 2 pieces")).toBeTruthy()
    expect(screen.getByText("0.0 MB – 32.0 MB")).toBeTruthy()
    expect(screen.getByRole("meter").getAttribute("value")).toBe("25")
    fireEvent.click(screen.getByRole("button", { name: "▸ 2 pieces" }))
    expect(screen.getAllByRole("meter").map((m) => m.getAttribute("value"))).toEqual(["25", "50", "0"])
    expect(screen.getByText("at 16.0 MB, 16.0 MB")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: "▾ 2 pieces" }))
    expect(screen.getAllByRole("meter")).toHaveLength(1)
  })
  it("says the received bytes of a whole object", () => {
    render(<DownloadList downloads={[{ key: "k", span: null, received: 2_500_000, total: null }]} />)
    expect([screen.getByText("whole"), screen.getByText("2.5 MB"), screen.getByText("?")]).toHaveLength(3)
  })
})

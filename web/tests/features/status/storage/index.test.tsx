import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { StorageTile, StorageView } from "@/features/status/storage"
import { keptOf, live, snapshot } from "../../../kit"

const held = live(
  snapshot({
    counters: { eviction: { stored_bytes: 41_270_000_000, disk_free_bytes: 3_480_000_000_000, passes: 25 } },
    rates_mb_s: { "downloads.stored_bytes": 40 },
    usage: [{ service: "steam", game: "10", name: "Game A", group: "1", objects: 3, bytes: 41_270_000_000 }],
  }),
)
const day = keptOf({ "downloads.stored_bytes": [1e9, 2e9], "eviction.failed": [0, 0] })

describe("features/status/storage", () => {
  it("shows the bytes stored and the free space in the tile", () => {
    render(<StorageTile live={held} kept={day} href="/x" current={false} />)
    expect(screen.getByText("41.27 GB")).toBeTruthy()
    expect(screen.getByText("3.48 TB free")).toBeTruthy()
  })
  it("counts the store's entries in the tile when eviction is off", () => {
    render(<StorageTile live={live(snapshot())} kept={undefined} href="/x" current={false} />)
    expect(screen.getByText("0 entries")).toBeTruthy()
  })
  it("shows how full the store is, the range's writes, the library by game and the counters", () => {
    render(<StorageView live={held} kept={day} />)
    expect(screen.getByRole("heading", { name: "Storage" })).toBeTruthy()
    expect(screen.getAllByText("40.0 MB/s").length).toBeGreaterThan(0)
    expect(screen.getByText("41.27 GB stored")).toBeTruthy()
    expect(screen.getByText("3.00 GB")).toBeTruthy()
    expect(screen.getByRole("link", { name: "Game A" })).toBeTruthy()
    expect(screen.getByText("All counters")).toBeTruthy()
  })
})

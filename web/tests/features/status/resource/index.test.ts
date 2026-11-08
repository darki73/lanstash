import { describe, expect, it } from "vitest"
import { overviewLink, resources, secondsPerSnapshot } from "@/features/status/resource"
import { en } from "@/i18n/catalogue"

describe("features/status/resource", () => {
  it("lists the resources of the monitor in rail order, each with a name and a link", () => {
    expect(resources).toEqual(["throughput", "downloads", "requests", "storage", "connections"])
    expect(resources.map((r) => en.status.resource[r])).toEqual([
      "Throughput",
      "Downloads",
      "Requests",
      "Storage",
      "Connections",
    ])
    expect(overviewLink("storage")).toBe("/overview/storage")
    expect(secondsPerSnapshot).toBe(1)
  })
})

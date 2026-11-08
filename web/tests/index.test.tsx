import { act, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"
import { FakeSource } from "./kit"

// Importing the entry loads the whole app's module graph, which takes seconds on a loaded machine.
describe("index", { timeout: 60_000 }, () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.resetModules()
    document.body.innerHTML = ""
  })

  it("fails clearly without #root", async () => {
    await expect(import("@/index")).rejects.toThrow("the page has no #root element")
  })
  it("mounts the app into #root and follows the status stream", async () => {
    const opened: string[] = []
    vi.stubGlobal(
      "EventSource",
      class extends FakeSource {
        constructor(url: string) {
          super(url)
          opened.push(url)
        }
      },
    )
    vi.stubGlobal("fetch", async () => new Response('{"version":"0.1.0"}'))
    document.body.innerHTML = '<div id="root"></div>'
    await act(async () => {
      await import("@/index")
    })
    expect(await screen.findByRole("navigation", { name: "Main" })).toBeTruthy()
    expect(opened).toContain("/api/v1/events")
  })
})

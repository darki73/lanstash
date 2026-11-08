import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { App } from "@/app"
import { compose } from "@/app/compose"
import { harness } from "../kit"

const quietWindow = {
  location: { pathname: "/", hash: "" },
  history: { pushState() {}, replaceState() {} },
  addEventListener() {},
  removeEventListener() {},
}

describe("app", () => {
  it("starts the stream while on screen and stops it when gone", () => {
    const { sources, open, later } = harness()
    const fetch = async () => ({ ok: false, json: async () => null })
    const { unmount } = render(
      <App composition={compose({ apiBase: "", openSource: open, fetch, window: quietWindow, later })} />,
    )
    expect(screen.getByText("connecting…")).toBeTruthy()
    expect(sources).toHaveLength(1)
    unmount()
    expect(sources[0]?.closed).toBe(true)
  })
})

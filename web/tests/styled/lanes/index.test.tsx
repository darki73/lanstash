import { render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Lanes } from "@/styled/lanes"

describe("styled/lanes", () => {
  it("draws each lane's stops in order, what travels between them and what comes of it", () => {
    render(
      <Lanes
        label="Ways"
        lanes={[
          {
            label: "Now",
            tone: "origin",
            stops: [
              { name: "PC", icon: "pc", via: "HTTPS" },
              { name: "CDN", icon: "cloud", passing: true },
            ],
            verdict: "nothing kept",
          },
          {
            label: "Later",
            tone: "cache",
            stops: [
              { name: "PC", icon: "pc", via: "HTTP" },
              { name: "stash", icon: "stash", marked: true },
            ],
            verdict: "kept",
          },
        ]}
      />,
    )
    const now = within(screen.getByRole("listitem", { name: "Now" }))
    expect(now.getByText("PC")).toBeTruthy()
    expect(now.getByText("HTTPS")).toBeTruthy()
    expect(now.getByText("nothing kept")).toBeTruthy()
    expect(within(screen.getByRole("listitem", { name: "Later" })).getByText("kept")).toBeTruthy()
  })
})

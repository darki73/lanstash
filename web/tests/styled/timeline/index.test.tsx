import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Timeline } from "@/styled/timeline"

describe("styled/timeline", () => {
  it("draws days with their entries: time, title (a link where there is a page), kind and versions", () => {
    render(
      <Timeline
        label="History"
        days={[
          {
            label: "Today",
            entries: [
              {
                key: "a",
                time: "14:32",
                title: "RAILGRADE",
                href: "/platforms/epicgames/ad85",
                subtitle: "Epic Games",
                kind: "new",
                label: "new build",
                detail: "v2",
                before: "v1",
              },
              { key: "b", time: "09:05", title: "Seven", kind: "done", label: "current", detail: "12" },
            ],
          },
        ]}
      />,
    )
    expect(screen.getByRole("region", { name: "History" })).toBeTruthy()
    expect(screen.getByRole("heading", { name: "Today" })).toBeTruthy()
    expect(screen.getByRole("link", { name: "RAILGRADE" }).getAttribute("href")).toBe("/platforms/epicgames/ad85")
    expect(screen.queryByRole("link", { name: "Seven" })).toBeNull()
    expect(screen.getByText("new build")).toBeTruthy()
    expect(screen.getByText("v1").className).toContain("line-through")
    expect(screen.getByText("14:32")).toBeTruthy()
    expect(screen.getAllByRole("listitem")).toHaveLength(2)
  })
})

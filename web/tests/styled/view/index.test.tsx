import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { View } from "@/styled/view"

describe("styled/view", () => {
  it("draws the title, what it is about, the aside and the parts", () => {
    render(
      <View title="Storage" subtitle="What the cache holds" aside="41.27 GB">
        parts
      </View>,
    )
    expect(screen.getByRole("region", { name: "Storage" })).toBeTruthy()
    expect(screen.getByRole("heading", { level: 2 }).textContent).toBe("Storage")
    expect(screen.getByText("What the cache holds")).toBeTruthy()
    expect(screen.getByText("41.27 GB")).toBeTruthy()
    expect(screen.getByText("parts")).toBeTruthy()
  })
})

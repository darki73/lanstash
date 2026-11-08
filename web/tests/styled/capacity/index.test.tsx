import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Capacity } from "@/styled/capacity"

describe("styled/capacity", () => {
  it("draws a part per holder at its share of the whole, with a legend", () => {
    render(
      <Capacity
        label="How full"
        held="6 GB stored"
        free="4 GB free"
        parts={[
          { key: "a", label: "Steam", value: "4 GB", share: 0.4 },
          { key: "b", label: "Battle.net", value: "2 GB", share: 0.2 },
        ]}
      />,
    )
    const bar = screen.getByRole("img", { name: "How full" })
    expect([...bar.children].map((p) => (p as HTMLElement).style.width)).toEqual(["40%", "20%"])
    expect(screen.getByText("Steam")).toBeTruthy()
    expect(screen.getByText("4 GB free")).toBeTruthy()
  })
})

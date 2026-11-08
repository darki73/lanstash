import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Readings } from "@/styled/readings"

describe("styled/readings", () => {
  it("lists each name with its figure and note", () => {
    render(
      <Readings
        rows={[
          { name: "hits", value: "3" },
          { name: "hit bytes", value: "4.00 GB", note: "2.0 MB/s" },
        ]}
      />,
    )
    expect(screen.getByText("hits").tagName).toBe("DT")
    expect(screen.getByText("4.00 GB").tagName).toBe("DD")
    expect(screen.getByText("2.0 MB/s")).toBeTruthy()
  })
})

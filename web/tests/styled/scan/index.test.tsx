import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Scan } from "@/styled/scan"

describe("styled/scan", () => {
  it("shows the code with its description and what to do beside it", () => {
    render(
      <Scan src="/code.svg?turn=2" alt="QR code to sign in">
        Scan this.
      </Scan>,
    )
    const img = screen.getByRole("img", { name: "QR code to sign in" })
    expect(img.getAttribute("src")).toBe("/code.svg?turn=2")
    expect(screen.getByText("Scan this.")).toBeTruthy()
  })
})

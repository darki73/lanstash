import { render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Ledger } from "@/styled/ledger"

describe("styled/ledger", () => {
  it("draws the headline, the split with each share, and the rows against the largest", () => {
    const { container } = render(
      <Ledger
        label="What lanstash did"
        headline="4.00 TB"
        caption="from the stash"
        split={[
          { label: "from the stash", value: "4.00 TB", share: 0.8, tone: "cache" },
          { label: "from the internet", value: "1.00 TB", share: 0.2, tone: "origin" },
        ]}
        rowsTitle="The stash"
        rows={[
          { key: "steam", label: "Steam", value: "1.85 TB", share: 1 },
          { key: "bn", label: "Battle.net", value: "719 GB", share: 0.39 },
        ]}
      />,
    )
    const band = screen.getByRole("region", { name: "What lanstash did" })
    expect(within(band).getByText("4.00 TB · 80 %")).toBeTruthy()
    expect(within(band).getByText("1.00 TB · 20 %")).toBeTruthy()
    const widths = [...container.querySelectorAll<HTMLElement>("[style]")].map((e) => e.style.width)
    expect(widths).toEqual(["80%", "20%", "100%", "39%"])
    expect(screen.getByText("The stash")).toBeTruthy()
  })
  it("lists plain facts in place of rows", () => {
    render(
      <Ledger
        label="Saved"
        headline="1 GB"
        caption="c"
        split={[]}
        rows={[]}
        facts={[{ label: "Prefill, updates, read-ahead", value: "5.00 GB", tone: "origin" }]}
      />,
    )
    expect(screen.getByText("Prefill, updates, read-ahead").tagName).toBe("DT")
    expect(screen.getByText("5.00 GB").tagName).toBe("DD")
  })
})

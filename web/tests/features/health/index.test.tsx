import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import type { HealthReader } from "@/contracts/health"
import { HealthView } from "@/features/health"

describe("features/health", () => {
  it("lists the checks with their level in words, and the worst of them", async () => {
    const health: HealthReader = async () => [
      { level: "ok", what: "store contents", detail: "336 games, 3917.70 GB" },
      { level: "warn", what: "open files", detail: "lanstash: 900 of 1024 (88 %)" },
    ]
    render(<HealthView health={health} every={0} />)
    expect(await screen.findByText("open files")).toBeTruthy()
    expect(screen.getByText("warning")).toBeTruthy()
    expect(screen.getByText("ok")).toBeTruthy()
    expect(screen.getByText("336 games, 3917.70 GB")).toBeTruthy()
    expect(screen.getByText("something to look at")).toBeTruthy()
  })
  it("says when there are none", async () => {
    render(<HealthView health={async () => undefined} every={0} />)
    expect(await screen.findByText("No health checks")).toBeTruthy()
  })
})

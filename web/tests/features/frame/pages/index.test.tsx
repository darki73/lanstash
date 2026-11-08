import { render, screen } from "@testing-library/react"
import { Suspense } from "react"
import { describe, expect, it } from "vitest"
import { HealthView } from "@/features/frame/pages"

describe("features/frame/pages", () => {
  it("loads a page when it is first drawn, showing the fallback until then", async () => {
    render(
      <Suspense fallback={<p>loading</p>}>
        <HealthView health={async () => undefined} />
      </Suspense>,
    )
    expect(screen.getByText("loading")).toBeTruthy()
    expect(await screen.findByRole("heading", { level: 2, name: "Health" })).toBeTruthy()
  })
})

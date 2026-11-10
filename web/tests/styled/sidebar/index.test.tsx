import { render, screen, within } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { AppFrame, SideLink, SideSection } from "@/styled/sidebar"

describe("styled/sidebar", () => {
  it("draws the sidebar's sections, the header's trail and status, the page and the footer", () => {
    render(
      <AppFrame
        title="lanstash"
        tagline="game download cache"
        trail={[
          { name: "Platforms" },
          { name: "Steam", href: "/platforms/steam" },
          { name: "Dota 2", href: "/platforms/steam/570" },
        ]}
        status="live"
        footer="lanstash 0.1.0"
        notice="update available"
        sidebar={
          <>
            <SideSection>
              <SideLink href="/overview" current label="Overview" />
            </SideSection>
            <SideSection title="Platforms">
              <SideLink href="/platforms/steam" current={false} label="Steam" figure="6.54 GB" />
            </SideSection>
          </>
        }
      >
        the page
      </AppFrame>,
    )
    expect(screen.getByRole("navigation", { name: "Main" })).toBeTruthy()
    expect(screen.getByRole("region", { name: "Platforms" })).toBeTruthy()
    expect(screen.getByRole("heading", { level: 2, name: "Platforms" })).toBeTruthy()
    expect(screen.getByRole("link", { name: "Overview" }).getAttribute("aria-current")).toBe("page")
    expect(screen.getByRole("link", { name: "Steam 6.54 GB" }).getAttribute("aria-current")).toBeNull()
    expect(screen.getByRole("list", { name: "You are here" }).textContent).toBe("Platforms/Steam/Dota 2")
    const here = within(screen.getByRole("list", { name: "You are here" }))
    expect(here.getAllByRole("link").map((a) => [a.textContent, a.getAttribute("href")])).toEqual([
      ["Steam", "/platforms/steam"],
    ])
    expect(here.getByText("Dota 2").getAttribute("aria-current")).toBe("page")
    expect(here.getByText("Platforms").getAttribute("aria-current")).toBeNull()
    expect(screen.getByRole("main").textContent).toBe("the page")
    expect(screen.getByRole("contentinfo").textContent).toBe("lanstash 0.1.0update available")
    expect(screen.getByRole("contentinfo").className, "kept in view at the bottom").toContain("sticky bottom-0")
    expect(screen.getByText("live")).toBeTruthy()
  })
  it("leaves the footer out when there is nothing for it", () => {
    render(<AppFrame title="lanstash" trail={[{ name: "Overview" }]} />)
    expect(screen.queryByRole("contentinfo")).toBeNull()
  })
})

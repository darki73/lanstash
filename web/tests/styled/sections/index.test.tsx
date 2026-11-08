import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { SectionLink, Sections } from "@/styled/sections"

describe("styled/sections", () => {
  it("lists the parts of a page beside the one shown, marking it", () => {
    render(
      <Sections
        label="Steam's sections"
        links={
          <>
            <SectionLink href="/p/steam" current label="Cached" figure="117" />
            <SectionLink href="/p/steam/prefill" current={false} label="Prefill" figure="59 %" dot="busy" />
            <SectionLink href="/p/steam/account" current={false} label="Account" dot="attention" />
          </>
        }
      >
        the cached games
      </Sections>,
    )
    const nav = screen.getByRole("navigation", { name: "Steam's sections" })
    expect(nav.textContent).toContain("Cached117")
    expect(screen.getByRole("link", { name: /Cached/ }).getAttribute("aria-current")).toBe("page")
    expect(screen.getByRole("link", { name: /Prefill/ }).getAttribute("aria-current")).toBeNull()
    expect(screen.getByRole("img", { name: "at work" })).toBeTruthy()
    expect(screen.getByRole("img", { name: "needs a look" })).toBeTruthy()
    expect(screen.getByText("the cached games")).toBeTruthy()
  })
})

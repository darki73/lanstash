import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Tags } from "@/styled/tags"

describe("styled/tags", () => {
  it("draws each name as a pill in its tone, with its title", () => {
    render(
      <Tags
        items={[
          { text: "*.steamcontent.com", tone: "alert", title: "goes to the internet" },
          { text: "cache.lan", tone: "quiet" },
        ]}
      />,
    )
    const missing = screen.getByText("*.steamcontent.com")
    expect(missing.className).toContain("text-origin")
    expect(missing.getAttribute("title")).toBe("goes to the internet")
    expect(screen.getByText("cache.lan").className).toContain("text-muted")
  })
})

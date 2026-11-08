import { render } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Head } from "@/styled/head"

describe("styled/head", () => {
  it("puts the title and the description in the document's head", () => {
    const { rerender } = render(<Head title="Steam · lanstash" description="a cache" />)
    expect(document.title).toBe("Steam · lanstash")
    expect(document.head.querySelector('meta[name="description"]')?.getAttribute("content")).toBe("a cache")
    rerender(<Head title="DNS · lanstash" />)
    expect(document.title).toBe("DNS · lanstash")
  })
})

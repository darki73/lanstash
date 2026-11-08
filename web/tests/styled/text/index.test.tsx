import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Link, Muted, Path, Quiet } from "@/styled/text"

describe("styled/text", () => {
  it("draws muted words, and a quiet message with its reason", () => {
    render(
      <>
        <Muted>up 1h</Muted>
        <Quiet title="Nothing downloading" hint="Objects show up here." />
      </>,
    )
    expect(screen.getByText("up 1h")).toBeTruthy()
    expect(screen.getByText("Nothing downloading")).toBeTruthy()
    expect(screen.getByText("Objects show up here.")).toBeTruthy()
  })
  it("draws a link to another page", () => {
    render(<Link href="/platforms/blizzard">Back to Battle.net</Link>)
    expect(screen.getByRole("link", { name: "Back to Battle.net" }).getAttribute("href")).toBe("/platforms/blizzard")
  })
  it("draws a path on one line, whole when hovered", () => {
    render(<Path>/depot/1551361/chunk/b1d5f3bf058446259c69ee29b41cc689c104c83c</Path>)
    const path = screen.getByText("/depot/1551361/chunk/b1d5f3bf058446259c69ee29b41cc689c104c83c")
    expect(path.getAttribute("title")).toBe(path.textContent)
  })
})

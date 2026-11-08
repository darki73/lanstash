import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Queue, QueueItem } from "@/styled/queue"

describe("styled/queue", () => {
  it("lists what waits with its art, name, detail and actions", () => {
    render(
      <Queue title="Up next" count={2} note="in this order" empty="Nothing waits.">
        <QueueItem
          name="Whiskerwood"
          href="/g/1"
          image="/art/1"
          detail="94.5 MB"
          actions={<button type="button">Remove</button>}
        />
        <QueueItem name="No art" />
      </Queue>,
    )
    expect(screen.getByRole("heading", { name: "Up next (2)" })).toBeTruthy()
    expect(screen.getByText("in this order")).toBeTruthy()
    expect(screen.getByRole("link", { name: "Whiskerwood" }).getAttribute("href")).toBe("/g/1")
    expect(screen.getByText("94.5 MB")).toBeTruthy()
    expect(screen.getAllByRole("listitem")).toHaveLength(2)
    expect(screen.getByText("No")).toBeTruthy()
  })
  it("says so when nothing waits", () => {
    render(<Queue title="Up next" count={0} empty="Nothing waits." />)
    expect(screen.getByText("Nothing waits.")).toBeTruthy()
    expect(screen.queryByRole("list")).toBeNull()
  })
})

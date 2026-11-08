import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Choices } from "@/styled/choices"

describe("styled/choices", () => {
  it("presses the chosen one, says where each stands, and reports a new choice", () => {
    const picked: string[] = []
    render(
      <Choices
        label="System"
        options={[
          { id: "a", label: "Alpha", note: "ready", tone: "good" },
          { id: "b", label: "Beta", note: "not checked", tone: "waiting" },
        ]}
        value="a"
        onChange={(id) => picked.push(id)}
      />,
    )
    expect(screen.getByRole("button", { name: /Alpha/ }).getAttribute("aria-pressed")).toBe("true")
    expect(screen.getByText("not checked")).toBeTruthy()
    fireEvent.click(screen.getByRole("button", { name: /Beta/ }))
    expect(picked).toEqual(["b"])
  })
})

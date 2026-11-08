import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { Cell, Row, Table } from "@/styled/table"

describe("styled/table", () => {
  it("draws heads and rows", () => {
    render(
      <Table columns={["game", "size"]}>
        <Row tone="strong">
          <Cell>steam</Cell>
          <Cell>1.00 GB</Cell>
        </Row>
        <Row tone="quiet">
          <Cell indent path muted>
            a/b
          </Cell>
          <Cell />
        </Row>
      </Table>,
    )
    expect(screen.getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["game", "size"])
    expect(screen.getAllByRole("row")).toHaveLength(3)
    expect(screen.getByText("a/b").tagName).toBe("TD")
  })
})

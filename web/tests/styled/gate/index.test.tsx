import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Gate } from "@/styled/gate"

const fields = (name: string, password: string) =>
  [
    { label: "Name", value: name, onChange: () => {}, kind: "username" },
    { label: "Password", value: password, onChange: () => {}, kind: "current-password" },
  ] as const

describe("styled/gate", () => {
  it("sends the form only once every field is filled, and says what went wrong", () => {
    const onSubmit = vi.fn()
    const { rerender } = render(
      <Gate title="Sign in" subtitle="s" fields={fields("boss", "")} submit="Sign in" onSubmit={onSubmit} />,
    )
    fireEvent.submit(screen.getByRole("button", { name: "Sign in" }))
    expect(onSubmit).not.toHaveBeenCalled()
    rerender(
      <Gate
        title="Sign in"
        subtitle="s"
        fields={fields("boss", "pw")}
        submit="Sign in"
        onSubmit={onSubmit}
        error="wrong"
      />,
    )
    fireEvent.submit(screen.getByRole("button", { name: "Sign in" }))
    expect(onSubmit).toHaveBeenCalledOnce()
    expect(screen.getByRole("alert").textContent).toBe("wrong")
  })
  it("names its fields for password managers and hides the password", () => {
    render(<Gate title="t" subtitle="s" fields={fields("", "")} submit="Go" onSubmit={() => {}} hint="first admin" />)
    const password = screen.getByLabelText("Password") as HTMLInputElement
    expect([password.type, password.autocomplete]).toEqual(["password", "current-password"])
    expect((screen.getByLabelText("Name") as HTMLInputElement).autocomplete).toBe("username")
    expect(screen.getByText("first admin")).toBeTruthy()
  })
})

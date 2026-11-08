import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Field, Many, Select, TextInput } from "@/styled/field"

describe("styled/field", () => {
  it("lays out a setting's label, badge, help and controls", () => {
    render(
      <Field label="Size limit" help="The most the stash may hold" badge={<span>config file</span>}>
        <button type="button">Save</button>
      </Field>,
    )
    expect(screen.getByText("Size limit")).toBeTruthy()
    expect(screen.getByText("config file")).toBeTruthy()
    expect(screen.getByText("The most the stash may hold")).toBeTruthy()
    expect(screen.getByRole("button", { name: "Save" })).toBeTruthy()
  })
  it("takes typing, submits on Enter and gives up on Escape", () => {
    const changed = vi.fn()
    const entered = vi.fn()
    const escaped = vi.fn()
    render(<TextInput label="Size limit value" value="8000G" onChange={changed} onEnter={entered} onEscape={escaped} />)
    const box = screen.getByRole("textbox", { name: "Size limit value" })
    fireEvent.change(box, { target: { value: "6T" } })
    expect(changed).toHaveBeenCalledWith("6T")
    fireEvent.keyDown(box, { key: "Enter" })
    fireEvent.keyDown(box, { key: "Escape" })
    expect([entered.mock.calls.length, escaped.mock.calls.length]).toEqual([1, 1])
  })
})

describe("styled/field choices", () => {
  it("offers values as one types, and picks one from a dropdown", () => {
    const picked = vi.fn()
    const { container } = render(
      <>
        <TextInput label="Language value" value="" onChange={() => {}} options={["english", "german"]} />
        <Select label="Platform" value="Windows" options={["Windows", "OSX"]} onChange={picked} />
      </>,
    )
    // A box that offers values is a combobox to assistive technology.
    const id = screen.getByRole("combobox", { name: "Language value" }).getAttribute("list")
    expect(
      [...(container.querySelector(`datalist[id="${id}"]`)?.querySelectorAll("option") ?? [])].map((o) => o.value),
    ).toEqual(["english", "german"])
    fireEvent.change(screen.getByRole("combobox", { name: "Platform" }), { target: { value: "OSX" } })
    expect(picked).toHaveBeenCalledWith("OSX")
  })
  it("shows a value by its name where it has one, and hides a secret as it is typed", () => {
    render(
      <>
        <Select
          label="Kind"
          value="adguard"
          options={["adguard", "other"]}
          names={{ adguard: "AdGuard Home" }}
          onChange={() => {}}
        />
        <TextInput label="Password" value="" onChange={() => {}} secret />
      </>,
    )
    const options = [...screen.getByRole("combobox", { name: "Kind" }).querySelectorAll("option")]
    expect(options.map((o) => [o.value, o.textContent])).toEqual([
      ["adguard", "AdGuard Home"],
      ["other", "other"],
    ])
    const secret = screen.getByLabelText("Password") as HTMLInputElement
    expect([secret.type, secret.autocomplete]).toEqual(["password", "new-password"])
  })
  it("picks several values: drops one with ×, but not the last, and adds one of the others", () => {
    const changed = vi.fn()
    const { rerender } = render(
      <Many
        label="Languages"
        values={["english", "german"]}
        options={["english", "german", "polish"]}
        onChange={changed}
      />,
    )
    fireEvent.click(screen.getByRole("button", { name: "Drop german from Languages" }))
    expect(changed).toHaveBeenLastCalledWith(["english"])
    const add = screen.getByRole("combobox", { name: "Add to Languages" })
    expect([...add.querySelectorAll("option")].map((o) => o.value)).toEqual(["", "polish"])
    fireEvent.change(add, { target: { value: "polish" } })
    expect(changed).toHaveBeenLastCalledWith(["english", "german", "polish"])
    rerender(<Many label="Languages" values={["english"]} options={["english"]} onChange={changed} />)
    expect(screen.queryByRole("button", { name: /Drop/ }), "the last one stays").toBeNull()
    expect(screen.queryByRole("combobox"), "nothing left to add").toBeNull()
  })
})

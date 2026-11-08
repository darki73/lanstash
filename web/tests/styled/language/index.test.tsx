import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { Language } from "@/styled/language"

const three = [
  { code: "en", name: "English" },
  { code: "ru", name: "Русский" },
  { code: "de", name: "Deutsch" },
]

describe("styled/language", () => {
  it("shows the language in use by its own name, and lists them all when opened", () => {
    const pick = vi.fn()
    render(<Language label="Language" options={three} value="en" onChange={pick} />)
    const button = screen.getByRole("combobox", { name: "Language: English" })
    expect(button.getAttribute("aria-expanded")).toBe("false")
    expect(screen.queryByRole("listbox")).toBeNull()
    fireEvent.click(button)
    const options = screen.getAllByRole("option")
    expect(options.map((o) => [o.textContent, o.getAttribute("lang"), o.getAttribute("aria-selected")])).toEqual([
      ["Englishen", "en", "true"],
      ["Русскийru", "ru", "false"],
      ["Deutschde", "de", "false"],
    ])
    fireEvent.click(screen.getByRole("option", { name: /Русский/ }))
    expect(pick).toHaveBeenCalledWith("ru")
    expect(screen.queryByRole("listbox"), "closed once chosen").toBeNull()
  })
  it("is worked by the keyboard, and closes on Escape or a click elsewhere", () => {
    const pick = vi.fn()
    render(<Language label="Language" options={three} value="en" onChange={pick} />)
    const button = screen.getByRole("combobox", { name: "Language: English" })
    fireEvent.keyDown(button, { key: "ArrowDown" })
    expect(screen.getByRole("listbox")).toBeTruthy()
    fireEvent.keyDown(button, { key: "ArrowDown" })
    fireEvent.keyDown(button, { key: "ArrowDown" })
    fireEvent.keyDown(button, { key: "Enter" })
    expect(pick).toHaveBeenCalledWith("de")
    fireEvent.click(button)
    fireEvent.keyDown(button, { key: "Escape" })
    expect(screen.queryByRole("listbox")).toBeNull()
    fireEvent.click(button)
    fireEvent.mouseDown(document.body)
    expect(screen.queryByRole("listbox")).toBeNull()
  })
})

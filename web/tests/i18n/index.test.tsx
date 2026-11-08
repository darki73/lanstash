import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"
import { I18nProvider, offered, useAgo, useLanguage, useT } from "@/i18n"

function Probe() {
  const t = useT("frame")
  const ago = useAgo()
  const { code, dir } = useLanguage()
  const now = 10 * 86_400_000
  return (
    <output>
      {[
        t("nav.overview"),
        t("version", { version: "1.2" }),
        ago(now, 0),
        ago(now, now - 60_000),
        ago(now, now - 86_400_000 * 2),
        code,
        dir,
      ].join("|")}
    </output>
  )
}

describe("i18n", () => {
  it("gives English words, values and plural forms, with or without a provider", () => {
    const want = "Overview|lanstash 1.2|never|1 min ago|2 days ago|en|ltr"
    const { unmount } = render(<Probe />)
    expect(screen.getByRole("status").textContent).toBe(want)
    unmount()
    render(
      <I18nProvider language="en">
        <Probe />
      </I18nProvider>,
    )
    expect(screen.getByRole("status").textContent).toBe(want)
  })
  it("offers each language it ships by its own name", () => {
    expect(offered).toEqual([
      { code: "en", name: "English" },
      { code: "ru", name: "Русский" },
    ])
  })
})

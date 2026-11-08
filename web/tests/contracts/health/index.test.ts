import { describe, expect, it } from "vitest"
import { type HealthCheck, noHealth, worst } from "@/contracts/health"

const check = (level: HealthCheck["level"]): HealthCheck => ({ level, what: "x", detail: "" })

describe("contracts/health", () => {
  it("says the worst level", () => {
    expect(worst([])).toBe("ok")
    expect(worst([check("ok"), check("warn")])).toBe("warn")
    expect(worst([check("warn"), check("fail"), check("ok")])).toBe("fail")
  })
  it("has a reader for a page with none", async () => {
    expect(await noHealth()).toBeUndefined()
  })
})

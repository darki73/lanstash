import { describe, expect, it } from "vitest"
import { activity } from "@/features/frame/meta"
import { snapshot } from "../../../kit"

const at = (rates: Record<string, number>) => activity({ ...snapshot(), rates_mb_s: rates })

describe("features/frame/meta", () => {
  it("says how fast lanstash fetches and serves, only while it does", () => {
    expect(at({ "downloads.fetched_bytes": 123.4, "lifetime.served_bytes": 4.56 })).toEqual({
      fetching: "123",
      serving: "4.6",
    })
    expect(at({ "downloads.fetched_bytes": 0.05, "lifetime.served_bytes": 0 })).toEqual({})
    expect(activity(undefined)).toEqual({})
  })
})

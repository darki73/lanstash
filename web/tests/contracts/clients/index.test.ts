import { describe, expect, it } from "vitest"
import { cacheShare, clientName, noClients } from "@/contracts/clients"

describe("contracts/clients", () => {
  it("calls a client by its given name, else the network's, else its address", () => {
    expect(clientName({ address: "10.0.0.5", label: "Office", host: "Game-PC" })).toBe("Office")
    expect(clientName({ address: "10.0.0.5", label: null, host: "Game-PC" })).toBe("Game-PC")
    expect(clientName({ address: "10.0.0.5", label: null, host: null })).toBe("10.0.0.5")
  })
  it("says how much came from the cache", () => {
    expect(cacheShare(3, 1)).toBe(0.75)
    expect(cacheShare(0, 0)).toBeUndefined()
  })
  it("has a reader for a page with none", async () => {
    expect(await noClients(0, 1)).toBeUndefined()
  })
})

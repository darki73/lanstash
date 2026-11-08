import { describe, expect, it } from "vitest"
import { clientNamer, clientsReader } from "@/adapters/api/clients"
import type { Fetch } from "@/adapters/api/version"

const office = {
  address: "10.0.0.5",
  label: "Office",
  host: null,
  hit: 300,
  miss: 100,
  games: [{ service: "steam", game: "7", hit: 300, miss: 100 }],
}

const recording = (ok: boolean, body: unknown) => {
  const asked: string[] = []
  const fetch: Fetch = async (url) => {
    asked.push(url)
    return { ok, json: async () => body }
  }
  return { asked, fetch }
}

describe("adapters/api/clients", () => {
  it("asks what each client got over a range", async () => {
    const { asked, fetch } = recording(true, { clients: [office] })
    expect(await clientsReader(fetch, "http://box")(0, 60_000.4)).toEqual([office])
    expect(asked).toEqual(["http://box/api/v1/clients?from=0&to=60000"])
  })
  it("reads anything else as nothing", async () => {
    for (const body of [{ clients: [{ ...office, hit: "x" }] }, { clients: [{ ...office, games: [{}] }] }, null, {}]) {
      expect(await clientsReader(recording(true, body).fetch, "")(0, 1)).toBeUndefined()
    }
    expect(await clientsReader(recording(false, { clients: [office] }).fetch, "")(0, 1)).toBeUndefined()
    const failing: Fetch = async () => {
      throw new Error("offline")
    }
    expect(await clientsReader(failing, "")(0, 1)).toBeUndefined()
  })
  it("names a client, escaped, and takes the name away when it is empty", async () => {
    const put = recording(true, {})
    const del = recording(true, {})
    const name = clientNamer(put.fetch, del.fetch, "http://box")
    expect(await name("fd00::5", " Den & co ")).toBeUndefined()
    expect(put.asked).toEqual(["http://box/api/v1/clients/fd00%3A%3A5?label=Den%20%26%20co"])
    expect(await name("10.0.0.5", "  ")).toBeUndefined()
    expect(del.asked).toEqual(["http://box/api/v1/clients/10.0.0.5"])
  })
  it("gives the reason a name was refused", async () => {
    const refused = recording(false, { error: "a name has at most 64 characters" })
    expect(await clientNamer(refused.fetch, refused.fetch, "")("10.0.0.5", "x")).toBe(
      "a name has at most 64 characters",
    )
    const failing: Fetch = async () => {
      throw new Error("offline")
    }
    expect(await clientNamer(failing, failing, "")("10.0.0.5", "x")).toBe("lanstash did not answer")
  })
})

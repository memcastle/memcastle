import { describe, expect, it, vi } from "vitest"
import { ApiError, MemCastleClient } from "../src/api/client.ts"
import type { MemoryMode } from "../src/api/types.ts"
import { fakeFetch, UNAUTHORIZED } from "./support/fetch.ts"

function client(answer: Parameters<typeof fakeFetch>[0], options: { token?: string | null; mode?: MemoryMode; onUnauthorized?: () => void } = {}) {
  const { fetch, requests } = fakeFetch(answer)
  const api = new MemCastleClient({ fetch, token: () => options.token ?? null, mode: () => options.mode ?? "full", onUnauthorized: options.onUnauthorized })
  return { api, requests }
}

describe("a request", () => {
  it("carries the bearer token and the memory mode, and asks for JSON", async () => {
    const { api, requests } = client({ body: [] }, { token: "secret-token", mode: "read_only" })

    await api.wings()

    expect(requests[0]?.headers).toMatchObject({ Authorization: "Bearer secret-token", "X-MemCastle-Mode": "read_only", Accept: "application/json" })
  })

  it("sends no Authorization header when there is no token", async () => {
    const { api, requests } = client({ body: [] })

    await api.wings()

    expect(requests[0]?.headers).not.toHaveProperty("Authorization")
  })

  it("lets the login form test a token that is not the session's yet", async () => {
    const { api, requests } = client({ body: {} }, { token: "held" })

    await api.status("typed")

    expect(requests[0]?.headers.Authorization).toBe("Bearer typed")
  })

  it("leaves out empty query values, so an empty form field is not a filter", async () => {
    const { api, requests } = client({ body: [] })

    await api.jobs({ status: undefined, kind: "mine", limit: 20 })
    await api.search({ q: "castle", wing: "", ranking: "auto" })

    expect(requests[0]?.url).toBe("/api/jobs?kind=mine&limit=20")
    expect(requests[1]?.url).toBe("/api/search?q=castle&ranking=auto")
  })

  it("marks the dashboard as the requester of what it submits", async () => {
    const { api, requests } = client({ body: {} })

    await api.submitJob({ type: "repair", dry_run: true })

    expect(requests[0]).toMatchObject({ method: "POST", url: "/api/jobs", body: { type: "repair", dry_run: true, requested_by: "web" } })
  })

  it("edits a miner and its schedule through authenticated resource routes", async () => {
    const { api, requests } = client({ body: {} }, { token: "t" })

    await api.setMiner("my miner", { source: "directory", room: "imports", schedule: { every: "1d", enabled: false } })
    await api.scheduleMiner("my miner", true)
    await api.enableMiner("my miner", false)
    await api.runMiner("my miner", false)

    expect(requests.map((request) => [request.method, request.url])).toEqual([
      ["PUT", "/api/miners/my%20miner"],
      ["POST", "/api/miners/my%20miner/schedule/enable"],
      ["POST", "/api/miners/my%20miner/disable"],
      ["POST", "/api/miners/my%20miner/run"],
    ])
    expect(requests[0]?.body).toMatchObject({ room: "imports", schedule: { enabled: false } })
    expect(requests[3]?.body).toMatchObject({ full: false, requested_by: "web" })
    expect(requests.every((request) => request.headers.Authorization === "Bearer t")).toBe(true)
  })

  it("escapes each segment of a drawer path but keeps the slashes of a drawer name", async () => {
    const { api, requests } = client({ body: {} })

    await api.drawer("my wing", "room/1", "notes/2026 plan")

    expect(requests[0]?.url).toBe("/api/wings/my%20wing/rooms/room%2F1/drawers/notes/2026%20plan")
  })

  it("paginates drawer previews and resolves evidence through the authenticated REST client", async () => {
    const { api, requests } = client({ body: [] }, { token: "t", mode: "read_only" })
    await api.drawers("work", "notes", 50, { id: "drawer:one", created_at: "2026-10-01T00:00:00Z" })
    await api.drawerLocation("drawer:one")
    expect(requests[0]?.url).toContain("before_created_at=2026-10-01T00%3A00%3A00Z&before_id=drawer%3Aone")
    expect(requests[1]?.url).toBe("/api/drawers/drawer%3Aone")
    expect(requests.every((request) => request.headers.Authorization === "Bearer t" && request.headers["X-MemCastle-Mode"] === "read_only")).toBe(true)
  })
})

describe("a failure", () => {
  it("becomes an ApiError with the daemon's own diagnostic", async () => {
    const { api } = client({ status: 404, body: { code: "memcastle::job::not_found", error: "no job x", help: "list jobs" } })

    const error = await api.job("x").catch((caught: unknown) => caught)

    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({ status: 404, code: "memcastle::job::not_found", message: "no job x", help: "list jobs" })
  })

  it("is still an ApiError when the body is not the daemon's", async () => {
    const { fetch } = { fetch: (async () => new Response("<html>bad gateway</html>", { status: 502 })) as unknown as typeof globalThis.fetch }
    const api = new MemCastleClient({ fetch, token: () => null, mode: () => "full" })

    await expect(api.status()).rejects.toMatchObject({ status: 502, code: "http_502" })
  })

  it("says the daemon did not answer when the network fails, and how to check", async () => {
    const api = new MemCastleClient({
      fetch: (async () => {
        throw new TypeError("fetch failed")
      }) as unknown as typeof fetch,
      token: () => null,
      mode: () => "full",
    })

    await expect(api.status()).rejects.toMatchObject({ status: 0, code: "network", help: expect.stringContaining("memcastle status") })
  })

  it("tells the session its token was refused, but not when no token was sent", async () => {
    const refused = vi.fn()
    const held = client(UNAUTHORIZED, { token: "revoked", onUnauthorized: refused })
    const probe = client(UNAUTHORIZED, { token: null, onUnauthorized: refused })

    await held.api.status().catch(() => undefined)
    expect(refused).toHaveBeenCalledTimes(1)
    await probe.api.status().catch(() => undefined)
    expect(refused).toHaveBeenCalledTimes(1)
  })

  it("does not end the session for an error that is not a refusal", async () => {
    const refused = vi.fn()
    const { api } = client({ status: 403, body: { code: "memcastle::memory::mode_forbidden", error: "read only" } }, { token: "t", onUnauthorized: refused })

    await api.submitJob({ type: "audit" }).catch(() => undefined)

    expect(refused).not.toHaveBeenCalled()
  })
})

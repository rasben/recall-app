/**
 * Tests for the telemetry worker.
 *
 * We import the handler directly and supply a lightweight in-memory KV mock,
 * so no Workers runtime or Cloudflare-specific test pool is needed.
 */
import { describe, it, expect, beforeEach } from "vitest";
import worker from "../src/index.js";

const VALID_UUID = "f47ac10b-58cc-4372-a567-0e02b2c3d479";
const VALID_UUID_2 = "a1b2c3d4-e5f6-4789-8012-abcdef012345";

// Minimal in-memory KV mock that covers the subset used by the handler.
function makeKV() {
  const store = new Map();
  return {
    async put(key, value, _opts) { store.set(key, value); },
    async get(key) { return store.get(key) ?? null; },
    async list() { return { keys: [...store.keys()].map(name => ({ name })), list_complete: true }; },
    _store: store,
  };
}

async function call(kv, method, path, body) {
  const init = { method, headers: {} };
  if (body !== undefined) {
    init.body = JSON.stringify(body);
    init.headers["Content-Type"] = "application/json";
  }
  const req = new Request(`http://localhost${path}`, init);
  const env = { INSTALLS: kv };
  return worker.fetch(req, env);
}

// ── POST /ping ────────────────────────────────────────────────────────────────

describe("POST /ping", () => {
  let kv;
  beforeEach(() => { kv = makeKV(); });

  it("accepts a valid UUID v4 and returns 200 ok", async () => {
    const res = await call(kv, "POST", "/ping", { id: VALID_UUID });
    expect(res.status).toBe(200);
    expect(await res.text()).toBe("ok");
  });

  it("stores the id in KV", async () => {
    await call(kv, "POST", "/ping", { id: VALID_UUID });
    expect(kv._store.get(VALID_UUID)).toBe("1");
  });

  it("is idempotent — re-pinging the same id stays 200", async () => {
    await call(kv, "POST", "/ping", { id: VALID_UUID });
    const res = await call(kv, "POST", "/ping", { id: VALID_UUID });
    expect(res.status).toBe(200);
    expect(kv._store.size).toBe(1);
  });

  it("rejects a missing id with 400", async () => {
    const res = await call(kv, "POST", "/ping", {});
    expect(res.status).toBe(400);
  });

  it("rejects a non-UUID string with 400", async () => {
    const res = await call(kv, "POST", "/ping", { id: "not-a-uuid" });
    expect(res.status).toBe(400);
  });

  it("rejects malformed JSON with 400", async () => {
    const req = new Request("http://localhost/ping", {
      method: "POST",
      body: "{{bad json",
      headers: { "Content-Type": "application/json" },
    });
    const res = await worker.fetch(req, { INSTALLS: kv });
    expect(res.status).toBe(400);
  });
});

// ── GET /stats ────────────────────────────────────────────────────────────────

describe("GET /stats", () => {
  let kv;
  beforeEach(async () => {
    kv = makeKV();
    await kv.put(VALID_UUID, "1");
    await kv.put(VALID_UUID_2, "1");
  });

  it("returns 200 with JSON body", async () => {
    const res = await call(kv, "GET", "/stats");
    expect(res.status).toBe(200);
    expect(res.headers.get("Content-Type")).toContain("application/json");
  });

  it("active count matches the number of KV entries", async () => {
    const res = await call(kv, "GET", "/stats");
    const data = await res.json();
    expect(data.active).toBe(2);
  });

  it("active count is 0 when KV is empty", async () => {
    const res = await call(makeKV(), "GET", "/stats");
    const data = await res.json();
    expect(data.active).toBe(0);
  });
});

// ── CORS ──────────────────────────────────────────────────────────────────────

describe("CORS", () => {
  const kv = makeKV();

  it("OPTIONS preflight returns 204 with CORS headers", async () => {
    const res = await call(kv, "OPTIONS", "/ping");
    expect(res.status).toBe(204);
    expect(res.headers.get("Access-Control-Allow-Origin")).toBe("*");
  });

  it("every response includes Access-Control-Allow-Origin: *", async () => {
    for (const res of await Promise.all([
      call(kv, "GET", "/stats"),
      call(kv, "POST", "/ping", { id: VALID_UUID }),
      call(kv, "GET", "/unknown"),
    ])) {
      expect(res.headers.get("Access-Control-Allow-Origin")).toBe("*");
    }
  });
});

// ── Unknown routes ────────────────────────────────────────────────────────────

describe("unknown routes", () => {
  it("returns 404 for unrecognised paths", async () => {
    const res = await call(makeKV(), "GET", "/unknown");
    expect(res.status).toBe(404);
  });
});

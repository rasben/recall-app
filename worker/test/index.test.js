/**
 * Tests for the telemetry worker.
 *
 * We import the handler directly and back the `DB` binding with Node's
 * built-in `node:sqlite`, wrapped in the small subset of the D1 API the
 * handler uses (prepare → bind → run / first / all). D1 is SQLite underneath,
 * so the real schema.sql and the real queries run unchanged — no Workers
 * runtime or Cloudflare-specific test pool needed.
 */
import { describe, it, expect, beforeEach } from "vitest";
import { readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import worker from "../src/index.js";

const SCHEMA = readFileSync(new URL("../schema.sql", import.meta.url), "utf8");

const VALID_UUID = "f47ac10b-58cc-4372-a567-0e02b2c3d479";
const VALID_UUID_2 = "a1b2c3d4-e5f6-4789-8012-abcdef012345";
const TODAY = new Date().toISOString().slice(0, 10);

function daysAgo(n) {
  const d = new Date();
  d.setUTCDate(d.getUTCDate() - n);
  return d.toISOString().slice(0, 10);
}

/** Minimal D1 look-alike over node:sqlite. */
function makeD1() {
  const db = new DatabaseSync(":memory:");
  db.exec(SCHEMA);
  return {
    prepare(sql) {
      const stmt = db.prepare(sql);
      let args = [];
      const api = {
        bind(...a) {
          args = a;
          return api;
        },
        async run() {
          const info = stmt.run(...args);
          return { success: true, meta: { changes: info.changes } };
        },
        async first(column) {
          const row = stmt.get(...args) ?? null;
          return column === undefined ? row : row?.[column] ?? null;
        },
        async all() {
          return { success: true, results: stmt.all(...args) };
        },
      };
      return api;
    },
    _db: db,
  };
}

function rows(d1) {
  return d1._db.prepare("SELECT * FROM pings ORDER BY install_id, day").all();
}

async function call(d1, method, path, body) {
  const init = { method, headers: {} };
  if (body !== undefined) {
    init.body = typeof body === "string" ? body : JSON.stringify(body);
    init.headers["Content-Type"] = "application/json";
  }
  const req = new Request(`http://localhost${path}`, init);
  return worker.fetch(req, { DB: d1 });
}

function fullPing(overrides = {}) {
  return {
    id: VALID_UUID,
    date: TODAY,
    schema: 2,
    version: "1.6.0",
    os: "macos",
    arch: "aarch64",
    lang: "da",
    theme: "dark",
    group_mode: "task",
    install_age: "31to90d",
    config: {
      sources: { git: true, github: true, calendar: true, jira: false, zulip: true },
      github_events: ["PullRequestEvent"],
      jira_events: [],
      ical_urls: "1",
      custom_export_prompt: false,
    },
    counters: { "nav.prev": 7, "harvest.mark": 3, "export.copy": 1 },
    ...overrides,
  };
}

// ── POST /ping ────────────────────────────────────────────────────────────────

describe("POST /ping", () => {
  let d1;
  beforeEach(() => {
    d1 = makeD1();
  });

  it("accepts a full schema-2 ping and stores typed columns plus the payload", async () => {
    const res = await call(d1, "POST", "/ping", fullPing());
    expect(res.status).toBe(200);
    expect(await res.text()).toBe("ok");

    const [row] = rows(d1);
    expect(row.install_id).toBe(VALID_UUID);
    expect(row.day).toBe(TODAY);
    expect(row.version).toBe("1.6.0");
    expect(row.os).toBe("macos");
    expect(row.arch).toBe("aarch64");
    expect(row.lang).toBe("da");
    expect(JSON.parse(row.payload).counters["nav.prev"]).toBe(7);
  });

  it("accepts a legacy { id } ping and falls back to the server date", async () => {
    const res = await call(d1, "POST", "/ping", { id: VALID_UUID });
    expect(res.status).toBe(200);
    const [row] = rows(d1);
    expect(row.day).toBe(TODAY);
    expect(row.version).toBeNull();
    expect(row.os).toBeNull();
  });

  it("upserts — re-pinging the same install on the same day keeps one row with the newest payload", async () => {
    await call(d1, "POST", "/ping", fullPing({ counters: { "nav.prev": 1 } }));
    const res = await call(d1, "POST", "/ping", fullPing({ counters: { "nav.prev": 9 } }));
    expect(res.status).toBe(200);
    const all = rows(d1);
    expect(all).toHaveLength(1);
    expect(JSON.parse(all[0].payload).counters["nav.prev"]).toBe(9);
  });

  it("keeps one row per day for the same install", async () => {
    await call(d1, "POST", "/ping", fullPing({ date: daysAgo(1) }));
    await call(d1, "POST", "/ping", fullPing({ date: TODAY }));
    expect(rows(d1)).toHaveLength(2);
  });

  it("normalises the install id to lowercase", async () => {
    await call(d1, "POST", "/ping", { id: VALID_UUID.toUpperCase() });
    expect(rows(d1)[0].install_id).toBe(VALID_UUID);
  });

  it("ignores a malformed date and uses the server date instead", async () => {
    await call(d1, "POST", "/ping", fullPing({ date: "yesterday-ish" }));
    expect(rows(d1)[0].day).toBe(TODAY);
  });

  it("nulls out over-long or non-string typed fields but still stores the ping", async () => {
    await call(d1, "POST", "/ping", fullPing({ version: "x".repeat(100), os: 42 }));
    const [row] = rows(d1);
    expect(row.version).toBeNull();
    expect(row.os).toBeNull();
    expect(row.arch).toBe("aarch64");
  });

  it("rejects a missing id with 400", async () => {
    const res = await call(d1, "POST", "/ping", {});
    expect(res.status).toBe(400);
    expect(rows(d1)).toHaveLength(0);
  });

  it("rejects a non-UUID string with 400", async () => {
    const res = await call(d1, "POST", "/ping", { id: "not-a-uuid" });
    expect(res.status).toBe(400);
  });

  it("rejects a non-object body with 400", async () => {
    expect((await call(d1, "POST", "/ping", "[]")).status).toBe(400);
    expect((await call(d1, "POST", "/ping", "null")).status).toBe(400);
  });

  it("rejects malformed JSON with 400", async () => {
    const res = await call(d1, "POST", "/ping", "{{bad json");
    expect(res.status).toBe(400);
  });

  it("rejects an oversized body with 413", async () => {
    const res = await call(d1, "POST", "/ping", fullPing({ junk: "z".repeat(40 * 1024) }));
    expect(res.status).toBe(413);
    expect(rows(d1)).toHaveLength(0);
  });
});

// ── GET /stats ────────────────────────────────────────────────────────────────

describe("GET /stats", () => {
  let d1;
  beforeEach(async () => {
    d1 = makeD1();
    // Install 1: seen 10 days ago on 1.5.0, then today on 1.6.0 → counts once, as 1.6.0.
    await call(d1, "POST", "/ping", fullPing({ date: daysAgo(10), version: "1.5.0" }));
    await call(d1, "POST", "/ping", fullPing({ date: TODAY, version: "1.6.0" }));
    // Install 2: only seen 20 days ago, on Windows.
    await call(
      d1,
      "POST",
      "/ping",
      fullPing({ id: VALID_UUID_2, date: daysAgo(20), version: "1.5.0", os: "windows", arch: "x86_64" }),
    );
    // A stale install outside the 35-day window must not count.
    await call(d1, "POST", "/ping", fullPing({ id: "11111111-2222-4333-8444-555555555555", date: daysAgo(60) }));
  });

  it("returns 200 with JSON body", async () => {
    const res = await call(d1, "GET", "/stats");
    expect(res.status).toBe(200);
    expect(res.headers.get("Content-Type")).toContain("application/json");
  });

  it("counts distinct installs per window", async () => {
    const data = await (await call(d1, "GET", "/stats")).json();
    expect(data.active_35d).toBe(2);
    expect(data.active).toBe(2); // legacy alias
    expect(data.active_7d).toBe(1);
    expect(data.today).toBe(1);
  });

  it("groups installs by their latest version and platform", async () => {
    const data = await (await call(d1, "GET", "/stats")).json();
    expect(data.by_version).toEqual({ "1.5.0": 1, "1.6.0": 1 });
    expect(data.by_platform).toEqual({ "macos/aarch64": 1, "windows/x86_64": 1 });
  });

  it("returns zeros on an empty database", async () => {
    const data = await (await call(makeD1(), "GET", "/stats")).json();
    expect(data).toEqual({
      active: 0,
      active_35d: 0,
      active_7d: 0,
      today: 0,
      by_version: {},
      by_platform: {},
    });
  });

  it("labels legacy pings without a version as unknown", async () => {
    const fresh = makeD1();
    await call(fresh, "POST", "/ping", { id: VALID_UUID });
    const data = await (await call(fresh, "GET", "/stats")).json();
    expect(data.by_version).toEqual({ unknown: 1 });
    expect(data.by_platform).toEqual({});
  });
});

// ── GET /stats/features ───────────────────────────────────────────────────────

describe("GET /stats/features", () => {
  let d1;
  beforeEach(async () => {
    d1 = makeD1();
    await call(d1, "POST", "/ping", fullPing({ date: daysAgo(1), counters: { "nav.prev": 7, "harvest.mark": 3 } }));
    await call(d1, "POST", "/ping", fullPing({ date: TODAY, counters: { "nav.prev": 2, "export.copy": 1 } }));
    await call(d1, "POST", "/ping", fullPing({ id: VALID_UUID_2, date: TODAY, counters: { "nav.prev": 1 } }));
    // Legacy ping with no counters contributes nothing.
    await call(d1, "POST", "/ping", { id: "11111111-2222-4333-8444-555555555555" });
    // Outside a 7-day window.
    await call(d1, "POST", "/ping", fullPing({ id: "22222222-3333-4444-8555-666666666666", date: daysAgo(50), counters: { "nav.prev": 100 } }));
  });

  it("sums counters across pings and counts distinct installs per key", async () => {
    const data = await (await call(d1, "GET", "/stats/features?days=7")).json();
    expect(data.days).toBe(7);
    expect(data.counters).toEqual({
      "nav.prev": { total: 10, installs: 2 },
      "harvest.mark": { total: 3, installs: 1 },
      "export.copy": { total: 1, installs: 1 },
    });
  });

  it("widens the window when asked", async () => {
    const data = await (await call(d1, "GET", "/stats/features?days=90")).json();
    expect(data.counters["nav.prev"]).toEqual({ total: 110, installs: 3 });
  });

  it("defaults to 30 days and clamps nonsense", async () => {
    expect((await (await call(d1, "GET", "/stats/features")).json()).days).toBe(30);
    expect((await (await call(d1, "GET", "/stats/features?days=0")).json()).days).toBe(1);
    expect((await (await call(d1, "GET", "/stats/features?days=99999")).json()).days).toBe(365);
    expect((await (await call(d1, "GET", "/stats/features?days=abc")).json()).days).toBe(30);
  });
});

// ── CORS ──────────────────────────────────────────────────────────────────────

describe("CORS", () => {
  const d1 = makeD1();

  it("OPTIONS preflight returns 204 with CORS headers", async () => {
    const res = await call(d1, "OPTIONS", "/ping");
    expect(res.status).toBe(204);
    expect(res.headers.get("Access-Control-Allow-Origin")).toBe("*");
  });

  it("every response includes Access-Control-Allow-Origin: *", async () => {
    for (const res of await Promise.all([
      call(d1, "GET", "/stats"),
      call(d1, "GET", "/stats/features"),
      call(d1, "POST", "/ping", { id: VALID_UUID }),
      call(d1, "GET", "/unknown"),
    ])) {
      expect(res.headers.get("Access-Control-Allow-Origin")).toBe("*");
    }
  });
});

// ── Errors and unknown routes ─────────────────────────────────────────────────

describe("errors and unknown routes", () => {
  it("returns 404 for unrecognised paths", async () => {
    const res = await call(makeD1(), "GET", "/unknown");
    expect(res.status).toBe(404);
  });

  it("returns 500 (not a crash) when the database throws", async () => {
    const broken = {
      prepare() {
        throw new Error("D1 is down");
      },
    };
    const req = new Request("http://localhost/stats", { method: "GET" });
    const res = await worker.fetch(req, { DB: broken });
    expect(res.status).toBe(500);
  });
});

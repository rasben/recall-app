/**
 * Recall telemetry worker
 *
 * POST /ping   { id, date?, version?, os?, arch?, lang?, config?, counters?, ... }
 *   → upserts one row per (install id, day) into D1 (`pings` table). The whole
 *     body is kept as JSON; a few fields are copied into typed columns for
 *     cheap grouping. Legacy `{ id }` pings from older app versions are accepted.
 *
 * GET  /stats
 *   → active installs (today / 7d / 35d) and the version + OS split of the
 *     installs seen in the last 35 days.
 *
 * GET  /stats/features?days=30
 *   → per-counter totals over the window: { key: { total, installs } }.
 *
 * No IP addresses, no personal data, no identifying information beyond the
 * random install UUID. See src-tauri/src/telemetry.rs for what the app sends.
 */

const CORS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type",
};

const ACTIVE_WINDOW_DAYS = 35;
const MAX_BODY_BYTES = 32 * 1024;
const MAX_FEATURE_WINDOW_DAYS = 365;
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const DAY_RE = /^\d{4}-\d{2}-\d{2}$/;

function text(body, status = 200) {
  return new Response(body, { status, headers: CORS });
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { ...CORS, "Content-Type": "application/json" },
  });
}

/** A short string column value, or null if absent / too long / not a string. */
function shortString(value, max = 32) {
  return typeof value === "string" && value.length > 0 && value.length <= max ? value : null;
}

async function handlePing(request, env) {
  const raw = await request.text();
  if (raw.length > MAX_BODY_BYTES) return text("payload too large", 413);

  let body;
  try {
    body = JSON.parse(raw);
  } catch {
    return text("bad request", 400);
  }
  if (body === null || typeof body !== "object" || Array.isArray(body)) {
    return text("bad request", 400);
  }

  const id = body.id;
  if (typeof id !== "string" || !UUID_RE.test(id)) return text("invalid id", 400);

  // Trust the client's local calendar day when present (it decides "once a
  // day" locally); fall back to the server's UTC date for legacy pings.
  const day = DAY_RE.test(body.date) ? body.date : new Date().toISOString().slice(0, 10);

  await env.DB.prepare(
    `INSERT INTO pings (install_id, day, version, os, arch, lang, payload)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
     ON CONFLICT(install_id, day) DO UPDATE SET
       received_at = datetime('now'),
       version = excluded.version,
       os = excluded.os,
       arch = excluded.arch,
       lang = excluded.lang,
       payload = excluded.payload`,
  )
    .bind(
      id.toLowerCase(),
      day,
      shortString(body.version),
      shortString(body.os),
      shortString(body.arch),
      shortString(body.lang),
      JSON.stringify(body),
    )
    .run();

  return text("ok");
}

/** Rows → { label: count } for the two-column group-by queries below. */
function toCounts(rows, labelKey) {
  const out = {};
  for (const row of rows) out[row[labelKey] ?? "unknown"] = row.n;
  return out;
}

async function handleStats(env) {
  const window = `-${ACTIVE_WINDOW_DAYS} days`;

  const distinctSince = (modifier) =>
    env.DB.prepare(
      `SELECT COUNT(DISTINCT install_id) AS n FROM pings WHERE day >= date('now', ?1)`,
    )
      .bind(modifier)
      .first("n");

  // Latest ping per install inside the window, then group by version / OS.
  // SQLite returns the other bare columns from the row that holds MAX(day).
  const latest = `SELECT install_id, version, os, arch, MAX(day) AS day
                  FROM pings WHERE day >= date('now', ?1) GROUP BY install_id`;

  const [today, active7d, active35d, byVersion, byOs] = await Promise.all([
    distinctSince("+0 days"),
    distinctSince("-7 days"),
    distinctSince(window),
    env.DB.prepare(`SELECT version, COUNT(*) AS n FROM (${latest}) GROUP BY version ORDER BY version`)
      .bind(window)
      .all(),
    env.DB.prepare(
      `SELECT os || '/' || arch AS platform, COUNT(*) AS n FROM (${latest})
       WHERE os IS NOT NULL GROUP BY platform ORDER BY platform`,
    )
      .bind(window)
      .all(),
  ]);

  return json({
    active: active35d ?? 0, // kept for the old dashboard shape
    active_35d: active35d ?? 0,
    active_7d: active7d ?? 0,
    today: today ?? 0,
    by_version: toCounts(byVersion.results, "version"),
    by_platform: toCounts(byOs.results, "platform"),
  });
}

async function handleFeatures(url, env) {
  const requested = Number.parseInt(url.searchParams.get("days") ?? "30", 10);
  const days = Number.isFinite(requested)
    ? Math.min(Math.max(requested, 1), MAX_FEATURE_WINDOW_DAYS)
    : 30;

  // Explode every payload's `counters` object into (key, value) rows and sum.
  // Payloads without counters (legacy pings) contribute nothing.
  const { results } = await env.DB.prepare(
    `SELECT j.key AS key, SUM(j.value) AS total, COUNT(DISTINCT p.install_id) AS installs
     FROM pings AS p, json_each(p.payload, '$.counters') AS j
     WHERE p.day >= date('now', ?1)
     GROUP BY j.key ORDER BY j.key`,
  )
    .bind(`-${days} days`)
    .all();

  const counters = {};
  for (const row of results) counters[row.key] = { total: row.total, installs: row.installs };
  return json({ days, counters });
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: CORS });
    }

    try {
      if (request.method === "POST" && url.pathname === "/ping") {
        return await handlePing(request, env);
      }
      if (request.method === "GET" && url.pathname === "/stats") {
        return await handleStats(env);
      }
      if (request.method === "GET" && url.pathname === "/stats/features") {
        return await handleFeatures(url, env);
      }
    } catch (err) {
      console.error("telemetry worker error", err);
      return text("internal error", 500);
    }

    return text("not found", 404);
  },
};

/**
 * Recall telemetry worker
 *
 * POST /ping  { id: "<uuid-v4>" }
 *   → stores the install ID in KV with a 35-day TTL (rolling window).
 *     Re-pinging the same ID just refreshes its expiry — no duplicates.
 *
 * GET  /stats
 *   → { active: N }  (number of installs seen in the last 35 days)
 *
 * No IP addresses, no personal data, no identifying information beyond
 * the random install UUID which lives only in this KV namespace.
 */

const CORS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type",
};

const TTL_SECONDS = 35 * 24 * 60 * 60; // 35 days rolling window
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // Preflight
    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: CORS });
    }

    // POST /ping
    if (request.method === "POST" && url.pathname === "/ping") {
      let body;
      try {
        body = await request.json();
      } catch {
        return new Response("bad request", { status: 400, headers: CORS });
      }

      const id = body?.id;
      if (typeof id !== "string" || !UUID_RE.test(id)) {
        return new Response("invalid id", { status: 400, headers: CORS });
      }

      await env.INSTALLS.put(id, "1", { expirationTtl: TTL_SECONDS });
      return new Response("ok", { status: 200, headers: CORS });
    }

    // GET /stats
    if (request.method === "GET" && url.pathname === "/stats") {
      // KV list returns up to 1000 keys per call; fine for a small app.
      // If you ever hit >1000 active users, add pagination here.
      const list = await env.INSTALLS.list();
      const active = list.keys.length + (list.list_complete ? 0 : "?"); // "?" signals truncation
      return new Response(JSON.stringify({ active }), {
        status: 200,
        headers: { ...CORS, "Content-Type": "application/json" },
      });
    }

    return new Response("not found", { status: 404, headers: CORS });
  },
};

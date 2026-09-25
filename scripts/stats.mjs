#!/usr/bin/env node
/**
 * Recall telemetry viewer — a small terminal UI over the worker's public
 * `/stats` and `/stats/features` endpoints (see worker/src/index.js).
 *
 *   npm run stats                # interactive TUI
 *   npm run stats -- --days 90   # start on a 90-day feature window
 *   npm run stats -- --once      # print once and exit (also the default when piped)
 *   npm run stats -- --url http://localhost:8787   # point at `wrangler dev`
 *
 * Keys: ↑/↓ or j/k scroll · space/b page · g/G top/bottom · 1–4 or [ ] window · r refresh · q quit
 *
 * No dependencies; needs Node 18+ for global fetch.
 */

import { stdin, stdout, argv, env, exit } from "node:process";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

const DEFAULT_URL = "https://recall-telemetry.recall-app.workers.dev";
const WINDOWS = [7, 30, 90, 365];
const BAR_WIDTH = 18;
const FETCH_TIMEOUT_MS = 10_000;

/** Human labels for counter-key prefixes; unknown prefixes fall back to the raw prefix. */
const GROUPS = {
  app: "App",
  welcome: "Welcome screen",
  onboarding: "Onboarding",
  timeline: "Timeline",
  nav: "Date navigation",
  group_mode: "Grouping",
  filter: "Source filters",
  harvest: "Harvest check marks",
  export: "Export for AI",
  link: "Outbound links",
  settings: "Settings",
  cache: "Cache",
  update: "Update toast",
  ical: "Calendar sync",
};

/** Health matrices: keys look like `<prefix>.<source>.<bucket>`. */
const HEALTH = {
  load_ms: { title: "Day fetch duration", buckets: ["lt1s", "1to3s", "3to10s", "gt10s"] },
  range_ms: { title: "Range fetch duration", buckets: ["lt1s", "1to3s", "3to10s", "gt10s"] },
  events: { title: "Events per loaded day", buckets: ["0", "1to10", "11to50", "50plus"] },
  error: { title: "Errors by class", buckets: ["auth", "rate_limit", "network", "config", "other"] },
};
const BUCKET_LABELS = {
  lt1s: "<1s", "1to3s": "1–3s", "3to10s": "3–10s", gt10s: ">10s",
  "0": "0", "1to10": "1–10", "11to50": "11–50", "50plus": "50+",
  auth: "auth", rate_limit: "rate limit", network: "network", config: "config", other: "other",
};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

function parseArgs(args) {
  const opts = { url: DEFAULT_URL, days: 30, once: false, help: false };
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--once") opts.once = true;
    else if (a === "--help" || a === "-h") opts.help = true;
    else if (a === "--days") opts.days = Number.parseInt(args[++i], 10);
    else if (a.startsWith("--days=")) opts.days = Number.parseInt(a.slice(7), 10);
    else if (a === "--url") opts.url = args[++i];
    else if (a.startsWith("--url=")) opts.url = a.slice(6);
    else {
      stdout.write(`Unknown argument: ${a}\n`);
      opts.help = true;
    }
  }
  if (!Number.isFinite(opts.days) || opts.days < 1) opts.days = 30;
  if (opts.url) opts.url = opts.url.replace(/\/+$/, "");
  return opts;
}

// ---------------------------------------------------------------------------
// ANSI helpers
// ---------------------------------------------------------------------------

const useColor = stdout.isTTY && !("NO_COLOR" in env);
const style = (code) => (s) => (useColor ? `\x1b[${code}m${s}\x1b[0m` : String(s));
const bold = style("1");
const dim = style("2");
const accent = style("38;5;208");
const accentBold = style("1;38;5;208");
const inverse = style("7");
const red = style("31");

const ANSI_RE = /\x1b\[[0-9;]*m/g;
const visibleWidth = (s) => s.replace(ANSI_RE, "").length;
const padEnd = (s, n) => s + " ".repeat(Math.max(0, n - visibleWidth(s)));
const padStart = (s, n) => " ".repeat(Math.max(0, n - visibleWidth(s))) + s;
const truncate = (s, n) => (visibleWidth(s) <= n ? s : s.replace(ANSI_RE, "").slice(0, Math.max(0, n - 1)) + "…");

const fmt = (n) => Number(n ?? 0).toLocaleString("en-US");

/** Bar width that leaves `reserved` columns for labels and numbers, never below 6. */
const fitBar = (width, reserved) => Math.max(6, Math.min(BAR_WIDTH, width - reserved));

function bar(value, max, width = BAR_WIDTH) {
  const filled = max > 0 ? Math.round((width * value) / max) : 0;
  return accent("█".repeat(filled)) + dim("░".repeat(width - filled));
}

/** Lay panels (arrays of lines) side by side, each padded to `widths[i]`. */
function columns(panels, widths, gap = 3) {
  const height = Math.max(...panels.map((p) => p.length));
  const out = [];
  for (let i = 0; i < height; i++) {
    out.push(
      panels
        .map((p, j) => padEnd(truncate(p[i] ?? "", widths[j]), widths[j]))
        .join(" ".repeat(gap))
        .replace(/\s+$/, ""),
    );
  }
  return out;
}

function tile(label, value, width) {
  const inner = width - 2;
  return [
    "╭" + "─".repeat(inner) + "╮",
    "│" + padEnd(" " + dim(label.toUpperCase()), inner) + "│",
    "│" + padEnd(" " + accentBold(fmt(value)), inner) + "│",
    "╰" + "─".repeat(inner) + "╯",
  ];
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

async function getJson(url) {
  const res = await fetch(url, { signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) });
  if (!res.ok) throw new Error(`HTTP ${res.status} from ${url}`);
  return res.json();
}

async function load(baseUrl, days) {
  const [stats, features] = await Promise.all([
    getJson(`${baseUrl}/stats`),
    getJson(`${baseUrl}/stats/features?days=${days}`),
  ]);
  return { stats, features, fetchedAt: new Date() };
}

// ---------------------------------------------------------------------------
// Rendering (pure: state + width → lines)
// ---------------------------------------------------------------------------

function sectionTitle(text, right = "") {
  const left = bold(text.toUpperCase());
  return right ? `${left}  ${dim(right)}` : left;
}

function countTable(counts, width) {
  const entries = Object.entries(counts ?? {}).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  if (entries.length === 0) return [dim("no data yet")];
  const max = Math.max(...entries.map(([, n]) => n));
  const labelW = Math.min(Math.max(...entries.map(([l]) => l.length)), Math.max(8, width - 6 - 7));
  const barW = fitBar(width, labelW + 7);
  return entries.map(([label, n]) => `${padEnd(truncate(label, labelW), labelW)}  ${bar(n, max, barW)} ${padStart(fmt(n), 4)}`);
}

function renderInstalls(stats, width) {
  const lines = [sectionTitle("Installs", "distinct installs that sent a summary in the period")];
  const tileW = Math.floor((width - 6) / 3);
  lines.push(
    ...columns(
      [tile("Today", stats.today, tileW), tile("Last 7 days", stats.active_7d, tileW), tile("Last 35 days", stats.active_35d ?? stats.active, tileW)],
      [tileW, tileW, tileW],
    ),
  );
  lines.push("");
  const half = Math.floor((width - 3) / 2);
  lines.push(
    ...columns(
      [
        [dim("By version"), ...countTable(stats.by_version, half)],
        [dim("By platform"), ...countTable(stats.by_platform, half)],
      ],
      [half, half],
    ),
  );
  return lines;
}

function windowSelector(days) {
  return WINDOWS.map((d, i) => {
    const label = d === 365 ? "1y" : `${d}d`;
    return d === days ? inverse(` ${i + 1} ${label} `) : dim(` ${i + 1} ${label} `);
  }).join(" ");
}

function renderFeatures(counters, days, width) {
  const lines = [sectionTitle(`Feature usage · last ${days} days`) + "   " + windowSelector(days)];
  lines.push(dim("total = times it happened across all installs · installs = distinct installs that did it at least once"));

  const groups = new Map();
  for (const [key, { total, installs }] of Object.entries(counters)) {
    const prefix = key.split(".")[0];
    if (HEALTH[prefix]) continue;
    if (!groups.has(prefix)) groups.set(prefix, []);
    groups.get(prefix).push({ key, total, installs });
  }
  if (groups.size === 0) {
    lines.push("", dim("no feature counters in this window"));
    return lines;
  }
  const order = Object.keys(GROUPS);
  const rank = (p) => (order.indexOf(p) === -1 ? 999 : order.indexOf(p));
  const sorted = [...groups.entries()].sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b));

  // key + 2 + bar + 1 + total(7) + 1 + installs(9): shrink the bar first, then the key column.
  const keyW = Math.min(Math.max(...[...groups.values()].flat().map((r) => r.key.length)), Math.max(12, width - 6 - 20));
  const barW = fitBar(width, keyW + 20);
  for (const [prefix, rows] of sorted) {
    rows.sort((a, b) => b.total - a.total || a.key.localeCompare(b.key));
    const max = Math.max(...rows.map((r) => r.total));
    lines.push("", accent(GROUPS[prefix] ?? prefix));
    lines.push(dim(`${padEnd("counter", keyW)}  ${" ".repeat(barW)} ${padStart("total", 7)} ${padStart("installs", 9)}`));
    for (const r of rows) {
      lines.push(`${padEnd(truncate(r.key, keyW), keyW)}  ${bar(r.total, max, barW)} ${padStart(fmt(r.total), 7)} ${padStart(fmt(r.installs), 9)}`);
    }
  }
  return lines;
}

function renderHealth(counters, days) {
  const lines = [sectionTitle("Health", `per-source fetch timings, event volumes and error classes · last ${days} days`)];
  let any = false;
  for (const [prefix, { title, buckets }] of Object.entries(HEALTH)) {
    const bySource = new Map();
    for (const [key, { total }] of Object.entries(counters)) {
      const parts = key.split(".");
      if (parts[0] !== prefix || parts.length < 3) continue;
      const source = parts.slice(1, -1).join(".");
      const bucket = parts.at(-1);
      if (!bySource.has(source)) bySource.set(source, {});
      bySource.get(source)[bucket] = (bySource.get(source)[bucket] ?? 0) + total;
    }
    if (bySource.size === 0) continue;
    any = true;
    const sources = [...bySource.keys()].sort();
    const srcW = Math.max(6, ...sources.map((s) => s.length));
    const colW = Math.max(6, ...buckets.map((b) => (BUCKET_LABELS[b] ?? b).length + 1));
    lines.push("", accent(title));
    lines.push(dim(padEnd("source", srcW) + buckets.map((b) => padStart(BUCKET_LABELS[b] ?? b, colW)).join("") + padStart("total", 8)));
    for (const s of sources) {
      const row = bySource.get(s);
      const total = Object.values(row).reduce((a, b) => a + b, 0);
      lines.push(padEnd(s, srcW) + buckets.map((b) => padStart(row[b] ? fmt(row[b]) : dim("·"), colW)).join("") + padStart(fmt(total), 8));
    }
  }
  if (!any) lines.push("", dim("no health counters in this window"));
  return lines;
}

function renderBody(state, width) {
  const { data, error, loading, days, url } = state;
  const host = url.replace(/^https?:\/\//, "");
  const when = data ? `updated ${data.fetchedAt.toLocaleTimeString("en-GB")}` : "";
  const status = loading ? accent("loading…") : error ? red(error) : dim(when);
  const left = truncate(`${accentBold("Recall")} ${bold("telemetry")}  ${dim(host)}`, width - visibleWidth(status) - 2);
  const header = padEnd(left, width - visibleWidth(status)) + status;

  const lines = [header, dim("─".repeat(width)), ""];
  if (!data) {
    if (error) lines.push(red(`Could not load statistics: ${error}`), "", dim("press r to retry"));
    else lines.push(dim("loading…"));
    return lines;
  }
  const counters = data.features.counters ?? {};
  lines.push(...renderInstalls(data.stats, width), "", "");
  lines.push(...renderFeatures(counters, data.features.days ?? days, width), "", "");
  lines.push(...renderHealth(counters, data.features.days ?? days));
  return lines;
}

// ---------------------------------------------------------------------------
// One-shot mode
// ---------------------------------------------------------------------------

async function printOnce(opts) {
  const width = Math.max(60, stdout.columns ?? 100);
  const state = { url: opts.url, days: opts.days, data: null, error: null, loading: false };
  try {
    state.data = await load(opts.url, opts.days);
  } catch (err) {
    state.error = err.message;
  }
  stdout.write(renderBody(state, width).join("\n") + "\n");
  return state.error ? 1 : 0;
}

// ---------------------------------------------------------------------------
// Interactive mode
// ---------------------------------------------------------------------------

async function interactive(opts) {
  const state = { url: opts.url, days: opts.days, data: null, error: null, loading: false, scroll: 0, requestId: 0 };

  const enter = () => stdout.write("\x1b[?1049h\x1b[2J\x1b[?25l");
  const leave = () => stdout.write("\x1b[?25h\x1b[?1049l");
  const quit = (code = 0) => {
    leave();
    try {
      stdin.setRawMode(false);
    } catch {}
    exit(code);
  };

  function draw() {
    const width = Math.max(60, stdout.columns ?? 100);
    const height = Math.max(10, stdout.rows ?? 30);
    const body = renderBody(state, width);
    const viewport = height - 1; // last row is the footer
    const maxScroll = Math.max(0, body.length - viewport);
    state.scroll = Math.min(Math.max(0, state.scroll), maxScroll);

    const out = ["\x1b[H"];
    for (let i = 0; i < viewport; i++) {
      const line = body[state.scroll + i] ?? "";
      out.push(truncate(line, width) + "\x1b[K\n");
    }
    const pos = maxScroll > 0 ? ` ${state.scroll + 1}–${Math.min(body.length, state.scroll + viewport)}/${body.length}` : "";
    const keys = "↑↓ scroll · 1-4 window · r refresh · q quit";
    out.push(padEnd(dim(keys), width - pos.length) + dim(pos) + "\x1b[K");
    stdout.write(out.join(""));
  }

  async function refresh() {
    const id = ++state.requestId;
    state.loading = true;
    state.error = null;
    draw();
    try {
      const data = await load(state.url, state.days);
      if (id !== state.requestId) return; // a newer request took over
      state.data = data;
    } catch (err) {
      if (id !== state.requestId) return;
      state.error = err.message;
    } finally {
      if (id === state.requestId) state.loading = false;
    }
    draw();
  }

  function setWindow(days) {
    if (days === state.days) return;
    state.days = days;
    refresh();
  }

  function onKey(chunk) {
    const key = chunk.toString();
    const viewport = Math.max(10, stdout.rows ?? 30) - 1;
    switch (key) {
      case "q":
      case "\x03": // ctrl-c
      case "\x1b": // esc
        return quit();
      case "r":
        return refresh();
      case "j":
      case "\x1b[B":
        state.scroll += 1;
        break;
      case "k":
      case "\x1b[A":
        state.scroll -= 1;
        break;
      case " ":
      case "\x1b[6~": // page down
        state.scroll += viewport - 2;
        break;
      case "b":
      case "\x1b[5~": // page up
        state.scroll -= viewport - 2;
        break;
      case "g":
      case "\x1b[H":
        state.scroll = 0;
        break;
      case "G":
      case "\x1b[F":
        state.scroll = Number.MAX_SAFE_INTEGER;
        break;
      case "[":
        return setWindow(WINDOWS[(WINDOWS.indexOf(state.days) - 1 + WINDOWS.length) % WINDOWS.length] ?? WINDOWS[0]);
      case "]":
        return setWindow(WINDOWS[(WINDOWS.indexOf(state.days) + 1) % WINDOWS.length] ?? WINDOWS[0]);
      default:
        if (/^[1-4]$/.test(key)) return setWindow(WINDOWS[Number(key) - 1]);
        return;
    }
    draw();
  }

  enter();
  stdin.setRawMode(true);
  stdin.resume();
  stdin.on("data", onKey);
  stdout.on("resize", draw);
  process.on("SIGINT", () => quit());
  process.on("SIGTERM", () => quit());
  await refresh();
}

// ---------------------------------------------------------------------------

const opts = parseArgs(argv.slice(2));
if (opts.help) {
  stdout.write(`Usage: node scripts/stats.mjs [--days N] [--once] [--url BASE]

  --days N   feature-usage window in days (default 30; 1–365)
  --once     print once and exit instead of the interactive view
  --url      telemetry worker base URL (default ${DEFAULT_URL})
`);
  exit(0);
}

if (opts.once || !stdout.isTTY || !stdin.isTTY) {
  exit(await printOnce(opts));
} else {
  await interactive(opts);
}

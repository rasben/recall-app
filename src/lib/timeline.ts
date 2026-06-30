import type { TimelineEvent, TimelineEventSource } from "../bindings";

export type { TimelineEvent, TimelineEventSource } from "../bindings";

/** Canonical display label per source — the single source of truth shared by
 *  the timeline badges, the source filter, and the export. */
export const SOURCE_LABELS: Record<TimelineEventSource, string> = {
  git: "Git",
  github: "GitHub",
  calendar: "Calendar",
  gmail: "Gmail",
  drive: "Drive",
  jira: "Jira",
  zulip: "Zulip",
};

/** A timeline row is either a single event or a tight burst of git commits. */
export type TimelineRow =
  | { kind: "event"; event: TimelineEvent }
  | { kind: "group"; events: TimelineEvent[]; key: string };
/** `gapMinutes` is the elapsed time since the previous row ended (null for the
 *  day's first row); used to draw "elapsed" chips and idle dividers. */
export type IndexedRow = { row: TimelineRow; index: number; gapMinutes: number | null };
export type HourGroup = { hour: string; items: IndexedRow[] };

/** Commits within this many seconds of the previous one collapse into a group. */
export const COMMIT_GROUP_WINDOW_SECONDS = 5;

/** Hide elapsed chips below this (sub-task noise). */
export const GAP_MIN_MINUTES = 3;
/** Above this an elapsed gap reads as a break, shown as an idle divider. */
export const GAP_IDLE_MINUTES = 45;

/** Format a Date as a local-calendar `YYYY-MM-DD` (never UTC). */
function toLocalIso(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function todayIso(): string {
  return toLocalIso(new Date());
}

export function addDaysIso(iso: string, days: number): string {
  const [y, m, d] = iso.split("-").map(Number);
  const date = new Date(y, m - 1, d);
  date.setDate(date.getDate() + days);
  return toLocalIso(date);
}

export function formatDayHeadingParts(
  iso: string,
  locale = "en-US",
): {
  weekday: string;
  monthDay: string;
} {
  const d = new Date(iso + "T12:00:00");
  return {
    weekday: d.toLocaleDateString(locale, { weekday: "long" }),
    monthDay: d.toLocaleDateString(locale, { month: "long", day: "numeric" }),
  };
}

export function formatDayHeading(iso: string, locale = "en-US"): string {
  const { weekday, monthDay } = formatDayHeadingParts(iso, locale);
  return `${weekday}, ${monthDay}`;
}

/**
 * Optimistic toggle: adds id if absent, removes if present.
 * Returns [new set, true if id was added / false if removed].
 */
export function applyOptimisticToggle(
  doneIds: Set<string>,
  id: string,
): [Set<string>, boolean] {
  const wasAdded = !doneIds.has(id);
  const copy = new Set(doneIds);
  if (wasAdded) copy.add(id);
  else copy.delete(id);
  return [copy, wasAdded];
}

/** Undoes the optimistic toggle when the backend call fails. */
export function rollbackOptimisticToggle(
  doneIds: Set<string>,
  id: string,
  wasAdded: boolean,
): Set<string> {
  const copy = new Set(doneIds);
  if (wasAdded) copy.delete(id);
  else copy.add(id);
  return copy;
}

export function groupEventsByHour(rows: TimelineRow[]): HourGroup[] {
  const groups: HourGroup[] = [];
  let currentHour = "";
  // Tracked across hour boundaries so a gap spanning, say, 10:55 → 11:20 is
  // measured correctly even though the two rows land in different hour buckets.
  let prevEndTs: number | null = null;
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    const time = rowTime(row);
    const hour = time.slice(0, 2) + ":00";
    if (hour !== currentHour) {
      currentHour = hour;
      groups.push({ hour, items: [] });
    }
    const gapMinutes =
      prevEndTs === null
        ? null
        : Math.max(0, Math.round((rowStartTimestamp(row) - prevEndTs) / 60));
    groups[groups.length - 1].items.push({ row, index: i, gapMinutes });
    prevEndTs = rowEndTimestamp(row);
  }
  return groups;
}

function rowTime(row: TimelineRow): string {
  return row.kind === "event" ? row.event.time : row.events[row.events.length - 1].time;
}

function rowStartTimestamp(row: TimelineRow): number {
  return row.kind === "event" ? row.event.timestamp : row.events[0].timestamp;
}

/** When a row "ends" for gap purposes. Calendar events carry a real duration in
 *  `detail`, so their end is start + duration — otherwise the gap after a 1h30m
 *  meeting would wrongly count the meeting itself as idle time. */
function rowEndTimestamp(row: TimelineRow): number {
  if (row.kind === "group") return row.events[row.events.length - 1].timestamp;
  const ev = row.event;
  if (ev.source === "calendar") {
    const minutes = parseDurationToMinutes(ev.detail);
    if (minutes !== null) return ev.timestamp + minutes * 60;
  }
  return ev.timestamp;
}

/** Parse a calendar duration detail ("20m", "1h", "1h 30m") to minutes, or null. */
export function parseDurationToMinutes(detail: string | null | undefined): number | null {
  if (!detail) return null;
  const m = detail.trim().match(/^(?:(\d+)h)?\s*(?:(\d+)m)?$/);
  if (!m || (!m[1] && !m[2])) return null;
  return (m[1] ? parseInt(m[1], 10) : 0) * 60 + (m[2] ? parseInt(m[2], 10) : 0);
}

/** Compact, neutral elapsed label: "18m", "1h", "2h 5m". */
export function formatGapLabel(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h && m) return `${h}h ${m}m`;
  if (h) return `${h}h`;
  return `${m}m`;
}

/** Stable key uniquely identifying the "repo" portion of a git event id. */
function gitRepoKey(event: TimelineEvent): string | null {
  if (event.source !== "git") return null;
  // id format: "git:{repo_path}:{hash}". Strip the trailing hash so we group
  // commits from the same repo. Hashes are hex and have no ":" so split from
  // the right.
  const lastColon = event.id.lastIndexOf(":");
  if (lastColon < 0) return null;
  return event.id.slice(0, lastColon);
}

/** A Dependabot merge/bump commit — the noisy, auto-generated kind worth
 *  collapsing on sight regardless of how spread out in time they are. */
export function isDependabotCommit(event: TimelineEvent): boolean {
  return event.source === "git" && /dependabot/i.test(event.title);
}

/**
 * Collapse runs of git commits in the same repo into one group row. Normal
 * commits group only when each is within `COMMIT_GROUP_WINDOW_SECONDS` of the
 * previous (a rebase/push burst); Dependabot commits group across any gap (they
 * trickle in over minutes but are pure noise), as long as the run stays in the
 * same repo. Non-git events and lone commits become `{kind: "event"}` rows.
 */
export function groupCloseCommits(events: TimelineEvent[]): TimelineRow[] {
  const out: TimelineRow[] = [];
  let current: TimelineEvent[] = [];
  let currentRepo: string | null = null;
  let currentIsDependabot = false;

  const flush = () => {
    if (current.length === 0) return;
    if (current.length === 1) {
      out.push({ kind: "event", event: current[0] });
    } else {
      const first = current[0];
      const last = current[current.length - 1];
      out.push({
        kind: "group",
        events: current,
        key: `${first.id}..${last.id}+${current.length}`,
      });
    }
    current = [];
    currentRepo = null;
    currentIsDependabot = false;
  };

  for (const event of events) {
    const repoKey = gitRepoKey(event);
    if (repoKey === null) {
      flush();
      out.push({ kind: "event", event });
      continue;
    }
    const isDependabot = isDependabotCommit(event);
    if (current.length === 0) {
      current = [event];
      currentRepo = repoKey;
      currentIsDependabot = isDependabot;
      continue;
    }
    const prev = current[current.length - 1];
    const sameRepo = repoKey === currentRepo;
    // A Dependabot run absorbs any further same-repo Dependabot commit; a normal
    // burst only absorbs nearby same-repo non-Dependabot commits.
    const extend = currentIsDependabot
      ? sameRepo && isDependabot
      : sameRepo && !isDependabot && event.timestamp - prev.timestamp <= COMMIT_GROUP_WINDOW_SECONDS;
    if (extend) {
      current.push(event);
    } else {
      flush();
      current = [event];
      currentRepo = repoKey;
      currentIsDependabot = isDependabot;
    }
  }
  flush();
  return out;
}

/** Project prefixes recognized in event text by default. `groupByTicket` also
 *  auto-extends this from the JIRA keys present in the day (JIRA events carry
 *  the key verbatim), so listing the user's main projects here only matters on
 *  days that have git/GitHub work but no JIRA activity. */
export const DEFAULT_TICKET_PREFIXES = ["DDF", "PLS"];

/** A ticket-like token: an uppercase prefix, a hyphen, and a number (e.g.
 *  "DDF-312"). No trailing boundary so branch-style "DDF-339_location" matches;
 *  case-sensitive and prefix-gated (below) so "utf-8" / "RFC-3339" don't. */
const TICKET_TOKEN_RE = /\b([A-Z][A-Z0-9]+)-(\d+)/g;

/** The JIRA key out of a `jira:{KEY}:{day}:{action}` event id, or null. */
function ticketIdFromJiraId(id: string): string | null {
  const m = id.match(/^jira:([A-Z][A-Z0-9]+-\d+):/);
  return m ? m[1] : null;
}

/**
 * Extract a ticket id (e.g. "DDF-312") from an event, or null. JIRA events take
 * it from their id (always reliable). Other sources are scanned in title then
 * detail, but only tokens whose prefix is in `prefixes` count — that keeps
 * version strings ("RFC-3339"), PR refs ("PR-123") and the like from matching.
 */
export function extractTicketId(
  ev: TimelineEvent,
  prefixes: Set<string> = new Set(DEFAULT_TICKET_PREFIXES),
): string | null {
  if (ev.source === "jira") {
    const fromId = ticketIdFromJiraId(ev.id);
    if (fromId) return fromId;
  }
  for (const text of [ev.title, ev.detail ?? ""]) {
    for (const m of text.matchAll(TICKET_TOKEN_RE)) {
      if (prefixes.has(m[1])) return `${m[1]}-${m[2]}`;
    }
  }
  return null;
}

export type TicketGroup = {
  id: string;
  events: TimelineEvent[];
  firstTs: number;
  lastTs: number;
};

/**
 * Bucket events that share a ticket id into cross-source groups, recording each
 * group's first/last timestamp so a span can be shown. The recognized prefix set
 * is seeded from {@link DEFAULT_TICKET_PREFIXES} and extended with every JIRA key
 * in `events`. Events with no ticket id land in `ungrouped`. Groups are sorted by
 * first timestamp; events within a group by timestamp.
 */
export function groupByTicket(events: TimelineEvent[]): {
  ticketGroups: TicketGroup[];
  ungrouped: TimelineEvent[];
} {
  const prefixes = new Set(DEFAULT_TICKET_PREFIXES);
  for (const ev of events) {
    if (ev.source === "jira") {
      const key = ticketIdFromJiraId(ev.id);
      if (key) prefixes.add(key.split("-")[0]);
    }
  }

  const byId = new Map<string, TimelineEvent[]>();
  const ungrouped: TimelineEvent[] = [];
  for (const ev of events) {
    const id = extractTicketId(ev, prefixes);
    if (id) {
      const arr = byId.get(id);
      if (arr) arr.push(ev);
      else byId.set(id, [ev]);
    } else {
      ungrouped.push(ev);
    }
  }

  const ticketGroups: TicketGroup[] = [];
  for (const [id, evs] of byId) {
    const sorted = [...evs].sort((a, b) => a.timestamp - b.timestamp);
    ticketGroups.push({
      id,
      events: sorted,
      firstTs: sorted[0].timestamp,
      lastTs: sorted[sorted.length - 1].timestamp,
    });
  }
  ticketGroups.sort((a, b) => a.firstTs - b.firstTs);
  return { ticketGroups, ungrouped };
}

/** What a task-view card is keyed on, shown so a repo/stream card isn't mistaken
 *  for a ticket. */
export type TaskGroupKeyType = "ticket" | "repo" | "stream";

/** A task-view row: either a multi-event group card or a standalone event. */
export type TaskRow =
  | {
      kind: "group";
      key: string;
      label: string;
      keyType: TaskGroupKeyType;
      events: TimelineEvent[];
      firstTs: number;
      lastTs: number;
    }
  | { kind: "event"; event: TimelineEvent };

/** Stream name out of a `zulip:stream:{name}:{day}` id (name may contain ':'). */
function streamNameFromId(id: string): string | null {
  const m = id.match(/^zulip:stream:(.+):\d{4}-\d{2}-\d{2}$/);
  return m ? m[1] : null;
}

function taskRowStartTs(row: TaskRow): number {
  return row.kind === "group" ? row.firstTs : row.event.timestamp;
}

/**
 * Re-shape a day's events around the unit of work. Ticket groups come first
 * (cross-source, from {@link groupByTicket}); ticket-less events then cluster on
 * a secondary key — git by repo, Zulip by stream — so nothing scatters. Anything
 * with no useful key, and any group that would hold a single event, renders as a
 * standalone row. Rows are ordered by start time.
 */
export function groupByTask(events: TimelineEvent[]): TaskRow[] {
  const { ticketGroups, ungrouped } = groupByTicket(events);

  const groups: Array<Extract<TaskRow, { kind: "group" }>> = ticketGroups.map((g) => ({
    kind: "group",
    key: `ticket:${g.id}`,
    label: g.id,
    keyType: "ticket",
    events: g.events,
    firstTs: g.firstTs,
    lastTs: g.lastTs,
  }));

  const secondary = new Map<
    string,
    { label: string; keyType: TaskGroupKeyType; events: TimelineEvent[] }
  >();
  const loose: TimelineEvent[] = [];
  for (const ev of ungrouped) {
    let key: string | null = null;
    let label = "";
    let keyType: TaskGroupKeyType = "repo";
    if (ev.source === "git") {
      const repoKey = gitRepoKey(ev);
      if (repoKey) {
        key = repoKey;
        // git detail is "{repo_name} — {short}"; the name is the readable label.
        label = ev.detail?.split(" — ")[0] ?? "repo";
        keyType = "repo";
      }
    } else if (ev.source === "zulip") {
      const stream = streamNameFromId(ev.id);
      if (stream) {
        key = `stream:${stream}`;
        label = `#${stream}`;
        keyType = "stream";
      }
    }
    if (key) {
      const entry = secondary.get(key);
      if (entry) entry.events.push(ev);
      else secondary.set(key, { label, keyType, events: [ev] });
    } else {
      loose.push(ev);
    }
  }
  for (const [key, entry] of secondary) {
    if (entry.events.length >= 2) {
      const sorted = [...entry.events].sort((a, b) => a.timestamp - b.timestamp);
      groups.push({
        kind: "group",
        key,
        label: entry.label,
        keyType: entry.keyType,
        events: sorted,
        firstTs: sorted[0].timestamp,
        lastTs: sorted[sorted.length - 1].timestamp,
      });
    } else {
      loose.push(...entry.events);
    }
  }

  // A one-event "group" is just noise — demote ticket/secondary singletons.
  const rows: TaskRow[] = [];
  for (const g of groups) {
    if (g.events.length >= 2) rows.push(g);
    else rows.push({ kind: "event", event: g.events[0] });
  }
  for (const ev of loose) rows.push({ kind: "event", event: ev });

  rows.sort((a, b) => taskRowStartTs(a) - taskRowStartTs(b));
  return rows;
}

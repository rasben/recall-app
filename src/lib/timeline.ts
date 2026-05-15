import type { TimelineEvent } from "../bindings";

export type { TimelineEvent, TimelineEventSource } from "../bindings";

/** A timeline row is either a single event or a tight burst of git commits. */
export type TimelineRow =
  | { kind: "event"; event: TimelineEvent }
  | { kind: "group"; events: TimelineEvent[]; key: string };
export type IndexedRow = { row: TimelineRow; index: number };
export type HourGroup = { hour: string; items: IndexedRow[] };

/** Commits within this many seconds of the previous one collapse into a group. */
export const COMMIT_GROUP_WINDOW_SECONDS = 5;

export function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export function addDaysIso(iso: string, days: number): string {
  const d = new Date(iso + "T12:00:00");
  d.setDate(d.getDate() + days);
  return d.toISOString().slice(0, 10);
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
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    const time = rowTime(row);
    const hour = time.slice(0, 2) + ":00";
    if (hour !== currentHour) {
      currentHour = hour;
      groups.push({ hour, items: [] });
    }
    groups[groups.length - 1].items.push({ row, index: i });
  }
  return groups;
}

function rowTime(row: TimelineRow): string {
  return row.kind === "event" ? row.event.time : row.events[row.events.length - 1].time;
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

/**
 * Collapse runs of git commits in the same repo whose timestamps are within
 * `COMMIT_GROUP_WINDOW_SECONDS` of the previous commit's. A slow trickle still
 * collapses as long as each step is within the window. Non-git events and
 * single commits become `{kind: "event"}` rows.
 */
export function groupCloseCommits(events: TimelineEvent[]): TimelineRow[] {
  const out: TimelineRow[] = [];
  let current: TimelineEvent[] = [];
  let currentRepo: string | null = null;

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
  };

  for (const event of events) {
    const repoKey = gitRepoKey(event);
    if (repoKey === null) {
      flush();
      out.push({ kind: "event", event });
      continue;
    }
    if (current.length === 0) {
      current = [event];
      currentRepo = repoKey;
      continue;
    }
    const prev = current[current.length - 1];
    const sameRepo = repoKey === currentRepo;
    const closeEnough = event.timestamp - prev.timestamp <= COMMIT_GROUP_WINDOW_SECONDS;
    if (sameRepo && closeEnough) {
      current.push(event);
    } else {
      flush();
      current = [event];
      currentRepo = repoKey;
    }
  }
  flush();
  return out;
}

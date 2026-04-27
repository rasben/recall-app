import type { TimelineEvent } from "../bindings";

export type { TimelineEvent, TimelineEventSource } from "../bindings";

export type IndexedEvent = { event: TimelineEvent; index: number };
export type HourGroup = { hour: string; items: IndexedEvent[] };

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

export function groupEventsByHour(events: TimelineEvent[]): HourGroup[] {
  const groups: HourGroup[] = [];
  let currentHour = "";
  for (let i = 0; i < events.length; i++) {
    const ev = events[i];
    const hour = ev.time.slice(0, 2) + ":00";
    if (hour !== currentHour) {
      currentHour = hour;
      groups.push({ hour, items: [] });
    }
    groups[groups.length - 1].items.push({ event: ev, index: i });
  }
  return groups;
}

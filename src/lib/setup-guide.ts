import type { TimelineEvent, TimelineEventSource } from "../bindings";
import { addDaysIso } from "./timeline";

/** The setup guide's steps after the welcome screen, in order. */
export const GUIDE_STEPS = ["sources", "preview", "claude"] as const;
export type GuideStep = "welcome" | (typeof GUIDE_STEPS)[number];

/**
 * The most recent Monday–Friday strictly before `todayIso` (`YYYY-MM-DD`), the
 * same day `recall timeline --date last-workday` covers: on a Monday, Friday.
 */
export function lastWorkdayIso(todayIso: string): string {
  let day = addDaysIso(todayIso, -1);
  while (isWeekend(day)) day = addDaysIso(day, -1);
  return day;
}

function isWeekend(iso: string): boolean {
  const weekday = new Date(`${iso}T12:00:00`).getDay();
  return weekday === 0 || weekday === 6;
}

/** Events per source, most first (ties keep first-seen order). */
export function countBySource(events: TimelineEvent[]): [TimelineEventSource, number][] {
  const counts = new Map<TimelineEventSource, number>();
  for (const ev of events) counts.set(ev.source, (counts.get(ev.source) ?? 0) + 1);
  return [...counts].sort((a, b) => b[1] - a[1]);
}

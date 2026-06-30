import { todayIso, type TimelineEventSource } from "./timeline";

export const navState = $state({
  selectedDate: todayIso(),
  dayCounts: {} as Record<string, number>,
  /** Sources the user has toggled off in the timeline view. Session-only — not persisted. */
  hiddenSources: new Set<TimelineEventSource>(),
  /** How the day's events are grouped: chronologically or by work item. Session-only. */
  groupMode: "time" as "time" | "task",
  /** When set, the page opens the Settings view and scrolls to the matching
   *  `#settings-{section}` panel. The page resets it to null once handled. */
  openSettingsSection: null as string | null,
});

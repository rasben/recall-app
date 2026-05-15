import { todayIso, type TimelineEventSource } from "./timeline";

export const navState = $state({
  selectedDate: todayIso(),
  dayCounts: {} as Record<string, number>,
  /** Sources the user has toggled off in the timeline view. Session-only — not persisted. */
  hiddenSources: new Set<TimelineEventSource>(),
});

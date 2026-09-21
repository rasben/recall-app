import type { HarvestTimeEntry } from "../bindings";

/** Sum of `hours` across entries (running timers included). */
export function totalHours(entries: readonly HarvestTimeEntry[]): number {
  return entries.reduce((sum, e) => sum + e.hours, 0);
}

/**
 * Decimal hours → `H:MM`, rounded to the nearest minute, matching Harvest's
 * own "hours:minutes" display (2.11 → `2:07`, 0.5 → `0:30`, 7 → `7:00`).
 */
export function formatHours(hours: number): string {
  if (!Number.isFinite(hours) || hours < 0) return "0:00";
  const totalMinutes = Math.round(hours * 60);
  const h = Math.floor(totalMinutes / 60);
  const m = totalMinutes % 60;
  return `${h}:${String(m).padStart(2, "0")}`;
}

import { todayIso } from "./timeline";

export const navState = $state({
  selectedDate: todayIso(),
  dayCounts: {} as Record<string, number>,
});

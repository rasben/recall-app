import type { SettingsUi } from "../bindings";
import { SOURCE_LABELS, type TimelineEventSource } from "./timeline";

export type GroupMode = "time" | "task";

/** Fallback for a missing settings_ui document. Persist the whole struct so
 *  the Rust and TS models stay aligned (see AGENTS.md). */
export const DEFAULT_SETTINGS_UI: SettingsUi = {
  theme: "system",
  group_mode: "time",
  hidden_sources: [],
};

/** The persisted grouping mode is a free string; anything unknown means "time". */
export function parseGroupMode(value: string | null | undefined): GroupMode {
  return value === "task" ? "task" : "time";
}

/** Keep only strings that are real timeline sources, so a source removed in a
 *  later version (or a corrupt value) cannot linger in the hidden set. */
export function parseHiddenSources(
  values: readonly string[] | null | undefined,
): Set<TimelineEventSource> {
  const known = new Set(Object.keys(SOURCE_LABELS));
  const out = new Set<TimelineEventSource>();
  for (const v of values ?? []) {
    if (known.has(v)) out.add(v as TimelineEventSource);
  }
  return out;
}

/** The subset of settings_ui that the timeline view owns, in a stable shape
 *  (sorted sources) so two snapshots can be compared by JSON string. */
export function viewStatePatch(
  groupMode: GroupMode,
  hiddenSources: ReadonlySet<TimelineEventSource>,
): Pick<SettingsUi, "group_mode" | "hidden_sources"> {
  return { group_mode: groupMode, hidden_sources: [...hiddenSources].sort() };
}

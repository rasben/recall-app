import { commands, type SettingsUi } from "../bindings";
import { todayIso, type TimelineEventSource } from "./timeline";
import { DEFAULT_SETTINGS_UI, parseGroupMode, parseHiddenSources, viewStatePatch, type GroupMode } from "./view-state";

export const navState = $state({
  selectedDate: todayIso(),
  dayCounts: {} as Record<string, number>,
  /** Sources the user has toggled off in the timeline view. Persisted in
   *  settings_ui (see `loadViewState` / `persistViewState`). */
  hiddenSources: new Set<TimelineEventSource>(),
  /** How the day's events are grouped: chronologically or by work item.
   *  Persisted in settings_ui. */
  groupMode: "time" as GroupMode,
  /** When set, the page opens the Settings view and scrolls to the matching
   *  `#settings-{section}` panel. The page resets it to null once handled. */
  openSettingsSection: null as string | null,
});

/** Apply the persisted grouping mode and hidden sources from settings_ui. */
export function loadViewState(ui: SettingsUi | null | undefined): void {
  navState.groupMode = parseGroupMode(ui?.group_mode);
  navState.hiddenSources = parseHiddenSources(ui?.hidden_sources);
}

/** Write the current grouping mode and hidden sources into settings_ui,
 *  keeping the other fields (theme) as they are on disk. */
export async function persistViewState(): Promise<void> {
  const current = (await commands.getSettingsUi()) ?? DEFAULT_SETTINGS_UI;
  await commands.setSettingsUi({
    ...current,
    ...viewStatePatch(navState.groupMode, navState.hiddenSources),
  });
}

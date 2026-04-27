# To-Do's

Roadmap and ideas for Recall. Items aren't strictly ordered — pick what feels useful.

## Product — close the Harvest loop

The whole point of the app is to feed Harvest, but it doesn't actually talk to Harvest yet. The checkmark is a manual "I did this elsewhere" marker.

- Cheap version: "Copy as Harvest entry" on each event (formatted note + suggested project/task), or a deep link that opens Harvest with the entry prefilled.
- Suggested durations / event grouping. Events are points in time; Harvest wants durations. Heuristic: group consecutive events from the same source/repo/ticket into a block, duration = gap until next block, capped at e.g. 90 min. Pairs naturally with Harvest submit.

## Data sources

- Gmail (requires Google OAuth — see AGENTS.md for why this is deferred)
  - Sent emails
  - Read emails
- Google Drive (requires Google OAuth)
  - Edited/Created files
  - Read files
- Zulip — expanded data
  - Messages you've read
- Slack. Reload uses Zulip, but client orgs often don't, and many Reload devs are in client Slacks. User-token install is straightforward.
- Linear. Same trivial PAT auth model as JIRA.
- Local IDE / editor activity. A small "I had this repo focused for X minutes" signal from VS Code or a generic file-watcher on the repo dir would catch the long stretches of work that produce no commits or comments — the most under-represented category in time tracking.

## Privacy & security

- Move credentials out of plain SQLite into the OS keychain
  (e.g. `tauri-plugin-stronghold` or the `keyring` crate — macOS Keychain / Windows Credential Manager / Linux Secret Service).
- Add a screen that shows the last 50 commands / API calls that have been run.
  - E.g. the terminal commands run by git data sources, or the APIs called by JIRA.
- "Export & purge all data" button. Pairs naturally with the keychain migration.

## Releases & distribution

- Tauri auto-updater. Today: redownload the DMG. Tauri has the `updater` plugin — set it up against the GitHub Releases artifacts already published.

## Quality

- Frontend tests. `npm test` is Rust-only. The Svelte side is now non-trivial (DayTimeline has debounce, prefetch, source-progress events, Harvest-done sync). Vitest + a couple of `@testing-library/svelte` tests around `groupEventsByHour`, the debounce behavior, and `toggleDone` rollback-on-error would prevent the obvious regressions.

## Bugs

- Using `local git` source on Windows opens a bunch of terminal windows. Makes it look like a virus.

## UX polish

- Per-source visibility toggle in the timeline header (hide GitHub for a moment without touching settings).
- "What's missing in Harvest today" inverted view (show un-checked only).

## Fun

- Add more transitions and animations.
- Add a TUI.
  - Either a real TUI, or a fake one, making the app easily navigated with the keyboard.

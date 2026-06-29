# To-Do's

Roadmap and ideas for Recall. Items aren't strictly ordered — pick what feels useful.

## Product — close the Harvest loop

The whole point of the app is to feed Harvest, but it doesn't actually talk to Harvest yet. The checkmark is a manual "I did this elsewhere" marker.

- Cheap version: "Copy as Harvest entry" on each event (formatted note + suggested project/task), or a deep link that opens Harvest with the entry prefilled.
- Suggested durations / event grouping. Events are points in time; Harvest wants durations. Heuristic: group consecutive events from the same source/repo/ticket into a block, duration = gap until next block, capped at e.g. 90 min. Pairs naturally with Harvest submit.

## Data sources

- Gmail — likely path is IMAP + Gmail App Password (same paste-a-token UX as JIRA/Zulip), avoiding the Google OAuth burden. Fall back to OAuth if Workspace admins have IMAP/app passwords disabled. See AGENTS.md.
  - Sent emails
  - Read emails
- Google Drive (requires Google OAuth — no IMAP-style escape hatch)
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

## Bugs

- Using `local git` source on Windows opens a bunch of terminal windows. Makes it look like a virus.

## UX polish

- Per-source visibility toggle in the timeline header (hide GitHub for a moment without touching settings).
- "What's missing in Harvest today" inverted view (show un-checked only).

## Fun

- Add more transitions and animations.
- Add a TUI.
  - Either a real TUI, or a fake one, making the app easily navigated with the keyboard.

## Timeline readability — make a day legible

The day timeline is good at "what happened when" but bad at "what did I work on, and roughly how long" — which is the whole point for Harvest. Three structural causes: every row is **equal visual weight** (a dependabot merge looks like a shipped feature); events are **points with no duration** except calendar; and work is scattered across sources by **ticket** (e.g. DDF-312 = commits + PR + JIRA + Zulip) but the feed never connects them.

**Honesty constraint that shapes all of this:** only calendar events carry a true duration. git/GitHub/JIRA/Zulip are durationless points, so we can surface real calendar durations, cross-source ticket clustering, and presence/elapsed windows — but everything inferred must be labelled a **span/presence window**, never "time spent". A tool whose failure mode is wrong Harvest entries must not invent hours.

### Core improvements (the structural fix for overview + time legibility)

- **Ticket-id extraction (data layer).** Add pure helpers to `src/lib/timeline.ts`: `extractTicketId(ev)` using a word-bounded, case-sensitive regex restricted to a **known-prefix allowlist** (DDF, PLS, …) so it doesn't match `UTF-8` / `RFC-3339` / `PR-123` / `CHANGE-2046`; and `groupByTicket(events)` keying by id and recording first/last timestamp. JIRA titles carry the key reliably; git/GitHub only when typed. This logic currently lives only as prose in the export prompt (`translations.ts:202` EN / `:409` DA) — promote it to code. *Don't ship standalone* — it's dead code without a consumer; bundle with the task-view below. Files: `src/lib/timeline.ts`, `src/lib/timeline.test.ts`. Effort: S.
- **`By time | By task` grouping toggle with ticket cards.** The headline change. Add `groupMode` to `navState`; keep the existing `groupCloseCommits → groupEventsByHour` pipeline untouched for "by time"; for "by task" feed `visibleEvents` through `groupByTicket` and render a new `TimelineTicketGroup.svelte` modelled on the existing `TimelineCommitGroup.svelte` (reuse expand/collapse + per-child `onToggle`). Ticket-less events fall back to secondary keys so nothing dumps into an undifferentiated "Other": Zulip stream (already in the id `zulip:stream:{stream}:{day}`), git repo (`gitRepoKey()` exists). Label each card's key type (`DDF-312` vs `repo: recall` vs `#DDF`). Turns a ~25-row wall into ~5 cards. Files: `src/components/DayTimeline.svelte`, `src/lib/timeline.ts`, `src/components/TimelineTicketGroup.svelte` (new), `src/lib/nav-state.svelte.ts`. Effort: M. **Note:** the card header span is a wall-clock window, NOT time spent — label it "span", never "spent".
- **Inline gap / idle labels.** Faint `+18m` / `+2h 5m` chips between rows and a visible "idle" divider for gaps > ~45m, so a work cluster's first-to-last span and the lunch gap are legible without mental math — the cheapest *honest* time signal. Gotcha: `groupEventsByHour` discards adjacency, so compute gaps on the flat row list *before* hour-grouping and thread `prevTimestamp` through; special-case the gap *after* a calendar row (it measures from the meeting's start, so it double-counts the meeting unless handled). Label neutrally (`+2h elapsed`, not `2h spent`). Files: `src/lib/timeline.ts`, `src/components/DayTimeline.svelte`. Effort: M.

### Optional / later

- **Sticky per-day summary header.** A strip between the date nav and the feed: per-source counts, summed meeting time (real, from calendar detail), active span, and tickets-touched. Purely additive, no backend change. Build after ticket extraction lands; hoist the private `sourceConfig` map out of `TimelineEvent.svelte` to reuse the icons/colors; derive the active span from **timestamps**, not `events[0].time` (overnight calendar rows display `00:00`). Effort: M.
- **Zulip session-splitting.** Split each stream's day into time-gap sessions (new session when the gap exceeds ~45 min) instead of one whole-day bucket, so morning vs afternoon activity shows separately and the span-in-title becomes meaningful. Makes Zulip rows more numerous on busy days, so pair it with grouping/de-emphasis. Bundle the resulting id change with the DM-per-partner change (see gotcha below). Files: `src-tauri/src/commands/timeline/zulip.rs`. Effort: M.

### Skip (evaluated, lower-value or infeasible)

- **Work-only toggle / noise classifier / un-loggable rows / configurable muted streams.** All ride on a brittle keyword/stream-name classifier; false-hiding real work is dangerous for a Harvest tool, and whole sources can already be hidden via the existing source filter.
- **Source color-rail, denser hour-spine.** Cosmetic; per-*source* coloring fights the cross-*ticket* grouping that's the actual need, and a per-hour event count is the weakest time proxy (distorted by collapsed bursts).
- **Weekly digest view / "summarize week" export preset.** The data model can't total per-task time, and the default export prompt already produces Harvest-shaped per-day summaries with a one-click 7-day preset — redundant.
- **Git branch-name ticket backfill (`%D`).** Empirically blank for exactly the merged feature branches it targets; if the task-view "Other" bucket ever gets large, parse the commit *subject* instead.

### Gotcha for whoever implements the Zulip changes

Changing Zulip event **ids** (DM-per-partner, session-splitting) orphans existing Zulip "done" checkmarks (UUIDv5 keyed on the id string in `harvest_done.rs:15-17`). Acceptable for a solo dev — but bundle all Zulip id changes into **one** change so the ids churn only once.

### If you do only three things

1. Calendar duration pill (S) — surface the one true time signal you have.
2. Ticket extraction + task-view toggle (M) — the 25-row wall becomes ~5 cross-source work cards. The real overview fix.
3. Zulip span + DM-per-partner + dominant topic (S, one id churn) — fixes the lumping and feeds better titles into the task cards.

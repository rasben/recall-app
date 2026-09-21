# To-Do's

Roadmap for Recall, in priority order. Pick from the top unless something below has become urgent. Last re-prioritized 2026-09-21.

**Where the product stands (2026-09-21):** ~5 active installs (telemetry), one open external issue (#101), no commits since v1.5 on 2026-06-30. The app's pitch is "fill in Harvest", but it never talks to Harvest — today the user reads the timeline and types into Harvest by hand. Everything below is ordered by how much it changes what the user *does*, not what they see.

**Honesty constraint that shapes all of this:** only calendar events carry a true duration. git/GitHub/JIRA/Zulip are durationless points, so we can surface real calendar durations, cross-source ticket clustering, and presence/elapsed windows — but everything inferred must be labelled a **span/presence window**, never "time spent". A tool whose failure mode is wrong Harvest entries must not invent hours. Any Harvest submit must show the suggested hours as an editable field the user confirms; never auto-submit.

---

## 1. Harvest read — show what's already logged

The smallest slice with a visible payoff. Harvest API v2 fits the existing paste-a-token pattern exactly: personal access token + account ID, no OAuth.

- New `settings_harvest.rs` + `Harvest.svelte` panel (token, account id, test-connection). Same shape as Jira/Zulip.
- Fetch the selected day's time entries (`GET /v2/time_entries?from=&to=` with `user_id` = me) and render a strip under the date nav: hours logged, per project.
- The manual Harvest checkmark becomes derived from what Harvest actually has (keep the manual toggle as an override for now; retire it once the write path lands).
- The "what's missing in Harvest today" inverted view falls out of this for free: un-logged rows vs. logged hours.

Effort: M.

## 2. Harvest write — log from a task card

- In the by-task view, each ticket/repo card gets "Log to Harvest". Prefill notes from the ticket ID + event titles; prefill hours from the card's span, **editable**; pick project/task from `GET /v2/users/me/project_assignments`.
- Remember the ticket-prefix (or repo) → project/task mapping after first use so the second entry is one click.
- `POST /v2/time_entries` with project, task, spent_date, hours, notes.
- After a successful post, re-fetch the day's entries (item 1) so the strip and checkmarks update.

Effort: M–L. Depends on 1.

## 3. Jira range truncation

- `events_for_range` sends `maxResults: 100` with no pagination. A month fetch or export for someone who touched >100 tickets silently drops the rest, and the truncated result is cached. Paginate `search/jql` (`nextPageToken`).
- Known limitation worth fixing while in there: the event time is the ticket's last `updated` timestamp by *anyone*, not the time of the user's action. The issue changelog endpoint gives per-action timestamps.

Effort: S–M.

## 4. Parallelize sources; prefetch as a range

- `get_timeline_for_day` runs Git → GitHub → Calendar → Jira → Zulip sequentially. They are independent; run them concurrently (the git module already does this per repo with `thread::scope`). Day load drops to the slowest source.
- The 6-day prefetch on startup issues six separate full fetches; each GitHub fetch re-runs the same search queries against the 30/minute search budget. Prefetch through the existing `collect_range_events` helper in one pass instead.

Effort: S–M.

## 5. Source registry refactor (do before adding the next source)

Adding a source today touches ~8 places: two dispatch blocks in `timeline/mod.rs`, `MONTH_SOURCES_TOTAL` in `TimelineDateNav.svelte`, the hand-built `enabledSources` list in `DayTimeline.svelte`, the icon map in `TimelineEvent.svelte`, `SOURCE_LABELS`, the Welcome list, and the settings tab.

- Rust: a small `Source` trait (`name`, `events_for_range`, `test_connection`) + one registry slice; both dispatch blocks iterate it.
- TS: one shared source descriptor (label, icon, colour) that the badge, filter, Welcome and loading overlay all read.

Effort: M. Pure refactor; pairs with items 1–2 since Harvest is the next source.

## 6. Credentials into the OS keychain

Plain-text SQLite is acceptable for ~5 users and the README is honest about it. It is a blocker for recommending the app to the rest of Reload. `keyring` crate (macOS Keychain / Windows Credential Manager / Linux Secret Service), with a one-time migration from the `settings` table. Pair with an "Export & purge all data" button.

Effort: M.

## 7. Frontend fixes (small, independent)

- **Row click marks as logged.** Reading is the common action, logging the rare one — the primary click is a footgun. Make the Harvest mark an explicit control; let the row click open the link or expand. (Becomes moot for the checkmark once item 1 lands, but the click target problem stays.)
- **Default language is Danish** regardless of locale (`detectLang` falls back to `"da"`). Default from `navigator.language`.
- **Grouping mode and source filter reset every launch.** Persist in `settings_ui`.
- **Arrow keys** (←/→) for day navigation. Cheap, and covers most of the old "fake TUI" wish.
- **Sticky per-day summary header.** Hours logged (from item 1), active span, tickets touched, summed meeting time. Derive the span from **timestamps**, not `events[0].time` (overnight calendar rows display `00:00`). Build after item 1.

## 8. Ask the users before building

Plausible, but validate with the actual installs first — none of these should be built on spec.

- **Slack.** Reload uses Zulip, but client orgs often don't. User-token install is straightforward.
- **Linear.** Same trivial PAT model as JIRA.
- **Local IDE / editor activity.** Would catch the long stretches of work that produce no commits — the most under-represented category. But it is a new daemon-style component (file watcher / editor extension), and item 2 is worth more than all three of these combined.
- **Gmail (sent mail via IMAP + App Password).** Weak time signal for a developer. Only if someone asks.
- **Tauri auto-updater.** The in-app "new version" toast is adequate at this scale.

## Later / optional

- **Zulip session-splitting.** Split each stream's day into time-gap sessions (~45 min) instead of one whole-day bucket. See the id-churn gotcha below. Effort: M.

---

## Killed (evaluated, not doing)

- **Google Drive.** Requires Google OAuth app verification for a five-user app, and edited-document timestamps are a weak time signal. Remove the dead placeholders: `TimelineEventSource::Drive`, the `drive` entry in `sourceConfig`/`SOURCE_LABELS`, and the Welcome "planned" chip.
- **Zulip "messages you've read".** Reading is not billable work; violates the honesty constraint.
- **A real TUI.** Arrow-key navigation (item 7) instead.
- **"Last 50 commands / API calls" screen.** A debug log file covers the need.
- **Work-only toggle / noise classifier / un-loggable rows / configurable muted streams.** All ride on a brittle keyword classifier; false-hiding real work is dangerous for a Harvest tool, and whole sources can already be hidden via the source filter.
- **Source colour-rail, denser hour-spine.** Cosmetic; per-*source* colouring fights the cross-*ticket* grouping that's the actual need.
- **Weekly digest view / "summarize week" export preset.** Revisit only after item 1: with Harvest hours per day, a week view of "logged vs. active span" becomes meaningful. Without it the data model can't total per-task time and the 7-day export preset already covers the summary.
- **Git branch-name ticket backfill (`%D`).** Empirically blank for exactly the merged feature branches it targets; parse the commit *subject* instead if the task-view "Other" bucket grows.
- **More animations.** NyanCat/Travolta/waiting GIFs are the app's personality — keep them, don't grow them.

---

## Gotchas

- **Zulip event ids.** Changing Zulip event ids (session-splitting etc.) orphans existing Zulip "done" checkmarks (UUIDv5 keyed on the id string in `harvest_done.rs`). Bundle all Zulip id changes into **one** change so the ids churn only once. Less important once item 1 derives the checkmark from Harvest.
- **Partial results must not be cached.** Errors block caching, and so does the *incomplete days* set that `collect_range_events` returns: GitHub reports the days it could not cover (search result cap). Jira's un-paginated `maxResults: 100` (item 3) still returns successfully with missing data and gets cached as complete; any source with a horizon or result cap must report it through that set from day one.

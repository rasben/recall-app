import type { RecallEvent } from "$lib/timeline";

export const MOCK_TIMELINE_EVENTS: RecallEvent[] = [
  { time: "08:12", source: "git", title: "fix: resolve null pointer in timeline parser", detail: "recall — 3 files changed" },
  { time: "08:45", source: "git", title: "chore: update Cargo.lock", detail: "recall — 1 file changed" },
  { time: "09:00", source: "calendar", title: "Daily standup", detail: "Google Meet · 15 min" },
  { time: "09:30", source: "jira", title: "PROJ-142 moved to In Progress", detail: "Timeline feature implementation" },
  {
    time: "10:15",
    source: "github",
    title: "Opened PR #87: Add timeline component",
    detail: "recall · 4 commits, 12 files",
    url: "https://github.com/rasben/recall/pull/87",
  },
  { time: "10:45", source: "gmail", title: "Re: Infrastructure costs Q2", detail: "Sent to ops-team@reload.dk" },
  { time: "11:00", source: "calendar", title: "Sprint planning", detail: "Google Meet · 60 min" },
  { time: "12:30", source: "zulip", title: "Message in #frontend", detail: '"Has anyone seen the new Svelte 5 runes docs?"' },
  { time: "13:15", source: "git", title: "feat: add date picker to timeline view", detail: "recall — 5 files changed" },
  {
    time: "14:00",
    source: "github",
    title: "Reviewed PR #85: Fix dark mode toggle",
    detail: "recall · approved",
    url: "https://github.com/rasben/recall/pull/85",
  },
  { time: "14:30", source: "drive", title: "Edited: Q2 Roadmap", detail: "Google Sheets" },
  { time: "15:00", source: "jira", title: "PROJ-142 moved to Review", detail: "Timeline feature implementation" },
  { time: "16:10", source: "git", title: "fix: dark mode on timeline cards", detail: "recall — 2 files changed" },
];

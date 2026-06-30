import { describe, expect, it } from "vitest";
import {
  applyOptimisticToggle,
  extractTicketId,
  formatGapLabel,
  groupByTask,
  groupByTicket,
  groupCloseCommits,
  groupEventsByHour,
  parseDurationToMinutes,
  rollbackOptimisticToggle,
} from "./timeline";
import type { TimelineEvent, TimelineEventSource, TimelineRow } from "./timeline";

function makeSourced(
  source: TimelineEventSource,
  id: string,
  title: string,
  detail: string | null = null,
  timestamp = 0,
): TimelineEvent {
  return { id, time: "10:00", timestamp, source, title, detail, url: null };
}

function makeEvent(time: string, id = time, timestamp = 0): TimelineEvent {
  return { id, time, timestamp, source: "git", title: "test", detail: null, url: null };
}

function makeCommit(repo: string, hash: string, time: string, timestamp: number): TimelineEvent {
  return {
    id: `git:${repo}:${hash}`,
    time,
    timestamp,
    source: "git",
    title: `commit ${hash}`,
    detail: null,
    url: null,
  };
}

function ev(event: TimelineEvent): TimelineRow {
  return { kind: "event", event };
}

// ---------------------------------------------------------------------------
// groupEventsByHour
// ---------------------------------------------------------------------------

describe("groupEventsByHour", () => {
  it("returns empty array for no events", () => {
    expect(groupEventsByHour([])).toEqual([]);
  });

  it("groups events in the same hour into one bucket", () => {
    const rows = [ev(makeEvent("10:05")), ev(makeEvent("10:30")), ev(makeEvent("10:59"))];
    const groups = groupEventsByHour(rows);
    expect(groups).toHaveLength(1);
    expect(groups[0].hour).toBe("10:00");
    expect(groups[0].items).toHaveLength(3);
  });

  it("creates a separate group for each distinct hour", () => {
    const rows = [ev(makeEvent("09:00")), ev(makeEvent("10:00")), ev(makeEvent("11:00"))];
    const groups = groupEventsByHour(rows);
    expect(groups).toHaveLength(3);
    expect(groups.map((g) => g.hour)).toEqual(["09:00", "10:00", "11:00"]);
  });

  it("preserves original array indices", () => {
    const rows = [ev(makeEvent("08:00")), ev(makeEvent("08:30")), ev(makeEvent("09:00"))];
    const groups = groupEventsByHour(rows);
    expect(groups[0].items[0].index).toBe(0);
    expect(groups[0].items[1].index).toBe(1);
    expect(groups[1].items[0].index).toBe(2);
  });
});

describe("groupEventsByHour gap tracking", () => {
  it("reports null gap for the first row and elapsed minutes after", () => {
    const rows = [
      ev(makeSourced("git", "a", "x", null, 36000)),
      ev(makeSourced("git", "b", "y", null, 36000 + 1800)),
    ];
    const items = groupEventsByHour(rows).flatMap((g) => g.items);
    expect(items[0].gapMinutes).toBeNull();
    expect(items[1].gapMinutes).toBe(30);
  });

  it("measures the gap from a calendar event's end, not its start", () => {
    const rows = [
      ev(makeSourced("calendar", "c", "Meeting", "1h 30m", 0)),
      ev(makeSourced("git", "g", "commit", null, 5400 + 600)),
    ];
    const items = groupEventsByHour(rows).flatMap((g) => g.items);
    // 6000s start − 5400s (0 + 90m) end = 10 min, not 100.
    expect(items[1].gapMinutes).toBe(10);
  });
});

// ---------------------------------------------------------------------------
// parseDurationToMinutes / formatGapLabel
// ---------------------------------------------------------------------------

describe("parseDurationToMinutes", () => {
  it("parses the calendar detail shapes", () => {
    expect(parseDurationToMinutes("20m")).toBe(20);
    expect(parseDurationToMinutes("1h")).toBe(60);
    expect(parseDurationToMinutes("1h 30m")).toBe(90);
    expect(parseDurationToMinutes("24h")).toBe(1440);
  });

  it("returns null for empty or non-duration text", () => {
    expect(parseDurationToMinutes(null)).toBeNull();
    expect(parseDurationToMinutes("")).toBeNull();
    expect(parseDurationToMinutes("soon")).toBeNull();
  });
});

describe("formatGapLabel", () => {
  it("formats minutes compactly", () => {
    expect(formatGapLabel(18)).toBe("18m");
    expect(formatGapLabel(60)).toBe("1h");
    expect(formatGapLabel(125)).toBe("2h 5m");
  });
});

// ---------------------------------------------------------------------------
// groupCloseCommits
// ---------------------------------------------------------------------------

describe("groupCloseCommits", () => {
  it("returns single-event rows for non-git events", () => {
    const event: TimelineEvent = {
      id: "jira:ABC-1:done",
      time: "10:00",
      timestamp: 1700000000,
      source: "jira",
      title: "x",
      detail: null,
      url: null,
    };
    const rows = groupCloseCommits([event]);
    expect(rows).toEqual([{ kind: "event", event }]);
  });

  it("collapses commits within 5s in the same repo", () => {
    const events = [
      makeCommit("/repo/a", "aaaa", "10:00", 1700000000),
      makeCommit("/repo/a", "bbbb", "10:00", 1700000002),
      makeCommit("/repo/a", "cccc", "10:00", 1700000005),
    ];
    const rows = groupCloseCommits(events);
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("group");
    if (rows[0].kind === "group") expect(rows[0].events).toHaveLength(3);
  });

  it("splits a group when the gap exceeds 5s", () => {
    const events = [
      makeCommit("/repo/a", "aaaa", "10:00", 1700000000),
      makeCommit("/repo/a", "bbbb", "10:00", 1700000003),
      makeCommit("/repo/a", "cccc", "10:00", 1700000013),
    ];
    const rows = groupCloseCommits(events);
    expect(rows).toHaveLength(2);
    expect(rows[0].kind).toBe("group");
    expect(rows[1].kind).toBe("event");
  });

  it("does not merge commits from different repos even if close in time", () => {
    const events = [
      makeCommit("/repo/a", "aaaa", "10:00", 1700000000),
      makeCommit("/repo/b", "bbbb", "10:00", 1700000001),
    ];
    const rows = groupCloseCommits(events);
    expect(rows).toHaveLength(2);
    expect(rows.every((r) => r.kind === "event")).toBe(true);
  });

  it("chains via transitive proximity", () => {
    // 0, 4, 8, 12 → each within 5s of the previous → one group.
    const events = [
      makeCommit("/repo/a", "aaaa", "10:00", 1700000000),
      makeCommit("/repo/a", "bbbb", "10:00", 1700000004),
      makeCommit("/repo/a", "cccc", "10:00", 1700000008),
      makeCommit("/repo/a", "dddd", "10:00", 1700000012),
    ];
    const rows = groupCloseCommits(events);
    expect(rows).toHaveLength(1);
    if (rows[0].kind === "group") expect(rows[0].events).toHaveLength(4);
  });

  it("flushes the current group when an unrelated event interrupts", () => {
    const events: TimelineEvent[] = [
      makeCommit("/repo/a", "aaaa", "10:00", 1700000000),
      makeCommit("/repo/a", "bbbb", "10:00", 1700000002),
      {
        id: "calendar:foo",
        time: "10:00",
        timestamp: 1700000003,
        source: "calendar",
        title: "meeting",
        detail: null,
        url: null,
      },
      makeCommit("/repo/a", "cccc", "10:00", 1700000004),
    ];
    const rows = groupCloseCommits(events);
    expect(rows).toHaveLength(3);
    expect(rows[0].kind).toBe("group");
    expect(rows[1].kind).toBe("event");
    expect(rows[2].kind).toBe("event");
  });
});

// ---------------------------------------------------------------------------
// extractTicketId
// ---------------------------------------------------------------------------

describe("extractTicketId", () => {
  it("reads the key from a jira event id", () => {
    const ev = makeSourced("jira", "jira:DDF-339:2026-06-23:Edited", "Edited DDF-339");
    expect(extractTicketId(ev)).toBe("DDF-339");
  });

  it("finds an allowlisted prefix in a git commit subject", () => {
    const ev = makeSourced("git", "git:/r:abc", "Require branch for eventseries. DDF-312");
    expect(extractTicketId(ev)).toBe("DDF-312");
  });

  it("matches branch-style ids with a trailing underscore", () => {
    const ev = makeSourced("github", "github:1", "Pull request #995 merged: branch DDF-339_location-fix");
    expect(extractTicketId(ev)).toBe("DDF-339");
  });

  it("ignores tokens whose prefix is not allowlisted", () => {
    expect(extractTicketId(makeSourced("git", "git:/r:a", "Bump to RFC-3339 and UTF-8"))).toBeNull();
    expect(extractTicketId(makeSourced("github", "github:2", "Closed PR-123"))).toBeNull();
  });

  it("falls back to detail when the title has no ticket", () => {
    const ev = makeSourced("git", "git:/r:a", "Tweak fields", "dpl-web — PLS-178 follow-up");
    expect(extractTicketId(ev)).toBe("PLS-178");
  });

  it("returns null when nothing matches", () => {
    expect(extractTicketId(makeSourced("git", "git:/r:a", "Update dependencies"))).toBeNull();
  });

  it("accepts extra prefixes discovered at the call site", () => {
    const ev = makeSourced("git", "git:/r:a", "Fix ABC-7 regression");
    expect(extractTicketId(ev)).toBeNull();
    expect(extractTicketId(ev, new Set(["ABC"]))).toBe("ABC-7");
  });
});

// ---------------------------------------------------------------------------
// groupByTicket
// ---------------------------------------------------------------------------

describe("groupByTicket", () => {
  it("buckets events sharing a ticket across sources", () => {
    const events = [
      makeSourced("git", "git:/r:a", "Merge #983 DDF-312", null, 30),
      makeSourced("jira", "jira:DDF-312:2026-06-23:Edited", "Edited DDF-312", null, 10),
      makeSourced("github", "github:9", "Pull request merged: DDF-312_ux", null, 20),
    ];
    const { ticketGroups, ungrouped } = groupByTicket(events);
    expect(ungrouped).toHaveLength(0);
    expect(ticketGroups).toHaveLength(1);
    expect(ticketGroups[0].id).toBe("DDF-312");
    expect(ticketGroups[0].events).toHaveLength(3);
    expect(ticketGroups[0].firstTs).toBe(10);
    expect(ticketGroups[0].lastTs).toBe(30);
  });

  it("auto-extends prefixes from jira keys so non-default projects group", () => {
    const events = [
      makeSourced("jira", "jira:ABC-5:2026-06-23:Edited", "Edited ABC-5", null, 10),
      makeSourced("git", "git:/r:a", "Fix the thing. ABC-5", null, 20),
    ];
    const { ticketGroups } = groupByTicket(events);
    expect(ticketGroups).toHaveLength(1);
    expect(ticketGroups[0].id).toBe("ABC-5");
  });

  it("leaves ticket-less events ungrouped", () => {
    const events = [
      makeSourced("git", "git:/r:a", "Update dependencies", null, 10),
      makeSourced("jira", "jira:DDF-1:2026-06-23:Edited", "Edited DDF-1", null, 20),
    ];
    const { ticketGroups, ungrouped } = groupByTicket(events);
    expect(ticketGroups).toHaveLength(1);
    expect(ungrouped).toHaveLength(1);
    expect(ungrouped[0].title).toBe("Update dependencies");
  });

  it("sorts groups by first timestamp", () => {
    const events = [
      makeSourced("jira", "jira:DDF-9:2026-06-23:Edited", "Edited DDF-9", null, 100),
      makeSourced("jira", "jira:PLS-2:2026-06-23:Edited", "Edited PLS-2", null, 50),
    ];
    const { ticketGroups } = groupByTicket(events);
    expect(ticketGroups.map((g) => g.id)).toEqual(["PLS-2", "DDF-9"]);
  });
});

// ---------------------------------------------------------------------------
// groupByTask
// ---------------------------------------------------------------------------

describe("groupByTask", () => {
  it("makes a ticket card from cross-source events", () => {
    const rows = groupByTask([
      makeSourced("git", "git:/r:a", "Require branch. DDF-312", null, 20),
      makeSourced("jira", "jira:DDF-312:2026-06-23:Edited", "Edited DDF-312", null, 10),
    ]);
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("group");
    if (rows[0].kind === "group") {
      expect(rows[0].keyType).toBe("ticket");
      expect(rows[0].label).toBe("DDF-312");
      expect(rows[0].events).toHaveLength(2);
    }
  });

  it("clusters ticket-less git commits by repo", () => {
    const rows = groupByTask([
      makeSourced("git", "git:/repo/recall:a", "Bump deps", "recall — aaaaaaa", 10),
      makeSourced("git", "git:/repo/recall:b", "Bump more deps", "recall — bbbbbbb", 20),
    ]);
    expect(rows).toHaveLength(1);
    if (rows[0].kind === "group") {
      expect(rows[0].keyType).toBe("repo");
      expect(rows[0].label).toBe("recall");
    }
  });

  it("clusters ticket-less zulip messages by stream", () => {
    const rows = groupByTask([
      makeSourced("zulip", "zulip:stream:lounge:2026-06-23", "Sent 1 message in #lounge", null, 10),
      makeSourced("zulip", "zulip:stream:lounge:2026-06-23", "Sent 2 messages in #lounge", null, 20),
    ]);
    expect(rows).toHaveLength(1);
    if (rows[0].kind === "group") {
      expect(rows[0].keyType).toBe("stream");
      expect(rows[0].label).toBe("#lounge");
    }
  });

  it("demotes single-event groups to standalone rows, ordered by start time", () => {
    const rows = groupByTask([
      makeSourced("jira", "jira:DDF-9:2026-06-23:Edited", "Edited DDF-9", null, 50),
      makeSourced("calendar", "calendar:x", "Standup", "20m", 10),
    ]);
    expect(rows.every((r) => r.kind === "event")).toBe(true);
    expect(rows.map((r) => (r.kind === "event" ? r.event.title : ""))).toEqual([
      "Standup",
      "Edited DDF-9",
    ]);
  });
});

// ---------------------------------------------------------------------------
// applyOptimisticToggle
// ---------------------------------------------------------------------------

describe("applyOptimisticToggle", () => {
  it("adds id when not present, reports wasAdded=true", () => {
    const [set, wasAdded] = applyOptimisticToggle(new Set(), "abc");
    expect(set.has("abc")).toBe(true);
    expect(wasAdded).toBe(true);
  });

  it("removes id when present, reports wasAdded=false", () => {
    const [set, wasAdded] = applyOptimisticToggle(new Set(["abc"]), "abc");
    expect(set.has("abc")).toBe(false);
    expect(wasAdded).toBe(false);
  });

  it("does not mutate the original set", () => {
    const original = new Set(["a", "b"]);
    applyOptimisticToggle(original, "a");
    expect(original.has("a")).toBe(true);
    expect(original.size).toBe(2);
  });

  it("leaves other ids untouched", () => {
    const [set] = applyOptimisticToggle(new Set(["x", "y"]), "x");
    expect(set.has("y")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// rollbackOptimisticToggle
// ---------------------------------------------------------------------------

describe("rollbackOptimisticToggle", () => {
  it("removes id that was added (wasAdded=true)", () => {
    const set = rollbackOptimisticToggle(new Set(["abc"]), "abc", true);
    expect(set.has("abc")).toBe(false);
  });

  it("adds back id that was removed (wasAdded=false)", () => {
    const set = rollbackOptimisticToggle(new Set(), "abc", false);
    expect(set.has("abc")).toBe(true);
  });

  it("does not mutate the original set", () => {
    const original = new Set(["abc"]);
    rollbackOptimisticToggle(original, "abc", true);
    expect(original.has("abc")).toBe(true);
  });

  it("round-trips correctly: apply then rollback restores original state", () => {
    const before = new Set(["x"]);
    const [after, wasAdded] = applyOptimisticToggle(before, "x");
    const restored = rollbackOptimisticToggle(after, "x", wasAdded);
    expect(restored).toEqual(before);
  });

  it("round-trips correctly for an add that is rolled back", () => {
    const before = new Set<string>();
    const [after, wasAdded] = applyOptimisticToggle(before, "new");
    const restored = rollbackOptimisticToggle(after, "new", wasAdded);
    expect(restored).toEqual(before);
  });
});

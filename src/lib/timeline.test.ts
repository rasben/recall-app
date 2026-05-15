import { describe, expect, it } from "vitest";
import {
  applyOptimisticToggle,
  groupCloseCommits,
  groupEventsByHour,
  rollbackOptimisticToggle,
} from "./timeline";
import type { TimelineEvent, TimelineRow } from "./timeline";

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

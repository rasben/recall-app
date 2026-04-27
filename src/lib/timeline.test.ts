import { describe, expect, it } from "vitest";
import {
  applyOptimisticToggle,
  groupEventsByHour,
  rollbackOptimisticToggle,
} from "./timeline";
import type { TimelineEvent } from "./timeline";

function makeEvent(time: string, id = time): TimelineEvent {
  return { id, time, source: "git", title: "test", detail: null, url: null };
}

// ---------------------------------------------------------------------------
// groupEventsByHour
// ---------------------------------------------------------------------------

describe("groupEventsByHour", () => {
  it("returns empty array for no events", () => {
    expect(groupEventsByHour([])).toEqual([]);
  });

  it("groups events in the same hour into one bucket", () => {
    const events = [makeEvent("10:05"), makeEvent("10:30"), makeEvent("10:59")];
    const groups = groupEventsByHour(events);
    expect(groups).toHaveLength(1);
    expect(groups[0].hour).toBe("10:00");
    expect(groups[0].items).toHaveLength(3);
  });

  it("creates a separate group for each distinct hour", () => {
    const events = [makeEvent("09:00"), makeEvent("10:00"), makeEvent("11:00")];
    const groups = groupEventsByHour(events);
    expect(groups).toHaveLength(3);
    expect(groups.map((g) => g.hour)).toEqual(["09:00", "10:00", "11:00"]);
  });

  it("preserves original array indices", () => {
    const events = [makeEvent("08:00"), makeEvent("08:30"), makeEvent("09:00")];
    const groups = groupEventsByHour(events);
    expect(groups[0].items[0].index).toBe(0);
    expect(groups[0].items[1].index).toBe(1);
    expect(groups[1].items[0].index).toBe(2);
  });

  it("does not merge non-consecutive same-hour events", () => {
    // 14:00, 15:00, 14:00 — third event is a new group (not merged with first)
    const events = [makeEvent("14:00", "a"), makeEvent("15:00", "b"), makeEvent("14:00", "c")];
    const groups = groupEventsByHour(events);
    expect(groups).toHaveLength(3);
    expect(groups[2].items[0].event.id).toBe("c");
  });

  it("attaches the correct event object to each item", () => {
    const ev = makeEvent("07:15", "unique-id");
    const groups = groupEventsByHour([ev]);
    expect(groups[0].items[0].event).toBe(ev);
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

import { describe, expect, it } from "vitest";
import type { TimelineEvent, TimelineEventSource } from "../bindings";
import { countBySource, lastWorkdayIso } from "./setup-guide";

function ev(source: TimelineEventSource): TimelineEvent {
  return { id: Math.random().toString(), time: "09:00", timestamp: 0, source, title: "x", detail: null, url: null };
}

describe("lastWorkdayIso", () => {
  it("is yesterday on Tuesday to Friday", () => {
    expect(lastWorkdayIso("2026-10-27")).toBe("2026-10-26"); // Tue → Mon
    expect(lastWorkdayIso("2026-10-30")).toBe("2026-10-29"); // Fri → Thu
  });

  it("skips back over the weekend", () => {
    expect(lastWorkdayIso("2026-10-26")).toBe("2026-10-23"); // Mon → Fri
    expect(lastWorkdayIso("2026-10-25")).toBe("2026-10-23"); // Sun → Fri
    expect(lastWorkdayIso("2026-10-24")).toBe("2026-10-23"); // Sat → Fri
  });
});

describe("countBySource", () => {
  it("counts per source, most first", () => {
    expect(countBySource([ev("jira"), ev("git"), ev("git"), ev("calendar"), ev("git")])).toEqual([
      ["git", 3],
      ["jira", 1],
      ["calendar", 1],
    ]);
  });

  it("is empty for no events", () => {
    expect(countBySource([])).toEqual([]);
  });
});

import { describe, expect, it } from "vitest";
import {
  addDaysIso,
  formatDayHeading,
  formatDayHeadingParts,
  SOURCE_LABELS,
  todayIso,
} from "./timeline";
import type { TimelineEventSource } from "./timeline";

describe("addDaysIso", () => {
  it("adds and subtracts calendar days", () => {
    expect(addDaysIso("2024-03-05", 1)).toBe("2024-03-06");
    expect(addDaysIso("2024-03-05", -5)).toBe("2024-02-29");
    expect(addDaysIso("2024-03-05", 0)).toBe("2024-03-05");
  });

  it("crosses month and year boundaries", () => {
    expect(addDaysIso("2024-01-31", 1)).toBe("2024-02-01");
    expect(addDaysIso("2024-12-31", 1)).toBe("2025-01-01");
    expect(addDaysIso("2025-01-01", -1)).toBe("2024-12-31");
  });

  it("handles leap days", () => {
    expect(addDaysIso("2024-02-28", 1)).toBe("2024-02-29");
    expect(addDaysIso("2023-02-28", 1)).toBe("2023-03-01");
  });

  it("stays on the calendar across DST changes", () => {
    // European and US DST transitions in 2024; a UTC-based implementation
    // would land on the wrong day in some zones.
    expect(addDaysIso("2024-03-30", 1)).toBe("2024-03-31");
    expect(addDaysIso("2024-03-31", 1)).toBe("2024-04-01");
    expect(addDaysIso("2024-03-09", 1)).toBe("2024-03-10");
    expect(addDaysIso("2024-10-26", 1)).toBe("2024-10-27");
    expect(addDaysIso("2024-11-03", -1)).toBe("2024-11-02");
  });

  it("zero-pads month and day", () => {
    expect(addDaysIso("2024-09-30", 1)).toBe("2024-10-01");
    expect(addDaysIso("2024-10-01", -1)).toBe("2024-09-30");
  });
});

describe("todayIso", () => {
  it("is the local calendar date as YYYY-MM-DD", () => {
    const now = new Date();
    const expected = [
      now.getFullYear(),
      String(now.getMonth() + 1).padStart(2, "0"),
      String(now.getDate()).padStart(2, "0"),
    ].join("-");
    expect(todayIso()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(todayIso()).toBe(expected);
  });
});

describe("formatDayHeading", () => {
  it("formats in US English by default", () => {
    expect(formatDayHeadingParts("2024-03-05")).toEqual({
      weekday: "Tuesday",
      monthDay: "March 5",
    });
    expect(formatDayHeading("2024-03-05")).toBe("Tuesday, March 5");
  });

  it("formats in Danish", () => {
    expect(formatDayHeading("2024-03-05", "da-DK")).toBe("tirsdag, 5. marts");
  });

  it("does not shift the date across time zones", () => {
    // Midnight boundaries are where a UTC parse would drift a day.
    expect(formatDayHeadingParts("2024-01-01").weekday).toBe("Monday");
    expect(formatDayHeadingParts("2024-12-31").weekday).toBe("Tuesday");
  });
});

describe("SOURCE_LABELS", () => {
  it("has a non-empty label for every timeline source", () => {
    const sources: TimelineEventSource[] = [
      "git",
      "github",
      "calendar",
      "gmail",
      "drive",
      "jira",
      "zulip",
    ];
    expect(Object.keys(SOURCE_LABELS).sort()).toEqual([...sources].sort());
    for (const s of sources) {
      expect(SOURCE_LABELS[s].trim()).not.toBe("");
    }
  });
});

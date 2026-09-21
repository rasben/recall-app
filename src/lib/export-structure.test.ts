import { describe, expect, it } from "vitest";
import { toJson, toMarkdown } from "./export";
import type { ExportDay, TimelineEvent } from "../bindings";

function ev(partial: Partial<TimelineEvent> & { time: string }): TimelineEvent {
  return {
    id: partial.id ?? `id-${partial.time}`,
    time: partial.time,
    timestamp: partial.timestamp ?? 0,
    source: partial.source ?? "git",
    title: partial.title ?? "Did a thing",
    detail: partial.detail ?? null,
    url: partial.url ?? null,
  };
}

const DAYS: ExportDay[] = [
  { date: "2024-03-05", events: [] },
  {
    date: "2024-03-06",
    events: [
      ev({
        time: "09:15",
        source: "github",
        title: "Opened PR",
        url: "https://github.com/x/y/pull/1",
      }),
      ev({
        time: "11:00",
        source: "calendar",
        title: "Standup",
        detail: "15m",
      }),
    ],
  },
];

describe("toMarkdown structure", () => {
  const md = toMarkdown(DAYS, "2024-03-05", "2024-03-06");

  it("puts the range in the title", () => {
    expect(
      md.startsWith("# Activity timeline (2024-03-05 → 2024-03-06)\n"),
    ).toBe(true);
  });

  it("renders every day in order with an ISO date and a readable heading", () => {
    const headings = md.split("\n").filter((l) => l.startsWith("## "));
    expect(headings).toEqual([
      "## 2024-03-05 — Tuesday, March 5",
      "## 2024-03-06 — Wednesday, March 6",
    ]);
  });

  it("uses display labels for sources and appends the url", () => {
    expect(md).toContain(
      "- **09:15** `GitHub` — Opened PR (https://github.com/x/y/pull/1)",
    );
    expect(md).toContain("- **11:00** `Calendar` — Standup — 15m");
  });

  it("marks the empty day and ends with exactly one newline", () => {
    expect(md).toContain(
      "## 2024-03-05 — Tuesday, March 5\n\n_No tracked activity._\n",
    );
    expect(md.endsWith("\n")).toBe(true);
    expect(md.endsWith("\n\n")).toBe(false);
  });

  it("localizes the day heading", () => {
    const da = toMarkdown(DAYS, "2024-03-05", "2024-03-06", "da-DK");
    expect(da).toContain("## 2024-03-05 — tirsdag, 5. marts");
  });
});

describe("toJson structure", () => {
  const parsed = JSON.parse(toJson(DAYS, "2024-03-05", "2024-03-06"));

  it("includes the range and every day in order", () => {
    expect(parsed.range).toEqual({ start: "2024-03-05", end: "2024-03-06" });
    expect(parsed.days.map((d: { date: string }) => d.date)).toEqual([
      "2024-03-05",
      "2024-03-06",
    ]);
  });

  it("keeps empty days as empty arrays", () => {
    expect(parsed.days[0].events).toEqual([]);
  });

  it("emits only the reader-facing fields, in a stable order", () => {
    expect(Object.keys(parsed.days[1].events[0])).toEqual([
      "time",
      "source",
      "title",
      "url",
    ]);
    expect(Object.keys(parsed.days[1].events[1])).toEqual([
      "time",
      "source",
      "title",
      "detail",
    ]);
    expect(parsed.days[1].events[0].source).toBe("github");
  });

  it("is pretty-printed with two-space indentation", () => {
    expect(toJson(DAYS, "2024-03-05", "2024-03-06")).toContain(
      '\n  "range": {\n    "start"',
    );
  });
});

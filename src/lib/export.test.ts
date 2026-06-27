import { describe, expect, it } from "vitest";
import { rangeLabel, toJson, toMarkdown } from "./export";
import type { ExportDay, TimelineEvent } from "../bindings";

function ev(partial: Partial<TimelineEvent> & { time: string }): TimelineEvent {
  return {
    id: partial.id ?? partial.time,
    time: partial.time,
    timestamp: partial.timestamp ?? 0,
    source: partial.source ?? "git",
    title: partial.title ?? "did a thing",
    detail: partial.detail ?? null,
    url: partial.url ?? null,
  };
}

describe("rangeLabel", () => {
  it("shows a single date when start equals end", () => {
    expect(rangeLabel("2026-06-23", "2026-06-23")).toBe("2026-06-23");
  });

  it("shows an arrow range when start differs from end", () => {
    expect(rangeLabel("2026-06-20", "2026-06-26")).toBe("2026-06-20 → 2026-06-26");
  });
});

describe("toMarkdown", () => {
  const days: ExportDay[] = [
    {
      date: "2026-06-23",
      events: [
        ev({ time: "09:15", source: "github", title: "Opened PR #12", url: "https://example.com/pr/12" }),
        ev({ time: "10:02", source: "git", title: "fix bug", detail: "line one\n   line two" }),
      ],
    },
    { date: "2026-06-24", events: [] },
  ];

  it("renders a heading and one bullet per event", () => {
    const md = toMarkdown(days, "2026-06-23", "2026-06-24", "en-US");
    expect(md).toContain("## 2026-06-23");
    expect(md).toContain("- **09:15** `GitHub` — Opened PR #12 (https://example.com/pr/12)");
  });

  it("collapses multi-line detail onto a single bullet", () => {
    const md = toMarkdown(days, "2026-06-23", "2026-06-24", "en-US");
    expect(md).toContain("- **10:02** `Git` — fix bug — line one line two");
    expect(md).not.toContain("line one\n");
  });

  it("notes days with no activity", () => {
    const md = toMarkdown(days, "2026-06-23", "2026-06-24", "en-US");
    expect(md).toContain("_No tracked activity._");
  });

  it("prepends the given prompt before the heading", () => {
    const prompt = "Summarize these days.";
    const md = toMarkdown(days, "2026-06-23", "2026-06-24", "en-US", prompt);
    expect(md.startsWith(prompt)).toBe(true);
    expect(md.indexOf(prompt)).toBeLessThan(md.indexOf("# Activity timeline"));
  });

  it("omits the prompt when none is given", () => {
    const md = toMarkdown(days, "2026-06-23", "2026-06-24", "en-US");
    expect(md.startsWith("# Activity timeline")).toBe(true);
  });

  it("uses the provided structural labels (localization)", () => {
    const md = toMarkdown(days, "2026-06-23", "2026-06-24", "da-DK", null, {
      title: "Aktivitetstidslinje",
      intro: "Eksporteret fra Recall.",
      noActivity: "Ingen registreret aktivitet.",
    });
    expect(md).toContain("# Aktivitetstidslinje (2026-06-23 → 2026-06-24)");
    expect(md).toContain("_Eksporteret fra Recall._");
    expect(md).toContain("_Ingen registreret aktivitet._");
    expect(md).not.toContain("Activity timeline");
  });
});

describe("toJson", () => {
  it("drops id/timestamp and omits empty detail/url", () => {
    const days: ExportDay[] = [
      { date: "2026-06-23", events: [ev({ time: "09:15", source: "git", title: "commit" })] },
    ];
    const parsed = JSON.parse(toJson(days, "2026-06-23", "2026-06-23"));
    expect(parsed.range).toEqual({ start: "2026-06-23", end: "2026-06-23" });
    expect(parsed.days[0].events[0]).toEqual({ time: "09:15", source: "git", title: "commit" });
  });

  it("keeps detail and url when present", () => {
    const days: ExportDay[] = [
      { date: "2026-06-23", events: [ev({ time: "09:15", title: "t", detail: "d", url: "https://x.test" })] },
    ];
    const parsed = JSON.parse(toJson(days, "2026-06-23", "2026-06-23"));
    expect(parsed.days[0].events[0].detail).toBe("d");
    expect(parsed.days[0].events[0].url).toBe("https://x.test");
  });
});

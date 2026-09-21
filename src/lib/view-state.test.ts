import { describe, expect, it } from "vitest";
import { parseGroupMode, parseHiddenSources, viewStatePatch } from "./view-state";

describe("parseGroupMode", () => {
  it("accepts the two known modes", () => {
    expect(parseGroupMode("time")).toBe("time");
    expect(parseGroupMode("task")).toBe("task");
  });

  it("falls back to time for anything else", () => {
    expect(parseGroupMode("hour")).toBe("time");
    expect(parseGroupMode("")).toBe("time");
    expect(parseGroupMode(null)).toBe("time");
    expect(parseGroupMode(undefined)).toBe("time");
  });
});

describe("parseHiddenSources", () => {
  it("keeps known sources and drops unknown ones", () => {
    const set = parseHiddenSources(["zulip", "slack", "git", ""]);
    expect([...set].sort()).toEqual(["git", "zulip"]);
  });

  it("dedupes and tolerates a missing list", () => {
    expect([...parseHiddenSources(["jira", "jira"])]).toEqual(["jira"]);
    expect(parseHiddenSources(null).size).toBe(0);
    expect(parseHiddenSources(undefined).size).toBe(0);
  });
});

describe("viewStatePatch", () => {
  it("produces a stable, sorted shape", () => {
    const a = viewStatePatch("task", new Set(["zulip", "git"]));
    const b = viewStatePatch("task", new Set(["git", "zulip"]));
    expect(JSON.stringify(a)).toBe(JSON.stringify(b));
    expect(a).toEqual({ group_mode: "task", hidden_sources: ["git", "zulip"] });
  });
});

import { describe, expect, it } from "vitest";
import { groupByTask, groupCloseCommits, isDependabotCommit } from "./timeline";
import type { TimelineEvent, TimelineEventSource } from "./timeline";

function make(
  source: TimelineEventSource,
  id: string,
  timestamp: number,
  title: string,
  detail: string | null = null,
): TimelineEvent {
  return { id, time: "10:00", timestamp, source, title, detail, url: null };
}

function commit(
  repo: string,
  hash: string,
  timestamp: number,
  title = "Fix thing",
): TimelineEvent {
  return make(
    "git",
    `git:${repo}:${hash}`,
    timestamp,
    title,
    `${repo.split("/").pop()} — ${hash.slice(0, 7)}`,
  );
}

describe("isDependabotCommit", () => {
  it("matches git commits mentioning dependabot, case-insensitively", () => {
    expect(
      isDependabotCommit(commit("/r", "a", 1, "Bump lodash (dependabot)")),
    ).toBe(true);
    expect(
      isDependabotCommit(
        commit("/r", "a", 1, "Merge pull request from Dependabot/npm"),
      ),
    ).toBe(true);
  });

  it("ignores non-git events and ordinary commits", () => {
    expect(
      isDependabotCommit(make("github", "github:1", 1, "dependabot bump")),
    ).toBe(false);
    expect(isDependabotCommit(commit("/r", "a", 1, "Refactor parser"))).toBe(
      false,
    );
  });
});

describe("groupCloseCommits group key", () => {
  it("encodes first id, last id and size", () => {
    const rows = groupCloseCommits([
      commit("/repo/a", "aaaaaaaa", 100),
      commit("/repo/a", "bbbbbbbb", 102),
      commit("/repo/a", "cccccccc", 104),
    ]);
    expect(rows).toHaveLength(1);
    const row = rows[0];
    expect(row.kind).toBe("group");
    if (row.kind === "group") {
      expect(row.key).toBe("git:/repo/a:aaaaaaaa..git:/repo/a:cccccccc+3");
      expect(row.events.map((e) => e.id)).toEqual([
        "git:/repo/a:aaaaaaaa",
        "git:/repo/a:bbbbbbbb",
        "git:/repo/a:cccccccc",
      ]);
    }
  });
});

describe("groupByTask secondary keys", () => {
  it("labels a repo card from the readable repo name in the commit detail", () => {
    const rows = groupByTask([
      commit("/Users/me/code/recall", "aaaaaaaa", 100),
      commit("/Users/me/code/recall", "bbbbbbbb", 5000),
    ]);
    expect(rows).toHaveLength(1);
    const row = rows[0];
    expect(row.kind).toBe("group");
    if (row.kind === "group") {
      expect(row.key).toBe("git:/Users/me/code/recall");
      expect(row.label).toBe("recall");
      expect(row.keyType).toBe("repo");
      expect(row.firstTs).toBe(100);
      expect(row.lastTs).toBe(5000);
    }
  });

  it("labels a stream card with a hash prefix", () => {
    const rows = groupByTask([
      make("zulip", "zulip:stream:general:2024-03-05", 100, "Said hi"),
      make("zulip", "zulip:stream:general:2024-03-05", 200, "Said bye"),
    ]);
    expect(rows).toHaveLength(1);
    const row = rows[0];
    if (row.kind === "group") {
      expect(row.key).toBe("stream:general");
      expect(row.label).toBe("#general");
      expect(row.keyType).toBe("stream");
    } else {
      throw new Error("expected a group row");
    }
  });

  it("keeps stream names that contain colons", () => {
    const rows = groupByTask([
      make("zulip", "zulip:stream:team: backend:2024-03-05", 100, "a"),
      make("zulip", "zulip:stream:team: backend:2024-03-05", 200, "b"),
    ]);
    expect(rows).toHaveLength(1);
    if (rows[0].kind === "group") expect(rows[0].label).toBe("#team: backend");
  });

  it("does not cluster zulip DMs, which carry no stream key", () => {
    const rows = groupByTask([
      make("zulip", "zulip:dm:alice:2024-03-05", 100, "dm 1"),
      make("zulip", "zulip:dm:alice:2024-03-05", 200, "dm 2"),
    ]);
    expect(rows.map((r) => r.kind)).toEqual(["event", "event"]);
  });

  it("orders ticket cards, repo cards and loose events by start time", () => {
    const rows = groupByTask([
      commit("/repo/a", "aaaaaaaa", 300),
      make(
        "jira",
        "jira:DDF-1:2024-03-05:Commented",
        200,
        "DDF-1 Do the thing",
      ),
      commit("/repo/a", "bbbbbbbb", 400),
      make("calendar", "calendar:x", 100, "Standup"),
      commit("/repo/b", "cccccccc", 250, "DDF-1 wire it up"),
    ]);
    expect(rows.map((r) => (r.kind === "group" ? r.key : r.event.id))).toEqual([
      "calendar:x",
      "ticket:DDF-1",
      "git:/repo/a",
    ]);
  });

  it("returns no rows for no events", () => {
    expect(groupByTask([])).toEqual([]);
  });
});

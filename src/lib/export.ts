import type { ExportDay, TimelineEvent } from "../bindings";
import { formatDayHeading, SOURCE_LABELS } from "./timeline";

function sourceLabel(source: TimelineEvent["source"]): string {
  return SOURCE_LABELS[source] ?? source;
}

/** Localizable structural strings for the Markdown export. */
export type MarkdownLabels = {
  title: string;
  intro: string;
  noActivity: string;
};

const DEFAULT_MARKDOWN_LABELS: MarkdownLabels = {
  title: "Activity timeline",
  intro:
    "Exported from Recall, a personal work-tracking app. Times are local. Each bullet is one tracked activity (commit, PR, calendar event, ticket update, message, …).",
  noActivity: "No tracked activity.",
};

/** `2026-06-23` for a single day, `2026-06-20 → 2026-06-26` for a range. */
export function rangeLabel(start: string, end: string): string {
  return start === end ? start : `${start} → ${end}`;
}

/** Collapse internal whitespace/newlines so a multi-line detail stays on one bullet. */
function inlineDetail(detail: string): string {
  return detail.replace(/\s+/g, " ").trim();
}

function eventMarkdown(ev: TimelineEvent): string {
  let line = `- **${ev.time}** \`${sourceLabel(ev.source)}\` — ${ev.title}`;
  if (ev.detail) {
    const detail = inlineDetail(ev.detail);
    if (detail) line += ` — ${detail}`;
  }
  if (ev.url) line += ` (${ev.url})`;
  return line;
}

/**
 * Render the export as Markdown: a heading per day with one bullet per
 * activity, in chronological order. Days with no tracked activity are kept
 * (with a note) so the reader sees the full coverage of the requested range.
 * When `prompt` is given it's prepended (before a `---` divider) so the copied
 * text doubles as a ready-to-run instruction. The caller supplies both `prompt`
 * and `labels` already translated to the active UI language; `labels` defaults
 * to English so callers that don't care about localization stay simple.
 */
export function toMarkdown(
  days: ExportDay[],
  start: string,
  end: string,
  locale = "en-US",
  prompt?: string | null,
  labels: MarkdownLabels = DEFAULT_MARKDOWN_LABELS,
): string {
  const lines: string[] = [];
  if (prompt) lines.push(prompt, "", "---", "");
  lines.push(`# ${labels.title} (${rangeLabel(start, end)})`, "", `_${labels.intro}_`, "");

  for (const day of days) {
    lines.push(`## ${day.date} — ${formatDayHeading(day.date, locale)}`, "");
    if (day.events.length === 0) {
      lines.push(`_${labels.noActivity}_`, "");
      continue;
    }
    for (const ev of day.events) lines.push(eventMarkdown(ev));
    lines.push("");
  }

  return lines.join("\n").trimEnd() + "\n";
}

/**
 * Render the export as pretty-printed JSON. The noisy UI-only fields (`id`,
 * `timestamp`) are dropped; `detail`/`url` are omitted when absent.
 */
export function toJson(days: ExportDay[], start: string, end: string): string {
  return JSON.stringify(
    {
      range: { start, end },
      days: days.map((day) => ({
        date: day.date,
        events: day.events.map((ev) => ({
          time: ev.time,
          source: ev.source,
          title: ev.title,
          ...(ev.detail ? { detail: ev.detail } : {}),
          ...(ev.url ? { url: ev.url } : {}),
        })),
      })),
    },
    null,
    2,
  );
}

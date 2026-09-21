export type DayNavAction = "prev" | "next" | "today";

/** The subset of Element the helpers need, so they can be unit-tested without a DOM. */
export interface KeyTargetLike {
  tagName?: string;
  isContentEditable?: boolean;
  closest?: (selector: string) => unknown;
}

/** Elements (or ancestors) that own the keyboard: form fields, and any open
 *  bits-ui popover/select/dialog content, which bits-ui marks with
 *  `data-state="open"`. The calendar popover uses arrow keys itself. */
export const KEY_OWNER_SELECTOR =
  'input, textarea, select, [contenteditable=""], [contenteditable="true"], [data-state="open"], [role="dialog"], [role="menu"], [role="listbox"]';

/** True when a key press on `target` belongs to a text field or an open popover. */
export function isTypingTarget(target: KeyTargetLike | null | undefined): boolean {
  if (!target) return false;
  const tag = (target.tagName ?? "").toUpperCase();
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (target.isContentEditable) return true;
  return Boolean(target.closest?.(KEY_OWNER_SELECTOR));
}

export interface DayNavKeyOptions {
  /** Alt/Ctrl/Meta held: leave the key to the OS or webview (e.g. Alt+← = back). */
  modifier?: boolean;
  /** Focus is in a text field or an open popover. */
  typing?: boolean;
  /** The selected day is today: there is no "next" day to go to. */
  atToday?: boolean;
}

/** Map a key name to a day-navigation action: ←/→ shift by one day, t/T jumps
 *  to today. Returns null when the key should be left alone. */
export function dayNavActionForKey(key: string, opts: DayNavKeyOptions = {}): DayNavAction | null {
  if (opts.modifier || opts.typing) return null;
  switch (key) {
    case "ArrowLeft":
      return "prev";
    case "ArrowRight":
      return opts.atToday ? null : "next";
    case "t":
    case "T":
      return opts.atToday ? null : "today";
    default:
      return null;
  }
}

/** Convenience wrapper for a real KeyboardEvent. */
export function dayNavActionForEvent(e: KeyboardEvent, atToday: boolean): DayNavAction | null {
  return dayNavActionForKey(e.key, {
    modifier: e.altKey || e.ctrlKey || e.metaKey,
    typing: isTypingTarget(e.target as KeyTargetLike | null),
    atToday,
  });
}

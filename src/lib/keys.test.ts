import { describe, expect, it } from "vitest";
import { dayNavActionForKey, isTypingTarget, KEY_OWNER_SELECTOR, type KeyTargetLike } from "./keys";

describe("dayNavActionForKey", () => {
  it("maps arrows and t to day navigation", () => {
    expect(dayNavActionForKey("ArrowLeft")).toBe("prev");
    expect(dayNavActionForKey("ArrowRight")).toBe("next");
    expect(dayNavActionForKey("t")).toBe("today");
    expect(dayNavActionForKey("T")).toBe("today");
  });

  it("ignores other keys", () => {
    expect(dayNavActionForKey("ArrowUp")).toBeNull();
    expect(dayNavActionForKey("Enter")).toBeNull();
    expect(dayNavActionForKey("a")).toBeNull();
    expect(dayNavActionForKey("")).toBeNull();
  });

  it("does nothing while a modifier is held", () => {
    expect(dayNavActionForKey("ArrowLeft", { modifier: true })).toBeNull();
    expect(dayNavActionForKey("t", { modifier: true })).toBeNull();
  });

  it("does nothing while typing or inside an open popover", () => {
    expect(dayNavActionForKey("ArrowRight", { typing: true })).toBeNull();
    expect(dayNavActionForKey("t", { typing: true })).toBeNull();
  });

  it("has no next day and no today jump when already on today", () => {
    expect(dayNavActionForKey("ArrowRight", { atToday: true })).toBeNull();
    expect(dayNavActionForKey("t", { atToday: true })).toBeNull();
    expect(dayNavActionForKey("ArrowLeft", { atToday: true })).toBe("prev");
  });
});

describe("isTypingTarget", () => {
  const plain = (tagName: string, closestHit = false): KeyTargetLike => ({
    tagName,
    isContentEditable: false,
    closest: () => (closestHit ? {} : null),
  });

  it("treats form fields as typing targets", () => {
    expect(isTypingTarget(plain("INPUT"))).toBe(true);
    expect(isTypingTarget(plain("textarea"))).toBe(true);
    expect(isTypingTarget(plain("SELECT"))).toBe(true);
  });

  it("treats contenteditable as a typing target", () => {
    expect(isTypingTarget({ tagName: "DIV", isContentEditable: true, closest: () => null })).toBe(true);
  });

  it("treats anything inside an open popover as a typing target", () => {
    let asked = "";
    const inPopover: KeyTargetLike = {
      tagName: "BUTTON",
      closest: (sel) => {
        asked = sel;
        return {};
      },
    };
    expect(isTypingTarget(inPopover)).toBe(true);
    expect(asked).toBe(KEY_OWNER_SELECTOR);
  });

  it("lets plain page elements through", () => {
    expect(isTypingTarget(plain("BODY"))).toBe(false);
    expect(isTypingTarget(plain("BUTTON"))).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
    expect(isTypingTarget(undefined)).toBe(false);
  });
});

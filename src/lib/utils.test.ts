import { describe, expect, it } from "vitest";
import { cn } from "./utils";

describe("cn", () => {
  it("joins class names and drops falsy values", () => {
    expect(cn("a", false, undefined, null, "c")).toBe("a c");
  });

  it("lets the last conflicting Tailwind utility win", () => {
    expect(cn("p-2 text-sm", "p-4")).toBe("text-sm p-4");
  });

  it("accepts arrays and conditional objects", () => {
    expect(cn(["a", { b: true, c: false }])).toBe("a b");
  });
});

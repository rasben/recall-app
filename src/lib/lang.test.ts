import { describe, expect, it } from "vitest";
import { langFromNavigator } from "./lang";

describe("langFromNavigator", () => {
  it("maps Danish tags to da", () => {
    expect(langFromNavigator("da")).toBe("da");
    expect(langFromNavigator("da-DK")).toBe("da");
    expect(langFromNavigator("DA-dk")).toBe("da");
    expect(langFromNavigator("da_DK")).toBe("da");
    expect(langFromNavigator("da-GL")).toBe("da");
  });

  it("maps everything else to en", () => {
    expect(langFromNavigator("en-US")).toBe("en");
    expect(langFromNavigator("en-GB")).toBe("en");
    expect(langFromNavigator("de-DE")).toBe("en");
    expect(langFromNavigator("sv-SE")).toBe("en");
    expect(langFromNavigator("nb-NO")).toBe("en");
  });

  it("does not confuse Danish with tags that merely start with da", () => {
    expect(langFromNavigator("dak")).toBe("en");
    expect(langFromNavigator("dav-KE")).toBe("en");
  });

  it("falls back to en when the tag is missing or empty", () => {
    expect(langFromNavigator(undefined)).toBe("en");
    expect(langFromNavigator(null)).toBe("en");
    expect(langFromNavigator("")).toBe("en");
    expect(langFromNavigator("   ")).toBe("en");
  });
});

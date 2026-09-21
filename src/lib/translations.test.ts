import { describe, expect, it } from "vitest";
import { translations, type Lang } from "./translations";

/** Every language table, typed loosely so we can iterate keys as plain strings. */
const TABLES = translations as Record<Lang, Record<string, string>>;
const LANGS = Object.keys(TABLES) as Lang[];
const EN_KEYS = Object.keys(TABLES.en).sort();

/** `{name}` placeholders in a string, sorted, so tables can be compared. */
function placeholders(s: string): string[] {
  return [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
}

describe("translations", () => {
  it("ships English and Danish", () => {
    expect([...LANGS].sort()).toEqual(["da", "en"]);
  });

  it("has a non-trivial English table", () => {
    expect(EN_KEYS.length).toBeGreaterThan(50);
  });

  it.each(LANGS)("%s has exactly the English key set", (lang) => {
    const keys = Object.keys(TABLES[lang]).sort();
    const missing = EN_KEYS.filter((k) => !keys.includes(k));
    const extra = keys.filter((k) => !EN_KEYS.includes(k));
    expect(missing, `keys missing from "${lang}"`).toEqual([]);
    expect(extra, `keys only present in "${lang}"`).toEqual([]);
  });

  it.each(LANGS)("%s has no empty strings", (lang) => {
    const empty = Object.entries(TABLES[lang])
      .filter(([, v]) => v.trim() === "")
      .map(([k]) => k);
    expect(empty).toEqual([]);
  });

  it.each(LANGS)("%s uses the same {placeholders} as English", (lang) => {
    const mismatched = EN_KEYS.filter(
      (k) =>
        placeholders(TABLES.en[k]).join(",") !==
        placeholders(TABLES[lang][k] ?? "").join(","),
    );
    expect(
      mismatched,
      `placeholder sets differ from English in "${lang}"`,
    ).toEqual([]);
  });

  it("names keys as lowercase dotted paths", () => {
    const bad = EN_KEYS.filter((k) => !/^[a-z0-9_]+(\.[a-z0-9_]+)+$/.test(k));
    expect(bad).toEqual([]);
  });

  it("names the languages in each language's own table", () => {
    expect(TABLES.en["lang.en"]).toBe("English");
    expect(TABLES.en["lang.da"]).toBe("Danish");
    expect(TABLES.da["lang.en"]).not.toBe("");
    expect(TABLES.da["lang.da"]).not.toBe("");
  });
});

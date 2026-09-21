import { afterEach, describe, expect, it } from "vitest";
import { i18n, langLocale, setLang, t } from "./i18n.svelte";
import { translations } from "./translations";

// The module keeps global state; leave it in a known language after each test.
afterEach(() => {
  setLang("en");
});

describe("setLang / langLocale", () => {
  it("switches the active language", () => {
    setLang("da");
    expect(i18n.lang).toBe("da");
    setLang("en");
    expect(i18n.lang).toBe("en");
  });

  it("maps the language to a BCP 47 locale", () => {
    setLang("da");
    expect(langLocale()).toBe("da-DK");
    setLang("en");
    expect(langLocale()).toBe("en-US");
  });
});

describe("t", () => {
  it("returns the string for the active language", () => {
    setLang("en");
    expect(t("lang.da")).toBe(translations.en["lang.da"]);
    setLang("da");
    expect(t("lang.da")).toBe(translations.da["lang.da"]);
  });

  it("substitutes {placeholders} from params", () => {
    setLang("en");
    const title = translations.en["page.new_version.title"];
    expect(title).toContain("{version}");
    expect(t("page.new_version.title", { version: "1.2.3" })).toBe(
      title.replaceAll("{version}", "1.2.3"),
    );
    const hint = translations.en["settings.git.path_hint"];
    expect(hint).toContain("{path}");
    expect(t("settings.git.path_hint", { path: "/Users/me/code" })).toBe(
      hint.replaceAll("{path}", "/Users/me/code"),
    );
  });

  it("leaves unmatched placeholders in place and ignores unused params", () => {
    setLang("en");
    expect(t("page.new_version.title", { nope: "x" })).toBe(
      "New version available: v{version}",
    );
    expect(t("lang.en", { version: "x" })).toBe("English");
  });

  it("substitutes in the active language, not only English", () => {
    setLang("da");
    const expected = translations.da["page.new_version.title"].replaceAll(
      "{version}",
      "9.9",
    );
    expect(t("page.new_version.title", { version: "9.9" })).toBe(expected);
    expect(expected).not.toContain("{version}");
  });
});

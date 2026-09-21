import { afterEach, describe, expect, it } from "vitest";
import {
  defaultExportPrompt,
  isDefaultExportPrompt,
  resolveExportPrompt,
} from "./export-prompt";
import { setLang } from "./i18n.svelte";
import { translations } from "./translations";

const EN = translations.en["export.prompt"];
const DA = translations.da["export.prompt"];

afterEach(() => {
  setLang("en");
});

describe("defaultExportPrompt", () => {
  it("has a distinct built-in prompt per language", () => {
    expect(EN.trim()).not.toBe("");
    expect(DA.trim()).not.toBe("");
    expect(EN).not.toBe(DA);
  });

  it("follows the active UI language", () => {
    setLang("en");
    expect(defaultExportPrompt()).toBe(EN);
    setLang("da");
    expect(defaultExportPrompt()).toBe(DA);
  });
});

describe("isDefaultExportPrompt", () => {
  it("recognises the built-in default of any language", () => {
    expect(isDefaultExportPrompt(EN)).toBe(true);
    expect(isDefaultExportPrompt(DA)).toBe(true);
  });

  it("ignores surrounding whitespace", () => {
    expect(isDefaultExportPrompt(`  ${EN}\n\n`)).toBe(true);
  });

  it("rejects custom text and the empty string", () => {
    expect(isDefaultExportPrompt("Summarise my week.")).toBe(false);
    expect(isDefaultExportPrompt(EN + " extra")).toBe(false);
    expect(isDefaultExportPrompt("")).toBe(false);
  });
});

describe("resolveExportPrompt", () => {
  it("uses the active-language default when nothing is stored", () => {
    setLang("da");
    expect(resolveExportPrompt("")).toBe(DA);
    expect(resolveExportPrompt("   \n")).toBe(DA);
    setLang("en");
    expect(resolveExportPrompt("")).toBe(EN);
  });

  it("re-resolves a stored built-in default so it follows the language", () => {
    setLang("en");
    expect(resolveExportPrompt(DA)).toBe(EN);
    setLang("da");
    expect(resolveExportPrompt(EN)).toBe(DA);
  });

  it("returns custom text verbatim, untrimmed", () => {
    const custom = "  Summarise as bullet points.  ";
    setLang("da");
    expect(resolveExportPrompt(custom)).toBe(custom);
  });
});

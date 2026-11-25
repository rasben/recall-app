import { describe, expect, it } from "vitest";
import type { CliAvailability } from "../bindings";
import { cliUnavailableKey } from "./cli-availability";
import { translations } from "./translations";

const ALL: CliAvailability[] = [
  "Linkable",
  "OnPath",
  "Manual",
  "NotInApplications",
  "AppImage",
  "Windows",
  "Missing",
];

describe("cliUnavailableKey", () => {
  it("has no message for stable locations", () => {
    expect(["Linkable", "OnPath", "Manual"].map((a) => cliUnavailableKey(a as CliAvailability))).toEqual([
      null,
      null,
      null,
    ]);
  });

  it("explains every unusable location with a real translation", () => {
    for (const availability of ALL) {
      const key = cliUnavailableKey(availability);
      if (key) expect(translations.en[key]).toBeTruthy();
    }
    expect(ALL.filter((a) => cliUnavailableKey(a) !== null)).toEqual([
      "NotInApplications",
      "AppImage",
      "Windows",
      "Missing",
    ]);
  });
});

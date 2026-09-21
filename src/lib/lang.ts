import type { Lang } from "./translations";

/** Map a BCP 47 tag (e.g. `navigator.language`) to a UI language: any Danish
 *  variant (`da`, `da-DK`, `da-GL`) is Danish, everything else is English. */
export function langFromNavigator(tag: string | null | undefined): Lang {
  const primary = (tag ?? "").trim().toLowerCase().split(/[-_]/)[0];
  return primary === "da" ? "da" : "en";
}

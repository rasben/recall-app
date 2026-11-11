import type { CliAvailability } from "../bindings";
import type { TranslationKey } from "./translations";

/**
 * Why the bundled `recall` CLI can't be used from where the app is running,
 * as a translation key; `null` when its location is stable (installed with
 * the package, linkable on macOS, or linkable by hand).
 */
export function cliUnavailableKey(availability: CliAvailability): TranslationKey | null {
  switch (availability) {
    case "Linkable":
    case "OnPath":
    case "Manual":
      return null;
    case "NotInApplications":
      return "cli.unavailable.not_in_applications";
    case "AppImage":
      return "cli.unavailable.appimage";
    case "Windows":
      return "cli.unavailable.windows";
    case "Missing":
      return "cli.unavailable.missing";
  }
}

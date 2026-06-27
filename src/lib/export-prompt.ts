import { translations } from "./translations";
import { i18n } from "./i18n.svelte";

const PROMPT_KEY = "export.prompt" as const;

/** The built-in default prompt in the currently-active UI language. */
export function defaultExportPrompt(): string {
  return translations[i18n.lang][PROMPT_KEY];
}

/**
 * True if `value` matches one of the built-in default prompts (any language).
 * Such a value is treated as "still the default", so it keeps following the UI
 * language rather than being pinned as a custom override.
 */
export function isDefaultExportPrompt(value: string): boolean {
  const v = value.trim();
  return Object.values(translations).some((map) => map[PROMPT_KEY].trim() === v);
}

/**
 * Resolve the prompt to actually use for an export, given the user's stored
 * custom prompt. Empty, or matching a built-in default, → the active-language
 * default (so it follows the language). Otherwise the custom text verbatim.
 */
export function resolveExportPrompt(stored: string): string {
  if (stored.trim() === "" || isDefaultExportPrompt(stored)) {
    return defaultExportPrompt();
  }
  return stored;
}

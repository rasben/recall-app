import { translations, type Lang, type TranslationKey } from "./translations";
import { langFromNavigator } from "./lang";

function detectLang(): Lang {
  try {
    const saved = localStorage.getItem("recall:lang");
    if (saved === "en" || saved === "da") return saved;
  } catch {
    // localStorage unavailable
  }
  // Nothing saved yet: follow the OS/browser locale (Danish → da, else en).
  return langFromNavigator(typeof navigator === "undefined" ? undefined : navigator.language);
}

export const i18n = $state({ lang: detectLang() });

export function setLang(lang: Lang) {
  i18n.lang = lang;
  try {
    localStorage.setItem("recall:lang", lang);
    return true;
  } catch {
    // localStorage unavailable
    return false;
  }
}

export function t(
  key: TranslationKey,
  params?: Record<string, string>,
): string {
  const map = translations[i18n.lang];
  let str: string = map[key] ?? translations.en[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      str = str.replaceAll(`{${k}}`, v);
    }
  }
  return str;
}

export function langLocale(): string {
  return i18n.lang === "da" ? "da-DK" : "en-US";
}

// The one i18next instance and the viewer's choice (docs/features/ui-languages.md
// "The instance (`index.ts`)", "The choice"). Only src/i18n/ imports i18next or react-i18next (D3): the
// rest of the UI reads `t` from useTranslate() or takes a Translate argument, so no string is
// resolved at module load (D5) and a switch re-renders what shows text, remounting nothing.
import i18next, { type TFunction } from "i18next";
import { initReactI18next, useTranslation } from "react-i18next";
import { CATALOGS, isLanguage, type Language } from "./catalog";

export { isLanguage, LANGUAGES, type Language } from "./catalog";

/** i18next's `t`, its keys and placeholders typed by `en` (i18next.d.ts). */
export type Translate = TFunction;

/** Where the choice is kept: one per browser, for every project, local or hub (D6). */
export const LANGUAGE_KEY = "specengine.language";

/** The browser's first language: Russian when its primary subtag is `ru`, else English. */
function browserLanguage(): Language {
  const first = (navigator.languages as readonly string[] | undefined)?.[0] ?? navigator.language;
  const primary = (first.split("-")[0] ?? "").toLowerCase();
  return primary === "ru" ? "ru" : "en";
}

/** The language a page starts in: the stored choice, else the browser's first language, else English. Nothing is stored. */
export function startLanguage(): Language {
  try {
    const stored = window.localStorage.getItem(LANGUAGE_KEY);
    if (stored !== null && isLanguage(stored)) {
      return stored;
    }
  } catch {
    // A read that throws counts as no choice.
  }
  return browserLanguage();
}

const instance = i18next.createInstance();

// The one listener: `<html lang>` follows the language, from init on, before the first render (D8).
instance.on("languageChanged", (language: string) => {
  document.documentElement.lang = isLanguage(language) ? language : "en";
});

// Once, synchronously, before anything renders: both catalogs in the main chunk, no backend, no
// detector, no load to fail (D4). React escapes, and a placeholder's value is never read again for
// `{{` or `$t(`.
void instance.use(initReactI18next).init({
  resources: { en: { translation: CATALOGS.en }, ru: { translation: CATALOGS.ru } },
  lng: startLanguage(),
  fallbackLng: "en",
  supportedLngs: ["en", "ru"],
  keySeparator: ".",
  nsSeparator: false,
  interpolation: { escapeValue: false, skipOnVariables: true },
  returnNull: false,
  returnEmptyString: false,
  returnObjects: false,
  react: { useSuspense: false },
  initAsync: false,
  debug: false,
  saveMissing: false,
});

/** The chrome's `t`, re-rendering its component when the language changes. */
export function useTranslate(): Translate {
  return useTranslation().t;
}

/**
 * Shows `next` now and stores nothing. For the select's `choose` (which stores first) and for tests
 * (each test's English start, a reload's start) only; no view calls it.
 */
export function switchLanguage(next: Language): void {
  void instance.changeLanguage(next);
}

/** Stores the choice (a refused write is ignored: the choice holds until a reload) and shows it. */
function choose(next: Language): void {
  try {
    window.localStorage.setItem(LANGUAGE_KEY, next);
  } catch {
    // Kept for this page only.
  }
  switchLanguage(next);
}

/** The language shown, and the select's way to choose another. */
export function useLanguage(): { language: Language; choose: (next: Language) => void } {
  const { i18n } = useTranslation();
  const shown = i18n.language;
  return { language: isLanguage(shown) ? shown : "en", choose };
}

/** A `t` fixed to one language, whatever is shown. For tests only (`enT`, `ruT` in src/test/text.ts); views read `useTranslate()`. */
export function translateIn(language: Language): Translate {
  return instance.getFixedT(language);
}

// The one i18next instance and the viewer's choice (docs/canon/ui-languages.md "Catalog",
// "Choice", "Switcher"). Only src/i18n/ imports i18next or react-i18next: the rest of the UI reads
// `t` from useTranslate() or takes a Translate argument, so no string is resolved at module load and
// a switch re-renders what shows text, remounting nothing. English is in the main chunk and loads
// with it; Russian is a chunk of its own, reached through the one `import("./ru")` below and loaded
// when a page starts in it or the viewer first chooses it.
import i18next, { type TFunction } from "i18next";
import { useSyncExternalStore } from "react";
import { initReactI18next, useTranslation } from "react-i18next";
import { isLanguage, type Language } from "./catalog";
import { en } from "./en";

export { isLanguage, LANGUAGES, type Language } from "./catalog";

/** i18next's `t`, its keys and placeholders typed by `en` (i18next.d.ts). */
export type Translate = TFunction;

/** Where the choice is kept: one per browser, for every project, local or hub. */
export const LANGUAGE_KEY = "specengine.language";

/** How long a page starting in Russian stays empty for its chunk before it shows English meanwhile. */
export const LANGUAGE_WAIT_MS = 2000;

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

// The one listener: `<html lang>` follows the language shown, from init on (never a pending one).
instance.on("languageChanged", (language: string) => {
  document.documentElement.lang = isLanguage(language) ? language : "en";
});

// Once, synchronously, before anything renders: English only, no backend, no detector. Russian joins
// by `addResourceBundle` when its chunk lands; until then a Russian key reads English. React
// escapes, and a placeholder's value is never read again for `{{` or `$t(`.
void instance.use(initReactI18next).init({
  resources: { en: { translation: en } },
  lng: "en",
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

/** Russian's load in flight or done; cleared by a failure, so the next call imports again. */
let russian: Promise<boolean> | null = null;

/** The languages whose catalog the instance holds. */
const loaded = new Set<Language>(["en"]);

/**
 * Makes a language's catalog ready: English is; Russian imports its chunk once per page and joins the
 * instance. Resolves whether it is ready; never rejects.
 */
export function loadLanguage(language: Language): Promise<boolean> {
  if (language === "en") {
    return Promise.resolve(true);
  }
  russian ??= import("./ru").then(
    (module) => {
      instance.addResourceBundle("ru", "translation", module.ru, true, true);
      loaded.add("ru");
      return true;
    },
    () => {
      russian = null;
      return false;
    },
  );
  return russian;
}

/** What the select shows besides the language: a choice waiting for its chunk, a load that failed. */
interface LanguageState {
  readonly pending: Language | null;
  readonly failed: boolean;
}

let state: LanguageState = { pending: null, failed: false };
const listeners = new Set<() => void>();

function setState(next: LanguageState): void {
  state = next;
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function snapshot(): LanguageState {
  return state;
}

/** Counts the viewer's choices: a load that lands after a later choice changes nothing. */
let choices = 0;

/**
 * Shows `next` now and stores nothing, for a language already loaded. For the select's `choose`, the
 * start, and tests (each test's English start, a reload's start) only; no view calls it.
 */
export function switchLanguage(next: Language): void {
  void instance.changeLanguage(next);
}

/**
 * The bootstrap's gate (main.tsx chains it by `.then`): English is ready at once. A Russian start
 * waits for its chunk up to LANGUAGE_WAIT_MS and shows Russian; past that the page shows English and
 * turns Russian when the chunk lands, unless the viewer has chosen meanwhile. A failed chunk leaves
 * English, the stored choice and `language.failed`. Never rejects.
 */
export function readyLanguage(): Promise<void> {
  const start = startLanguage();
  if (start === "en") {
    return Promise.resolve();
  }
  const asked = choices;
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, LANGUAGE_WAIT_MS);
    void loadLanguage(start).then((loaded) => {
      clearTimeout(timer);
      if (asked === choices) {
        if (loaded) {
          switchLanguage(start);
        } else {
          setState({ pending: null, failed: true });
        }
      }
      resolve();
    });
  });
}

/**
 * The select's choice: stored (a refused write is ignored: the choice holds until a reload), shown at
 * once when loaded, else when its chunk lands, unless another choice came meanwhile.
 */
function choose(next: Language): void {
  try {
    window.localStorage.setItem(LANGUAGE_KEY, next);
  } catch {
    // Kept for this page only.
  }
  choices += 1;
  const asked = choices;
  if (loaded.has(next)) {
    setState({ pending: null, failed: false });
    switchLanguage(next);
    return;
  }
  setState({ pending: next, failed: false });
  void loadLanguage(next).then((ready) => {
    if (asked !== choices) {
      return;
    }
    // The language changes only once its catalog is in: never a Russian page of English values.
    if (ready) {
      switchLanguage(next);
    }
    setState({ pending: null, failed: !ready });
  });
}

/** The chrome's `t`, re-rendering its component when the language changes. */
export function useTranslate(): Translate {
  return useTranslation().t;
}

/** The language shown, a choice waiting for its chunk, a load that failed, and the select's way to choose. */
export function useLanguage(): { language: Language; pending: Language | null; failed: boolean; choose: (next: Language) => void } {
  const { i18n } = useTranslation();
  const { pending, failed } = useSyncExternalStore(subscribe, snapshot);
  const shown = i18n.language;
  return { language: isLanguage(shown) ? shown : "en", pending, failed, choose };
}

/**
 * A language's own name in that language, as the browser's `Intl` names it, its first letter upper
 * case: right before its catalog loads. The code itself when `Intl` cannot name it.
 */
export function languageName(language: Language): string {
  try {
    const name = new Intl.DisplayNames([language], { type: "language" }).of(language);
    if (name === undefined || name === "" || name === language) {
      return language;
    }
    return `${name.charAt(0).toLocaleUpperCase(language)}${name.slice(1)}`;
  } catch {
    return language;
  }
}

/** A `t` fixed to one language, whatever is shown, for a loaded language. For tests only (`enT`, `ruT` in src/test/text.ts); views read `useTranslate()`. */
export function translateIn(language: Language): Translate {
  return instance.getFixedT(language);
}

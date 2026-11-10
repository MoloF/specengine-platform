// The viewer's theme (docs/features/design-foundation.md "Themes", "Theme choice"): System, Light
// or Dark, one per browser profile as the language is; a hub keeps none. With none stored the page
// follows the system's `prefers-color-scheme` through `color-scheme: light dark` (tokens.css), no
// script needed; a choice pins `data-theme` on <html>, which tokens.css turns into one scheme. The
// bootstrap applies the stored choice before the first render; only the header's select writes it.

/** The choices, in the select's order. */
export const THEMES = ["system", "light", "dark"] as const;

export type Theme = (typeof THEMES)[number];

/** Where the choice is kept: `light` or `dark`; absent, System. */
export const THEME_KEY = "specengine.theme";

/** The attribute on <html> that pins a scheme while one is chosen. */
const THEME_ATTRIBUTE = "data-theme";

export function isTheme(value: string): value is Theme {
  return (THEMES as readonly string[]).includes(value);
}

/** The stored choice; nothing stored, another value or a storage that throws reads System, silently. */
export function storedTheme(): Theme {
  try {
    const stored = window.localStorage.getItem(THEME_KEY);
    return stored === "light" || stored === "dark" ? stored : "system";
  } catch {
    return "system";
  }
}

/** Shows a choice: Light or Dark pins `data-theme`, System removes it and the page follows the system. */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") {
    root.removeAttribute(THEME_ATTRIBUTE);
  } else {
    root.setAttribute(THEME_ATTRIBUTE, theme);
  }
}

/** The select's act: store the choice (System removes the key), then show it. A refused write holds for this page. */
export function chooseTheme(theme: Theme): void {
  try {
    if (theme === "system") {
      window.localStorage.removeItem(THEME_KEY);
    } else {
      window.localStorage.setItem(THEME_KEY, theme);
    }
  } catch {
    // Shown for this page alone; the next load reads System.
  }
  applyTheme(theme);
}

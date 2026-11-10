import { useId, useState } from "react";
import { useTranslate } from "../i18n";
import { Icon, type IconName } from "../ui/Icon";
import { chooseTheme, isTheme, storedTheme, THEMES, type Theme } from "./theme";

/** The icon in front of each choice's value. */
const THEME_ICONS: Readonly<Record<Theme, IconName>> = { system: "themeSystem", light: "themeLight", dark: "themeDark" };

/**
 * The Theme select in the header, after Language (docs/features/design-foundation.md "Themes"):
 * System, Light and Dark. A choice is stored per viewer and shown at once, no reload, no request; focus
 * stays here. Its label is visually hidden, the value shown with an icon in front, so its name is
 * "Theme" in either language.
 */
export function ThemeSwitcher() {
  const t = useTranslate();
  const id = useId();
  const [theme, setTheme] = useState<Theme>(storedTheme);
  return (
    <div className="header-select theme-switcher">
      <label htmlFor={id} className="sr-only">
        {t("app.theme.label")}
      </label>
      <Icon name={THEME_ICONS[theme]} />
      <select
        id={id}
        className="field header-field"
        value={theme}
        onChange={(event) => {
          const next = event.target.value;
          if (isTheme(next)) {
            chooseTheme(next);
            setTheme(next);
          }
        }}
      >
        {THEMES.map((each) => (
          <option key={each} value={each}>
            {t(`app.theme.${each}`)}
          </option>
        ))}
      </select>
    </div>
  );
}

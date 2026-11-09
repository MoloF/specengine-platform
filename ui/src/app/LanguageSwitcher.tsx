import { useId } from "react";
import { isLanguage, LANGUAGES, useLanguage, useTranslate } from "../i18n";

/**
 * The Language select in the header (docs/features/ui-languages.md D7): always shown, beside
 * Workspace; each option is its language's own name, tagged with that language. A choice switches
 * the chrome at once, reads nothing and keeps focus here; it holds for this browser until changed.
 */
export function LanguageSwitcher() {
  const t = useTranslate();
  const { language, choose } = useLanguage();
  const id = useId();
  return (
    <div className="language-switcher">
      <label htmlFor={id}>{t("language.label")}</label>
      <select
        id={id}
        className="field"
        value={language}
        onChange={(event) => {
          const next = event.target.value;
          if (isLanguage(next)) {
            choose(next);
          }
        }}
      >
        {LANGUAGES.map((each) => (
          <option key={each} value={each} lang={each}>
            {t("language.self", { lng: each })}
          </option>
        ))}
      </select>
    </div>
  );
}

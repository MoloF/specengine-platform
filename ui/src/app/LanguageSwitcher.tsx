import { useId } from "react";
import { isLanguage, LANGUAGES, languageName, useLanguage, useTranslate } from "../i18n";
import { Icon } from "../ui/Icon";

/**
 * The Language select in the header (docs/canon/ui-languages.md "Switcher"): beside Workspace; each
 * option is its language's own name from `Intl`, tagged with that language, so Russian reads right
 * before its chunk loads. A choice switches the chrome at once, or once Russian's chunk lands (the
 * select shows the choice, busy, meanwhile); it reads nothing from the daemon and keeps focus here. A
 * chunk that failed leaves English and says so until the next choice. In the one-line header its label
 * is visually hidden, the value shown with an icon in front; its name stays "Language".
 */
export function LanguageSwitcher() {
  const t = useTranslate();
  const { language, pending, failed, choose } = useLanguage();
  const id = useId();
  return (
    <div className="header-select language-switcher">
      <label htmlFor={id} className="sr-only">
        {t("language.label")}
      </label>
      <Icon name="language" />
      <select
        id={id}
        className="field header-field"
        value={pending ?? language}
        aria-busy={pending === null ? undefined : true}
        onChange={(event) => {
          const next = event.target.value;
          if (isLanguage(next)) {
            choose(next);
          }
        }}
      >
        {LANGUAGES.map((each) => (
          <option key={each} value={each} lang={each}>
            {languageName(each)}
          </option>
        ))}
      </select>
      {failed && (
        <p role="status" className="language-failed">
          {t("language.failed")}
        </p>
      )}
    </div>
  );
}

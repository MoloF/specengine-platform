import { useTranslate } from "../i18n";
import { Icon, type IconName } from "./Icon";

/** How one value of a closed table looks: its label, its colour role and its icon. */
export interface Look {
  label: string;
  /** A colour role of tokens.css, e.g. `severity-high`; unknown values take a neutral one. */
  tone: string;
  icon: IconName;
  /** The label is a served value no table knows, shown raw: tagged English as the daemon's words are (docs/canon/ui-languages.md "Served text"). */
  served?: true;
}

/** A status never shown by colour alone: icon and label; `name` prefixes it for screen readers where no label sits beside it. */
export function Badge({ name, look }: { name?: string; look: Look }) {
  const t = useTranslate();
  return (
    <span className={`badge tone-${look.tone}`} data-tone={look.tone}>
      <Icon name={look.icon} />
      {name !== undefined && <span className="sr-only">{t("ui.badgeName", { name })}</span>}
      <span className="badge-label" lang={look.served === true ? "en" : undefined}>
        {look.label}
      </span>
    </span>
  );
}

import { Icon, type IconName } from "./Icon";

// A status as a lozenge (docs/features/design-foundation.md "Primitives (`ui/src/ui/`)"): a label and an icon,
// never colour alone, in one of six appearances, each a surface and its text colour from tokens.css.
// 20 px tall, 12/16 text at weight 500, in sentence case; at most 200 px wide, a longer label wrapping
// by whole lines and never cut (W11).

export type LozengeAppearance = "neutral" | "information" | "success" | "warning" | "danger" | "discovery";

/** The six appearances, in the spec's order. */
export const LOZENGE_APPEARANCES: readonly LozengeAppearance[] = ["neutral", "information", "success", "warning", "danger", "discovery"];

/** A status's look: what a Lozenge draws. */
export interface LozengeLook {
  appearance: LozengeAppearance;
  icon: IconName;
  label: string;
}

/** `lang` tags a served word no table knows, as the daemon's words are (docs/canon/ui-languages.md "Served text"). */
export function Lozenge({ appearance, icon, label, lang }: LozengeLook & { lang?: "en" }) {
  return (
    <span className="lozenge" data-appearance={appearance}>
      <Icon name={icon} />
      <span className="lozenge-label" lang={lang}>
        {label}
      </span>
    </span>
  );
}

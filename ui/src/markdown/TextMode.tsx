// The Rendered/Source switch of a spec text (ui/README.md "Screen rules", Text): Rendered is the
// default; Source is the text as sent, numbered (src/tree/TextView.tsx). The view holding the
// choice keeps it from node to node.

import { useTranslate } from "../i18n";

export type TextMode = "rendered" | "source";

export const DEFAULT_TEXT_MODE: TextMode = "rendered";

const MODES: readonly TextMode[] = ["rendered", "source"];

/** Two toggle buttons, one pressed; `label` names what they show. */
export function TextModeSwitch({ mode, label, onChange }: { mode: TextMode; label: string; onChange: (mode: TextMode) => void }) {
  const t = useTranslate();
  return (
    <div className="text-mode" role="group" aria-label={label}>
      {MODES.map((value) => (
        <button
          key={value}
          type="button"
          className="text-mode-option"
          aria-pressed={mode === value}
          onClick={() => {
            onChange(value);
          }}
        >
          {value === "rendered" ? t("markdown.rendered") : t("markdown.source")}
        </button>
      ))}
    </div>
  );
}

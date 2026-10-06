// The Rendered/Source switch of a spec text (docs/features/ui-markdown.md "Description and interactions"): Rendered
// is the default; Source is the text as sent, numbered (src/tree/TextView.tsx). The view holding
// the choice keeps it from node to node.

export type TextMode = "rendered" | "source";

export const DEFAULT_TEXT_MODE: TextMode = "rendered";

const MODES: readonly (readonly [TextMode, string])[] = [
  ["rendered", "Rendered"],
  ["source", "Source"],
];

/** Two toggle buttons, one pressed; `label` names what they show. */
export function TextModeSwitch({ mode, label, onChange }: { mode: TextMode; label: string; onChange: (mode: TextMode) => void }) {
  return (
    <div className="text-mode" role="group" aria-label={label}>
      {MODES.map(([value, name]) => (
        <button
          key={value}
          type="button"
          className="text-mode-option"
          aria-pressed={mode === value}
          onClick={() => {
            onChange(value);
          }}
        >
          {name}
        </button>
      ))}
    </div>
  );
}

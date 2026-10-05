import { createContext, use } from "react";
import { DECISION_KEYS, WORDING } from "../inbox/decisions";
import { Dialog } from "../ui/Dialog";

const OpenShortcuts = createContext<() => void>(() => undefined);

/** Lets a view open the shell's "Keyboard shortcuts" dialog (the `?` key). */
export const ShortcutsOpener = OpenShortcuts;

export function useOpenShortcuts(): () => void {
  return use(OpenShortcuts);
}

/** Letter keys exactly as typed (lower case); the decisions with the words of their buttons. */
const KEYS: readonly (readonly [keys: readonly string[], action: string])[] = [
  [["j", "Down arrow"], "Next proposal"],
  [["k", "Up arrow"], "Previous proposal"],
  ...DECISION_KEYS.map(([kind, key]) => [[key], WORDING[kind].verb] as const),
  [["?"], "Show this list"],
  [["Esc"], "Close a dialog"],
];

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog title="Keyboard shortcuts" onClose={onClose}>
      <p className="dialog-text">
        Letter keys act while focus is in the Inbox queue, outside a text field, without Ctrl, Alt or Cmd. The
        decision keys open the dialog of that decision for the selected proposal.
      </p>
      <dl className="shortcut-list">
        {KEYS.map(([keys, action]) => (
          <div key={action} className="shortcut">
            <dt>
              {keys.map((key, index) => (
                <span key={key}>
                  {index > 0 && " or "}
                  <kbd>{key}</kbd>
                </span>
              ))}
            </dt>
            <dd>{action}</dd>
          </div>
        ))}
      </dl>
      <div className="dialog-actions">
        <button type="button" className="button" onClick={onClose} data-autofocus="">
          Close
        </button>
      </div>
    </Dialog>
  );
}

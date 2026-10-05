import { createContext, use } from "react";
import { DECISION_KEYS, WORDING } from "../inbox/decisions";
import { Dialog } from "../ui/Dialog";
import type { SectionId } from "./sections";

const OpenShortcuts = createContext<() => void>(() => undefined);

/** Lets a view open the shell's "Keyboard shortcuts" dialog (the `?` key). */
export const ShortcutsOpener = OpenShortcuts;

export function useOpenShortcuts(): () => void {
  return use(OpenShortcuts);
}

type Shortcut = readonly [keys: readonly string[], action: string];

/** Letter keys exactly as typed (lower case); the decisions with the words of their buttons. */
const INBOX_KEYS: readonly Shortcut[] = [
  [["j", "Down arrow"], "Next proposal"],
  [["k", "Up arrow"], "Previous proposal"],
  ...DECISION_KEYS.map(([kind, key]) => [[key], WORDING[kind].verb] as const),
  [["?"], "Show this list"],
  [["Esc"], "Close a dialog"],
];

/** The Spec tree's keys (docs/features/ui-tree-node.md "Keys"): the tree, the tabs, the search. */
const TREE_KEYS: readonly Shortcut[] = [
  [["j", "Down arrow"], "Tree, hits: next row"],
  [["k", "Up arrow"], "Tree, hits: previous row"],
  [["Right arrow"], "Tree: expand, else go to the first child; tabs: next tab"],
  [["Left arrow"], "Tree: collapse, else go to the parent; tabs: previous tab"],
  [["Home", "End"], "First or last row or tab"],
  [["Enter"], "Tree, hits: open the node, focus stays; search field: search"],
  [["Enter", "Space"], "Tabs: show the focused tab"],
  [["Esc"], "Search: back to the tree; a dialog: close it"],
  [["?"], "Show this list"],
];

const INTRO: Partial<Record<SectionId, string>> = {
  inbox:
    "Letter keys act while focus is in the Inbox queue, outside a text field, without Ctrl, Alt or Cmd. The decision keys open the dialog of that decision for the selected proposal.",
  tree: "Keys act where focus is (the tree, the hits, the tabs), outside a text field, without Ctrl, Alt or Cmd. Arrows only move; Enter and clicks open, one history entry each.",
};

function keysOf(section: SectionId | null): readonly Shortcut[] {
  if (section === "tree") {
    return TREE_KEYS;
  }
  return INBOX_KEYS;
}

export function ShortcutsDialog({ section = null, onClose }: { section?: SectionId | null; onClose: () => void }) {
  const keys = keysOf(section);
  return (
    <Dialog title="Keyboard shortcuts" onClose={onClose}>
      <p className="dialog-text">{INTRO[section === "tree" ? "tree" : "inbox"]}</p>
      <dl className="shortcut-list">
        {keys.map(([names, action]) => (
          <div key={action} className="shortcut">
            <dt>
              {names.map((key, index) => (
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

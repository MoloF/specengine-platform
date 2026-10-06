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

/** The palette's chord, the one key that acts anywhere (docs/features/ui-home.md "Keys"); in every list. */
const JUMP: Shortcut = [["Cmd-K", "Ctrl-K"], "Jump to a section, task, proposal, node or project"];

/** Letter keys exactly as typed (lower case); the decisions with the words of their buttons. */
const INBOX_KEYS: readonly Shortcut[] = [
  [["j", "Down arrow"], "Next proposal"],
  [["k", "Up arrow"], "Previous proposal"],
  ...DECISION_KEYS.map(([kind, key]) => [[key], WORDING[kind].verb] as const),
  JUMP,
  [["?"], "Show this list"],
  [["Esc"], "Close a dialog"],
];

/** No section (the home, a hash naming nothing): the chord, this list, Esc. */
const GENERAL_KEYS: readonly Shortcut[] = [JUMP, [["?"], "Show this list"], [["Esc"], "Close a dialog"]];

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
  JUMP,
  [["?"], "Show this list"],
];

/** The Graph's keys (docs/features/ui-graph.md "Keyboard"): the canvas, the tabs, the REF field. */
const GRAPH_KEYS: readonly Shortcut[] = [
  [["j", "Down arrow"], "Canvas: next box in the column"],
  [["k", "Up arrow"], "Canvas: previous box in the column"],
  [["Left arrow", "Right arrow"], "Canvas: the nearest box of the column beside; tabs: the other tab"],
  [["Home", "End"], "Canvas: first or last box of the column"],
  [["Enter"], "Canvas: open the box's details; REF field: show its graph"],
  [["o"], "Canvas: open the box in the spec tree"],
  [["c"], "Canvas: centre the graph on the box, options kept"],
  [["Esc"], "Close the details, focus back on the box; a dialog: close it"],
  JUMP,
  [["?"], "Show this list"],
];

/** The Tasks screen's keys (docs/features/ui-tasks.md "Description and interactions", Keyboard): no key acts on a task. */
const TASKS_KEYS: readonly Shortcut[] = [
  [["j", "Down arrow"], "Task list: next task"],
  [["k", "Up arrow"], "Task list: previous task"],
  [["Home", "End"], "Task list: first or last task"],
  [["Enter"], "Task list: open the task, focus stays on its row"],
  [["Left arrow", "Right arrow"], "Tabs: previous or next tab"],
  [["Enter", "Space"], "Tabs: show the focused tab"],
  [["Esc"], "In a task: back to its row in the list; a dialog: close it"],
  JUMP,
  [["?"], "Show this list"],
];

/** Said in every intro: the chord is the one key that acts from anywhere (docs/features/ui-home.md "Keys"). */
const CHORD_EXCEPTED = "Cmd-K or Ctrl-K excepted: it opens Jump to from anywhere, a text field included.";

const GENERAL_INTRO = `Keys act where focus is, outside a text field, without Ctrl, Alt or Cmd; ${CHORD_EXCEPTED}`;

const INTRO: Partial<Record<SectionId, string>> = {
  inbox: `Letter keys act while focus is in the Inbox queue, outside a text field, without Ctrl, Alt or Cmd; ${CHORD_EXCEPTED} The decision keys open the dialog of that decision for the selected proposal.`,
  tree: `Keys act where focus is (the tree, the hits, the tabs), outside a text field, without Ctrl, Alt or Cmd; ${CHORD_EXCEPTED} Arrows only move; Enter and clicks open, one history entry each.`,
  graph: `Keys act on the canvas's focused box, outside a text field, without Ctrl, Alt or Cmd; ${CHORD_EXCEPTED} One box holds the tab stop; arrows only move it. The List view holds every node and edge as text.`,
  tasks: `Keys act where focus is (the task list, the tabs), outside a text field, without Ctrl, Alt or Cmd; ${CHORD_EXCEPTED} Arrows only move; Enter and clicks open, one history entry each. No key changes a task: owner actions are commands you copy to a terminal.`,
};

/** A section's list; with none (the home, a hash naming nothing) the general one. */
function keysOf(section: SectionId | null): readonly Shortcut[] {
  if (section === "tree") {
    return TREE_KEYS;
  }
  if (section === "graph") {
    return GRAPH_KEYS;
  }
  if (section === "tasks") {
    return TASKS_KEYS;
  }
  if (section === "inbox") {
    return INBOX_KEYS;
  }
  return GENERAL_KEYS;
}

function introOf(section: SectionId | null): string {
  return (section === null ? undefined : INTRO[section]) ?? GENERAL_INTRO;
}

export function ShortcutsDialog({ section = null, onClose }: { section?: SectionId | null; onClose: () => void }) {
  const keys = keysOf(section);
  return (
    <Dialog title="Keyboard shortcuts" onClose={onClose}>
      <p className="dialog-text">{introOf(section)}</p>
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

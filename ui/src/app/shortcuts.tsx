import { createContext, use } from "react";
import { useTranslate, type Translate } from "../i18n";
import type { en } from "../i18n/en";
import { DECISION_KEYS, verbOf } from "../inbox/decisions";
import { Dialog } from "../ui/Dialog";
import type { SectionId } from "./sections";

const OpenShortcuts = createContext<() => void>(() => undefined);

/** Lets a view open the shell's "Keyboard shortcuts" dialog (the `?` key). */
export const ShortcutsOpener = OpenShortcuts;

export function useOpenShortcuts(): () => void {
  return use(OpenShortcuts);
}

/** A key the list names: a cap of `app.keys`, or a character as typed (a letter, `?`), the same in every language. */
type Cap = keyof typeof en.app.keys | { typed: string };

type Shortcut = readonly [caps: readonly Cap[], action: (t: Translate) => string];

function typed(key: string): Cap {
  return { typed: key };
}

/** The palette's chord, the one key that acts anywhere (ui/README.md "Screen rules", Keyboard); in every list. */
const JUMP: Shortcut = [["cmdK", "ctrlK"], (t) => t("app.shortcuts.jump")];
const SHOW_LIST: Shortcut = [[typed("?")], (t) => t("app.shortcuts.showList")];
const CLOSE_DIALOG: Shortcut = [["esc"], (t) => t("app.shortcuts.closeDialog")];

/** Letter keys exactly as typed (lower case); the decisions with the words of their buttons. */
const INBOX_KEYS: readonly Shortcut[] = [
  [[typed("j"), "down"], (t) => t("app.shortcuts.inbox.next")],
  [[typed("k"), "up"], (t) => t("app.shortcuts.inbox.previous")],
  ...DECISION_KEYS.map(([kind, key]): Shortcut => [[typed(key)], (t) => verbOf(kind, t)]),
  JUMP,
  SHOW_LIST,
  CLOSE_DIALOG,
];

/** No section (the home, a hash naming nothing): the chord, this list, Esc. */
const GENERAL_KEYS: readonly Shortcut[] = [JUMP, SHOW_LIST, CLOSE_DIALOG];

/** The Spec tree's keys (docs/features/ui-tree-node.md "Implementation", Keys): the tree, the tabs, the search. */
const TREE_KEYS: readonly Shortcut[] = [
  [[typed("j"), "down"], (t) => t("app.shortcuts.tree.next")],
  [[typed("k"), "up"], (t) => t("app.shortcuts.tree.previous")],
  [["right"], (t) => t("app.shortcuts.tree.right")],
  [["left"], (t) => t("app.shortcuts.tree.left")],
  [["home", "end"], (t) => t("app.shortcuts.tree.ends")],
  [["enter"], (t) => t("app.shortcuts.tree.enter")],
  [["enter", "space"], (t) => t("app.shortcuts.tree.tab")],
  [["esc"], (t) => t("app.shortcuts.tree.esc")],
  JUMP,
  SHOW_LIST,
];

/** The Graph's keys (docs/features/ui-graph.md "Implementation", Keyboard): the canvas, the tabs, the REF field. */
const GRAPH_KEYS: readonly Shortcut[] = [
  [[typed("j"), "down"], (t) => t("app.shortcuts.graph.next")],
  [[typed("k"), "up"], (t) => t("app.shortcuts.graph.previous")],
  [["left", "right"], (t) => t("app.shortcuts.graph.across")],
  [["home", "end"], (t) => t("app.shortcuts.graph.ends")],
  [["enter"], (t) => t("app.shortcuts.graph.enter")],
  [[typed("o")], (t) => t("app.shortcuts.graph.tree")],
  [[typed("c")], (t) => t("app.shortcuts.graph.centre")],
  [["esc"], (t) => t("app.shortcuts.graph.esc")],
  JUMP,
  SHOW_LIST,
];

/** The Flow's keys (docs/features/flow-canvas.md "Keyboard, motion, width"): the canvas, one tab stop; no key changes the spec. */
const FLOW_KEYS: readonly Shortcut[] = [
  [["down", "up"], (t) => t("app.shortcuts.flow.move")],
  [["left", "right"], (t) => t("app.shortcuts.flow.across")],
  [["enter"], (t) => t("app.shortcuts.flow.enter")],
  [["space"], (t) => t("app.shortcuts.flow.space")],
  [[typed("o")], (t) => t("app.shortcuts.flow.tree")],
  [[typed("c")], (t) => t("app.shortcuts.flow.centre")],
  [["esc"], (t) => t("app.shortcuts.flow.esc")],
  JUMP,
  SHOW_LIST,
];

/** The Tasks screen's keys (docs/features/ui-tasks.md "Implementation", List and Detail): no key acts on a task. */
const TASKS_KEYS: readonly Shortcut[] = [
  [[typed("j"), "down"], (t) => t("app.shortcuts.tasks.next")],
  [[typed("k"), "up"], (t) => t("app.shortcuts.tasks.previous")],
  [["home", "end"], (t) => t("app.shortcuts.tasks.ends")],
  [["enter"], (t) => t("app.shortcuts.tasks.enter")],
  [["left", "right"], (t) => t("app.shortcuts.tasks.tabs")],
  [["enter", "space"], (t) => t("app.shortcuts.tasks.tab")],
  [["esc"], (t) => t("app.shortcuts.tasks.esc")],
  JUMP,
  SHOW_LIST,
];

/** The Board's keys (docs/features/board-ui.md "Moving"): the cards, one tab stop; the Move menu. */
const BOARD_KEYS: readonly Shortcut[] = [
  [["down", "up"], (t) => t("app.shortcuts.board.move")],
  [["left", "right"], (t) => t("app.shortcuts.board.across")],
  [["home", "end"], (t) => t("app.shortcuts.board.ends")],
  [["enter"], (t) => t("app.shortcuts.board.enter")],
  [[typed("m")], (t) => t("app.shortcuts.board.menu")],
  [[typed("n")], (t) => t("app.shortcuts.board.new")],
  [["esc"], (t) => t("app.shortcuts.board.esc")],
  JUMP,
  SHOW_LIST,
];

/** Health's keys (docs/features/ui-health.md, keys as Tasks): the findings, one Tab stop; no key changes anything. */
const HEALTH_KEYS: readonly Shortcut[] = [
  [[typed("j"), "down"], (t) => t("app.shortcuts.health.next")],
  [[typed("k"), "up"], (t) => t("app.shortcuts.health.previous")],
  [["home", "end"], (t) => t("app.shortcuts.health.ends")],
  [["enter"], (t) => t("app.shortcuts.health.enter")],
  [["enter", "space"], (t) => t("app.shortcuts.health.group")],
  JUMP,
  SHOW_LIST,
  CLOSE_DIALOG,
];

/** A section's list; with none (the home, a hash naming nothing) the general one. */
function keysOf(section: SectionId | null): readonly Shortcut[] {
  if (section === "tree") {
    return TREE_KEYS;
  }
  if (section === "graph") {
    return GRAPH_KEYS;
  }
  if (section === "flow") {
    return FLOW_KEYS;
  }
  if (section === "tasks") {
    return TASKS_KEYS;
  }
  if (section === "inbox") {
    return INBOX_KEYS;
  }
  if (section === "health") {
    return HEALTH_KEYS;
  }
  if (section === "board") {
    return BOARD_KEYS;
  }
  return GENERAL_KEYS;
}

/** The intro, saying in each that the chord is the one key that acts from anywhere (docs/features/ui-home.md "Implementation", Keys). */
function introOf(section: SectionId | null, t: Translate): string {
  return t(`app.shortcuts.intro.${section ?? "general"}`, { chord: t("app.shortcuts.chordExcepted") });
}

function capText(cap: Cap, t: Translate): string {
  return typeof cap === "string" ? t(`app.keys.${cap}`) : cap.typed;
}

export function ShortcutsDialog({ section = null, onClose }: { section?: SectionId | null; onClose: () => void }) {
  const t = useTranslate();
  const keys = keysOf(section);
  return (
    <Dialog title={t("app.shortcuts.title")} onClose={onClose}>
      <p className="dialog-text">{introOf(section, t)}</p>
      <dl className="shortcut-list">
        {keys.map(([caps, action]) => {
          const said = action(t);
          return (
            <div key={said} className="shortcut">
              <dt>
                {caps.map((cap, index) => {
                  const text = capText(cap, t);
                  return (
                    <span key={text}>
                      {index > 0 && t("app.shortcuts.or")}
                      <kbd>{text}</kbd>
                    </span>
                  );
                })}
              </dt>
              <dd>{said}</dd>
            </div>
          );
        })}
      </dl>
      <div className="dialog-actions">
        <button type="button" className="button" onClick={onClose} data-autofocus="">
          {t("app.shortcuts.close")}
        </button>
      </div>
    </Dialog>
  );
}

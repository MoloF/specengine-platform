import type { Card, CardAllowed, CardFlow, CardMoveBody, CardRow, CardWrite } from "../api/types";
import type { Translate } from "../i18n";
import { AcceptCardDialog, AssignDialog, CancelCardDialog, ClearCardDialog, EditCardDialog, HandOffDialog, NewCardDialog, ReturnDialog, type HandOffSource, type ReturnTarget } from "./CardDialogs";
import { columnTitle, needsLine } from "./labels";
import type { MoveChoice } from "./MoveMenu";

// What a card's acts open, shared by the Board (a tile, the modal) and the card's page: the dialog
// of each write, and the lines of the Move menu. Each dialog is one call; what follows a write
// (focus, the live region, the notes) is the caller's (`ui/README.md` "Screen rules", Dialogs).

/** What Accept works on: a row from the Move menu, or the card read. */
export type AcceptTarget = Pick<CardRow, "id" | "title" | "revision" | "assignee" | "reviewer"> & Partial<Pick<CardRow, "targets">>;

/** The dialog open, each with what it works on and who gets focus back. */
export type OpenDialog =
  | { action: "new"; trigger: HTMLElement | null }
  | { action: "accept"; card: AcceptTarget; trigger: HTMLElement | null }
  | { action: "handoff"; card: HandOffSource; trigger: HTMLElement | null }
  | { action: "return"; card: ReturnTarget; to: string; before: string | null; trigger: HTMLElement | null }
  | { action: "edit" | "assign" | "cancel" | "clear"; card: Card; trigger: HTMLElement | null };

/** How a write's outcome is said around the card's ID, the answer's place at hand. */
export type Said = (id: string) => string;

/**
 * The open dialog. `current` gives each write the card as read now, so a submit goes on its current
 * revision; `onWritten` hears the answer, how to say it, and whether focus follows the card.
 */
export function CardActDialog({
  project,
  dialog,
  flowOf,
  flows,
  current,
  clearOn,
  t,
  onClose,
  onActingLine,
  onWritten,
}: {
  project: string;
  dialog: OpenDialog;
  flowOf: (name: string) => CardFlow | undefined;
  /** Every flow the list serves: a hand-off offers each one's disciplines. */
  flows: readonly CardFlow[];
  /** The card as read now: its row's revision for a row, its read for a card. */
  current: {
    row: (id: string) => CardRow | undefined;
    card: (card: Card) => Card;
  };
  /** What a Clear names: the mark's target. */
  clearOn: string | null;
  t: Translate;
  onClose: () => void;
  onActingLine: (line: string) => void;
  onWritten: (answer: CardWrite, said: Said, toCard: boolean) => void;
}) {
  const place = (answer: CardWrite) => columnTitle(flowOf(answer.flow ?? ""), answer.column ?? "");
  switch (dialog.action) {
    case "new":
      return (
        <NewCardDialog
          project={project}
          onClose={onClose}
          onActingLine={onActingLine}
          onDone={(answer) => {
            onWritten(answer, (id) => t("board.said.created", { id, column: place(answer) }), true);
          }}
        />
      );
    case "accept":
      return (
        <AcceptCardDialog
          project={project}
          card={current.row(dialog.card.id) ?? dialog.card}
          onClose={onClose}
          onActingLine={onActingLine}
          onDone={(answer) => {
            // From the Move menu the trigger is the card, which has moved; from the card's acts, its Accept, gone with the write.
            onWritten(answer, (id) => t("board.said.accepted", { id }), dialog.trigger?.dataset.card !== undefined);
          }}
        />
      );
    case "handoff":
      return (
        <HandOffDialog
          project={project}
          card={dialog.card}
          flows={flows}
          onClose={onClose}
          onActingLine={onActingLine}
          onDone={(answer) => {
            // The child is another card: focus goes back to where the hand-off was asked.
            onWritten(answer, (id) => t("board.said.handedOff", { id, place: `${answer.flow ?? ""}/${answer.column ?? ""}` }), false);
          }}
        />
      );
    case "return": {
      const row = current.row(dialog.card.id);
      const flow = flowOf(row?.flow ?? "");
      return (
        <ReturnDialog
          project={project}
          card={row ?? dialog.card}
          to={dialog.to}
          toTitle={columnTitle(flow, dialog.to)}
          before={dialog.before}
          onClose={onClose}
          onActingLine={onActingLine}
          onDone={(answer) => {
            onWritten(answer, (id) => t("board.said.returned", { id }), dialog.trigger?.dataset.card !== undefined);
          }}
        />
      );
    }
    case "edit":
      return (
        <EditCardDialog
          project={project}
          card={current.card(dialog.card)}
          onClose={onClose}
          onActingLine={onActingLine}
          onDone={(answer) => {
            onWritten(answer, (id) => t("board.said.saved", { id }), false);
          }}
        />
      );
    case "clear":
      return (
        <ClearCardDialog
          project={project}
          card={current.card(dialog.card)}
          on={clearOn}
          onClose={onClose}
          onActingLine={onActingLine}
          onDone={(answer) => {
            onWritten(answer, (id) => t("board.said.cleared", { id }), false);
          }}
        />
      );
    case "assign":
      return (
        <AssignDialog
          project={project}
          card={current.card(dialog.card)}
          onClose={onClose}
          onActingLine={onActingLine}
          onDone={(answer) => {
            onWritten(answer, (id) => t("board.said.assigned", { id }), false);
          }}
        />
      );
    case "cancel":
      return (
        <CancelCardDialog
          project={project}
          card={current.card(dialog.card)}
          onClose={onClose}
          onActingLine={onActingLine}
          onDone={(answer) => {
            onWritten(answer, (id) => t("board.said.cancelled", { id }), false);
          }}
        />
      );
  }
}

/** What a line of the Move menu does: a move posted, Accept's dialog, or the Return dialog for the flow's return column. */
export interface BoardChoice extends MoveChoice {
  act: "move" | "accept" | "return";
  /** What a move posts; for Return, the column it goes to. */
  asked: Omit<CardMoveBody, "revision" | "as">;
  /** What the live region says; null: the column the answer names. */
  spoken: string | null;
}

/**
 * The Move menu's lines: the column's moves in served order, the flow's return column as "Return…",
 * then Up, Down, then Accept in the accept column unless a hub's `allowed` omits it. Looked up,
 * never judged: the daemon answers what is chosen.
 */
export function moveChoicesOf({
  row,
  flow,
  moves,
  reorder,
  lane,
  accepts,
  t,
}: {
  row: Pick<CardRow, "id" | "column">;
  flow: CardFlow | undefined;
  moves: readonly CardAllowed[];
  /** Up's and Down's places, null where the card has no neighbour. */
  reorder: { up: { before: string } | null; down: { before: string | null } | null; column: string } | null;
  /** The name a reorder is spoken in. */
  lane: string;
  accepts: boolean;
  t: Translate;
}): BoardChoice[] {
  const back = flow?.return ?? null;
  const choices: BoardChoice[] = moves.map(({ to, by }) =>
    to === back && row.column !== back
      ? { key: `return:${to}`, act: "return", label: t("board.return.menu"), detail: needsLine(by, t), icon: "cardReturned", asked: { to }, spoken: null }
      : { key: `to:${to}`, act: "move", label: columnTitle(flow, to), detail: needsLine(by, t), icon: "arrowRight", asked: { to }, spoken: null },
  );
  if (reorder !== null) {
    const { up, down, column } = reorder;
    if (up !== null) {
      choices.push({ key: "up", act: "move", label: t("board.menu.up"), detail: null, icon: "cardUp", asked: { before: up.before }, spoken: t("board.said.movedUp", { id: row.id, lane }) });
    }
    if (down !== null) {
      choices.push({
        key: "down",
        act: "move",
        label: t("board.menu.down"),
        detail: null,
        icon: "cardDown",
        asked: down.before === null ? { to: column } : { before: down.before },
        spoken: t("board.said.movedDown", { id: row.id, lane }),
      });
    }
  }
  if (row.column === flow?.accept.from && accepts) {
    choices.push({ key: "accept", act: "accept", label: t("board.menu.accept"), detail: null, icon: "taskAccepted", asked: {}, spoken: null });
  }
  return choices;
}

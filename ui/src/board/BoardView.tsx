import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useCard, useCards, useCardWrite, useTrace } from "../api/queries";
import type { Card, CardFlow, CardMoveBody, CardRow, CardWrite } from "../api/types";
import { pushHash } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { aboutOf } from "../app/sections";
import { useOpenShortcuts } from "../app/shortcuts";
import { useTranslate } from "../i18n";
import { useAnnounce } from "../ui/announcer";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { hasModifier, hotkeyOf, isTextField, letterOf } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { actingName, isActingLine, useActing, useAllowed, useHubLogin } from "./acting";
import { BoardCard, type CardActions } from "./BoardCard";
import { BoardColumn } from "./BoardColumn";
import { AcceptCardDialog, AssignDialog, CancelCardDialog, ClearCardDialog, EditCardDialog, NewCardDialog } from "./CardDialogs";
import { CardPanel, isCard, type CardAction } from "./CardPanel";
import { useToday } from "./due";
import {
  ANY,
  assigneeChoices,
  COLUMN_SHOWN,
  disciplineChoices,
  dropMove,
  lanesOf,
  matchesFilters,
  NO_FILTERS,
  openRows,
  reorderOf,
  standingBefore,
  tabsOf,
  workspaceTabs,
  type BoardFilters,
  type BoardLane,
} from "./filter";
import { columnTitle, movesFrom, needsLine, noCards, noteNames, rowMark } from "./labels";
import { MoveMenu, type MoveChoice } from "./MoveMenu";


/** What the Shell keeps per project for this page's life; `flow` null: nothing chosen yet. */
export interface BoardState {
  flow: string | null;
  filters: BoardFilters;
  withCancelled: boolean;
}

export interface BoardMemory {
  recall: (project: string) => BoardState | undefined;
  remember: (project: string, state: BoardState) => void;
}

/** No memory: each Board opens as new. */
export const NO_BOARD_MEMORY: BoardMemory = { recall: () => undefined, remember: () => undefined };

/** The dialog open, each with what it works on and who gets focus back. */
type OpenDialog =
  | { action: "new"; trigger: HTMLElement | null }
  | { action: "accept"; card: Pick<CardRow, "id" | "title" | "revision" | "assignee" | "reviewer">; trigger: HTMLElement | null }
  | { action: Exclude<CardAction, "accept">; card: Card; trigger: HTMLElement | null };

/** The Move menu open: the card's ID, where its button stands. */
interface OpenMenu {
  id: string;
  anchor: DOMRect;
}

/** A refused move: the daemon's reason; `count` a new alert for each. */
interface Refusal {
  message: string;
  count: number;
}

/** What a line of the Move menu does. */
interface BoardChoice extends MoveChoice {
  /** What it posts; null: Accept, which opens its dialog. */
  asked: Omit<CardMoveBody, "revision" | "as"> | null;
  /** What the live region says; null: the column the answer names. */
  spoken: string | null;
}

/** A card written, its re-read settled: focus goes to it once the board shows it in that column. */
interface Followed {
  id: string;
  column: string;
}

/** A filter's select; the two lists are the read's own values. */
function FilterSelect({
  label,
  value,
  every,
  choices,
  onChange,
}: {
  label: string;
  value: string;
  every: string;
  choices: readonly { key: string; label: string }[];
  onChange: (value: string) => void;
}) {
  const id = useId();
  return (
    <div className="board-tool">
      <label className="field-label" htmlFor={id}>
        {label}
      </label>
      <select
        id={id}
        className="field"
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        <option value={ANY}>{every}</option>
        {choices.map((choice) => (
          <option key={choice.key} value={choice.key}>
            {choice.label}
          </option>
        ))}
      </select>
    </div>
  );
}

/** The daemon's notes on the list. */
function Notes({ notes, label }: { notes: readonly string[]; label: string }) {
  return (
    notes.length > 0 && (
      <ul className="note-list" aria-label={label}>
        {notes.map((note, index) => (
          <li key={`${String(index)}-${note}`} className="verbatim-note">
            {note}
          </li>
        ))}
      </ul>
    )
  );
}

/**
 * The Board (`#/<project>/board[/<C>]`, docs/features/board-ui.md): an ordinary kanban, one board per
 * flow. Columns, moves and who makes them come from the read, and the UI looks them up and never
 * judges: each move is posted, redrawn from the daemon's read, refused in its words. The route's ID
 * selects a card, opens its panel and its flow. One tab stop over the cards: arrows move it, Enter
 * opens, `m` moves, `n` adds; Esc closes and puts focus back on the card; nothing is optimistic.
 * The tabs are the chosen workspace's flows, in its order; Show all flows shows the rest for the
 * visit, and a card the route names in one of them shows them too (docs/features/workspaces.md "The UI").
 */
export function BoardView({
  project,
  cardId,
  memory = NO_BOARD_MEMORY,
  flows = null,
  flowsKnown = true,
  lens = null,
}: {
  project: string;
  cardId: string | null;
  memory?: BoardMemory;
  /** The chosen workspace's flows, in its order; null: every flow. */
  flows?: readonly string[] | null;
  /** False while the workspaces are read: the board waits to draw its tabs, so none flash and go. */
  flowsKnown?: boolean;
  /** The chosen workspace's name, held only to tell a change: a new workspace shows its own flows again. */
  lens?: string | null;
}) {
  const t = useTranslate();
  const openShortcuts = useOpenShortcuts();
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const titleId = useId();
  const actingId = useId();
  const actingLineId = useId();
  const cancelledId = useId();
  const tabsBase = useId();
  const [remembered] = useState(() => memory.recall(project));
  const [chosenFlow, setChosenFlow] = useState<string | null>(remembered?.flow ?? null);
  const [filters, setFilters] = useState<BoardFilters>(remembered?.filters ?? NO_FILTERS);
  const [withCancelled, setWithCancelled] = useState(remembered?.withCancelled ?? false);
  /** Show all flows was pressed for this visit, under the workspace `shownFor` names. */
  const [allFlows, setAllFlows] = useState(false);
  const [shownFor, setShownFor] = useState(lens);
  if (shownFor !== lens) {
    setShownFor(lens);
    setAllFlows(false);
  }
  /** The route's ID whose flow has been opened: a new ID opens its flow once, then a tab is the owner's. */
  const [flowOpenedFor, setFlowOpenedFor] = useState<string | null>(null);
  const [roving, setRoving] = useState<string | null>(cardId);
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const [dialog, setDialog] = useState<OpenDialog | null>(null);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  /** The daemon's line on the acting name, shown under its field until the name changes or a write goes through. */
  const [actingLine, setActingLine] = useState<string | null>(null);
  const [writeNotes, setWriteNotes] = useState<readonly string[]>([]);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set());
  const [dragged, setDragged] = useState<CardRow | null>(null);
  const [dropAt, setDropAt] = useState<{ lane: string; before: string | null } | null>(null);
  const [acting, setActing] = useActing();
  const login = useHubLogin();
  const allows = useAllowed(project);
  const results = useRef<HTMLDivElement>(null);
  const board = useRef<HTMLDivElement>(null);
  const panelHeading = useRef<HTMLHeadingElement>(null);
  /** The ID whose panel takes focus once it shows. */
  const headingFor = useRef<string | null>(null);
  const followed = useRef<Followed | null>(null);
  /** The board's handlers as of the last commit, behind `cardActions`, which never changes. */
  const latest = useRef<Pick<CardActions, "onOpen" | "onMove"> | null>(null);
  const moving = useRef(false);
  const menuWasOpen = useRef(false);
  const refusals = useRef(0);
  const failedBefore = useRef(false);

  const cards = useCards(project, withCancelled ? { with_cancelled: true } : {});
  const panel = useCard(project, cardId);
  // The tiles' marks are the list's; only an open panel asks what its targets read, as the list on focus.
  const trace = useTrace(project, {}, cardId !== null, true);
  const write = useCardWrite(project);
  const today = useToday();
  const failure = useRetainedFailure(cards.error, cards.isFetching);
  const retried = useRetryFocus(cards.dataUpdatedAt === 0 ? undefined : cards.dataUpdatedAt, () => results.current);

  useEffect(() => {
    memory.remember(project, { flow: chosenFlow, filters, withCancelled });
  }, [memory, project, chosenFlow, filters, withCancelled]);

  const list = cards.data;
  // A read that failed again says no target's state: what it last said may no longer hold.
  const rollups = useMemo(() => new Map(trace.error === null ? (trace.data?.nodes.map((node) => [node.id, node.rollup]) ?? []) : []), [trace.data, trace.error]);
  /** The list could not be read: no tile, no mark, whatever the last read said. */
  const boardFailed = cards.error !== null;
  const all = useMemo(() => list?.cards ?? [], [list]);
  const everyTab = useMemo(() => (list === undefined ? [] : tabsOf(list)), [list]);
  const lensTabs = useMemo(() => workspaceTabs(everyTab, flows), [everyTab, flows]);
  /** Some flows are left out of the tabs by the workspace. */
  const flowsHidden = lensTabs.length < everyTab.length;
  const tabs = allFlows ? everyTab : lensTabs;
  const visible = useMemo(() => all.filter((row) => matchesFilters(row, filters)), [all, filters]);

  // A new ID in the route opens its flow, once its row or its card has said which.
  const routed = panel.data !== undefined && isCard(panel.data) ? panel.data.flow : (all.find((row) => row.id === cardId)?.flow ?? null);
  // Judged once the list is read too: before it the workspace's tabs are none, and every flow would look hidden.
  if (flowsKnown && list !== undefined && cardId !== flowOpenedFor && (cardId === null || routed !== null)) {
    setFlowOpenedFor(cardId);
    if (routed !== null) {
      setChosenFlow(routed);
      // A card of a flow the workspace leaves out is never shown on a board that hides it.
      if (!lensTabs.some((candidate) => candidate.name === routed)) {
        setAllFlows(true);
      }
    }
  }

  const panelRow = all.find((row) => row.id === cardId);
  const shownMark = panelRow === undefined ? null : rowMark(panelRow);
  const tab = tabs.find((candidate) => candidate.name === chosenFlow) ?? tabs[0];
  const flow = tab?.flow;
  const lanes = useMemo(() => (tab === undefined ? [] : lanesOf(tab, visible, t)), [tab, visible, t]);
  const wholeLanes = useMemo(() => (tab === undefined ? [] : lanesOf(tab, all, t)), [tab, all, t]);
  const shownLanes = useMemo(
    () => lanes.map((lane) => ({ ...lane, rows: expanded.has(lane.key) ? lane.rows : lane.rows.slice(0, COLUMN_SHOWN) })),
    [lanes, expanded],
  );
  const order = useMemo(() => shownLanes.flatMap((lane) => lane.rows), [shownLanes]);
  const rovingId = order.find((row) => row.id === roving)?.id ?? order.find((row) => row.id === cardId)?.id ?? order[0]?.id ?? null;
  const { discipline: chosenDiscipline, assignee: chosenAssignee } = filters;
  const disciplines = useMemo(() => disciplineChoices(all, chosenDiscipline), [all, chosenDiscipline]);
  const assignees = useMemo(() => assigneeChoices(all, chosenAssignee, t), [all, chosenAssignee, t]);
  const menuRow = menu === null ? undefined : all.find((row) => row.id === menu.id);
  // A card a column of the flow no longer holds: the flow lists no move from there, its read does.
  const menuOutside = menuRow !== undefined && !(flow?.columns.some((column) => column.id === menuRow.column) ?? false);
  const outsideRead = useCard(project, menuOutside ? menuRow.id : null);
  // A card read away while its menu is open takes the menu with it; a board that failed takes the menu and a drag.
  if (menu !== null && (menuRow === undefined || boardFailed)) {
    setMenu(null);
  }
  if (dragged !== null && boardFailed) {
    setDragged(null);
    setDropAt(null);
  }

  function cardElement(id: string): HTMLElement | undefined {
    const found = board.current?.querySelectorAll<HTMLElement>("[data-card]") ?? [];
    return Array.from(found).find((element) => element.dataset.card === id);
  }

  /** Where a written card landed, for focus; a 200 always names it. Called once its re-read has settled. */
  function follow({ id, column }: CardWrite) {
    if (id !== null && column !== null) {
      followed.current = { id, column };
    }
  }

  /**
   * Ends a follow, so it never waits to steal focus later: the card shown there takes it; with the
   * board's reads settled, a card past a column's cap reveals its column and takes it on the next
   * commit, and one not on this board (a filter hides it, another flow holds it, it is gone) lets go,
   * a shown card taking focus only if focus fell to the body.
   */
  function takeFollowed() {
    const wanted = followed.current;
    if (wanted === null) {
      return;
    }
    const element = cardElement(wanted.id);
    if (element?.dataset.column === wanted.column) {
      followed.current = null;
      element.focus();
      return;
    }
    if (cards.isFetching) {
      return;
    }
    const cutOff = element === undefined ? lanes.find((lane) => lane.rows.some((row) => row.id === wanted.id)) : undefined;
    if (cutOff !== undefined && !expanded.has(cutOff.key)) {
      setExpanded((keys) => new Set(keys).add(cutOff.key));
      return;
    }
    followed.current = null;
    if (focusIsLost()) {
      (element ?? cardElement(rovingId ?? "") ?? results.current)?.focus();
    }
  }

  // The panel asked for by Enter or a click takes focus once it shows; a written card once it landed;
  // a menu gone with its card leaves focus on a shown card.
  useEffect(() => {
    if (headingFor.current !== null && headingFor.current === cardId && panelHeading.current !== null) {
      headingFor.current = null;
      panelHeading.current.focus();
    }
    takeFollowed();
    if (menuWasOpen.current && menu === null && focusIsLost()) {
      (cardElement(rovingId ?? "") ?? results.current)?.focus();
    }
    menuWasOpen.current = menu !== null;
    // The tile that had focus went with the board.
    if (boardFailed && !failedBefore.current && focusIsLost()) {
      results.current?.focus();
    }
    failedBefore.current = boardFailed;
  });

  function openPanel(id: string) {
    if (id === cardId) {
      panelHeading.current?.focus();
      return;
    }
    headingFor.current = id;
    pushHash(sectionHash(project, "board", id));
  }

  function closePanel() {
    const back = cardId;
    pushHash(sectionHash(project, "board"));
    const element = back === null ? undefined : cardElement(back);
    (element ?? cardElement(rovingId ?? "") ?? results.current)?.focus();
  }

  function focusCard(id: string) {
    setRoving(id);
    focusLater(() => cardElement(id));
  }

  function openMenu(row: CardRow, button: HTMLElement | null) {
    if (moving.current || row.accepted || row.cancelled) {
      return;
    }
    const element = cardElement(row.id);
    const anchor = button ?? element?.querySelector<HTMLElement>("[data-card-move]") ?? element;
    if (anchor !== undefined) {
      setMenu({ id: row.id, anchor: anchor.getBoundingClientRect() });
    }
  }

  function closeMenu() {
    if (menu !== null) {
      cardElement(menu.id)?.focus();
    }
    setMenu(null);
  }

  /** The one way a card moves: from the menu, from a drop. The answer says where it landed; the board reads again. */
  async function move(row: CardRow, asked: Omit<CardMoveBody, "revision" | "as">, spoken: string | null) {
    if (moving.current) {
      return;
    }
    moving.current = true;
    setRefusal(null);
    try {
      const answer = await write.mutateAsync({ write: "move", id: row.id, body: { ...asked, revision: row.revision, as: actingName(acting) } });
      setWriteNotes(answer.notes);
      setActingLine(null);
      follow(answer);
      announce(spoken ?? `${row.id} moved to ${columnTitle(flow, answer.column ?? row.column)}`);
    } catch (error) {
      const answer = apiErrorOf(error);
      if (isActingLine(answer)) {
        setActingLine(answer.message);
      } else {
        refusals.current += 1;
        setRefusal({ message: answer.message, count: refusals.current });
      }
    } finally {
      moving.current = false;
    }
  }

  /** The menu's lines: the column's moves in served order, then Up, Down, then Accept in the accept column, unless a hub's `allowed` omits it. */
  function choicesFor(row: CardRow): BoardChoice[] {
    const read = outsideRead.data;
    const moves = menuOutside ? (read !== undefined && isCard(read) ? read.allowed : []) : movesFrom(flow, row.column);
    const choices: BoardChoice[] = moves.map(({ to, by }) => ({
      key: `to:${to}`,
      label: columnTitle(flow, to),
      detail: needsLine(by, t),
      icon: "arrowRight",
      asked: { to },
      spoken: null,
    }));
    const lane = lanes.find((candidate) => candidate.rows.some((other) => other.id === row.id));
    const whole = wholeLanes.find((candidate) => candidate.key === lane?.key);
    if (lane !== undefined && whole !== undefined && lane.column !== null) {
      const { up, down } = reorderOf(row.id, openRows(lane.rows), openRows(whole.rows));
      if (up !== null) {
        choices.push({ key: "up", label: "Up", detail: null, icon: "cardUp", asked: { before: up.before }, spoken: `${row.id} moved up in ${lane.title}` });
      }
      if (down !== null) {
        choices.push({
          key: "down",
          label: "Down",
          detail: null,
          icon: "cardDown",
          asked: down.before === null ? { to: lane.column } : { before: down.before },
          spoken: `${row.id} moved down in ${lane.title}`,
        });
      }
    }
    if (row.column === flow?.accept.from && allows("accept")) {
      choices.push({ key: "accept", label: "Accept", detail: null, icon: "taskAccepted", asked: null, spoken: null });
    }
    return choices;
  }

  /** A line of the menu taken: the menu closes, focus on the card, then the move goes out or Accept's dialog opens. */
  function takeChoice(row: CardRow, choice: BoardChoice) {
    closeMenu();
    if (choice.asked === null) {
      setDialog({ action: "accept", card: row, trigger: cardElement(row.id) ?? null });
    } else {
      void move(row, choice.asked, choice.spoken);
    }
  }

  function keyTarget(key: string, id: string): CardRow | undefined {
    const laneAt = shownLanes.findIndex((lane) => lane.rows.some((row) => row.id === id));
    const lane = shownLanes[laneAt];
    if (lane === undefined) {
      return undefined;
    }
    const at = lane.rows.findIndex((row) => row.id === id);
    switch (key) {
      case "ArrowDown":
        return lane.rows[at + 1];
      case "ArrowUp":
        return lane.rows[at - 1];
      case "Home":
        return lane.rows[0];
      case "End":
        return lane.rows.at(-1);
      default: {
        // Across: the nearest column holding a card, at the same height or its last.
        const step = key === "ArrowRight" ? 1 : -1;
        for (let next = laneAt + step; next >= 0 && next < shownLanes.length; next += step) {
          const rows = shownLanes[next]?.rows ?? [];
          if (rows.length > 0) {
            return rows[Math.min(at, rows.length - 1)];
          }
        }
        return undefined;
      }
    }
  }

  function onBoardKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (hasModifier(event) || isTextField(event.target) || !(event.target instanceof Element)) {
      return;
    }
    const element = event.target.closest<HTMLElement>("[data-card]");
    const id = element?.dataset.card;
    const row = order.find((candidate) => candidate.id === id);
    if (element === null || row === undefined) {
      return;
    }
    switch (hotkeyOf(event)) {
      case "ArrowDown":
      case "ArrowUp":
      case "ArrowLeft":
      case "ArrowRight":
      case "Home":
      case "End": {
        const target = keyTarget(event.key, row.id);
        if (target !== undefined) {
          focusCard(target.id);
        }
        break;
      }
      case "Enter":
        if (event.target !== element) {
          return;
        }
        openPanel(row.id);
        break;
      case "m":
        openMenu(row, null);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  function openNew(trigger: HTMLElement | null) {
    setDialog({ action: "new", trigger });
  }

  function onViewKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target) || dialog !== null || menu !== null) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    } else if (letterOf(event) === "n") {
      event.preventDefault();
      openNew(document.activeElement instanceof HTMLElement ? document.activeElement : null);
    } else if (event.key === "Escape" && cardId !== null) {
      event.preventDefault();
      closePanel();
    }
  }

  function onDragOver(lane: BoardLane, before: string | null) {
    const place = standingBefore(before, openRows(wholeLanes.find((candidate) => candidate.key === lane.key)?.rows ?? []));
    if (dropAt?.lane !== lane.key || dropAt.before !== place) {
      setDropAt({ lane: lane.key, before: place });
    }
  }

  function onDragLeave(lane: BoardLane) {
    if (dropAt?.lane === lane.key) {
      setDropAt(null);
    }
  }

  function onDrop(lane: BoardLane, before: string | null) {
    const row = dragged;
    setDragged(null);
    setDropAt(null);
    const whole = wholeLanes.find((candidate) => candidate.key === lane.key);
    if (row === null || lane.column === null || whole === undefined) {
      return;
    }
    const asked = dropMove(row, lane.column, before, openRows(whole.rows));
    if (asked !== null) {
      void move(row, asked, null);
    }
  }

  useEffect(() => {
    latest.current = { onOpen: openPanel, onMove: openMenu };
  });
  // One object for the board's life, so a memoised card redraws only when its own row or marks do.
  const cardActions = useMemo<CardActions>(
    () => ({
      onOpen: (id) => {
        latest.current?.onOpen(id);
      },
      onMove: (row, button) => {
        latest.current?.onMove(row, button);
      },
      onFocus: setRoving,
      onDragStart: (row) => {
        setRefusal(null);
        setDragged(row);
      },
      onDragEnd: () => {
        setDragged(null);
        setDropAt(null);
      },
    }),
    [],
  );

  function showAll(lane: BoardLane) {
    setExpanded((keys) => new Set(keys).add(lane.key));
    const next = lane.rows[COLUMN_SHOWN];
    if (next !== undefined) {
      focusCard(next.id);
    }
  }

  function closeDialog() {
    const back = dialog?.trigger;
    setDialog(null);
    focusLater(() => (back?.isConnected === true ? back : (panelHeading.current ?? results.current)));
  }

  /** A dialog's write went through: the board has read again. `toCard`: focus goes to the card, else back to the dialog's trigger. */
  function written(answer: CardWrite, spoken: string, toCard: boolean) {
    setRefusal(null);
    setActingLine(null);
    setWriteNotes(answer.notes);
    if (toCard) {
      follow(answer);
    }
    closeDialog();
    announce(`${answer.id ?? "The card"} ${spoken}`);
  }

  function onAct(action: CardAction, card: Card, trigger: HTMLElement) {
    setDialog({ action, card, trigger });
  }

  /** The card a dialog works on, as read now: its revision is what a submit goes on; what is sent stays what the person changed. */
  function currentCard(card: Card): Card {
    if (panel.data !== undefined && isCard(panel.data) && panel.data.id === card.id) {
      return panel.data;
    }
    return card;
  }

  function flowOf(name: string): CardFlow | undefined {
    return list?.flows.find((candidate) => candidate.name === name);
  }

  function retry() {
    retried();
    void cards.refetch({ cancelRefetch: false });
  }

  function clearFilters() {
    setFilters(NO_FILTERS);
    focusLater(() => cardElement(rovingId ?? "") ?? results.current);
  }

  let dialogShown: ReactNode = null;
  if (dialog !== null) {
    if (dialog.action === "new") {
      dialogShown = (
        <NewCardDialog
          project={project}
          onClose={closeDialog}
          onActingLine={setActingLine}
          onDone={(answer) => {
            written(answer, `created in ${columnTitle(flowOf(answer.flow ?? ""), answer.column ?? "")}`, true);
          }}
        />
      );
    } else if (dialog.action === "accept") {
      const row = all.find((candidate) => candidate.id === dialog.card.id);
      dialogShown = (
        <AcceptCardDialog
          project={project}
          card={row ?? dialog.card}
          onClose={closeDialog}
          onActingLine={setActingLine}
          onDone={(answer) => {
            // From the Move menu the trigger is the card, which has moved; from the panel, its Accept, gone with the write.
            written(answer, "is accepted", dialog.trigger?.dataset.card !== undefined);
          }}
        />
      );
    } else if (dialog.action === "edit") {
      dialogShown = (
        <EditCardDialog
          project={project}
          card={currentCard(dialog.card)}
          onClose={closeDialog}
          onActingLine={setActingLine}
          onDone={(answer) => {
            written(answer, "is saved", false);
          }}
        />
      );
    } else if (dialog.action === "clear") {
      dialogShown = (
        <ClearCardDialog
          project={project}
          card={currentCard(dialog.card)}
          on={shownMark?.on ?? null}
          onClose={closeDialog}
          onActingLine={setActingLine}
          onDone={(answer) => {
            written(answer, "is cleared", false);
          }}
        />
      );
    } else if (dialog.action === "assign") {
      dialogShown = (
        <AssignDialog
          project={project}
          card={currentCard(dialog.card)}
          onClose={closeDialog}
          onActingLine={setActingLine}
          onDone={(answer) => {
            written(answer, "is assigned", false);
          }}
        />
      );
    } else {
      dialogShown = (
        <CancelCardDialog
          project={project}
          card={currentCard(dialog.card)}
          onClose={closeDialog}
          onActingLine={setActingLine}
          onDone={(answer) => {
            written(answer, "is cancelled", false);
          }}
        />
      );
    }
  }

  let body;
  if (list === undefined || tab === undefined || !flowsKnown) {
    body =
      (list === undefined || !flowsKnown) && failure === null ? (
        <div aria-busy="true">
          <Skeleton label={`Reading the cards of ${project}`} lines={6} />
        </div>
      ) : (
        <ErrorPanel
          title="The board could not be read"
          message={failure === null ? "The daemon served no flow." : apiErrorOf(failure).message}
          retrying={cards.isFetching}
          attempt={cards.errorUpdateCount}
          onRetry={retry}
        />
      );
  } else {
    const filtered = filters.discipline !== ANY || filters.assignee !== ANY;
    // A lane takes the notes that name it; the rest stand over the board.
    const laneNotes = (lane: BoardLane) =>
      lane.column !== null ? [] : list.notes.filter((note) => lane.rows.some((row) => noteNames(note, row.flow, row.column)));
    const placedNotes = new Set(lanes.flatMap(laneNotes));
    const boardBody = (
      <div
        ref={board}
        className="board-columns"
        role="group"
        aria-label={tabs.length > 1 ? `Board ${tab.name}` : "Board"}
        onKeyDown={onBoardKeyDown}
      >
        {shownLanes.map((lane, index) => {
          const notes = laneNotes(lanes[index] ?? lane);
          return (
            <BoardColumn
              key={lane.key}
              lane={lanes[index] ?? lane}
              expanded={expanded.has(lane.key)}
              dropping={dragged !== null}
              dropAtEnd={dropAt?.lane === lane.key && dropAt.before === null}
              note={<Notes notes={notes} label="Notes from the daemon on this lane" />}
              onShowAll={showAll}
              onDragOver={onDragOver}
              onDragLeave={onDragLeave}
              onDrop={onDrop}
            >
              {lane.rows.map((row) => (
                <BoardCard
                  key={row.id}
                  project={project}
                  row={row}
                  today={today}
                  tabbable={row.id === rovingId}
                  selected={row.id === cardId}
                  menuOpen={menu?.id === row.id}
                  dropAbove={dropAt?.lane === lane.key && dropAt.before === row.id}
                  actions={cardActions}
                />
              ))}
            </BoardColumn>
          );
        })}
      </div>
    );
    body = (
      <div className={cardId === null ? "board-stage" : "board-stage board-stage-panel"}>
        <div className="board-main">
          {!boardFailed && <Notes notes={list.notes.filter((note) => !placedNotes.has(note))} label="Notes from the daemon" />}
          <Notes notes={writeNotes} label="Notes from the daemon on the last write" />
          {!boardFailed && list.truncated && (
            <p className="note">
              <Icon name="info" />
              <span>{list.omitted ?? 0} cards not shown.</span>
            </p>
          )}
          {refusal !== null && (
            <div key={refusal.count} className="refusal" role="alert">
              <p className="refusal-title">
                <Icon name="alert" />
                <span>The daemon did not take it.</span>
              </p>
              <p className="verbatim">{refusal.message}</p>
            </div>
          )}
          {cards.error !== null ? (
            // No tile drawn on the last read: a card may be suspect now, and a clean tile would say it is not.
            <ErrorPanel title="The board could not be read" message={apiErrorOf(cards.error).message} retrying={cards.isFetching} attempt={cards.errorUpdateCount} onRetry={retry} />
          ) : (
            <>
              {list.cards.length === 0 && (
                <div className="empty-state">
                  <p>{noCards(t)}</p>
                </div>
              )}
              {all.length > 0 && visible.length === 0 && (
                <div className="empty-filter">
                  <p>No card matches the filters.</p>
                  {filtered && (
                    <button type="button" className="button" onClick={clearFilters}>
                      Clear filters
                    </button>
                  )}
                </div>
              )}
              {flowsHidden && (
                <div className="board-flows-line">
                  {!allFlows && (
                    <p className="note">
                      <Icon name="info" />
                      <span>{t("workspace.flowsShown", { shown: lensTabs.length, count: everyTab.length })}</span>
                    </p>
                  )}
                  <button
                    type="button"
                    className="button button-quiet"
                    onClick={() => {
                      setAllFlows(!allFlows);
                    }}
                  >
                    {allFlows ? t("workspace.showWorkspaceFlows") : t("workspace.showAllFlows")}
                  </button>
                </div>
              )}
              {tabs.length > 1 ? (
                <>
                  <Tabs
                    label="Boards"
                    base={tabsBase}
                    selected={tab.name}
                    onSelect={setChosenFlow}
                    tabs={tabs.map((candidate) => ({ id: candidate.name, label: <bdi>{candidate.name}</bdi> }))}
                  />
                  {tabs.map((candidate) => (
                    <TabPanel key={candidate.name} base={tabsBase} id={candidate.name} selected={candidate.name === tab.name}>
                      {candidate.name === tab.name && boardBody}
                    </TabPanel>
                  ))}
                </>
              ) : (
                boardBody
              )}
              <p className="muted board-hint">Arrow keys move between cards. Enter opens one, m moves it, n adds a card, ? lists every key.</p>
            </>
          )}
        </div>
        {cardId !== null && (
          <RegionBoundary key={cardId} name="card" heading={cardId}>
            <CardPanel
              project={project}
              id={cardId}
              query={panel}
              flowOf={flowOf}
              today={today}
              mark={boardFailed ? null : shownMark}
              rollups={rollups}
              markFailure={cards.error === null ? null : apiErrorOf(cards.error).message}
              stateFailure={trace.error === null ? null : apiErrorOf(trace.error).message}
              headingRef={panelHeading}
              onAct={onAct}
              onClose={closePanel}
            />
          </RegionBoundary>
        )}
      </div>
    );
  }

  const tools = list !== undefined && (
    <div className="board-tools">
      <button
        type="button"
        className="button button-primary"
        onClick={(event) => {
          openNew(event.currentTarget);
        }}
      >
        <Icon name="cardNew" />
        <span>New card</span>
      </button>
      <FilterSelect
        label="Discipline"
        value={filters.discipline}
        every="All disciplines"
        choices={disciplines}
        onChange={(discipline) => {
          setFilters((current) => ({ ...current, discipline }));
        }}
      />
      <FilterSelect
        label="Assignee"
        value={filters.assignee}
        every="Everyone"
        choices={assignees}
        onChange={(assignee) => {
          setFilters((current) => ({ ...current, assignee }));
        }}
      />
      <div className="board-tool board-tool-check">
        <input
          id={cancelledId}
          type="checkbox"
          checked={withCancelled}
          onChange={(event) => {
            setWithCancelled(event.target.checked);
          }}
        />
        <label htmlFor={cancelledId}>Show cancelled</label>
      </div>
      <div className="board-tool board-acting">
        <label className="field-label" htmlFor={actingId}>
          Acting as
        </label>
        <input
          id={actingId}
          className={login === null ? "field" : "field field-fixed"}
          type="text"
          autoComplete="off"
          spellCheck={false}
          value={acting}
          readOnly={login !== null}
          aria-invalid={actingLine !== null}
          aria-describedby={actingLine === null ? undefined : actingLineId}
          onChange={(event) => {
            setActingLine(null);
            setActing(event.target.value);
          }}
        />
        {actingLine !== null && (
          <p id={actingLineId} className="field-error" role="alert">
            {actingLine}
          </p>
        )}
      </div>
    </div>
  );

  return (
    <section className="view board-view" aria-labelledby={titleId} onKeyDown={onViewKeyDown}>
      <header className="view-head">
        <h1 id={titleId} tabIndex={-1}>
          Board
        </h1>
        <p className="view-about">{aboutOf("board", t)}</p>
      </header>
      {tools}
      <div ref={results} tabIndex={-1} className="board-results" aria-busy={cards.isFetching}>
        <RegionBoundary name="board">{body}</RegionBoundary>
      </div>
      {menu !== null && menuRow !== undefined && (
        <MoveMenu
          label={`Move ${menuRow.id}`}
          anchor={menu.anchor}
          choices={choicesFor(menuRow)}
          empty="No move from here."
          onChoose={(choice) => {
            takeChoice(menuRow, choice);
          }}
          onClose={closeMenu}
        />
      )}
      {dialogShown}
    </section>
  );
}

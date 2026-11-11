import { Fragment, useCallback, useEffect, useId, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useCard, useCards, useCardWrite, useCardWriting, useTrace } from "../api/queries";
import type { Card, CardAllowed, CardFlow, CardMoveBody, CardRow, CardWrite } from "../api/types";
import { pushHash, replaceHash } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { aboutOf } from "../app/sections";
import { useOpenShortcuts } from "../app/shortcuts";
import { useTranslate } from "../i18n";
import { useAnnounce } from "../ui/announcer";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { hasModifier, hotkeyOf, isTextField, letterOf } from "../ui/keys";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { shownSlot } from "../ui/text";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { useViewTop } from "../ui/useViewTop";
import { actingName, isActingLine, useActing, useAllowed, useHubLogin } from "./acting";
import { BoardCard, type CardActions } from "./BoardCard";
import { BoardColumn, BoardColumnHeader } from "./BoardColumn";
import { statusLook } from "./category";
import { CardActDialog, moveChoicesOf, type BoardChoice, type OpenDialog, type Said } from "./CardActs";
import { CardModal } from "./CardModal";
import { CardView, isCard, titleOf, type CardAction, type CardLinks } from "./CardPanel";
import { useToday } from "./due";
import {
  ANY,
  assigneeChoices,
  cellsOf,
  columnHeadsOf,
  COLUMN_SHOWN,
  disciplineChoices,
  dropMove,
  flowTitleOf,
  matchesFilters,
  NO_FILTERS,
  openRows,
  reorderOf,
  standingBefore,
  tabsOf,
  workspaceTabs,
  type BoardColumnCell,
  type BoardFilters,
} from "./filter";
import { movesFrom, noCards, noteNames, rowMark } from "./labels";
import { MoveMenu } from "./MoveMenu";
import { rollupsOf, type Rollup } from "./rollup";
import { LANE_MODES, recallLaneMode, rememberLaneMode, swimlanesOf, type LaneMode, type Swimlane } from "./swimlanes";

/** What the Shell keeps per project for this page's life; `flow` null: nothing chosen yet. */
export interface BoardState {
  flow: string | null;
  filters: BoardFilters;
  withCancelled: boolean;
  /** The lanes folded, by key: they stay folded through re-reads for the visit, never stored. */
  collapsed: readonly string[];
}

export interface BoardMemory {
  recall: (project: string) => BoardState | undefined;
  remember: (project: string, state: BoardState) => void;
}

/** No memory: each Board opens as new. */
export const NO_BOARD_MEMORY: BoardMemory = { recall: () => undefined, remember: () => undefined };

/** Where a Move menu was opened: a tile on the board, or the card's modal. */
type MenuPlace = "board" | "modal";

/** The Move menu open: the card's ID, where its button stands, who gets focus back. */
interface OpenMenu {
  id: string;
  anchor: DOMRect;
  trigger: HTMLElement | null;
  place: MenuPlace;
}

/** A refused move: the daemon's reason; `count` a new alert for each; said where the move was made. */
interface Refusal {
  message: string;
  count: number;
  place: MenuPlace;
}

/** A card written, its re-read settled: focus goes to it once the board shows it in that column. */
interface Followed {
  id: string;
  column: string;
}

/** No child: a tile counts nothing. */
const NO_ROLLUP: Rollup = { children: [], returned: 0 };

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
          <li key={`${String(index)}-${note}`} className="verbatim-note" lang="en">
            {note}
          </li>
        ))}
      </ul>
    )
  );
}

/** A notice's Dismiss: it goes, the grid stays where it is. */
function Dismiss({ onDismiss }: { onDismiss: () => void }) {
  const t = useTranslate();
  return (
    <button type="button" className="button button-quiet board-notice-dismiss" onClick={onDismiss}>
      <Icon name="rejected" />
      <span>{t("board.view.dismiss")}</span>
    </button>
  );
}

/** A plain click with the primary button; a modified one is the browser's. */
function isPlainClick(event: MouseEvent<HTMLElement>): boolean {
  return event.button === 0 && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
}

/**
 * A swimlane's head, spanning the grid, its label kept in view as the board scrolls sideways: what
 * the lane holds and a button folding it; a parent lane also names the parent, a link opening its
 * card, and where it stands, whichever flow that is: the flow's name and the column's lozenge.
 */
function LaneHead({
  project,
  lane,
  id,
  flowOf,
  collapsed,
  onToggle,
  onOpenCard,
}: {
  project: string;
  lane: Swimlane;
  id: string;
  /** A served flow by its name: a parent's lozenge looks its column up there, whichever flow that is. */
  flowOf: (name: string) => CardFlow | undefined;
  collapsed: boolean;
  onToggle: () => void;
  onOpenCard: (id: string) => void;
}) {
  const t = useTranslate();
  const { head } = lane;
  let label: ReactNode;
  if (head.cut === "assignee") {
    label = <bdi>{shownSlot(head.name)}</bdi>;
  } else if (head.cut === "unassigned") {
    label = t("board.lanes.unassigned");
  } else if (head.cut === "parent") {
    label = <bdi>{shownSlot(head.card?.title ?? head.id)}</bdi>;
  } else {
    label = t("board.lanes.everythingElse");
  }
  return (
    <div className="board-lane-head">
      <div className="board-lane-label">
        <h2 id={id} className="board-lane-title">
          <button type="button" className="board-lane-toggle" aria-expanded={!collapsed} onClick={onToggle}>
            <Icon name={collapsed ? "laneClosed" : "laneOpen"} />
            <span>{label}</span>
          </button>
        </h2>
        {head.cut === "parent" && (
          <>
            <a
              className="mono"
              href={sectionHash(project, "board", head.id)}
              onClick={(event) => {
                if (isPlainClick(event)) {
                  event.preventDefault();
                  onOpenCard(head.id);
                }
              }}
            >
              {shownSlot(head.id)}
            </a>
            {head.card !== undefined && (
              <span className="board-lane-status">
                <bdi className="muted">{shownSlot(flowTitleOf(flowOf(head.card.flow), head.card.flow))}</bdi>
                <Lozenge {...statusLook(flowOf(head.card.flow), head.card.column)} />
              </span>
            )}
          </>
        )}
        <span className="muted board-lane-count">{t("board.lanes.count", { count: lane.count })}</span>
      </div>
    </div>
  );
}

/**
 * The Board (`#/<project>/board[/<C>]`, `ui/README.md` "Screen rules"): an ordinary kanban, one
 * board per flow, that holds still. Columns, moves, accept and return come from the read, and the UI
 * looks them up and never judges: each move is posted, redrawn from the daemon's read, refused in its
 * words. One grid holds the sticky header row and every swimlane, so a cell stands under its head;
 * notices lie over the board's bottom edge, never in its flow, so no column header moves on a write,
 * a refusal, a filter, a lane or a re-read. The route's ID opens the card as a modal over the board,
 * its flow chosen once; opening from inside never switches the tab beneath. One tab stop over the
 * cards: arrows move it, Page Down and Page Up cross lanes, Enter opens, `m` moves, `h` hands off,
 * `r` returns, `n` adds; nothing is optimistic. The tabs are the chosen workspace's flows, in its
 * order; Show all flows shows the rest for the visit.
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
  const lanesId = useId();
  const tabsBase = useId();
  const headBase = useId();
  const [remembered] = useState(() => memory.recall(project));
  const [chosenFlow, setChosenFlow] = useState<string | null>(remembered?.flow ?? null);
  const [filters, setFilters] = useState<BoardFilters>(remembered?.filters ?? NO_FILTERS);
  const [withCancelled, setWithCancelled] = useState(remembered?.withCancelled ?? false);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set(remembered?.collapsed ?? []));
  const [laneMode, setLaneMode] = useState<LaneMode>(() => recallLaneMode(project));
  /** Show all flows was pressed for this visit, under the workspace `shownFor` names. */
  const [allFlows, setAllFlows] = useState(false);
  const [shownFor, setShownFor] = useState(lens);
  if (shownFor !== lens) {
    setShownFor(lens);
    setAllFlows(false);
  }
  /** The route's ID whose flow has been opened: a new ID opens its flow once, then a tab is the owner's. */
  const [flowOpenedFor, setFlowOpenedFor] = useState<string | null>(null);
  /** The ID last opened from inside the board (a tile, a lane's head, a link in the modal): its route switches no tab. */
  const [openedInside, setOpenedInside] = useState<string | null>(null);
  const [roving, setRoving] = useState<string | null>(cardId);
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const [dialog, setDialog] = useState<OpenDialog | null>(null);
  /** Where the open dialog was asked from: one over the modal goes with it. */
  const [dialogPlace, setDialogPlace] = useState<MenuPlace>("board");
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  /** What a write made in the modal said, shown in it, the board being inert beneath: the daemon's line on the acting name, its notes. */
  const [modalLine, setModalLine] = useState<string | null>(null);
  const [modalNotes, setModalNotes] = useState<readonly string[]>([]);
  /** The daemon's line on the acting name, shown under its field until the name changes or a write goes through. */
  const [actingLine, setActingLine] = useState<string | null>(null);
  const [writeNotes, setWriteNotes] = useState<readonly string[]>([]);
  /** The list's notes as last dismissed: they stay gone until the daemon says something else. */
  const [notesDismissed, setNotesDismissed] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set());
  const [dragged, setDragged] = useState<CardRow | null>(null);
  const [dropAt, setDropAt] = useState<{ cell: string; before: string | null } | null>(null);
  const [acting, setActing] = useActing();
  const login = useHubLogin();
  const allows = useAllowed(project);
  const writing = useCardWriting(project);
  /** The card the modal shows: the route's, held while a write is out, so Back takes effect once it settles. */
  const [shownId, setShownId] = useState<string | null>(cardId);
  if (shownId !== cardId && !writing) {
    setShownId(cardId);
  }
  /** The card the modal's own state belongs to: another card, or none, takes its refusal, lines, notes, menu and dialog away. */
  const [modalStateFor, setModalStateFor] = useState(shownId);
  if (modalStateFor !== shownId) {
    setModalStateFor(shownId);
    setModalLine(null);
    setModalNotes([]);
    if (refusal?.place === "modal") {
      setRefusal(null);
    }
    if (menu?.place === "modal") {
      setMenu(null);
    }
    if (dialog !== null && dialogPlace === "modal") {
      setDialog(null);
    }
    if (shownId === null) {
      setOpenedInside(null);
    }
  }
  const view = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const results = useRef<HTMLDivElement>(null);
  const board = useRef<HTMLDivElement>(null);
  const followed = useRef<Followed | null>(null);
  /** The board's handlers as of the last commit, behind `cardActions`, which never changes. */
  const latest = useRef<Pick<CardActions, "onOpen" | "onMove"> | null>(null);
  const moving = useRef(false);
  const menuWasOpen = useRef(false);
  const refusals = useRef(0);
  const failedBefore = useRef(false);
  /** This board pushed the modal's history entry: closing goes back, else it replaces. */
  const pushed = useRef(false);
  /** The tile the modal was opened from: focus goes back to it, wherever it stands then. */
  const openedFrom = useRef<string | null>(null);
  const lastShown = useRef<string | null>(shownId);

  useViewTop(view, "board-top");

  const cards = useCards(project, withCancelled ? { with_cancelled: true } : {});
  const panel = useCard(project, shownId);
  // The tiles' marks are the list's; only an open card asks what its targets read, as the list on focus.
  const trace = useTrace(project, {}, shownId !== null, true);
  const write = useCardWrite(project);
  const today = useToday();
  const failure = useRetainedFailure(cards.error, cards.isFetching);
  const retried = useRetryFocus(cards.dataUpdatedAt === 0 ? undefined : cards.dataUpdatedAt, () => results.current);
  // The scroller keeps a focused tile out from under its sticky header row: its padding is the row's
  // height, measured whenever a header row mounts (a new tab, a stage or a grid drawn again) and resizes.
  const measureHead = useCallback((head: HTMLDivElement | null) => {
    const scroller = head?.closest<HTMLElement>(".board-region");
    if (head === null || scroller === null || scroller === undefined) {
      return undefined;
    }
    const measure = () => {
      scroller.style.setProperty("--board-head-height", `${String(Math.ceil(head.getBoundingClientRect().height))}px`);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(head);
    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    memory.remember(project, { flow: chosenFlow, filters, withCancelled, collapsed: [...collapsed] });
  }, [memory, project, chosenFlow, filters, withCancelled, collapsed]);

  const list = cards.data;
  // A read that failed again says no target's state: what it last said may no longer hold.
  const rollups = useMemo(() => new Map(trace.error === null ? (trace.data?.nodes.map((node) => [node.id, node.rollup]) ?? []) : []), [trace.data, trace.error]);
  /** The list could not be read: no tile, no mark, whatever the last read said. */
  const boardFailed = cards.error !== null;
  const all = useMemo(() => list?.cards ?? [], [list]);
  const handOffs = useMemo(() => rollupsOf(all, list?.flows ?? []), [all, list]);
  const everyTab = useMemo(() => (list === undefined ? [] : tabsOf(list)), [list]);
  const lensTabs = useMemo(() => workspaceTabs(everyTab, flows), [everyTab, flows]);
  /** Some flows are left out of the tabs by the workspace. */
  const flowsHidden = lensTabs.length < everyTab.length;
  const tabs = allFlows ? everyTab : lensTabs;
  const visible = useMemo(() => all.filter((row) => matchesFilters(row, filters)), [all, filters]);

  // A new ID in the route opens its flow, once its row or its card has said which.
  const routed = panel.data !== undefined && isCard(panel.data) && panel.data.id === cardId ? panel.data.flow : (all.find((row) => row.id === cardId)?.flow ?? null);
  // Judged once the list is read too: before it the workspace's tabs are none, and every flow would look hidden.
  if (flowsKnown && list !== undefined && cardId !== flowOpenedFor && (cardId === null || routed !== null)) {
    setFlowOpenedFor(cardId);
    // Arriving by URL opens the card's flow; a card opened from inside leaves the tab beneath as it is.
    if (routed !== null && cardId !== openedInside) {
      setChosenFlow(routed);
      // A card of a flow the workspace leaves out is never shown on a board that hides it.
      if (!lensTabs.some((candidate) => candidate.name === routed)) {
        setAllFlows(true);
      }
    }
  }

  const shownRow = all.find((row) => row.id === shownId);
  const shownMark = shownRow === undefined ? null : rowMark(shownRow);
  const tab = tabs.find((candidate) => candidate.name === chosenFlow) ?? tabs[0];
  const flow = tab?.flow;
  // The heads come from every standing row of the flow, so no filter, lane or Show cancelled moves them; the lanes from the rows shown.
  const heads = useMemo(() => (tab === undefined ? [] : columnHeadsOf(tab, openRows(all), t)), [tab, all, t]);
  const headCells = useMemo(() => (tab === undefined ? [] : cellsOf(tab, heads, visible)), [tab, heads, visible]);
  const wholeCells = useMemo(() => (tab === undefined ? [] : cellsOf(tab, heads, all)), [tab, heads, all]);
  const lanes = useMemo(() => (tab === undefined ? [] : swimlanesOf(tab, heads, visible, laneMode, all)), [tab, heads, visible, laneMode, all]);
  const shownLanes = useMemo(
    () =>
      lanes.map((lane) => ({
        ...lane,
        cells: lane.cells.map((cell) => ({ ...cell, rows: collapsed.has(lane.key) ? [] : expanded.has(cell.key) ? cell.rows : cell.rows.slice(0, COLUMN_SHOWN) })),
      })),
    [lanes, expanded, collapsed],
  );
  const order = useMemo(() => shownLanes.flatMap((lane) => lane.cells.flatMap((cell) => cell.rows)), [shownLanes]);
  const rovingId = order.find((row) => row.id === roving)?.id ?? order.find((row) => row.id === cardId)?.id ?? order[0]?.id ?? null;
  const { discipline: chosenDiscipline, assignee: chosenAssignee } = filters;
  const disciplines = useMemo(() => disciplineChoices(all, chosenDiscipline), [all, chosenDiscipline]);
  const assignees = useMemo(() => assigneeChoices(all, chosenAssignee, t), [all, chosenAssignee, t]);
  const menuRow = menu === null ? undefined : all.find((row) => row.id === menu.id);
  const menuFlow = menuRow === undefined ? undefined : flowOf(menuRow.flow);
  // A card a column of the flow no longer holds: the flow lists no move from there, its read does.
  const menuOutside = menuRow !== undefined && !(menuFlow?.columns.some((column) => column.id === menuRow.column) ?? false);
  const outsideRead = useCard(project, menuOutside && menuRow.id !== shownId ? menuRow.id : null);
  // A card read away while its menu is open takes the menu with it; a board that failed takes the menu and a drag.
  if (menu !== null && (menuRow === undefined || boardFailed)) {
    setMenu(null);
  }
  if (dragged !== null && boardFailed) {
    setDragged(null);
    setDropAt(null);
  }

  function flowOf(name: string): CardFlow | undefined {
    return list?.flows.find((candidate) => candidate.name === name);
  }

  function cardElement(id: string): HTMLElement | undefined {
    const found = board.current?.querySelectorAll<HTMLElement>("[data-card]") ?? [];
    return Array.from(found).find((element) => element.dataset.card === id);
  }

  /** The modal's heading, when it is open. */
  function modalHeading(): HTMLElement | null {
    return document.querySelector<HTMLElement>(".card-modal .dialog-title");
  }

  /** A column's standing rows across every lane, shown or not: where a drop or a reorder places a card. */
  function standingIn(column: string): CardRow[] {
    return openRows(wholeCells.find((cell) => cell.column === column)?.rows ?? []);
  }

  /** Where a written card landed, for focus; a 200 always names it. Called once its re-read has settled. */
  function follow({ id, column }: CardWrite) {
    if (id !== null && column !== null) {
      followed.current = { id, column };
    }
  }

  /**
   * Ends a follow, so it never waits to steal focus later: the card shown there takes it; with the
   * board's reads settled, a card past a cell's cap reveals its cell and takes it on the next
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
    const cutOff = element === undefined ? lanes.flatMap((lane) => (collapsed.has(lane.key) ? [] : lane.cells)).find((cell) => cell.rows.some((row) => row.id === wanted.id)) : undefined;
    if (cutOff !== undefined && !expanded.has(cutOff.key)) {
      setExpanded((keys) => new Set(keys).add(cutOff.key));
      return;
    }
    followed.current = null;
    if (focusIsLost()) {
      (element ?? cardElement(rovingId ?? "") ?? results.current)?.focus();
    }
  }

  // A closed modal gives focus back to the tile it was opened from, in its column now, else to the
  // board's heading; a written card takes it once it landed; a menu gone with its card leaves focus
  // on a shown card.
  useEffect(() => {
    const was = lastShown.current;
    lastShown.current = shownId;
    if (was !== null && shownId === null) {
      const back = openedFrom.current ?? was;
      openedFrom.current = null;
      pushed.current = false;
      // A tile taking focus takes the roving tab stop too.
      (cardElement(back) ?? heading.current)?.focus();
    } else if (shownId !== was && shownId !== null && shownId !== openedInside) {
      // Another card named from outside the board (a link, Forward): its entry is not this board's to go back from.
      pushed.current = false;
      openedFrom.current = null;
    }
    if (shownId === null) {
      takeFollowed();
    }
    if (menuWasOpen.current && menu === null && focusIsLost()) {
      (modalHeading() ?? cardElement(rovingId ?? "") ?? results.current)?.focus();
    }
    menuWasOpen.current = menu !== null;
    // The tile that had focus went with the board.
    if (boardFailed && !failedBefore.current && focusIsLost()) {
      results.current?.focus();
    }
    failedBefore.current = boardFailed;
  });

  /** A card opened from the board: one history entry, its tab kept. */
  function openCard(id: string) {
    if (writing) {
      return;
    }
    if (id === shownId) {
      modalHeading()?.focus();
      return;
    }
    openedFrom.current ??= id;
    setOpenedInside(id);
    pushed.current = true;
    pushHash(sectionHash(project, "board", id));
  }

  /** A card link inside the modal: the entry replaced, the tab beneath kept. */
  function followCard(id: string) {
    if (writing) {
      return;
    }
    setOpenedInside(id);
    replaceHash(sectionHash(project, "board", id));
  }

  /** Esc, Close and the scrim: back when this board pushed the entry, else the board's own hash in its place. */
  function closeCard() {
    if (writing) {
      return;
    }
    if (pushed.current) {
      pushed.current = false;
      window.history.back();
    } else {
      replaceHash(sectionHash(project, "board"));
    }
  }

  const modalLinks: CardLinks = { project, page: false, onFollow: followCard };

  function focusCard(id: string) {
    setRoving(id);
    focusLater(() => cardElement(id));
  }

  function openMenu(row: CardRow, button: HTMLElement | null, place: MenuPlace) {
    if (moving.current || writing || row.accepted || row.cancelled) {
      return;
    }
    const element = cardElement(row.id);
    const anchor = button ?? element?.querySelector<HTMLElement>("[data-card-move]") ?? element;
    if (anchor !== undefined) {
      setMenu({ id: row.id, anchor: anchor.getBoundingClientRect(), trigger: button ?? element ?? null, place });
    }
  }

  /** The menu closes: focus back on its tile on the board, on its button in the modal. */
  function closeMenu() {
    if (menu !== null) {
      const back = menu.place === "modal" && menu.trigger?.isConnected === true ? menu.trigger : cardElement(menu.id);
      back?.focus();
    }
    setMenu(null);
  }

  /** The one way a card moves: from a menu, from a drop. The answer says where it landed; the board reads again. */
  async function move(row: CardRow, asked: Omit<CardMoveBody, "revision" | "as">, spoken: string | null, place: MenuPlace) {
    if (moving.current) {
      return;
    }
    moving.current = true;
    setRefusal(null);
    try {
      const answer = await write.mutateAsync({ write: "move", id: row.id, body: { ...asked, revision: row.revision, as: actingName(acting) } });
      setActingLine(null);
      setModalLine(null);
      if (place === "board") {
        setWriteNotes(answer.notes);
        follow(answer);
      } else {
        setModalNotes(answer.notes);
      }
      announce(spoken ?? t("board.said.moved", { id: row.id, column: columnTitleOf(flowOf(answer.flow ?? row.flow), answer.column ?? row.column) }));
    } catch (error) {
      const answer = apiErrorOf(error);
      if (isActingLine(answer)) {
        (place === "modal" ? setModalLine : setActingLine)(answer.message);
      } else {
        refusals.current += 1;
        setRefusal({ message: answer.message, count: refusals.current, place });
      }
    } finally {
      moving.current = false;
    }
  }

  /** A column's title in the flow that holds it; one the flow no longer lists raw. */
  function columnTitleOf(holder: CardFlow | undefined, column: string): string {
    return holder?.columns.find((candidate) => candidate.id === column)?.title ?? column;
  }

  /** The cell a card is drawn in on this board, if it is. */
  function cellOf(id: string): BoardColumnCell | undefined {
    return shownLanes.flatMap((lane) => lane.cells).find((cell) => cell.rows.some((row) => row.id === id));
  }

  /** The menu's lines for a row: its flow's moves (a card a column left behind, its read's), Return…, Up and Down where it is drawn, Accept. */
  function choicesFor(row: CardRow): BoardChoice[] {
    const read = row.id === shownId ? panel.data : outsideRead.data;
    const moves: readonly CardAllowed[] = menuOutside ? (read !== undefined && isCard(read) ? read.allowed : []) : movesFrom(menuFlow, row.column);
    const cell = cellOf(row.id);
    const reorder =
      cell === undefined || cell.column === null
        ? null
        : { ...reorderOf(row.id, openRows(cell.rows), standingIn(cell.column)), column: cell.column };
    return moveChoicesOf({ row, flow: menuFlow, moves, reorder, lane: cell?.title ?? "", accepts: allows("accept"), t });
  }

  /** A line of the menu taken: the menu closes, focus on its trigger, then the move goes out or Accept's or Return's dialog opens. */
  function takeChoice(row: CardRow, choice: BoardChoice, opened: OpenMenu) {
    closeMenu();
    const trigger = opened.trigger?.isConnected === true ? opened.trigger : (cardElement(row.id) ?? null);
    if (choice.act === "accept") {
      openDialog({ action: "accept", card: row, trigger }, opened.place);
    } else if (choice.act === "return") {
      openDialog({ action: "return", card: row, to: choice.asked.to ?? row.column, before: null, trigger }, opened.place);
    } else {
      void move(row, choice.asked, choice.spoken, opened.place);
    }
  }

  /** Whether `r` on a tile returns it: its flow names a return column it is not in, and the flow lists the move there. */
  function returnOf(row: CardRow): string | null {
    const back = flowOf(row.flow)?.return ?? null;
    if (back === null || row.column === back || row.accepted || row.cancelled) {
      return null;
    }
    return movesFrom(flowOf(row.flow), row.column).some((candidate) => candidate.to === back) ? back : null;
  }

  function keyTarget(key: string, id: string): CardRow | undefined {
    const laneAt = shownLanes.findIndex((lane) => lane.cells.some((cell) => cell.rows.some((row) => row.id === id)));
    const lane = shownLanes[laneAt];
    const cellAt = lane?.cells.findIndex((cell) => cell.rows.some((row) => row.id === id)) ?? -1;
    const cell = lane?.cells[cellAt];
    if (lane === undefined || cell === undefined) {
      return undefined;
    }
    const at = cell.rows.findIndex((row) => row.id === id);
    const filled = lane.cells.filter((candidate) => candidate.rows.length > 0);
    switch (key) {
      case "ArrowDown":
        return cell.rows[at + 1];
      case "ArrowUp":
        return cell.rows[at - 1];
      case "Home":
        return filled[0]?.rows[0];
      case "End":
        return filled.at(-1)?.rows.at(-1);
      case "PageDown":
      case "PageUp": {
        // The same column in the next or previous lane drawing a card there; none: focus stays.
        const step = key === "PageDown" ? 1 : -1;
        for (let next = laneAt + step; next >= 0 && next < shownLanes.length; next += step) {
          const rows = shownLanes[next]?.cells[cellAt]?.rows ?? [];
          if (rows.length > 0) {
            return rows[Math.min(at, rows.length - 1)];
          }
        }
        return undefined;
      }
      default: {
        // Across, in the lane: the nearest column holding a card, at the same height or its last.
        const step = key === "ArrowRight" ? 1 : -1;
        for (let next = cellAt + step; next >= 0 && next < lane.cells.length; next += step) {
          const rows = lane.cells[next]?.rows ?? [];
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
    const key = hotkeyOf(event);
    switch (key) {
      case "ArrowDown":
      case "ArrowUp":
      case "ArrowLeft":
      case "ArrowRight":
      case "Home":
      case "End":
      case "PageDown":
      case "PageUp": {
        const target = keyTarget(key, row.id);
        if (target !== undefined) {
          focusCard(target.id);
        }
        break;
      }
      case "Enter":
        if (event.target !== element) {
          return;
        }
        openCard(row.id);
        break;
      case "m":
        openMenu(row, null, "board");
        break;
      case "h":
        if (row.accepted || row.cancelled) {
          return;
        }
        openDialog({ action: "handoff", card: row, trigger: element }, "board");
        break;
      case "r": {
        const back = returnOf(row);
        if (back === null) {
          return;
        }
        openDialog({ action: "return", card: row, to: back, before: null, trigger: element }, "board");
        break;
      }
      default:
        return;
    }
    event.preventDefault();
  }

  /** A dialog opens, over the board or over the modal. */
  function openDialog(next: OpenDialog, place: MenuPlace) {
    setDialog(next);
    setDialogPlace(place);
  }

  function openNew(trigger: HTMLElement | null) {
    openDialog({ action: "new", trigger }, "board");
  }

  function onViewKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target) || dialog !== null || menu !== null) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    } else if (letterOf(event) === "n" && shownId === null) {
      event.preventDefault();
      openNew(document.activeElement instanceof HTMLElement ? document.activeElement : null);
    }
  }

  function onDragOver(cell: BoardColumnCell, before: string | null) {
    const place = standingBefore(before, cell.column === null ? [] : standingIn(cell.column));
    if (dropAt?.cell !== cell.key || dropAt.before !== place) {
      setDropAt({ cell: cell.key, before: place });
    }
  }

  function onDragLeave(cell: BoardColumnCell) {
    if (dropAt?.cell === cell.key) {
      setDropAt(null);
    }
  }

  /** A drop sends its cell's column and nothing else: a lane neither assigns nor re-parents. Into the return column from another, the Return dialog asks why first. */
  function onDrop(cell: BoardColumnCell, before: string | null) {
    const row = dragged;
    setDragged(null);
    setDropAt(null);
    if (row === null || cell.column === null) {
      return;
    }
    const asked = dropMove(row, cell.column, before, standingIn(cell.column));
    if (asked === null) {
      return;
    }
    const back = flowOf(row.flow)?.return ?? null;
    if (cell.column === back && row.column !== back) {
      openDialog({ action: "return", card: row, to: cell.column, before: asked.before ?? null, trigger: cardElement(row.id) ?? null }, "board");
      return;
    }
    void move(row, asked, null, "board");
  }

  useEffect(() => {
    latest.current = {
      onOpen: openCard,
      onMove: (row, button) => {
        openMenu(row, button, "board");
      },
    };
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

  function showAll(cell: BoardColumnCell) {
    setExpanded((keys) => new Set(keys).add(cell.key));
    const next = cell.rows[COLUMN_SHOWN];
    if (next !== undefined) {
      focusCard(next.id);
    }
  }

  function toggleLane(key: string) {
    setCollapsed((keys) => {
      const next = new Set(keys);
      if (!next.delete(key)) {
        next.add(key);
      }
      return next;
    });
  }

  function chooseLanes(mode: LaneMode) {
    setLaneMode(mode);
    rememberLaneMode(project, mode);
  }

  function closeDialog() {
    const back = dialog?.trigger;
    setDialog(null);
    focusLater(() => (back?.isConnected === true ? back : (modalHeading() ?? results.current)));
  }

  /** A dialog's write went through: the board has read again. `toCard`: focus goes to the card, else back to the dialog's trigger. */
  function written(answer: CardWrite, said: Said, toCard: boolean) {
    setRefusal(null);
    setActingLine(null);
    setModalLine(null);
    if (dialogPlace === "modal") {
      setModalNotes(answer.notes);
    } else {
      setWriteNotes(answer.notes);
    }
    if (toCard && shownId === null) {
      follow(answer);
    }
    closeDialog();
    announce(said(answer.id ?? t("board.said.theCard")));
  }

  /** A question or a decision raised on the card: nothing moved, nothing to follow; said in the live region. */
  function raised(said: string) {
    setRefusal(null);
    setActingLine(null);
    setModalLine(null);
    closeDialog();
    announce(said);
  }

  /** An act of the card's modal: the Move menu at its button, or the act's dialog. */
  function onAct(action: CardAction, card: Card, trigger: HTMLElement) {
    if (writing) {
      return;
    }
    if (action === "move") {
      const row = all.find((candidate) => candidate.id === card.id);
      if (row !== undefined) {
        openMenu(row, trigger, "modal");
      }
    } else if (action === "return") {
      const back = flowOf(card.flow)?.return ?? null;
      if (back !== null) {
        openDialog({ action: "return", card, to: back, before: null, trigger }, "modal");
      }
    } else {
      openDialog({ action, card, trigger }, "modal");
    }
  }

  /** The card a dialog works on, as read now: its revision is what a submit goes on; what is sent stays what the person changed. */
  function currentCard(card: Card): Card {
    if (panel.data !== undefined && isCard(panel.data) && panel.data.id === card.id) {
      return panel.data;
    }
    return card;
  }

  function retry() {
    retried();
    void cards.refetch({ cancelRefetch: false });
  }

  function clearFilters() {
    setFilters(NO_FILTERS);
    focusLater(() => cardElement(rovingId ?? "") ?? results.current);
  }

  const dialogShown =
    dialog === null ? null : (
      <CardActDialog
        project={project}
        dialog={dialog}
        flowOf={flowOf}
        flows={list?.flows ?? []}
        current={{ row: (id) => all.find((candidate) => candidate.id === id), card: currentCard }}
        clearOn={shownMark?.on ?? null}
        t={t}
        onClose={closeDialog}
        onActingLine={dialogPlace === "modal" ? setModalLine : setActingLine}
        onWritten={written}
        onRaised={raised}
      />
    );

  let body;
  if (list === undefined || tab === undefined || !flowsKnown) {
    body =
      (list === undefined || !flowsKnown) && failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("board.view.reading", { project })} lines={6} />
        </div>
      ) : (
        <ErrorPanel
          title={t("board.view.failed")}
          message={failure === null ? t("board.view.noFlow") : apiErrorOf(failure).message}
          retrying={cards.isFetching}
          attempt={cards.errorUpdateCount}
          onRetry={retry}
        />
      );
  } else {
    const filtered = filters.discipline !== ANY || filters.assignee !== ANY;
    // A cell outside the flow takes the notes that name its cards; the rest lie over the board.
    const cellNotes = (cell: BoardColumnCell) => (cell.column !== null ? [] : list.notes.filter((note) => cell.rows.some((row) => noteNames(note, row.flow, row.column))));
    const placedNotes = new Set(lanes.flatMap((lane) => lane.cells.flatMap(cellNotes)));
    const listNotes = boardFailed ? [] : list.notes.filter((note) => !placedNotes.has(note));
    const listNotesKey = listNotes.join("\n");
    const grid = (
      <div
        ref={board}
        className={lanes.length === 1 && lanes[0]?.head.cut === "all" ? "board-grid board-grid-single" : "board-grid"}
        role="group"
        aria-label={tabs.length > 1 ? t("board.view.boardOf", { name: flowTitleOf(tab.flow, tab.name) }) : t("board.view.title")}
        style={{ "--board-columns": String(heads.length) } as CSSProperties}
        onKeyDown={onBoardKeyDown}
      >
        <div ref={measureHead} className="board-head">
          {heads.map((head, index) => (
            <BoardColumnHeader key={head.key} head={head} flow={flow} id={`${headBase}-head-${String(index)}`} count={headCells[index]?.count ?? 0} />
          ))}
        </div>
        {shownLanes.map((lane, laneIndex) => {
          const laneId = `${headBase}-lane-${String(laneIndex)}`;
          const folded = collapsed.has(lane.key);
          const whole = lanes[laneIndex] ?? lane;
          return (
            <Fragment key={lane.key}>
              {lane.head.cut !== "all" && (
                <LaneHead
                  project={project}
                  lane={lane}
                  id={laneId}
                  flowOf={flowOf}
                  collapsed={folded}
                  onToggle={() => {
                    toggleLane(lane.key);
                  }}
                  onOpenCard={openCard}
                />
              )}
              {lane.cells.map((cell, index) => (
                <BoardColumn
                  key={cell.key}
                  cell={whole.cells[index] ?? cell}
                  flow={flow}
                  labelledBy={lane.head.cut === "all" ? `${headBase}-head-${String(index)}` : `${headBase}-head-${String(index)} ${laneId}`}
                  collapsed={folded}
                  expanded={expanded.has(cell.key)}
                  dropping={dragged !== null}
                  dropAtEnd={dropAt?.cell === cell.key && dropAt.before === null}
                  note={<Notes notes={cellNotes(whole.cells[index] ?? cell)} label={t("board.view.laneNotes")} />}
                  onShowAll={showAll}
                  onDragOver={onDragOver}
                  onDragLeave={onDragLeave}
                  onDrop={onDrop}
                >
                  {cell.rows.map((row) => {
                    const rollup = handOffs.get(row.id) ?? NO_ROLLUP;
                    return (
                      <BoardCard
                        key={row.id}
                        project={project}
                        row={row}
                        today={today}
                        tabbable={row.id === rovingId}
                        selected={row.id === shownId}
                        menuOpen={menu?.id === row.id && menu.place === "board"}
                        dropAbove={dropAt?.cell === cell.key && dropAt.before === row.id}
                        returned={flow !== undefined && flow.return !== null && row.column === flow.return}
                        handOffs={rollup.children.length}
                        returnedChildren={rollup.returned}
                        actions={cardActions}
                      />
                    );
                  })}
                </BoardColumn>
              ))}
            </Fragment>
          );
        })}
      </div>
    );
    const stage = (
      <div className="board-stage">
        <div className="board-region">
          {cards.error !== null ? (
            // No tile drawn on the last read: a card may be suspect now, and a clean tile would say it is not.
            <ErrorPanel title={t("board.view.failed")} message={apiErrorOf(cards.error).message} retrying={cards.isFetching} attempt={cards.errorUpdateCount} onRetry={retry} />
          ) : (
            grid
          )}
        </div>
        <div className="board-notices">
          {listNotes.length > 0 && notesDismissed !== listNotesKey && (
            <div className="board-notice">
              <Notes notes={listNotes} label={t("board.view.notes")} />
              <Dismiss
                onDismiss={() => {
                  setNotesDismissed(listNotesKey);
                }}
              />
            </div>
          )}
          {!boardFailed && list.truncated && (
            <p className="note board-notice">
              <Icon name="info" />
              <span>{t("board.view.omitted", { count: list.omitted ?? 0 })}</span>
            </p>
          )}
          {!boardFailed && list.cards.length === 0 && (
            <div className="empty-state board-notice">
              <p>{noCards(t)}</p>
            </div>
          )}
          {!boardFailed && all.length > 0 && visible.length === 0 && (
            <div className="empty-filter board-notice">
              <p>{t("board.view.noMatch")}</p>
              {filtered && (
                <button type="button" className="button" onClick={clearFilters}>
                  {t("board.view.clearFilters")}
                </button>
              )}
            </div>
          )}
          {refusal !== null && refusal.place === "board" && (
            <div key={refusal.count} className="refusal board-notice" role="alert">
              <p className="refusal-title">
                <Icon name="alert" />
                <span>{t("board.view.refused")}</span>
              </p>
              <p className="verbatim" lang="en">
                {refusal.message}
              </p>
              <Dismiss
                onDismiss={() => {
                  setRefusal(null);
                }}
              />
            </div>
          )}
          <div role="status">
            {writeNotes.length > 0 && (
              <div className="board-notice">
                <Notes notes={writeNotes} label={t("board.view.writeNotes")} />
                <Dismiss
                  onDismiss={() => {
                    setWriteNotes([]);
                  }}
                />
              </div>
            )}
          </div>
        </div>
      </div>
    );
    body = (
      <div className="board-frame">
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
              label={t("board.view.boards")}
              base={tabsBase}
              selected={tab.name}
              onSelect={setChosenFlow}
              tabs={tabs.map((candidate) => ({ id: candidate.name, label: <bdi>{flowTitleOf(candidate.flow, candidate.name)}</bdi> }))}
            />
            {tabs.map((candidate) => (
              <TabPanel key={candidate.name} base={tabsBase} id={candidate.name} selected={candidate.name === tab.name}>
                {candidate.name === tab.name && stage}
              </TabPanel>
            ))}
          </>
        ) : (
          stage
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
        <span>{t("board.view.newCard")}</span>
      </button>
      <FilterSelect
        label={t("board.view.discipline")}
        value={filters.discipline}
        every={t("board.view.allDisciplines")}
        choices={disciplines}
        onChange={(discipline) => {
          setFilters((current) => ({ ...current, discipline }));
        }}
      />
      <FilterSelect
        label={t("board.view.assignee")}
        value={filters.assignee}
        every={t("board.view.everyone")}
        choices={assignees}
        onChange={(assignee) => {
          setFilters((current) => ({ ...current, assignee }));
        }}
      />
      <div className="board-tool">
        <label className="field-label" htmlFor={lanesId}>
          {t("board.lanes.label")}
        </label>
        <select
          id={lanesId}
          className="field"
          value={laneMode}
          onChange={(event) => {
            const chosen = LANE_MODES.find((mode) => mode === event.target.value);
            if (chosen !== undefined) {
              chooseLanes(chosen);
            }
          }}
        >
          {LANE_MODES.map((mode) => (
            <option key={mode} value={mode}>
              {t(`board.lanes.${mode}`)}
            </option>
          ))}
        </select>
      </div>
      <div className="board-tool board-tool-check">
        <input
          id={cancelledId}
          type="checkbox"
          checked={withCancelled}
          onChange={(event) => {
            setWithCancelled(event.target.checked);
          }}
        />
        <label htmlFor={cancelledId}>{t("board.view.showCancelled")}</label>
      </div>
      <div className="board-tool board-acting">
        <label className="field-label" htmlFor={actingId}>
          {t("board.view.actingAs")}
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
          <p id={actingLineId} className="field-error" role="alert" lang="en">
            {actingLine}
          </p>
        )}
      </div>
    </div>
  );

  const menuShown = (place: MenuPlace) =>
    menu !== null &&
    menu.place === place &&
    menuRow !== undefined && (
      <MoveMenu
        label={t("board.menu.label", { id: menuRow.id })}
        anchor={menu.anchor}
        choices={choicesFor(menuRow)}
        empty={t("board.menu.empty")}
        onChoose={(choice) => {
          takeChoice(menuRow, choice, menu);
        }}
        onClose={closeMenu}
      />
    );

  return (
    <section ref={view} className="view board-view" aria-labelledby={titleId} onKeyDown={onViewKeyDown}>
      <header className="view-head">
        <h1 ref={heading} id={titleId} tabIndex={-1}>
          {t("board.view.title")}
        </h1>
        <p className="view-about">{aboutOf("board", t)}</p>
        <p className="muted board-hint">{t("board.view.hint")}</p>
      </header>
      {tools}
      <div ref={results} tabIndex={-1} className="board-results" aria-busy={cards.isFetching}>
        <RegionBoundary name={t("board.view.boundaryBoard")}>{body}</RegionBoundary>
      </div>
      {menuShown("board")}
      {shownId !== null && (
        <CardModal
          key={shownId}
          project={project}
          id={shownId}
          title={titleOf(panel)}
          pending={writing}
          onClose={closeCard}
          view={({ heading, tools }) => (
            <CardView
              heading={heading}
              tools={tools}
              notices={
                <>
                  {refusal !== null && refusal.place === "modal" && (
                    <div key={refusal.count} className="refusal" role="alert">
                      <p className="refusal-title">
                        <Icon name="alert" />
                        <span>{t("board.view.refused")}</span>
                      </p>
                      <p className="verbatim" lang="en">
                        {refusal.message}
                      </p>
                    </div>
                  )}
                  {modalLine !== null && (
                    // The acting name's field is on the board, inert beneath: its line is said here.
                    <p className="field-error" role="alert" lang="en">
                      {modalLine}
                    </p>
                  )}
                  <div role="status">{modalNotes.length > 0 && <Notes notes={modalNotes} label={t("board.view.writeNotes")} />}</div>
                </>
              }
              project={project}
              id={shownId}
              query={panel}
              flowOf={flowOf}
              today={today}
              mark={boardFailed ? null : shownMark}
              rollups={rollups}
              markFailure={cards.error === null ? null : apiErrorOf(cards.error).message}
              stateFailure={trace.error === null ? null : apiErrorOf(trace.error).message}
              links={modalLinks}
              onAct={onAct}
            />
          )}
        >
          {menuShown("modal")}
        </CardModal>
      )}
      {dialogShown}
    </section>
  );
}

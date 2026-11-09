import { lazy, Suspense, useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useFlow, useFlowGroups, useTraceSummary, useTreeOnDemand } from "../api/queries";
import type { FlowNode, FlowView as FlowAnswer, TreeNode } from "../api/types";
import { pushHash } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { aboutOf } from "../app/sections";
import { useOpenShortcuts } from "../app/shortcuts";
import { nodeName } from "../graph/layout";
import { leftOutText } from "../tree/LinksPanel";
import { useAnnounce } from "../ui/announcer";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import type { BoxCommand, Centring } from "./FlowCanvas";
import { FlowDetails } from "./FlowDetails";
import { FlowGaps, GapsClear, recallGapsHidden, rememberGapsHidden } from "./FlowGaps";
import { FlowLegend } from "./FlowLegend";
import { FlowList } from "./FlowList";
import { useTranslate } from "../i18n";
import { allClear, namedByRows, notDrawnLine, readCount, statesHiddenWords, typesOf } from "./labels";
import { boxId, isGroup, layoutFlow, readsOf, type FlowBox, type FlowInput } from "./layout";
import { TimelineView } from "./TimelineView";
import { ALL_STAGES, stageInForce, stageOptions, withoutStates, type StageOption } from "./timeline";

/** The canvas and its drawing library load with the first answer drawn, not with the app. */
const FlowCanvas = lazy(() => import("./FlowCanvas").then((module) => ({ default: module.FlowCanvas })));


export type FlowTab = "canvas" | "list" | "timeline";

/** What the Shell keeps per project for this page's life; null: nothing chosen yet. */
export interface FlowState {
  tab: FlowTab | null;
  /** The module root the open groups were read under, null for the whole project: another root's groups never reopen. */
  root: string | null;
  open: readonly string[] | null;
  /** The stage filter's value (src/flow/timeline.ts). */
  stage: string;
  /** The Timeline's open lanes, by `stageKey`. */
  lanes: readonly string[];
}

export interface FlowMemory {
  recall: (project: string) => FlowState | undefined;
  remember: (project: string, state: FlowState) => void;
}

/** No memory: each Flow opens as new. */
export const NO_FLOW_MEMORY: FlowMemory = { recall: () => undefined, remember: () => undefined };

/** Under this width the List comes first. */
const NARROW = "(max-width: 48rem)";

/** jsdom has no `matchMedia`: never narrow there. */
function isNarrow(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia(NARROW).matches;
}

/** A sole top group starts open: it is the whole picture, and folded it would say nothing. */
function soleGroupOf(top: FlowAnswer | undefined): string[] {
  const groups = (top?.nodes ?? []).filter(isGroup);
  return groups.length === 1 ? groups.map(nodeName) : [];
}

/** Whether a known box is a group to open. */
function isClosedBox(node: FlowNode | undefined): boolean {
  return node !== undefined && isGroup(node);
}

/** A name's `parent` chain in the tree, top down, the name itself left out; null when no row holds it. */
function chainOf(rows: readonly TreeNode[], name: string): string[] | null {
  const byName = new Map(rows.map((row) => [row.id ?? row.path, row]));
  let at = byName.get(name);
  if (at === undefined) {
    return null;
  }
  const chain: string[] = [];
  while (at !== undefined && at.parent !== null && !chain.includes(at.parent)) {
    chain.unshift(at.parent);
    at = byName.get(at.parent);
  }
  return chain;
}

/** A box of a read, and whether it sits in a group's lane. */
interface ShownBox {
  node: FlowNode;
  inLane: boolean;
}

/** A name to bring into the picture: one object per asking, so a gap item can ask again. */
interface Seek {
  name: string;
}

/** What the on-demand tree said for a seek. */
interface Chain {
  seek: Seek;
  /** Top down; null: the tree holds no such name. */
  names: string[] | null;
}

/** Why the tree could not be read for a seek. */
interface ChainFailure {
  seek: Seek;
  message: string;
}

/** The box a panel shows: one of the reads shown; a name whose group was closed keeps its panel, its read still in hand. */
function panelBox(name: string | null, nodes: ReadonlyMap<string, ShownBox>, answers: ReadonlyMap<string, FlowAnswer>): ShownBox | undefined {
  if (name === null) {
    return undefined;
  }
  const shown = nodes.get(name);
  if (shown !== undefined) {
    return shown;
  }
  for (const answer of answers.values()) {
    const node = answer.nodes.find((candidate) => nodeName(candidate) === name && candidate.group !== null);
    if (node !== undefined) {
      return { node, inLane: true };
    }
  }
  return undefined;
}

/** The select over the section: its options are the served lanes', and it compares nothing else. */
function StageFilter({ value, options, onChange }: { value: string; options: readonly StageOption[]; onChange: (value: string) => void }) {
  const id = useId();
  return (
    <div className="flow-stage-filter">
      <label className="field-label" htmlFor={id}>
        Stage
      </label>
      <select
        id={id}
        className="field"
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}

/** What the reads say besides their boxes: the daemon's notes, each once, what was left out. */
function TopNotes({ top, notes }: { top: FlowAnswer; notes: readonly string[] }) {
  const leftOut = leftOutText(top.left_out);
  return (
    <>
      {notes.length > 0 && (
        <ul className="note-list" aria-label="Notes from the daemon">
          {notes.map((note, index) => (
            <li key={`${String(index)}-${note}`}>{note}</li>
          ))}
        </ul>
      )}
      {leftOut !== null && (
        <p className="note">
          <Icon name="info" />
          <span>{leftOut}</span>
        </p>
      )}
    </>
  );
}

/**
 * A group whose read failed: the line in the daemon's words and Retry. A read with no answer loses
 * its error while it retries, so the failure is kept up, and the Retry that has focus with it, until
 * the read settles. A retry that worked takes the button away: `onSettled` then hands focus on.
 */
function GroupFailure({
  name,
  error,
  fetching,
  settled,
  onRetry,
  onSettled,
}: {
  name: string;
  error: Error | null;
  fetching: boolean;
  /** Changes each time the read settles, whatever its answer. */
  settled: string;
  onRetry: () => void;
  onSettled: (name: string) => void;
}) {
  const failure = useRetainedFailure(error, fetching);
  /** What `settled` was when Retry was pressed. */
  const retriedAt = useRef<string | null>(null);
  useEffect(() => {
    if (retriedAt.current === null || retriedAt.current === settled || fetching) {
      return;
    }
    retriedAt.current = null;
    if (error === null) {
      onSettled(name);
    }
  });
  if (failure === null) {
    return null;
  }
  return (
    <div className="flow-failed">
      <p className="note" role="alert">
        <Icon name="alert" />
        <span>
          <span className="mono">{name}</span> could not be read: {apiErrorOf(failure).message}
        </span>
      </p>
      <button
        type="button"
        className="button"
        aria-disabled={fetching}
        onClick={() => {
          if (!fetching) {
            retriedAt.current = settled;
            onRetry();
          }
        }}
      >
        <Icon name="retry" />
        <span>
          {fetching ? "Retrying" : "Retry"} <span className="sr-only">reading {name}</span>
        </span>
      </button>
    </div>
  );
}

/**
 * The Flow (`#/<project>/flow[/<name>]`, docs/features/flow-canvas.md): the daemon's flow as a
 * diagram running forward, groups folding into one box. Every read carries a depth: Top at 0, a
 * group `root=G&depth=1`, so a whole-flow read is never made and the UI lifts nothing. Each box
 * carries its served rollup, a closed group its counts, and a third tab reads the stages
 * (docs/features/trace-views.md): nothing here counts a card. The name in the route only selects.
 * The spec is read only here, its changes going through the AI as proposals; a card is written from
 * the panel, a queue row. Under a module the Top read is rooted at its root
 * (docs/features/workspaces.md "The UI"); every other read is as without one.
 */
export function FlowView({
  project,
  nodeRef,
  memory = NO_FLOW_MEMORY,
  moduleRoot = null,
  moduleKnown = true,
}: {
  project: string;
  nodeRef: string | null;
  memory?: FlowMemory;
  /** The chosen module's root, null for the whole project. */
  moduleRoot?: string | null;
  /** False while the workspaces are read: the Top read waits for its root (docs/features/workspaces.md WS-9). */
  moduleKnown?: boolean;
}) {
  const t = useTranslate();
  const openShortcuts = useOpenShortcuts();
  const announce = useAnnounce();
  const titleId = useId();
  const tabsBase = useId();
  const [remembered] = useState(() => memory.recall(project));
  const [narrow] = useState(isNarrow);
  const [chosenTab, setChosenTab] = useState<FlowTab | null>(remembered?.tab ?? null);
  const [chosenOpen, setChosenOpen] = useState<readonly string[] | null>(remembered !== undefined && remembered.root === moduleRoot ? remembered.open : null);
  const [chosenStage, setChosenStage] = useState(remembered?.stage ?? ALL_STAGES);
  const [openLanes, setOpenLanes] = useState<readonly string[]>(remembered?.lanes ?? []);
  /** Groups closed since they were read: their reads stay, so reopening reads nothing. */
  const [closed, setClosed] = useState<readonly string[]>([]);
  const [roving, setRoving] = useState<string | null>(null);
  const [marks, setMarks] = useState<readonly string[]>([]);
  const [centring, setCentring] = useState<Centring | null>(null);
  /** The route's name last sought, so a route change seeks once. */
  const [soughtRef, setSoughtRef] = useState<string | null>(null);
  const [seek, setSeek] = useState<Seek | null>(null);
  const [chain, setChain] = useState<Chain | null>(null);
  const [chainFailure, setChainFailure] = useState<ChainFailure | null>(null);
  /** The name a finished seek did not find: a note until the route or a gap item asks for another. */
  const [lostName, setLostName] = useState<string | null>(null);
  const [gapsHidden, setGapsHidden] = useState(recallGapsHidden);
  const results = useRef<HTMLDivElement>(null);
  const detailsHeading = useRef<HTMLHeadingElement>(null);
  const focusBox = useRef<(id: string) => void>(() => undefined);
  /** A box's name whose panel takes focus once it shows. */
  const headingFor = useRef<string | null>(null);
  /** The last commit drew the gap rows' button, and the rows unless hidden. */
  const rowsBefore = useRef(false);

  const topRead = useFlow(project, moduleRoot, moduleKnown);
  const failure = useRetainedFailure(topRead.error, topRead.isFetching);
  // Keyed on the read settling, not on its data: an equal answer keeps its object (structural sharing).
  const retried = useRetryFocus(topRead.dataUpdatedAt === 0 ? undefined : topRead.dataUpdatedAt, () => results.current);
  const readTree = useTreeOnDemand(project);
  const summary = useTraceSummary(project);
  const retriedStages = useRetryFocus(summary.dataUpdatedAt === 0 ? undefined : summary.dataUpdatedAt, () => results.current);
  const top = topRead.data;
  const tab = chosenTab ?? (narrow ? "list" : "canvas");
  const open = useMemo(() => chosenOpen ?? soleGroupOf(top), [chosenOpen, top]);
  const opened = useMemo(() => new Set(open), [open]);
  const asked = useMemo(() => [...new Set([...open, ...closed])], [open, closed]);
  const groups = useFlowGroups(project, asked, opened);

  useEffect(() => {
    // Not while the module is unknown: this mount's root is a guess, and a module's open groups would be lost.
    if (moduleKnown) {
      memory.remember(project, { tab: chosenTab, root: moduleRoot, open: chosenOpen, stage: chosenStage, lanes: openLanes });
    }
  }, [memory, project, moduleKnown, chosenTab, moduleRoot, chosenOpen, chosenStage, openLanes]);

  // A read that failed again keeps its picture, and says no state it can no longer vouch for.
  const answers = useMemo(() => new Map(groups.flatMap((group) => (group.data === undefined ? [] : [[group.name, group.error === null ? group.data : withoutStates(group.data)] as const]))), [groups]);
  const topFailed = topRead.error !== null;
  /** Top's rows have nothing to say; never on a failed read, or with no tools to hold the line. */
  const clear = top !== undefined && top.nodes.length > 0 && !topFailed && allClear(top.gaps);
  const input = useMemo<FlowInput | null>(() => (top === undefined ? null : { top: topFailed ? withoutStates(top) : top, groups: answers, open: opened }), [top, topFailed, answers, opened]);
  const reads = useMemo(() => (input === null ? [] : readsOf(input)), [input]);
  const notes = useMemo(() => [...new Set(reads.flatMap((read) => read.view.notes))], [reads]);
  const layout = useMemo(() => (input === null ? null : layoutFlow(input)), [input]);
  const types = useMemo(() => (top === undefined ? [] : typesOf(top)), [top]);
  const named = useMemo(() => namedByRows(topFailed ? [] : (top?.gaps ?? [])), [top, topFailed]);
  const marked = useMemo(() => new Set(nodeRef !== null && marks.includes(nodeRef) ? marks : []), [marks, nodeRef]);
  const stages = useMemo(() => summary.data?.stages ?? [], [summary.data]);
  const stageShown = stageInForce(chosenStage, stages);
  const opening = useMemo(() => new Set(groups.filter((group) => group.data === undefined && group.error === null && opened.has(group.name)).map((group) => group.name)), [groups, opened]);
  const refused = useMemo(() => groups.filter((group) => group.data !== undefined && group.data.reason !== null), [groups]);

  /** The boxes of the reads shown, drawn or past the cap: a name here needs no chain. */
  const nodes = useMemo(() => {
    const found = new Map<string, ShownBox>();
    for (const read of reads) {
      for (const node of [...read.boxes].reverse()) {
        found.set(nodeName(node), { node, inLane: read.group !== null });
      }
    }
    return found;
  }, [reads]);
  const hasTop = top !== undefined;
  const reading = opening.size > 0;
  const drawnOpen = useMemo(() => new Set(layout?.boxes.filter((box) => box.form === "open").map((box) => box.name) ?? []), [layout]);

  // A group read with a `reason` closes; its note stays.
  const refusedOpen = refused.filter((group) => opened.has(group.name));
  if (refusedOpen.length > 0) {
    setChosenOpen(open.filter((name) => !refusedOpen.some((group) => group.name === name)));
    setClosed([...new Set([...closed, ...refusedOpen.map((group) => group.name)])]);
  }

  // Each new name in the route is sought once: closing a group round it later reopens nothing.
  if (hasTop && nodeRef !== soughtRef) {
    setSoughtRef(nodeRef);
    setSeek(nodeRef === null ? null : { name: nodeRef });
    setLostName(null);
    if (nodeRef !== null && centring?.name !== nodeRef) {
      setCentring({ name: nodeRef, always: false });
    }
  }
  const sought = seek === null ? undefined : nodes.get(seek.name);
  if (seek !== null && sought !== undefined) {
    setSeek(null);
    setChain(null);
  }

  // The way to a name no read holds: its `parent` chain, from the tree, once the reads asked have landed.
  const seeking = seek !== null && sought === undefined && !reading ? seek : null;
  useEffect(() => {
    if (seeking === null || chain?.seek === seeking) {
      return;
    }
    let current = true;
    readTree().then(
      (tree) => {
        if (current) {
          setChainFailure(null);
          setChain({ seek: seeking, names: chainOf(tree.nodes, seeking.name) });
        }
      },
      (error: unknown) => {
        if (current) {
          setChainFailure({ seek: seeking, message: apiErrorOf(error).message });
        }
      },
    );
    return () => {
      current = false;
    };
  }, [seeking, chain, readTree]);

  // Each closed box on the chain opens, top down: the next is known once the last has landed. A box
  // whose read has a `reason` stays closed. A chain run out ends the seek: only a new one opens more.
  const chainNames = chain !== null && chain.seek === seek ? (chain.names ?? []) : [];
  const nextOnChain = chainNames.find((name) => !opened.has(name) && !refused.some((group) => group.name === name) && isClosedBox(nodes.get(name)?.node));
  const chainFailed = groups.some((group) => group.error !== null && chainNames.includes(group.name));
  if (refusedOpen.length === 0 && nextOnChain !== undefined) {
    setChosenOpen([...open, nextOnChain]);
  } else if (seek !== null && chain?.seek === seek && sought === undefined && refusedOpen.length === 0 && !reading && !chainFailed) {
    setLostName(seek.name);
    setSeek(null);
    setChain(null);
  }
  const lost = lostName !== null && lostName === nodeRef && !nodes.has(lostName) ? lostName : null;

  const selected = panelBox(nodeRef, nodes, answers);
  const selectedBox = useMemo(() => layout?.boxes.find((box) => box.name === nodeRef), [layout, nodeRef]);
  const drawn = useMemo(() => new Set(layout?.boxes.map((box) => box.id) ?? []), [layout]);
  const rovingId = roving !== null && drawn.has(roving) ? roving : (selectedBox?.id ?? layout?.lanes[0]?.columns[0]?.[0] ?? layout?.lanes[0]?.band[0]?.[0] ?? null);

  // The panel asked for by Enter takes focus once it shows.
  useEffect(() => {
    if (headingFor.current !== null && headingFor.current === nodeRef && selected !== undefined) {
      headingFor.current = null;
      detailsHeading.current?.focus();
    }
  });

  // A read turning clean takes the rows and their button: focus on one goes to the results, never the body.
  useEffect(() => {
    if (clear && rowsBefore.current && focusIsLost()) {
      results.current?.focus({ preventScroll: true });
    }
    rowsBefore.current = top !== undefined && !clear;
  });

  // Each group's opening is said once, when its read lands.
  const spoken = useRef(new Set<string>());
  useEffect(() => {
    for (const read of reads) {
      if (read.group !== null && !spoken.current.has(read.group)) {
        spoken.current.add(read.group);
        announce(`${read.group} opened: ${readCount({ nodes: read.boxes, edges: read.view.edges }, t)}`);
      }
    }
    for (const name of spoken.current) {
      if (!opened.has(name)) {
        spoken.current.delete(name);
      }
    }
  }, [reads, opened, announce, t]);

  const toggle = useCallback(
    (name: string) => {
      if (drawnOpen.has(name)) {
        // A close is the owner's word: a seek under way opens nothing more.
        setSeek(null);
        setChain(null);
        setClosed((names) => [...new Set([...names, name])]);
        setChosenOpen(open.filter((other) => other !== name));
        announce(`${name} closed`);
      } else if (!opened.has(name)) {
        setChosenOpen([...open, name]);
      } else if (layout !== null && answers.has(name) && layout.notDrawn > 0) {
        // Read and asked open, yet not drawn: the cap left it no room.
        announce(notDrawnLine(layout.notDrawn, layout.total, t));
      } else if (!answers.has(name) && !opening.has(name)) {
        // Asked open and its read failed: the control says Open, so the press reads again.
        void groups.find((group) => group.name === name)?.retry({ cancelRefetch: false });
      }
    },
    [announce, answers, drawnOpen, groups, layout, open, opened, opening, t],
  );

  /** A group's Retry worked and took its button away: focus goes to the group's head, or the results. */
  const afterGroupRetry = (name: string) => {
    if (!focusIsLost()) {
      return;
    }
    if (tab === "canvas" && drawn.has(boxId(name))) {
      setRoving(boxId(name));
      focusBox.current(boxId(name));
    } else {
      results.current?.focus();
    }
  };

  function select(name: string) {
    if (name !== nodeRef) {
      pushHash(sectionHash(project, "flow", name));
    }
  }

  /** Closes the panel; focus goes back to the box, on the List to its row. */
  function closeDetails() {
    const back = nodeRef;
    pushHash(sectionHash(project, "flow"));
    if (back === null) {
      return;
    }
    if (tab === "canvas" && drawn.has(boxId(back))) {
      setRoving(boxId(back));
      focusBox.current(boxId(back));
      return;
    }
    const rows = Array.from(results.current?.querySelectorAll<HTMLElement>("[data-flow-row]") ?? []);
    const link = rows.find((row) => row.dataset.flowRow === back)?.querySelector("a");
    (link ?? results.current)?.focus();
  }

  /** A gap item: routes to its first member, marks all, centres the first; focus stays on the item. */
  function activate(item: readonly string[]) {
    const first = item[0];
    if (first === undefined) {
      return;
    }
    setMarks(item);
    setCentring({ name: first, always: true });
    setChainFailure(null);
    setLostName(null);
    setSeek({ name: first });
    select(first);
  }

  function onCommand(box: FlowBox, command: BoxCommand) {
    switch (command) {
      case "select":
        select(box.name);
        break;
      case "details":
        headingFor.current = box.name;
        select(box.name);
        if (box.name === nodeRef) {
          headingFor.current = null;
          detailsHeading.current?.focus();
        }
        break;
      case "toggle":
        if (isGroup(box.node)) {
          toggle(box.name);
        }
        break;
      case "tree":
        pushHash(sectionHash(project, "tree", box.name));
        break;
      case "close":
        if (nodeRef !== null) {
          closeDetails();
        }
        break;
    }
  }

  const setFocusBox = useCallback((focus: (id: string) => void) => {
    focusBox.current = focus;
  }, []);

  function toggleLane(key: string) {
    setOpenLanes((lanes) => (lanes.includes(key) ? lanes.filter((lane) => lane !== key) : [...lanes, key]));
  }

  function onViewKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  function retryTop() {
    retried();
    void topRead.refetch({ cancelRefetch: false });
  }

  let body;
  if (top === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={`Reading the flow of ${project}`} lines={6} />
        </div>
      ) : (
        <ErrorPanel title="The flow could not be read" message={apiErrorOf(failure).message} retrying={topRead.isFetching} attempt={topRead.errorUpdateCount} onRetry={retryTop} />
      );
  } else if (top.reason !== null) {
    body = <ErrorPanel title="The flow could not be read" message={top.reason} retrying={topRead.isFetching} attempt={topRead.dataUpdatedAt} onRetry={retryTop} />;
  } else {
    const empty = top.nodes.length === 0;
    const gapsShown = !clear && (empty || !gapsHidden);
    const stage = ["flow-stage", !gapsShown && "no-gaps", !gapsShown && selected === undefined && "no-side"].filter(Boolean).join(" ");
    body = (
      <div className={stage}>
        {gapsShown && <FlowGaps gaps={top.gaps} unread={topFailed} marked={marked} onActivate={activate} />}
        <div className="flow-main">
          <TopNotes top={top} notes={notes} />
          {topRead.error !== null && (
            <div className="flow-failed">
              <p className="note" role="alert">
                <Icon name="alert" />
                <span>
                  The flow could not be read again: {apiErrorOf(topRead.error).message}
                  {tab !== "timeline" && (
                    <>
                      <br />
                      {statesHiddenWords(t)}
                    </>
                  )}
                </span>
              </p>
              <button type="button" className="button" aria-disabled={topRead.isFetching} onClick={topRead.isFetching ? undefined : retryTop}>
                <Icon name="retry" />
                <span>{topRead.isFetching ? "Retrying" : "Retry"}</span>
              </button>
            </div>
          )}
          {groups
            .filter((group) => opened.has(group.name))
            .map((group) => (
              <GroupFailure
                key={group.name}
                name={group.name}
                error={group.error}
                fetching={group.fetching}
                settled={group.settled}
                onRetry={() => {
                  void group.retry({ cancelRefetch: false });
                }}
                onSettled={afterGroupRetry}
              />
            ))}
          {summary.error !== null && tab !== "timeline" && (
            <div className="flow-failed">
              <p className="note" role="alert">
                <Icon name="alert" />
                <span>The stages could not be read: {apiErrorOf(summary.error).message}</span>
              </p>
              <button
                type="button"
                className="button"
                aria-disabled={summary.isFetching}
                onClick={
                  summary.isFetching
                    ? undefined
                    : () => {
                        retriedStages();
                        void summary.refetch({ cancelRefetch: false });
                      }
                }
              >
                <Icon name="retry" />
                <span>
                  {summary.isFetching ? "Retrying" : "Retry"} <span className="sr-only">reading the stages</span>
                </span>
              </button>
            </div>
          )}
          {refused
            .filter((group) => nodes.has(group.name))
            .map((group) => (
            <p key={group.name} className="note">
              <Icon name="info" />
              <span>
                <span className="mono">{group.name}</span> stays closed: <span className="verbatim">{group.data?.reason}</span>
              </span>
            </p>
          ))}
          {chainFailure !== null && chainFailure.seek === seek && (
            <p className="note" role="alert">
              <Icon name="alert" />
              <span>
                The way to <span className="mono">{chainFailure.seek.name}</span> could not be read: {chainFailure.message}
              </span>
            </p>
          )}
          {lost !== null && (
            <p className="note">
              <Icon name="info" />
              <span>
                No box here is named <span className="mono">{lost}</span>.
              </span>
              <a href={sectionHash(project, "tree", lost)}>Open {lost} in the Spec tree</a>
            </p>
          )}
          {empty ? (
            <div className="empty-state">
              <p>No flow in this project yet. Next: have your AI add flow links.</p>
            </div>
          ) : (
            <>
              <div className="flow-tools">
                <Tabs
                  label="Views of the flow"
                  base={tabsBase}
                  selected={tab}
                  onSelect={setChosenTab}
                  tabs={[
                    { id: "canvas", label: "Canvas" },
                    { id: "list", label: "List" },
                    { id: "timeline", label: "Timeline" },
                  ]}
                />
                {tab !== "timeline" && <StageFilter value={stageShown} options={stageOptions(stages, t)} onChange={setChosenStage} />}
                {clear ? (
                  <GapsClear />
                ) : (
                  <button
                    type="button"
                    className="button button-quiet"
                    aria-expanded={!gapsHidden}
                    onClick={() => {
                      rememberGapsHidden(!gapsHidden);
                      setGapsHidden(!gapsHidden);
                    }}
                  >
                    <Icon name="flowGap" />
                    <span>{gapsHidden ? "Show gaps" : "Hide gaps"}</span>
                  </button>
                )}
              </div>
              <TabPanel base={tabsBase} id="canvas" selected={tab === "canvas"}>
                {tab === "canvas" && layout !== null && (
                  <div className="graph-canvas-panel">
                    {layout.notDrawn > 0 && (
                      <p className="notice cut-notice flow-limit" role="note">
                        <span className="notice-title">
                          <Icon name="info" />
                          <span>{notDrawnLine(layout.notDrawn, layout.total, t)}</span>
                        </span>
                      </p>
                    )}
                    <RegionBoundary name="canvas">
                      <Suspense fallback={<Skeleton label="Loading the canvas" lines={4} />}>
                        <FlowCanvas
                          layout={layout}
                          types={types}
                          rovingId={rovingId}
                          selected={nodeRef}
                          marked={marked}
                          opening={opening}
                          named={named}
                          stage={stageShown}
                          centring={centring}
                          onRove={setRoving}
                          onCommand={onCommand}
                          focusRef={setFocusBox}
                        />
                      </Suspense>
                    </RegionBoundary>
                    <FlowLegend types={types} states={reads.some((read) => read.boxes.some((node) => node.rollup !== null))} />
                  </div>
                )}
              </TabPanel>
              <TabPanel base={tabsBase} id="list" selected={tab === "list"}>
                {tab === "list" && (
                  <RegionBoundary name="list">
                    <FlowList project={project} reads={reads} open={drawnOpen} named={named} selected={nodeRef} marked={marked} stage={stageShown} scrollTo={centring} onToggle={toggle} />
                  </RegionBoundary>
                )}
              </TabPanel>
              <TabPanel base={tabsBase} id="timeline" selected={tab === "timeline"}>
                {tab === "timeline" && (
                  <RegionBoundary name="timeline">
                    <TimelineView project={project} read={summary} openLanes={openLanes} selected={nodeRef} noted={notes} onToggle={toggleLane} />
                  </RegionBoundary>
                )}
              </TabPanel>
            </>
          )}
        </div>
        {selected !== undefined && nodeRef !== null && (
          <FlowDetails
            key={nodeRef}
            project={project}
            node={selected.node}
            inLane={selected.inLane}
            open={drawnOpen.has(nodeRef)}
            noRoom={selectedBox?.noRoom ?? false}
            headingRef={detailsHeading}
            onToggle={toggle}
            onClose={closeDetails}
          />
        )}
      </div>
    );
  }

  return (
    <section className="view flow-view" aria-labelledby={titleId} onKeyDown={onViewKeyDown}>
      <header className="view-head">
        <h1 id={titleId} tabIndex={-1}>
          Flow
        </h1>
        <p className="view-about">{aboutOf("flow", t)} The spec is read only here.</p>
      </header>
      <div ref={results} tabIndex={-1} className="flow-results" aria-busy={topRead.isFetching || opening.size > 0 || !moduleKnown}>
        <RegionBoundary name="flow">{body}</RegionBoundary>
      </div>
    </section>
  );
}

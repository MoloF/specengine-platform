import { lazy, Suspense, useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type KeyboardEvent, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { flowForm, useFlowRead, useScopeNode, useScopeStages, useTopOnDemand, useTreeOnDemand } from "../api/queries";
import type { CardWrite, FlowGap, FlowNode, FlowView as FlowAnswer, WorkspaceModule } from "../api/types";
import { pushHash, replaceHash } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { flowHash, isBareFlow, parseFlowPlace, sectionHash, type FlowEdgeKey, type FlowPlace } from "../app/routes";
import { aboutOf, sectionLabel } from "../app/sections";
import { useOpenShortcuts } from "../app/shortcuts";
import { NewCardDialog, type NewCardPreset } from "../board/CardDialogs";
import { nodeName } from "../graph/layout";
import { useTranslate } from "../i18n";
import { useAnnounce } from "../ui/announcer";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { Lozenge } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import type { BoxCommand, Centring } from "./FlowCanvas";
import { FlowGaps } from "./FlowGaps";
import { FlowLegend } from "./FlowLegend";
import { FlowList, type ListPlaces } from "./FlowList";
import { BoxView, EdgeView, ScopeView, type ScopeShown } from "./FlowPanel";
import { FlowPlaces, type GroupLink, type PathEntry, type PathShown, type StagesShown } from "./FlowPlaces";
import { FlowRecords } from "./FlowRecords";
import { askedStageWords, boxWords, edgeKeyOf, edgeKeyText, emptyStageWords, gapsLook, handTitle, leftOutWords, namedByRows, namesDocuments, notDrawnLine, stageWords, statesHiddenWords, typesOf } from "./labels";
import { isGroup, layoutFlow, type FlowBox, type LaidEdge } from "./layout";
import { stageAsked, stageKey, withoutStates } from "./timeline";
import { isRooted } from "../workspace/choice";

/** The canvas and its drawing library load with the first answer drawn, not with the app. */
const FlowCanvas = lazy(() => import("./FlowCanvas").then((module) => ({ default: module.FlowCanvas })));

export type FlowTab = "canvas" | "list";

/**
 * What the Shell keeps per project for the page's life: the tab, and the last place, which stands in
 * for a bare `#/<p>/flow` on entry (docs/features/writer-studio.md "State, client, tokens"). The hash
 * is the store; this replaces nothing else.
 */
export interface FlowState {
  tab: FlowTab | null;
  place: FlowPlace | null;
}

export interface FlowMemory {
  recall: (project: string) => FlowState | undefined;
  remember: (project: string, state: FlowState) => void;
}

/** No memory: each Flow opens as new. */
export const NO_FLOW_MEMORY: FlowMemory = { recall: () => undefined, remember: () => undefined };

/** Under this width the List comes first. */
const NARROW = "(max-width: 48rem)";

/** Under this width the panes stack, the page scrolls and the wheel with it (R11). */
const STACKED = "(max-width: 63.99rem)";

function subscribeMedia(query: string) {
  return (listener: () => void) => {
    if (typeof window.matchMedia !== "function") {
      return () => undefined;
    }
    const list = window.matchMedia(query);
    if (typeof list.addEventListener !== "function") {
      return () => undefined;
    }
    list.addEventListener("change", listener);
    return () => {
      list.removeEventListener("change", listener);
    };
  };
}

/** Whether a media query holds; jsdom has no `matchMedia`, so never there. */
function useMedia(query: string): boolean {
  const subscribe = useMemo(() => subscribeMedia(query), [query]);
  return useSyncExternalStore(subscribe, () => typeof window.matchMedia === "function" && window.matchMedia(query).matches);
}

/** D3: Top's one node is the lens root when it holds documents; else the top stands unrooted. */
function soleRootOf(top: FlowAnswer): string | null {
  const [only, ...more] = top.nodes;
  return only !== undefined && more.length === 0 && isGroup(only) ? nodeName(only) : null;
}

/** A selection of one place: the name it selects, or the arrow, or nothing. */
function selectedName(place: FlowPlace): string | null {
  return place.select !== null && "node" in place.select ? place.select.node : null;
}

function selectedEdgeKey(place: FlowPlace): FlowEdgeKey | null {
  return place.select !== null && "edge" in place.select ? place.select.edge : null;
}

/** A gap handed to the AI: its New card's fields. */
interface Hand {
  preset: NewCardPreset;
  trigger: HTMLElement | null;
}

/** The studio's top, measured: the shell above it may wrap, so its height is never a constant (R-header). */
function useStudioTop(studio: { current: HTMLElement | null }) {
  useLayoutEffect(() => {
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const element = studio.current;
        if (element !== null) {
          const top = Math.max(0, Math.round(element.getBoundingClientRect().top + window.scrollY));
          if (element.style.getPropertyValue("--flow-studio-top") !== `${String(top)}px`) {
            element.style.setProperty("--flow-studio-top", `${String(top)}px`);
          }
        }
      });
    };
    measure();
    window.addEventListener("resize", measure);
    const observer = new ResizeObserver(measure);
    const header = document.querySelector(".app-header");
    if (header !== null) {
      observer.observe(header);
    }
    const main = studio.current?.parentElement;
    if (main !== null && main !== undefined) {
      observer.observe(main);
    }
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", measure);
      observer.disconnect();
    };
  }, [studio]);
}

/** Where the Records dialog was opened from: focus goes back there when it closes. */
interface RecordsOpen {
  trigger: HTMLElement | null;
}

/**
 * Flow as a studio (`#/<p>/flow[/<name>][?module=&scope=&stage=&edge=]`,
 * docs/features/writer-studio.md, docs/features/flow-scenario.md): the path on the left, one scope's
 * scenario in the centre, left to right, the panel always on the right. Each picture is one read of
 * three forms (D2), cut to what holds part of the flow, never a whole read; what the cut leaves out is
 * Records for the AI, read only when asked. The place is the hash's, so a reload, a link and Back
 * draw one picture. A scope, stage or module change pushes; a selection, a seek's fix and a
 * correction replace (D5). Nothing here blocks: a gap is information, and the one write is a card the
 * owner makes from the panel (ADR-0012).
 */
export function FlowView({
  project,
  nodeRef,
  query,
  memory = NO_FLOW_MEMORY,
  modules,
  storedModule = null,
  onChooseModule,
  moduleKnown = true,
}: {
  project: string;
  /** The route's name segment. */
  nodeRef: string | null;
  /** The route's query, after the first raw `?`; undefined with none. */
  query?: string;
  memory?: FlowMemory;
  /** The served modules; undefined or none: no module bar. */
  modules?: readonly WorkspaceModule[];
  /** 1a's stored choice: the module of a hash naming none. */
  storedModule?: WorkspaceModule | null;
  /** The bar's choice, stored as 1a's key: written only here. */
  onChooseModule?: (name: string) => void;
  /** False while the workspaces are read: nothing is read before its module is known. */
  moduleKnown?: boolean;
}) {
  const t = useTranslate();
  const announce = useAnnounce();
  const openShortcuts = useOpenShortcuts();
  const titleId = useId();
  const tabsBase = useId();
  const aboutId = useId();
  const legendId = useId();
  const gapsId = useId();
  const studio = useRef<HTMLElement>(null);
  const region = useRef<HTMLElement>(null);
  const listRegion = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const panelHeading = useRef<HTMLHeadingElement>(null);
  const legendToggle = useRef<HTMLButtonElement>(null);
  const gapsToggle = useRef<HTMLButtonElement>(null);
  const aboutToggle = useRef<HTMLButtonElement>(null);
  const [remembered] = useState(() => memory.recall(project));
  const narrow = useMedia(NARROW);
  const stacked = useMedia(STACKED);
  const [chosenTab, setChosenTab] = useState<FlowTab | null>(remembered?.tab ?? null);
  const tab = chosenTab ?? (narrow ? "list" : "canvas");
  const [placesOpen, setPlacesOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [legendOpen, setLegendOpen] = useState(false);
  const [gapsOpen, setGapsOpen] = useState(false);
  const [records, setRecords] = useState<RecordsOpen | null>(null);
  const [marks, setMarks] = useState<readonly string[]>([]);
  const [centring, setCentring] = useState<Centring | null>(null);
  const [hand, setHand] = useState<Hand | null>(null);
  /** New card on the scope or a box, open: its target and the control it goes back to. */
  const [scopeCard, setScopeCard] = useState<{ target: string; trigger: HTMLElement | null } | null>(null);
  const [made, setMade] = useState<string | null>(null);
  /** The seek that found nothing, or failed, for one place: a note until the place changes. */
  const [lost, setLost] = useState<{ key: string; name: string } | null>(null);
  const [seekFailure, setSeekFailure] = useState<{ key: string; name: string; message: string } | null>(null);
  /** Where a selection was made: Esc puts focus back there. */
  const origin = useRef<HTMLElement | null>(null);
  /** A name whose panel takes the focus once it shows. */
  const headingFor = useRef<string | null>(null);
  /** The picture a Space entered: its first box takes the focus. */
  const [spaceInto, setSpaceInto] = useState<string | null>(null);
  /** The last box shown in the panel, kept mounted while the scope or an arrow shows, so coming back reads nothing again. */
  const [keptBox, setKeptBox] = useState<FlowNode | null>(null);

  // The place: the hash's. On entry a bare hash, the nav's link, stands for the project's last place.
  const bare = isBareFlow({ id: nodeRef, query });
  const [entry, setEntry] = useState<FlowPlace | null>(() => (bare ? (remembered?.place ?? null) : null));
  if (entry !== null && !bare) {
    // The last place took the bare hash's: from now on a bare hash is the lens root.
    setEntry(null);
  }
  const parsed = useMemo(() => parseFlowPlace({ id: nodeRef, query }), [nodeRef, query]);
  const place = entry ?? parsed;
  useEffect(() => {
    if (entry !== null) {
      replaceHash(flowHash(project, entry));
    }
  }, [entry, project]);

  // The module: the hash's; none named, the viewer's stored one; one not served is dropped (R2).
  // Flow draws from a root, so it takes rooted modules only: a direction named in the hash drops as an
  // unserved one does, and a stored direction reads as All modules, its key left as it is
  // (docs/features/departments.md "Acceptance criteria", DP-12).
  const served = useMemo(() => (modules ?? []).filter(isRooted), [modules]);
  const storedRooted = storedModule !== null && isRooted(storedModule) ? storedModule : null;
  const namedModule = place.module === null || place.module === "" ? null : (served.find((candidate) => candidate.name === place.module) ?? null);
  const unserved = moduleKnown && place.module !== null && place.module !== "" && namedModule === null;
  const module = place.module === null || unserved ? storedRooted : namedModule;
  const writtenModule = served.length > 0 ? (module?.name ?? "") : null;
  /** A place as the screen writes it: naming the module in force whenever one is served (D4). */
  const placeAt = useCallback((next: Omit<FlowPlace, "module">): FlowPlace => ({ ...next, module: writtenModule }), [writtenModule]);
  const write = useCallback((next: Omit<FlowPlace, "module">) => flowHash(project, placeAt(next)), [project, placeAt]);

  // The lens root (D3): the module's root; else Top's one node holding documents; else the unrooted top.
  const moduleRoot = module?.root ?? null;
  const topRead = useFlowRead(project, flowForm(null, null), moduleKnown && moduleRoot === null && place.scope === null);
  const top = moduleRoot === null ? topRead.data : undefined;
  const lensRoot: string | null | undefined = !moduleKnown ? undefined : (moduleRoot ?? (top === undefined ? undefined : soleRootOf(top)));
  const lensTitle = module?.title ?? top?.nodes[0]?.title ?? (typeof lensRoot === "string" ? lensRoot : null);
  const scope = place.scope ?? lensRoot;
  const rooted = typeof scope === "string";
  const scopeName = rooted ? scope : null;
  const stage = rooted ? place.stage : null;

  // The reads (D2): the picture, and under a root its lanes and its own text; nothing at the top but the top.
  const drawingForm = flowForm(scopeName, stage);
  const drawing = useFlowRead(project, drawingForm, moduleKnown && scope !== undefined);
  const scopeRead = useFlowRead(project, flowForm(scopeName, null), false);
  const lanes = useScopeStages(project, scopeName ?? "", moduleKnown && rooted);
  const scopeNode = useScopeNode(project, scopeName ?? "", moduleKnown && rooted);
  const readTree = useTreeOnDemand(project);
  const readTop = useTopOnDemand(project);
  const failure = useRetainedFailure(drawing.error, drawing.isFetching);
  const topFailure = useRetainedFailure(topRead.error, topRead.isFetching);

  const view = drawing.data;
  const failedAgain = drawing.error !== null && view !== undefined;
  const shown = useMemo(() => (view === undefined ? undefined : failedAgain ? withoutStates(view) : view), [view, failedAgain]);
  const refused = shown?.reason ?? null;
  const boxes = useMemo(() => (shown === undefined || refused !== null ? [] : shown.nodes.filter((node) => scopeName === null || nodeName(node) !== scopeName)), [shown, refused, scopeName]);
  const edges = useMemo(() => (shown === undefined || refused !== null ? [] : shown.edges), [shown, refused]);
  const layout = useMemo(() => (shown === undefined || refused !== null ? null : layoutFlow({ ...shown, nodes: boxes })), [shown, refused, boxes]);
  const types = useMemo(() => (shown === undefined ? [] : typesOf(shown)), [shown]);
  /** The titles the reads in hand give, by name: the drawing read's, and its scope's when held. */
  const heldTitles = useMemo(() => {
    const titles = new Map<string, string>();
    for (const node of [...(scopeRead.data?.nodes ?? []), ...(view?.nodes ?? [])]) {
      if (node.title !== null) {
        titles.set(nodeName(node), node.title);
      }
    }
    return titles;
  }, [scopeRead.data, view]);
  const named = useMemo(() => namedByRows(failedAgain ? [] : (shown?.gaps ?? [])), [shown, failedAgain]);
  // SA1: a rooted read holds its root as a node; a stage read only when it carries the stage, so the scope's read held stands in.
  const ownNode = useMemo(() => {
    if (scopeName === null) {
      return undefined;
    }
    const from = (read: FlowAnswer | undefined) => read?.nodes.find((node) => nodeName(node) === scopeName);
    return from(scopeRead.data) ?? from(view);
  }, [scopeName, scopeRead.data, view]);
  const landed = shown !== undefined;

  // The selection.
  const selName = selectedName(place);
  const selKey = selectedEdgeKey(place);
  const selKeyText = selKey === null ? null : edgeKeyText(selKey);
  const selBox = selName === null ? undefined : boxes.find((node) => nodeName(node) === selName);
  const selEdge = selKeyText === null ? undefined : edges.find((edge) => edgeKeyText(edgeKeyOf(edge)) === selKeyText);
  // A card made from the panel is said where it was made: another selection takes its note away.
  const selectionKey = selName ?? selKeyText ?? "";
  const [madeFor, setMadeFor] = useState(selectionKey);
  if (madeFor !== selectionKey) {
    setMadeFor(selectionKey);
    setMade(null);
  }
  const laidSelected = useMemo(() => (selKeyText === null ? null : (layout?.edges.find((laid) => edgeKeyText(edgeKeyOf(laid.edge)) === selKeyText)?.id ?? null)), [layout, selKeyText]);
  const marked = useMemo(() => new Set(selName !== null && marks.includes(selName) ? marks : []), [marks, selName]);
  const placeKey = flowHash(project, place);
  /** The place shown, as a seek's answer finds it: an answer for a place left is dropped. */
  const currentKey = useRef(placeKey);
  useEffect(() => {
    currentKey.current = placeKey;
  });

  if (selBox !== undefined && keptBox !== selBox) {
    setKeptBox(selBox);
  }

  useEffect(() => {
    if (entry === null) {
      memory.remember(project, { tab: chosenTab, place });
    }
  }, [memory, project, chosenTab, place, entry]);

  // A picture landing is said once, politely: its scope, its boxes, its stage.
  const spokenFor = useRef<string | null>(null);
  const drawingKey = JSON.stringify(drawingForm);
  const scopeTitle =
    scope === undefined ? "" : scopeName === null ? t("flow.topTitle") : (ownNode?.title ?? scopeNode.data?.nodes[0]?.title ?? scopeName);
  /** A name's words (flow-scenario D12): its title where a read in hand gives one, else the name. */
  const titleOf = useCallback((name: string): string => heldTitles.get(name) ?? (name === scopeName ? scopeTitle : name), [heldTitles, scopeName, scopeTitle]);
  const stageShown = stage === null ? null : askedStageWords(stage, t);
  useEffect(() => {
    if (!landed || refused !== null || spokenFor.current === drawingKey) {
      return;
    }
    spokenFor.current = drawingKey;
    announce(stageShown === null ? t("flow.showing", { scope: scopeTitle, count: boxes.length }) : t("flow.showingStage", { scope: scopeTitle, count: boxes.length, stage: stageShown }));
  }, [landed, refused, drawingKey, scopeTitle, boxes.length, stageShown, announce, t]);

  // Corrections, each a replace (D5): a module not served (R2), a stage at the unrooted top (R2), the
  // scope's own node selected (R3), an arrow this read does not hold (R4).
  useEffect(() => {
    if (!moduleKnown || entry !== null) {
      return;
    }
    let next = place;
    if (unserved) {
      next = { ...next, module: writtenModule };
    }
    if (scope === null && next.stage !== null) {
      next = { ...next, stage: null };
    }
    if (scopeName !== null && selName === scopeName) {
      next = { ...next, select: null };
    }
    const gone = landed && refused === null && selKey !== null && selEdge === undefined;
    if (gone) {
      next = { ...next, select: null };
    }
    if (next !== place) {
      replaceHash(flowHash(project, next));
      if (gone) {
        announce(t("flow.edgeGone"));
      }
    }
  }, [moduleKnown, entry, place, unserved, writtenModule, scope, scopeName, selName, landed, refused, selKey, selEdge, project, announce, t]);

  /**
   * R3: where a name stands, by its `parent` in the tree; null when the tree holds no such name. A
   * parent that is the lens root is the place with no scope: a lens root not known yet (the hash named
   * a scope) is found by the top read, never written as a scope.
   */
  const placeOf = useCallback(
    async (name: string): Promise<FlowPlace | null> => {
      const tree = await readTree();
      const row = tree.nodes.find((candidate) => (candidate.id ?? candidate.path) === name);
      if (row === undefined) {
        return null;
      }
      if (row.parent === null) {
        return { module: served.length > 0 ? "" : null, scope: null, stage: null, select: { node: name } };
      }
      const lens = lensRoot !== undefined ? lensRoot : soleRootOf(await readTop());
      return { module: writtenModule, scope: row.parent === lens ? null : row.parent, stage: null, select: { node: name } };
    },
    [readTree, readTop, served, writtenModule, lensRoot],
  );

  /** Seeks a name for one place: the fix replaces, a gap's pushes; a fix landing where it stands, or no row, says so. */
  const seek = useCallback(
    (name: string, key: string, push: boolean) => {
      placeOf(name).then(
        (found) => {
          if (currentKey.current !== key) {
            return;
          }
          const hash = found === null ? null : flowHash(project, found);
          if (hash === null || hash === key) {
            setLost({ key, name });
          } else if (push) {
            pushHash(hash);
          } else {
            replaceHash(hash);
          }
        },
        (error: unknown) => {
          if (currentKey.current === key) {
            setSeekFailure({ key, name, message: apiErrorOf(error).message });
          }
        },
      );
    },
    [placeOf, project],
  );

  // R3: a name the drawing read does not hold is sought once per place.
  const soughtFor = useRef<string | null>(null);
  const seekWanted = landed && refused === null && selName !== null && selName !== scopeName && selBox === undefined;
  useEffect(() => {
    if (!seekWanted || soughtFor.current === placeKey) {
      return;
    }
    soughtFor.current = placeKey;
    seek(selName, placeKey, false);
  }, [seekWanted, selName, placeKey, seek]);

  /** Where focus goes when it was lost: the tab's region, the canvas's or the List's, else the h1; never the body (R9). */
  const focusFallback = useCallback(() => {
    (tab === "canvas" ? region.current : listRegion.current)?.focus({ preventScroll: true });
    if (focusIsLost()) {
      heading.current?.focus({ preventScroll: true });
    }
  }, [tab]);

  // R9: a place change that took the focused element away hands focus to the tab's region, never the body.
  const placeBefore = useRef(placeKey);
  useEffect(() => {
    if (placeBefore.current === placeKey) {
      return;
    }
    placeBefore.current = placeKey;
    if (focusIsLost()) {
      focusFallback();
    }
  });

  // A panel asked for by Enter or a gap takes focus once it shows.
  useEffect(() => {
    if (headingFor.current !== null && headingFor.current === selName && selBox !== undefined) {
      headingFor.current = null;
      panelHeading.current?.focus();
    }
  });

  useStudioTop(studio);

  const enterPlace = useCallback((name: string) => placeAt({ scope: name, stage: null, select: null }), [placeAt]);
  const selectPlace = useCallback((name: string) => placeAt({ scope: place.scope, stage: place.stage, select: { node: name } }), [placeAt, place.scope, place.stage]);
  const enterHref = useCallback((name: string) => flowHash(project, enterPlace(name)), [project, enterPlace]);
  const selectHref = useCallback((name: string) => flowHash(project, selectPlace(name)), [project, selectPlace]);
  const listPlaces = useMemo<ListPlaces>(
    () => ({ project, box: selectPlace, edge: (key) => placeAt({ scope: place.scope, stage: place.stage, select: { edge: key } }), enter: enterPlace }),
    [project, selectPlace, placeAt, place.scope, place.stage, enterPlace],
  );

  function rememberOrigin(element: HTMLElement | null) {
    origin.current = element ?? (document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : null);
  }

  function select(name: string, from: HTMLElement | null) {
    rememberOrigin(from);
    if (name !== selName) {
      replaceHash(selectHref(name));
    }
  }

  function clearSelection() {
    if (place.select === null) {
      return;
    }
    const back = origin.current;
    replaceHash(write({ scope: place.scope, stage: place.stage, select: null }));
    if (back !== null && back.isConnected) {
      back.focus();
    } else {
      focusFallback();
    }
  }

  function enter(name: string, byKey: boolean) {
    setSpaceInto(byKey ? name : null);
    pushHash(enterHref(name));
  }

  function onCommand(box: FlowBox, command: BoxCommand, from: HTMLElement | null) {
    switch (command) {
      case "select":
        select(box.name, from);
        break;
      case "details":
        headingFor.current = box.name;
        select(box.name, from);
        if (box.name === selName) {
          headingFor.current = null;
          panelHeading.current?.focus();
        }
        break;
      case "enter":
      case "enterByKey":
        if (isGroup(box.node)) {
          enter(box.name, command === "enterByKey");
        }
        break;
      case "tree":
        pushHash(sectionHash(project, "tree", box.name));
        break;
    }
  }

  function onEdge(laid: LaidEdge) {
    origin.current = null;
    replaceHash(write({ scope: place.scope, stage: place.stage, select: { edge: edgeKeyOf(laid.edge) } }));
  }

  /** A gap item: selects its first member (a seek pushes its scope if undrawn), marks all, centres the first; focus to the panel heading. */
  function activate(item: readonly string[]) {
    const [first] = item;
    if (first === undefined) {
      return;
    }
    rememberOrigin(null);
    setGapsOpen(false);
    setMarks(item);
    setCentring({ name: first, always: true });
    headingFor.current = first;
    setLost(null);
    setSeekFailure(null);
    if (boxes.some((node) => nodeName(node) === first)) {
      if (first === selName) {
        headingFor.current = null;
        panelHeading.current?.focus();
      } else {
        replaceHash(selectHref(first));
      }
    } else {
      soughtFor.current = placeKey;
      seek(first, placeKey, true);
    }
  }

  /** A gap's item to the AI (D11): New card, its members the targets, its cause the goal; `no-start` hands the scope. */
  function handOff(gap: FlowGap, item: readonly string[]) {
    const targets = namesDocuments(gap) ? item : scopeName === null ? [] : [scopeName];
    if (targets.length === 0) {
      return;
    }
    setMade(null);
    setHand({ preset: { title: handTitle(gap.code, targets, t), goal: gap.cause ?? "", targets }, trigger: document.activeElement instanceof HTMLElement ? document.activeElement : null });
  }

  /** A focus or a click in the canvas closes what lies over it. */
  function closeOverlays() {
    setLegendOpen(false);
    setGapsOpen(false);
  }

  function openRecords(event: MouseEvent<HTMLElement>) {
    setRecords({ trigger: event.currentTarget });
  }

  function closeRecords() {
    const trigger = records?.trigger ?? null;
    setRecords(null);
    if (trigger?.isConnected === true) {
      trigger.focus();
    }
  }

  function onStudioKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
      return;
    }
    if (event.key !== "Escape") {
      return;
    }
    if (aboutOpen) {
      event.preventDefault();
      setAboutOpen(false);
      aboutToggle.current?.focus();
    } else if (legendOpen) {
      event.preventDefault();
      setLegendOpen(false);
      legendToggle.current?.focus();
    } else if (gapsOpen) {
      event.preventDefault();
      setGapsOpen(false);
      gapsToggle.current?.focus();
    } else if (place.select !== null) {
      event.preventDefault();
      clearSelection();
    }
  }

  // The notes of every read shown, each once, in the daemon's words; then what the picture left out, in ours.
  const notes = useMemo(() => [...new Set([...(shown?.notes ?? []), ...(rooted ? (lanes.data?.notes ?? []) : [])])], [shown, rooted, lanes.data]);
  const leftOut = shown === undefined ? null : leftOutWords(shown.left_out, t);
  const noted = notes.length + (leftOut === null ? 0 : 1);

  // The places pane's path (flow-scenario D11): the scope node's served `ancestors`, top first, then
  // the scope; read by the scope view's own read, no other. Entries above the lens root are left out;
  // an entry that is the lens root writes no scope; a lens root not known yet is named as a scope.
  const path = useMemo<PathShown | null>(() => {
    if (scope === undefined) {
      return null;
    }
    const here = (label: string, at: PathEntry["place"], depth: number): PathEntry => ({ key: "here", label, place: at, depth, current: true });
    if (scopeName === null || refused !== null) {
      const label = scopeName === null || lensRoot === null ? t("flow.topTitle") : (lensTitle ?? t("flow.topTitle"));
      return { entries: [here(label, placeAt({ scope: null, stage: null, select: null }), 0)], pending: false, failed: null };
    }
    const own = placeAt({ scope: place.scope, stage: null, select: null });
    const read = scopeNode.data;
    if (read === undefined) {
      const failed =
        scopeNode.error === null
          ? null
          : {
              message: apiErrorOf(scopeNode.error).message,
              retrying: scopeNode.isFetching,
              onRetry: () => {
                void scopeNode.refetch({ cancelRefetch: false });
              },
            };
      return { entries: [here(scopeTitle, own, 0)], pending: failed === null, failed };
    }
    const ancestors = read.nodes[0]?.ancestors ?? [];
    const names = ancestors.map((ancestor) => ancestor.id ?? ancestor.path);
    const from = typeof lensRoot === "string" ? names.indexOf(lensRoot) : -1;
    const kept = scopeName === lensRoot ? [] : from >= 0 ? ancestors.slice(from) : ancestors;
    const entries = kept.map((ancestor, depth): PathEntry => {
      const name = ancestor.id ?? ancestor.path;
      return { key: `a:${name}`, label: ancestor.title ?? ancestor.id ?? ancestor.path, place: placeAt({ scope: name === lensRoot ? null : name, stage: null, select: null }), depth, current: false };
    });
    return { entries: [...entries, here(scopeTitle, own, kept.length)], pending: false, failed: null };
  }, [scope, scopeName, refused, lensRoot, lensTitle, scopeNode, scopeTitle, placeAt, place.scope, t]);
  const groups = useMemo<GroupLink[]>(() => {
    if (stage === null) {
      return boxes.filter(isGroup).map((node) => ({ name: nodeName(node), label: boxWords(node), place: enterPlace(nodeName(node)) }));
    }
    const seen: string[] = [];
    for (const node of boxes) {
      if (node.group !== null && node.group !== scopeName && !seen.includes(node.group)) {
        seen.push(node.group);
      }
    }
    return seen.map((name) => ({ name, label: titleOf(name), place: enterPlace(name) }));
  }, [boxes, stage, scopeName, enterPlace, titleOf]);

  // Records for the AI's count (flow-scenario D6): the scope read's `not_in_flow`; in a stage view the
  // scope read's held, else no count (null); none at the unrooted top, none at 0, none from a read the
  // daemon did not cut.
  const recordsCount: number | null | undefined = !rooted || refused !== null ? undefined : stage === null ? (view?.not_in_flow ?? undefined) : scopeRead.data === undefined ? null : (scopeRead.data.not_in_flow ?? undefined);
  const recordsWords = recordsCount === null ? t("flow.records.openPlain") : recordsCount === undefined ? null : t("flow.records.open", { count: recordsCount });
  const recordsButton =
    recordsWords === null || recordsCount === 0 ? null : (
      <p className="flow-places-records">
        <Button appearance="subtle" aria-haspopup="dialog" onClick={openRecords}>
          {recordsWords}
        </Button>
      </p>
    );

  const stagesShown = useMemo<StagesShown>(() => {
    if (scope === null) {
      return { show: "top" };
    }
    if (lanes.data === undefined) {
      if (lanes.error === null) {
        return { show: "reading" };
      }
      return {
        show: "failed",
        message: apiErrorOf(lanes.error).message,
        retrying: lanes.isFetching,
        onRetry: () => {
          void lanes.refetch({ cancelRefetch: false });
        },
      };
    }
    return {
      show: "lanes",
      all: { place: placeAt({ scope: place.scope, stage: null, select: null }), label: t("flow.allStages"), current: stage === null },
      lanes: lanes.data.stages.map((lane) => ({
        key: stageKey(lane.stage),
        label: stageWords(lane.stage, t),
        place: placeAt({ scope: place.scope, stage: stageAsked(lane.stage), select: null }),
        current: stage === stageAsked(lane.stage),
        counts: lane.counts,
      })),
    };
  }, [scope, lanes, placeAt, place.scope, stage, t]);

  const header = (
    <header className="flow-head">
      <h1 id={titleId} ref={heading} tabIndex={-1}>
        {sectionLabel("flow", t)}
      </h1>
      <p className="flow-head-title" title={stageShown === null ? scopeTitle : t("flow.scopeAndStage", { scope: scopeTitle, stage: stageShown })}>
        <bdi>{stageShown === null ? scopeTitle : t("flow.scopeAndStage", { scope: scopeTitle, stage: stageShown })}</bdi>
      </p>
      <Tabs
        label={t("flow.tabsLabel")}
        base={tabsBase}
        selected={tab}
        onSelect={setChosenTab}
        tabs={[
          { id: "canvas", label: t("flow.tabCanvas") },
          { id: "list", label: t("flow.tabList") },
        ]}
      />
      <div className="flow-about">
        <button
          ref={aboutToggle}
          type="button"
          className="button button-quiet"
          aria-expanded={aboutOpen}
          aria-controls={aboutId}
          onClick={() => {
            setAboutOpen(!aboutOpen);
          }}
        >
          <Icon name="info" />
          <span>{noted === 0 ? t("flow.about") : t("flow.aboutNotes", { count: noted })}</span>
        </button>
        <div id={aboutId} className="flow-about-body" hidden={!aboutOpen}>
          <p>
            {aboutOf("flow", t)} {t("flow.readOnly")}
          </p>
          {notes.length > 0 && (
            <ul className="note-list" aria-label={t("flow.notesLabel")}>
              {notes.map((note) => (
                <li key={note} lang="en">
                  {note}
                </li>
              ))}
            </ul>
          )}
          {leftOut !== null && (
            <p className="note">
              <Icon name="info" />
              <span>{leftOut}</span>
            </p>
          )}
        </div>
      </div>
    </header>
  );

  // The canvas region's body: a skeleton, the daemon's refusal, the empty words, or the picture.
  let picture;
  const busy = !moduleKnown || (scope === undefined && topFailure === null) || drawing.isFetching;
  if (!moduleKnown || (scope === undefined && topFailure === null) || (scope !== undefined && view === undefined && failure === null)) {
    picture = (
      <div aria-busy="true">
        <Skeleton label={t("flow.reading", { project })} lines={6} />
      </div>
    );
  } else if (scope === undefined && topFailure !== null) {
    picture = (
      <ErrorPanel
        title={t("flow.readFailed")}
        message={apiErrorOf(topFailure).message}
        retrying={topRead.isFetching}
        attempt={topRead.errorUpdateCount}
        onRetry={() => {
          void topRead.refetch({ cancelRefetch: false });
        }}
      />
    );
  } else if (view === undefined) {
    picture = (
      <ErrorPanel
        title={t("flow.readFailed")}
        message={failure === null ? "" : apiErrorOf(failure).message}
        retrying={drawing.isFetching}
        attempt={drawing.errorUpdateCount}
        onRetry={() => {
          void drawing.refetch({ cancelRefetch: false });
        }}
      />
    );
  } else if (refused !== null) {
    picture = (
      <ErrorPanel
        title={t("flow.readFailed")}
        message={refused}
        retrying={drawing.isFetching}
        attempt={drawing.dataUpdatedAt}
        onRetry={() => {
          void drawing.refetch({ cancelRefetch: false });
        }}
      />
    );
  } else if (boxes.length === 0 && stage === null && scopeName !== null && ownNode?.in_flow === false && (view.not_in_flow ?? 0) > 0) {
    // A cut that leaves only the scope's own node, at a scope where nothing is in the flow: its documents are the AI's.
    const count = view.not_in_flow ?? 0;
    picture = (
      <EmptyState
        title={t("flow.emptyAside")}
        level={2}
        description={<p>{t("flow.emptyAsideMore", { count })}</p>}
        primary={
          <Button appearance="primary" aria-haspopup="dialog" onClick={openRecords}>
            {t("flow.records.open", { count })}
          </Button>
        }
      />
    );
  } else if (boxes.length === 0) {
    // A scope in the flow whose own documents are not: the scope's empty words, its records a click away.
    const aside = stage === null && scopeName !== null ? (view.not_in_flow ?? 0) : 0;
    picture = (
      <EmptyState
        title={stage !== null ? emptyStageWords(stage, t) : scopeName === null ? t("flow.emptyProject") : t("flow.emptyScope", { name: scopeTitle })}
        level={2}
        secondary={
          aside > 0 ? (
            <Button aria-haspopup="dialog" onClick={openRecords}>
              {t("flow.records.open", { count: aside })}
            </Button>
          ) : undefined
        }
      />
    );
  } else if (layout !== null) {
    picture = (
      <RegionBoundary name={t("flow.tabCanvas")}>
        <Suspense fallback={<Skeleton label={t("flow.reading", { project })} lines={4} />}>
          <FlowCanvas
            key={drawingKey}
            layout={layout}
            types={types}
            inLane={rooted}
            selected={selName}
            selectedEdge={laidSelected}
            marked={marked}
            named={named}
            centring={centring}
            focusFirst={spaceInto !== null && spaceInto === scopeName}
            wheelPans={!stacked}
            label={t("flow.canvasLabel")}
            onCommand={onCommand}
            onEdge={onEdge}
          />
        </Suspense>
      </RegionBoundary>
    );
  }

  const lines = (
    <>
      {failedAgain && (
        <div className="flow-failed">
          <p className="note" role="alert">
            <Icon name="alert" />
            <span>
              {t("flow.readAgainFailed", { message: apiErrorOf(drawing.error).message })} {statesHiddenWords(t)}
            </span>
          </p>
          <button
            type="button"
            className="button"
            aria-disabled={drawing.isFetching}
            onClick={
              drawing.isFetching
                ? undefined
                : () => {
                    void drawing.refetch({ cancelRefetch: false });
                  }
            }
          >
            <Icon name="retry" />
            <span>{drawing.isFetching ? t("ui.retrying") : t("ui.retry")}</span>
          </button>
        </div>
      )}
      {seekFailure !== null && seekFailure.key === placeKey && (
        <p className="note" role="alert">
          <Icon name="alert" />
          <span>{t("flow.seekFailed", { name: seekFailure.name, message: seekFailure.message })}</span>
        </p>
      )}
      {lost !== null && lost.key === placeKey && (
        <p className="note">
          <Icon name="info" />
          <span>{t("flow.lost", { name: lost.name })}</span> <a href={sectionHash(project, "tree", lost.name)}>{t("flow.lostTree", { name: lost.name })}</a>
        </p>
      )}
    </>
  );

  const held = ownNode ?? scopeNode.data?.nodes[0];
  const scopeShown: ScopeShown = {
    name: scopeName,
    title: scopeTitle,
    kind: held?.kind ?? null,
    where: held === undefined ? null : `${held.path}:${String(held.line)}`,
    counts: ownNode?.counts ?? (rooted ? (lanes.data?.counts ?? null) : null),
    read: scopeName === null ? null : scopeNode,
  };
  const gapsShown = shown !== undefined && refused === null;
  const boxShown = selBox ?? null;
  const madeNote =
    made === null ? null : (
      <p className="note">
        <Icon name="info" />
        <span>
          <span className="mono">{t("flow.cardMade", { id: made })}</span> <a href={sectionHash(project, "board", made)}>{t("flow.cardOnBoard")}</a>
        </span>
      </p>
    );
  const panelBody = (
    <>
      {keptBox !== null && (
        <div hidden={boxShown === null || nodeName(boxShown) !== nodeName(keptBox)}>
          <BoxView
            key={nodeName(keptBox)}
            project={project}
            node={boxShown !== null && nodeName(boxShown) === nodeName(keptBox) ? boxShown : keptBox}
            inLane={rooted}
            scopeName={scopeName}
            scopeTitle={scopeTitle}
            titleOf={titleOf}
            named={named}
            headingRef={boxShown !== null ? panelHeading : undefined}
            enterPlace={enterPlace}
            onBack={clearSelection}
            onNewCard={newCard}
          >
            {boxShown !== null && nodeName(boxShown) === nodeName(keptBox) ? madeNote : null}
          </BoxView>
        </div>
      )}
      {boxShown === null && selEdge !== undefined && (
        <EdgeView
          project={project}
          edge={selEdge}
          scopeTitle={scopeTitle}
          headingRef={panelHeading}
          isGroupName={(name) => boxes.some((node) => nodeName(node) === name && isGroup(node))}
          selectPlace={(name) => (boxes.some((node) => nodeName(node) === name) ? selectPlace(name) : null)}
          enterPlace={enterPlace}
          onBack={clearSelection}
        />
      )}
      {boxShown === null && selEdge === undefined && scope !== undefined && (
        <ScopeView
          project={project}
          scope={scopeShown}
          headingRef={panelHeading}
          onNewCard={newCard}
        >
          {madeNote}
        </ScopeView>
      )}
    </>
  );

  /** New card from the panel: the scope or the box its target. */
  function newCard(target: string, trigger: HTMLElement) {
    setMade(null);
    setScopeCard({ target, trigger });
  }

  const afterCard = (trigger: HTMLElement | null) => (answer: CardWrite | null) => {
    setHand(null);
    setScopeCard(null);
    if (answer !== null && answer.id !== null) {
      setMade(answer.id);
      announce(t("flow.cardMade", { id: answer.id }));
    }
    if (trigger?.isConnected === true) {
      trigger.focus();
    }
  };

  return (
    <section ref={studio} className="view flow-studio" aria-labelledby={titleId} aria-busy={busy} onKeyDown={onStudioKeyDown}>
      {header}
      <FlowPlaces
        project={project}
        here={write({ scope: place.scope, stage: place.stage, select: null })}
        open={placesOpen}
        onToggle={() => {
          setPlacesOpen(!placesOpen);
        }}
        modules={served}
        module={module?.name ?? ""}
        onModule={(name) => {
          onChooseModule?.(name);
        }}
        path={path}
        stages={stagesShown}
        groups={groups}
        records={recordsButton}
      />
      <div className="flow-centre">
        <div className="flow-view-area">
          <TabPanel base={tabsBase} id="canvas" selected={tab === "canvas"}>
            {tab === "canvas" && (
              <section ref={region} className="flow-canvas-region" aria-label={t("flow.canvasLabel")} tabIndex={-1} onFocus={closeOverlays} onPointerDown={closeOverlays}>
                {lines}
                {picture}
              </section>
            )}
          </TabPanel>
          <TabPanel base={tabsBase} id="list" selected={tab === "list"}>
            {tab === "list" && (
              <div ref={listRegion} className="flow-list-region" role="region" aria-label={t("flow.listLabel")} tabIndex={-1} onFocus={closeOverlays} onPointerDown={closeOverlays}>
                <RegionBoundary name={t("flow.tabList")}>
                  {lines}
                  {landed && refused === null ? (
                    <FlowList
                      boxes={boxes}
                      edges={edges}
                      scope={scopeName}
                      scopeTitle={scopeTitle}
                      stageView={stage !== null}
                      inLane={rooted}
                      named={named}
                      selected={selName}
                      selectedEdge={selKeyText}
                      marked={marked}
                      places={listPlaces}
                      titleOf={titleOf}
                      scrollTo={centring}
                      onSelect={(element) => {
                        origin.current = element;
                      }}
                    />
                  ) : (
                    picture
                  )}
                </RegionBoundary>
              </div>
            )}
          </TabPanel>
          {tab === "canvas" && legendOpen && <FlowLegend id={legendId} types={types} states={boxes.some((node) => node.rollup !== null || node.counts !== null)} />}
        </div>
        <div className="flow-bar">
          {tab === "canvas" && (
            <Button
              ref={legendToggle}
              appearance="subtle"
              spacing="compact"
              icon={legendOpen ? "chevronDown" : "chevronRight"}
              aria-expanded={legendOpen}
              aria-controls={legendId}
              onClick={() => {
                setGapsOpen(false);
                setLegendOpen(!legendOpen);
              }}
            >
              {t("flow.legendButton")}
            </Button>
          )}
          {gapsShown && (
            <Button
              ref={gapsToggle}
              appearance="subtle"
              spacing="compact"
              icon={gapsOpen ? "chevronDown" : "chevronRight"}
              className="flow-gaps-toggle"
              aria-expanded={gapsOpen}
              aria-controls={gapsId}
              onClick={() => {
                setLegendOpen(false);
                setGapsOpen(!gapsOpen);
              }}
            >
              {t("flow.gaps.toggle")} <Lozenge {...gapsLook(shown.gaps, failedAgain, t)} />
            </Button>
          )}
          {tab === "canvas" && layout !== null && layout.notDrawn > 0 && (
            <p className="flow-bar-cut" role="note">
              <Icon name="info" />
              <span>{notDrawnLine(layout.notDrawn, layout.total, t)}</span>
            </p>
          )}
        </div>
        {/* After its toggle in the DOM, so Tab goes from the toggle into it; the grid lays it over the canvas. */}
        {gapsOpen && shown !== undefined && <FlowGaps id={gapsId} gaps={shown.gaps} unread={failedAgain} rooted={rooted} marked={marked} onActivate={activate} onHand={handOff} />}
      </div>
      <aside className="flow-panel" aria-label={t("flow.panelLabel")}>
        {panelBody}
      </aside>
      {hand !== null && (
        <div
          onKeyDown={(event) => {
            event.stopPropagation();
          }}
        >
          <NewCardDialog project={project} preset={hand.preset} onClose={() => {
            afterCard(hand.trigger)(null);
          }} onDone={afterCard(hand.trigger)} />
        </div>
      )}
      {records !== null && scopeName !== null && (
        <div
          onKeyDown={(event) => {
            event.stopPropagation();
          }}
        >
          <FlowRecords project={project} scope={scopeName} scopeTitle={scopeTitle} count={recordsCount === undefined ? null : recordsCount} onClose={closeRecords} />
        </div>
      )}
      {scopeCard !== null && (
        <div
          onKeyDown={(event) => {
            event.stopPropagation();
          }}
        >
          <NewCardDialog project={project} target={scopeCard.target} onClose={() => {
            afterCard(scopeCard.trigger)(null);
          }} onDone={afterCard(scopeCard.trigger)} />
        </div>
      )}
    </section>
  );
}

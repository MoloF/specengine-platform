import {
  BaseEdge,
  EdgeLabelRenderer,
  Handle,
  MarkerType,
  MiniMap,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  useStore,
  useStoreApi,
  type AriaLabelConfig,
  type Edge,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { memo, useCallback, useEffect, useId, useMemo, useRef, useState, type FocusEvent, type KeyboardEvent, type MouseEvent } from "react";
import { HANDLE_SIZE } from "../graph/layout";
import { useTranslate, type Translate } from "../i18n";
import { TraceBar } from "../trace/TraceBar";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { focusIsLost } from "../ui/focus";
import { hasModifier, hotkeyOf, isTextField, letterOf } from "../ui/keys";
import { durationOf, EDGE_HIT, fitOf, FLOW_CLICK_DISTANCE, FLOW_PINCH_MIN_ZOOM, inView, type Rect } from "./geometry";
import { boxWords, countsText, dashOf, dividerWords, edgeName, edgeShownWords, gapMark, holdsText, isNoCard, isRepeat, rollupLook, startWord, typeColour, type GapMark } from "./labels";
import { BAND_CAPTION_WIDTH, boxAfterKey, isGroup, LANE_PAD, type FlowBox, type FlowDivider, type FlowLane, type FlowLayout, type LaidEdge } from "./layout";

// The Flow's canvas (`docs/canon/spec-flow-layout.md` "Layout"; ui/README.md "Screen rules", Canvas):
// React Flow draws one read's boxes and edges where the layout put them, the walk left to right, its
// stage dividers left of their columns; nothing drags, connects or deletes. A box says its title, never its ID. An edge with an end off the walk shows only while that
// end or the edge is selected, so no bundle stands on the canvas. A click on a box or an arrow selects
// it, a group is entered, never opened in place, so no frame lies under an arrow. The wheel pans and a
// pinch or the zoom key zooms; the picture fits once, when it is drawn, and then only the owner, `c`
// and a gap moves it. Every key option is null and React Flow's words are the catalog's; the boxes
// keep one tab stop the arrow keys move, and no arrow takes one: the keyboard reaches arrows through
// the List.

/** React Flow's words for what this canvas draws, in the catalog's: no controls are drawn, so none is named. */
function ariaLabels(t: Translate): Partial<AriaLabelConfig> {
  return {
    "node.a11yDescription.default": t("flow.aria.node"),
    "node.a11yDescription.keyboardDisabled": t("flow.aria.node"),
    "node.a11yDescription.ariaLiveMessage": () => "",
    "edge.a11yDescription.default": t("flow.aria.edge"),
    "minimap.ariaLabel": t("flow.aria.minimap"),
    "handle.ariaLabel": t("flow.aria.handle"),
  };
}

/** What a box says besides its node. */
type BoxMarks = {
  selected: boolean;
  marked: boolean;
  gap: GapMark | null;
};

type BoxData = { box: FlowBox; inLane: boolean } & BoxMarks;
type BandData = { lane: FlowLane };
type DividerData = { divider: FlowDivider };
type BoxNode = Node<BoxData, "box">;
type CanvasNode = BoxNode | Node<BandData, "band"> | Node<DividerData, "divider">;
type LinkData = { laid: LaidEdge; emphasis: boolean };
type LinkEdge = Edge<LinkData, "link">;

/** Marks the caret: a click there enters the group, anywhere else on the box selects. */
const TOGGLE = "data-flow-toggle";

/** A box's two ends, invisible: every path is the layout's, these only let React Flow draw the edge. */
function Handles() {
  return (
    <>
      <Handle type="source" position={Position.Right} id="out" isConnectable={false} aria-hidden="true" />
      <Handle type="target" position={Position.Left} id="in" isConnectable={false} aria-hidden="true" />
    </>
  );
}

/** Start or Entry, End. */
function WalkMarks({ box, inLane }: { box: FlowBox; inLane: boolean }) {
  const t = useTranslate();
  return (
    <>
      {box.node.start && (
        <span className="flow-mark flow-mark-start">
          <Icon name="flowStart" />
          {startWord(inLane, t)}
        </span>
      )}
      {box.node.end && (
        <span className="flow-mark flow-mark-end">
          <Icon name="flowEnd" />
          {t("flow.end")}
        </span>
      )}
    </>
  );
}

/** Selected, Marked: a tab on the box's edge, label and icon. */
function StateTabs({ data }: { data: BoxMarks }) {
  const t = useTranslate();
  if (!data.selected && !data.marked) {
    return null;
  }
  return (
    <span className="flow-tabs">
      {data.marked && (
        <span className="flow-tab flow-tab-marked">
          <Icon name="flowMarked" />
          {t("flow.marked")}
        </span>
      )}
      {data.selected && (
        <span className="flow-tab flow-tab-selected">
          <Icon name="selected" />
          {t("flow.selected")}
        </span>
      )}
    </span>
  );
}

/**
 * What the read says of a box's work: a step its rollup, a group its served counts and the bar they
 * make. No read of them, no line: a box never shows a state nobody served.
 */
function WorkState({ box }: { box: FlowBox }) {
  const t = useTranslate();
  const { node } = box;
  if (isGroup(node)) {
    if (node.counts === null) {
      return null;
    }
    const words = countsText(node.counts, t);
    return (
      <>
        <p className="flow-box-state" title={words}>
          {words}
        </p>
        <TraceBar counts={node.counts} className="flow-box-bar" />
      </>
    );
  }
  return (
    node.rollup !== null && (
      <p className="flow-box-state">
        <Badge look={rollupLook(node.rollup, t)} />
      </p>
    )
  );
}

/** A step or a group: its title, what marks it, then its work; no ID, no kind (flow-scenario D12). A step with no card is hollow. */
const BoxView = memo(function BoxView({ data }: NodeProps<BoxNode>) {
  const t = useTranslate();
  const { box, inLane } = data;
  const { node } = box;
  const group = isGroup(node);
  const marked = group || data.gap !== null || node.start || node.end;
  const hollow = !group && node.rollup !== null && isNoCard(node.rollup);
  const classes = [
    "flow-box",
    node.start && "is-start",
    node.end && "is-end",
    data.selected && "is-selected",
    data.marked && "is-marked",
    data.gap !== null && "has-gap",
    group && "is-group",
    hollow && "is-hollow",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <div className={classes} aria-hidden="true">
      <Handles />
      <StateTabs data={data} />
      <p className="flow-box-words">
        {group && (
          <span className="flow-caret" {...{ [TOGGLE]: "" }}>
            <Icon name="chevronRight" />
          </span>
        )}
        <bdi className="flow-box-title">{boxWords(node)}</bdi>
      </p>
      {marked && (
        <p className="flow-box-marks">
          {data.gap !== null && (
            <span className="flow-mark flow-mark-gap">
              <Icon name="flowGap" />
              {data.gap.shown}
            </span>
          )}
          <WalkMarks box={box} inLane={inLane} />
          {group && <span className="flow-mark">{holdsText(node.holds, t)}</span>}
        </p>
      )}
      <WorkState box={box} />
    </div>
  );
});

/** The band's caption: what it is, how many it holds, why they stand apart. */
const BandCaption = memo(function BandCaption({ data }: NodeProps<Node<BandData, "band">>) {
  const t = useTranslate();
  const count = data.lane.band.reduce((sum, shelf) => sum + shelf.length, 0);
  return (
    <p className="flow-band" aria-hidden="true">
      <span className="flow-band-name">{t("flow.offWalk")}</span> <span className="flow-band-count">{count}</span> <span className="flow-band-about">{t("flow.offWalkAbout")}</span>
    </p>
  );
});

/** A stage divider: a rule down the picture, the stage words raw at its top, two lines at most; hidden from assistive technology, never focused. */
const DividerView = memo(function DividerView({ data }: NodeProps<Node<DividerData, "divider">>) {
  return (
    <p className="flow-divider" aria-hidden="true">
      <bdi className="flow-divider-words">{dividerWords(data.divider.stages)}</bdi>
    </p>
  );
});

/** An edge: the layout's path, its type's colour, its dash, its words where it has any, and a 12 px band a click takes. */
const LinkView = memo(function LinkView({ id, markerEnd, style, data }: EdgeProps<LinkEdge>) {
  const t = useTranslate();
  if (data === undefined) {
    return null;
  }
  const words = edgeShownWords(data.laid.edge, t);
  return (
    <>
      <BaseEdge id={id} path={data.laid.path} markerEnd={markerEnd} style={style} interactionWidth={EDGE_HIT} />
      {words.length > 0 && (
        <EdgeLabelRenderer>
          <div
            className={`flow-edge-label${data.emphasis ? " is-emphasis" : ""}`}
            aria-hidden="true"
            style={{ transform: `translate(-50%, -50%) translate(${String(data.laid.label.x)}px, ${String(data.laid.label.y)}px)` }}
          >
            {words.map((word, index) => (
              <span key={`${String(index)}-${word}`} className="flow-edge-word">
                {word}
              </span>
            ))}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
});

const NODE_TYPES = { box: BoxView, band: BandCaption, divider: DividerView };
const EDGE_TYPES = { link: LinkView };

/** A box's accessible name: its words, its stage, marks and gaps (flow-scenario "Stage dividers"); its ID and kind stand in the panel. */
export function boxName(box: FlowBox, marks: BoxMarks, inLane: boolean, t: Translate): string {
  const { node } = box;
  const parts = [boxWords(node)];
  if (node.stage !== null) {
    parts.push(node.stage);
  }
  if (node.start) {
    parts.push(startWord(inLane, t));
  }
  if (node.end) {
    parts.push(t("flow.end"));
  }
  if (isGroup(node)) {
    parts.push(t("flow.said.group", { holds: holdsText(node.holds, t) }));
    if (node.counts !== null) {
      parts.push(countsText(node.counts, t));
    }
  } else if (node.rollup !== null) {
    parts.push(rollupLook(node.rollup, t).label);
  }
  if (marks.gap !== null) {
    parts.push(marks.gap.said);
  }
  if (marks.marked) {
    parts.push(t("flow.said.marked"));
  }
  if (marks.selected) {
    parts.push(t("flow.said.selected"));
  }
  return parts.join(", ");
}

function handlesOf(box: FlowBox) {
  const y = box.height / 2 - HANDLE_SIZE / 2;
  return [
    { id: "out", type: "source" as const, position: Position.Right, x: box.width - HANDLE_SIZE / 2, y, width: HANDLE_SIZE, height: HANDLE_SIZE },
    { id: "in", type: "target" as const, position: Position.Left, x: -HANDLE_SIZE / 2, y, width: HANDLE_SIZE, height: HANDLE_SIZE },
  ];
}

function nodeOf(box: FlowBox, marks: BoxMarks, inLane: boolean, tabStop: boolean, t: Translate): BoxNode {
  return {
    id: box.id,
    type: "box",
    position: { x: box.x, y: box.y },
    width: box.width,
    height: box.height,
    handles: handlesOf(box),
    data: { box, inLane, ...marks },
    ariaLabel: boxName(box, marks, inLane, t),
    domAttributes: { tabIndex: tabStop ? 0 : -1 },
    zIndex: 2,
    draggable: false,
    selectable: false,
    connectable: false,
    deletable: false,
  };
}

/**
 * The words of the band and of a divider: no box, no tab stop, hidden from assistive technology (the
 * List and the boxes' names say it), and no pointer: React Flow gives every node `pointer-events: all`
 * once a node takes clicks, then spreads its style over it, so a press meant for a box or an arrow
 * under these words reaches it (docs/features/handoff-progress.md "Acceptance criteria", C-4).
 */
const WORDS_ONLY = {
  zIndex: 2,
  draggable: false,
  selectable: false,
  connectable: false,
  deletable: false,
  focusable: false,
  domAttributes: { "aria-hidden": true },
  style: { pointerEvents: "none" },
} as const;

/** The caption's height: two lines of its words, which wrap in a narrow picture. */
const BAND_CAPTION_HEIGHT = 32;

function bandNode(lane: FlowLane, x: number, y: number, width: number): CanvasNode {
  return { id: "band", type: "band", position: { x, y }, width, height: BAND_CAPTION_HEIGHT, data: { lane }, ...WORDS_ONLY };
}

/** A divider's node: from its rule rightwards over its column, from its words' strip to below the lowest arc track. */
function dividerNode(divider: FlowDivider): CanvasNode {
  return { id: divider.id, type: "divider", position: { x: divider.x, y: divider.y }, width: divider.width, height: divider.height, data: { divider }, ...WORDS_ONLY };
}

/** An arrow; its name reads forwards, from its source box's words to its target's (E-1). */
function edgeOf(laid: LaidEdge, ends: { from: FlowBox | undefined; to: FlowBox | undefined }, types: readonly string[], emphasis: boolean, t: Translate): LinkEdge {
  const colour = typeColour(laid.edge.type, types);
  return {
    id: laid.id,
    type: "link",
    source: laid.source,
    target: laid.target,
    sourceHandle: "out",
    targetHandle: "in",
    data: { laid, emphasis },
    ariaLabel: edgeName(laid.edge, ends.from === undefined ? null : boxWords(ends.from.node), ends.to === undefined ? null : boxWords(ends.to.node), t),
    className: emphasis ? "is-emphasis" : undefined,
    style: { stroke: colour, strokeDasharray: dashOf(laid.edge, types), ...(isRepeat(laid.edge) ? { strokeWidth: "var(--flow-repeat-width)" } : {}) },
    markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14, color: colour },
    zIndex: 1,
    selectable: false,
    deletable: false,
    reconnectable: false,
  };
}

/** A strip in the corner, so it covers no box of a short canvas. */
const MINIMAP_SIZE = { width: 160, height: 48 };

/**
 * The canvas's commands for a box, decided by the view: a click selects, Enter also moves focus to
 * the panel; a caret or a double-click enters a group, and Space enters it with focus to the new
 * picture's first box.
 */
export type BoxCommand = "select" | "details" | "enter" | "enterByKey" | "tree";

const COMMANDS: Record<string, BoxCommand> = { Enter: "details", " ": "enterByKey", o: "tree" };

/** A name to bring into view once it is drawn; a new object each time. `always`: to the centre, even if it is already in view. */
export interface Centring {
  name: string;
  always: boolean;
}

export interface FlowCanvasProps {
  layout: FlowLayout;
  /** The sorted types of the drawing read: each edge's colour and plain dash. */
  types: readonly string[];
  /** A scope's lane: a start is its Entry; at the top, its Start. */
  inLane: boolean;
  /** The selected box's name. */
  selected: string | null;
  /** The selected arrow's laid id. */
  selectedEdge: string | null;
  marked: ReadonlySet<string>;
  /** The names a gap row of the drawing read names. */
  named: ReadonlySet<string>;
  centring: Centring | null;
  /** Once drawn, focus goes to the picture's first box: a group was entered by Space. */
  focusFirst: boolean;
  /** The wheel pans the picture; else it scrolls the page, and a drag pans (under 64rem). */
  wheelPans: boolean;
  /** The drawing's name. */
  label: string;
  onCommand: (box: FlowBox, command: BoxCommand, origin: HTMLElement | null) => void;
  /** An arrow clicked: it holds no focus, so Esc takes focus back to the canvas. */
  onEdge: (laid: LaidEdge) => void;
}

/** The fit, once: when the canvas first has a size. After that only the owner, `c` and a gap move the view. */
function FitOnce({ layout, touched }: { layout: FlowLayout; touched: { current: boolean } }) {
  const width = useStore((state) => state.width);
  const height = useStore((state) => state.height);
  const store = useStoreApi();
  const { setViewport } = useReactFlow();
  const fitted = useRef(false);
  useEffect(() => {
    if (fitted.current || touched.current || width <= 0 || height <= 0 || store.getState().panZoom === null) {
      return;
    }
    const viewport = fitOf(layout, width, height);
    if (viewport !== null) {
      fitted.current = true;
      void setViewport(viewport);
    }
  }, [layout, width, height, touched, store, setViewport]);
  return null;
}

function Canvas({ layout, types, inLane, selected, selectedEdge, marked, named, centring, focusFirst, wheelPans, label, onCommand, onEdge }: FlowCanvasProps) {
  const t = useTranslate();
  const wrapper = useRef<HTMLDivElement>(null);
  const store = useStoreApi();
  const { setCenter } = useReactFlow();
  const hintId = useId();
  const touched = useRef(false);
  const centred = useRef<Centring | null>(null);
  const firstFocused = useRef(false);
  /** Focus is on a box of this canvas: a re-read taking that box away hands it to the tab stop. */
  const focusInside = useRef(false);
  const [roving, setRoving] = useState<string | null>(null);
  const labels = useMemo(() => ariaLabels(t), [t]);

  const selectedId = useMemo(() => layout.boxes.find((box) => box.name === selected)?.id ?? null, [layout, selected]);
  const firstId = layout.lanes[0]?.columns[0]?.[0] ?? layout.lanes[0]?.band[0]?.[0] ?? null;
  const drawn = useMemo(() => new Set(layout.boxes.map((box) => box.id)), [layout]);
  const rovingId = roving !== null && drawn.has(roving) ? roving : (selectedId ?? firstId);

  const nodes = useMemo<CanvasNode[]>(() => {
    const boxes = layout.boxes.map((box) =>
      nodeOf(
        box,
        {
          selected: box.name === selected,
          marked: marked.has(box.name),
          gap: gapMark(box.node, named, t),
        },
        inLane,
        box.id === rovingId,
        t,
      ),
    );
    const bands = layout.lanes.flatMap((lane) => {
      const first = layout.boxes.find((box) => box.id === lane.band[0]?.[0]);
      // The caption's least width, clamped to the picture: it never runs past the layout's right edge.
      return lane.bandY === null || first === undefined ? [] : [bandNode(lane, first.x, lane.bandY, Math.min(Math.max(layout.width - first.x - LANE_PAD, BAND_CAPTION_WIDTH), layout.width - first.x))];
    });
    const dividers = layout.lanes.flatMap((lane) => lane.dividers.map(dividerNode));
    return [...dividers, ...boxes, ...bands];
  }, [layout, selected, marked, named, inLane, rovingId, t]);
  // An edge with an end off the walk stands only while that end or the edge is selected (flow-scenario D9).
  const edges = useMemo(() => {
    const byId = new Map(layout.boxes.map((box) => [box.id, box]));
    return layout.edges
      .filter((laid) => !laid.offWalk || laid.id === selectedEdge || (selectedId !== null && (laid.source === selectedId || laid.target === selectedId)))
      .map((laid) => edgeOf(laid, { from: byId.get(laid.source), to: byId.get(laid.target) }, types, laid.id === selectedEdge || (selectedId !== null && (laid.source === selectedId || laid.target === selectedId)), t));
  }, [layout, types, selectedEdge, selectedId, t]);

  const boxElement = useCallback((id: string): HTMLElement | null => {
    const found = Array.from(wrapper.current?.querySelectorAll<HTMLElement>(".react-flow__node") ?? []);
    return found.find((element) => element.dataset.id === id) ?? null;
  }, []);

  const centreOn = useCallback(
    (box: Rect) => {
      const [, , zoom] = store.getState().transform;
      void setCenter(box.x + Math.min(box.width, 400) / 2, box.y + box.height / 2, { zoom, duration: durationOf("--duration-normal") });
    },
    [setCenter, store],
  );

  // A Space into a group: the new picture's first box takes the focus, once React Flow has drawn it.
  const nodesDrawn = useStore((state) => state.nodeLookup.size);
  useEffect(() => {
    if (!focusFirst || firstFocused.current || firstId === null) {
      return;
    }
    const element = boxElement(firstId);
    if (element !== null) {
      firstFocused.current = true;
      element.focus();
    }
  }, [focusFirst, firstId, boxElement, nodesDrawn]);

  // A gap item's first member: centred once it is drawn and the canvas has a size.
  const canvasWidth = useStore((state) => state.width);
  useEffect(() => {
    if (centring === null || centred.current === centring) {
      return;
    }
    const box = layout.boxes.find((candidate) => candidate.name === centring.name);
    const { transform, width, height } = store.getState();
    if (box === undefined || canvasWidth <= 0) {
      return;
    }
    centred.current = centring;
    const [x, y, zoom] = transform;
    if (centring.always || !inView(box, { x, y, zoom, width, height })) {
      touched.current = true;
      centreOn(box);
    }
  }, [centring, layout, centreOn, store, canvasWidth]);

  // A focused box a re-read took away hands focus to the tab stop, never the body.
  useEffect(() => {
    if (rovingId !== null && focusInside.current && focusIsLost()) {
      boxElement(rovingId)?.focus();
    }
  }, [layout, rovingId, boxElement]);

  // The minimap only covers a picture already whole in view.
  const pictureWhole = useStore((state) => {
    const [x, y, zoom] = state.transform;
    return inView({ x: 0, y: 0, width: layout.width, height: layout.height }, { x, y, zoom, width: state.width, height: state.height });
  });

  function boxOf(target: EventTarget | null): FlowBox | undefined {
    const id = target instanceof Element ? target.closest<HTMLElement>(".react-flow__node")?.dataset.id : undefined;
    return id === undefined ? undefined : layout.boxes.find((candidate) => candidate.id === id);
  }

  /**
   * A box focused from the keyboard (`:focus-visible`, as React Flow's own node wrapper tells it), out
   * of sight, is centred at the current zoom. A mouse, touch or pen press focuses without it, and moves
   * nothing, so the box stays under the pointer for its click
   * (docs/features/handoff-progress.md "Acceptance criteria", C-3).
   */
  function onFocus(event: FocusEvent<HTMLElement>) {
    const box = boxOf(event.target);
    focusInside.current = box !== undefined;
    if (box === undefined) {
      return;
    }
    if (box.id !== rovingId) {
      setRoving(box.id);
    }
    if (!(event.target instanceof Element) || !event.target.matches(":focus-visible")) {
      return;
    }
    const { transform, width, height } = store.getState();
    const [x, y, zoom] = transform;
    if (!inView(box, { x, y, zoom, width, height })) {
      touched.current = true;
      centreOn(box);
    }
  }

  function onBlur(event: FocusEvent<HTMLElement>) {
    if (event.relatedTarget !== null && !(wrapper.current?.contains(event.relatedTarget) ?? false)) {
      focusInside.current = false;
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    const box = boxOf(event.target);
    if (box === undefined) {
      return;
    }
    const origin = boxElement(box.id);
    if (letterOf(event) === "c") {
      event.preventDefault();
      touched.current = true;
      centreOn(box);
      return;
    }
    const key = hotkeyOf(event);
    const command = Object.hasOwn(COMMANDS, key) ? COMMANDS[key] : undefined;
    if (command !== undefined) {
      event.preventDefault();
      onCommand(box, command, origin);
      return;
    }
    if (!["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"].includes(event.key)) {
      return;
    }
    event.preventDefault();
    const next = boxAfterKey(layout, box.id, event.key);
    if (next !== null) {
      setRoving(next);
      boxElement(next)?.focus();
    }
  }

  function onNodeClick(event: MouseEvent, id: string) {
    const box = layout.boxes.find((candidate) => candidate.id === id);
    if (box === undefined) {
      return;
    }
    setRoving(id);
    const caret = event.target instanceof Element && event.target.closest(`[${TOGGLE}]`) !== null;
    onCommand(box, caret && isGroup(box.node) ? "enter" : "select", boxElement(id));
  }

  function onNodeDoubleClick(id: string) {
    const box = layout.boxes.find((candidate) => candidate.id === id);
    if (box !== undefined && isGroup(box.node)) {
      onCommand(box, "enter", boxElement(id));
    }
  }

  return (
    <div ref={wrapper} className="graph-canvas flow-canvas" data-wheel={wheelPans ? "pans" : "scrolls"} onKeyDown={onKeyDown} onFocus={onFocus} onBlur={onBlur}>
      <p id={hintId} className="sr-only">
        {t("flow.canvasHint")}
      </p>
      <ReactFlow<CanvasNode, LinkEdge>
        nodes={nodes}
        edges={edges}
        nodeTypes={NODE_TYPES}
        edgeTypes={EDGE_TYPES}
        aria-label={label}
        aria-describedby={hintId}
        zIndexMode="manual"
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable={false}
        edgesFocusable={false}
        edgesReconnectable={false}
        selectionOnDrag={false}
        zoomOnDoubleClick={false}
        zoomOnScroll={false}
        panOnScroll={wheelPans}
        preventScrolling={wheelPans}
        disableKeyboardA11y
        deleteKeyCode={null}
        selectionKeyCode={null}
        multiSelectionKeyCode={null}
        panActivationKeyCode={null}
        proOptions={{ hideAttribution: true }}
        ariaLabelConfig={labels}
        defaultMarkerColor={null}
        minZoom={FLOW_PINCH_MIN_ZOOM}
        maxZoom={2}
        paneClickDistance={FLOW_CLICK_DISTANCE}
        onMoveStart={(event) => {
          if (event !== null) {
            touched.current = true;
          }
        }}
        onNodeClick={(event, node) => {
          onNodeClick(event, node.id);
        }}
        onNodeDoubleClick={(_event, node) => {
          onNodeDoubleClick(node.id);
        }}
        onEdgeClick={(_event, edge) => {
          const laid = edge.data?.laid;
          if (laid !== undefined) {
            onEdge(laid);
          }
        }}
      >
        {/* Bottom left, and it takes no pointer (app.css): a box under it takes its click; it neither pans nor zooms, so it loses nothing. */}
        {!pictureWhole && <MiniMap position="bottom-left" style={MINIMAP_SIZE} pannable={false} zoomable={false} />}
        <FitOnce layout={layout} touched={touched} />
      </ReactFlow>
    </div>
  );
}

/** The read-only canvas of one read: keyed by the view per picture, so each picture fits once. */
export function FlowCanvas(props: FlowCanvasProps) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}

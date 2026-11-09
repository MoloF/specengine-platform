import {
  BaseEdge,
  ControlButton,
  Controls,
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
import { memo, useCallback, useEffect, useId, useMemo, useRef, type CSSProperties, type FocusEvent, type KeyboardEvent, type MouseEvent } from "react";
import { HANDLE_SIZE } from "../graph/layout";
import { useTranslate, type Translate } from "../i18n";
import { TraceBar } from "../trace/TraceBar";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { focusIsLost } from "../ui/focus";
import { hasModifier, hotkeyOf, isTextField, letterOf } from "../ui/keys";
import { durationOf, fitOf, fittedCanvasHeight, inView, type Rect } from "./geometry";
import { boxWords, countsText, dashOf, edgeName, edgeWords, gapMark, holdsText, isNoCard, isRepeat, rollupLook, startWord, typeColour, type GapMark } from "./labels";
import { boxAfterKey, enclosingHead, HEAD_HEIGHT, LANE_PAD, type FlowBox, type FlowLane, type FlowLayout, type LaidEdge } from "./layout";
import { outsideStage } from "./timeline";

// The Flow's canvas (ui/README.md "Screen rules", Canvas; docs/features/flow-canvas.md): React Flow
// draws the layout's boxes, frames and edges where the layout put them; nothing drags, connects,
// selects or deletes. Every key option is null, its own keyboard and words are off or replaced, the
// wheel scrolls the page, and the boxes keep one tab stop that the arrow keys move. The stacking is
// the layout's, by nesting: a frame, its lane's edges, its lane's boxes.

/** Every key of React Flow's `defaultAriaLabelConfig`, replaced: nothing here can be changed. */
export const FLOW_ARIA_LABELS = {
  "node.a11yDescription.default": "A box of the flow. Arrow keys go to other boxes; Enter selects it; Space opens or closes a group.",
  "node.a11yDescription.keyboardDisabled": "A box of the flow. Arrow keys go to other boxes; Enter selects it; Space opens or closes a group.",
  "node.a11yDescription.ariaLiveMessage": () => "",
  "edge.a11yDescription.default": "A link between two boxes; the List view names every link as text.",
  "controls.ariaLabel": "Canvas zoom",
  "controls.zoomIn.ariaLabel": "Zoom in",
  "controls.zoomOut.ariaLabel": "Zoom out",
  "controls.fitView.ariaLabel": "Fit the flow to the view, its text readable",
  "controls.interactive.ariaLabel": "Canvas lock",
  "minimap.ariaLabel": "Overview of the flow",
  "handle.ariaLabel": "Link end",
} satisfies AriaLabelConfig;

/** What a box says besides its node. */
type BoxMarks = {
  selected: boolean;
  marked: boolean;
  opening: boolean;
  gap: GapMark | null;
  /** Outside the stage filter by its own stage: it keeps its place, dimmed. */
  dimmed: boolean;
};

type BoxData = { box: FlowBox } & BoxMarks;
type BandData = { lane: FlowLane };
type BoxNode = Node<BoxData, "box">;
type CanvasNode = BoxNode | Node<BandData, "band">;
type LinkData = { laid: LaidEdge; emphasis: boolean };
type LinkEdge = Edge<LinkData, "link">;

/** An open group's frame is an outline on the minimap: its boxes show through. */
const FRAME_CLASS = "flow-node-frame";

/** Marks the caret: a click there opens or closes, anywhere else on the box selects. */
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

function Caret({ open }: { open: boolean }) {
  return (
    <span className="flow-caret" {...{ [TOGGLE]: "" }}>
      <Icon name={open ? "chevronDown" : "chevronRight"} />
    </span>
  );
}

/** Start or Entry, End. */
function WalkMarks({ box }: { box: FlowBox }) {
  const t = useTranslate();
  return (
    <>
      {box.node.start && (
        <span className="flow-mark flow-mark-start">
          <Icon name="flowStart" />
          {startWord(box.group !== null, t)}
        </span>
      )}
      {box.node.end && (
        <span className="flow-mark flow-mark-end">
          <Icon name="flowEnd" />
          End
        </span>
      )}
    </>
  );
}

/** Selected, Marked, Opening: a tab on the box's edge, label and icon. */
function StateTabs({ data }: { data: BoxMarks }) {
  if (!data.selected && !data.marked && !data.opening) {
    return null;
  }
  return (
    <span className="flow-tabs">
      {data.opening && <span className="flow-tab">Opening</span>}
      {data.marked && (
        <span className="flow-tab flow-tab-marked">
          <Icon name="flowMarked" />
          Marked
        </span>
      )}
      {data.selected && (
        <span className="flow-tab flow-tab-selected">
          <Icon name="selected" />
          Selected
        </span>
      )}
    </span>
  );
}

function boxClasses(base: string, box: FlowBox, data: BoxMarks): string {
  return [
    base,
    box.node.start && "is-start",
    box.node.end && "is-end",
    data.selected && "is-selected",
    data.marked && "is-marked",
    data.gap !== null && "has-gap",
    data.dimmed && "is-dimmed",
    box.group !== null && "in-lane",
  ]
    .filter(Boolean)
    .join(" ");
}

/**
 * What the read says of a box's work: a step its rollup, a closed group its served counts and the bar
 * they make. No read of them, no line: a box never shows a state nobody served.
 */
function WorkState({ box }: { box: FlowBox }) {
  const t = useTranslate();
  const { node } = box;
  if (box.form === "closed") {
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

/** A step or a closed group: its words, name, kind, what marks it, then its work. A step with no card is hollow. */
function StepBox({ data }: { data: BoxData }) {
  const t = useTranslate();
  const { box } = data;
  const { node } = box;
  const closed = box.form === "closed";
  const marked = closed || data.gap !== null || node.start || node.end;
  const hollow = !closed && node.rollup !== null && isNoCard(node.rollup);
  return (
    <div className={`${boxClasses("flow-box", box, data)}${closed ? " is-group" : ""}${hollow ? " is-hollow" : ""}`} aria-hidden="true">
      <Handles />
      <StateTabs data={data} />
      <p className="flow-box-words">
        {closed && <Caret open={false} />}
        <span className="flow-box-title">{boxWords(node)}</span>
      </p>
      <p className="flow-box-name">
        <span className="mono flow-box-id">{box.name}</span>
        {node.kind !== null && <span className="mono flow-box-kind">{node.kind}</span>}
      </p>
      {marked && (
        <p className="flow-box-marks">
          {data.gap !== null && (
            <span className="flow-mark flow-mark-gap">
              <Icon name="flowGap" />
              {data.gap.shown}
            </span>
          )}
          <WalkMarks box={box} />
          {closed && <span className="flow-mark">{holdsText(node.holds, t)}</span>}
        </p>
      )}
      <WorkState box={box} />
    </div>
  );
}

/** An open group: its head over the frame its lane sits in. */
function FrameBox({ data }: { data: BoxData }) {
  const t = useTranslate();
  const { box } = data;
  const { node } = box;
  return (
    <div className={`${boxClasses("flow-frame", box, data)} nesting-${String(box.nesting % 2)}`} aria-hidden="true">
      <Handles />
      <StateTabs data={data} />
      <p className="flow-head">
        <Caret open />
        <span className="flow-head-title">{boxWords(node)}</span>
        <span className="mono flow-box-id">{box.name}</span>
        {node.kind !== null && <span className="mono flow-box-kind">{node.kind}</span>}
        <WalkMarks box={box} />
        <span className="flow-mark">{holdsText(node.holds, t)}</span>
        {data.gap !== null && (
          <span className="flow-mark flow-mark-gap">
            <Icon name="flowGap" />
            {data.gap.shown}
          </span>
        )}
      </p>
    </div>
  );
}

/** One node for all three forms: a group opening keeps its node, and the focus on it. */
const FlowNodeView = memo(function FlowNodeView({ data }: NodeProps<BoxNode>) {
  return data.box.form === "open" ? <FrameBox data={data} /> : <StepBox data={data} />;
});

/** The band's caption. */
const BandCaption = memo(function BandCaption() {
  return (
    <p className="flow-band" aria-hidden="true">
      Not on the walk
    </p>
  );
});

/** An edge: the layout's path, its type's colour, its dash, its words where it has any. */
const LinkView = memo(function LinkView({ id, markerEnd, style, data }: EdgeProps<LinkEdge>) {
  if (data === undefined) {
    return null;
  }
  const words = edgeWords(data.laid.edge);
  return (
    <>
      <BaseEdge id={id} path={data.laid.path} markerEnd={markerEnd} style={style} interactionWidth={0} />
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

const NODE_TYPES = { box: FlowNodeView, band: BandCaption };
const EDGE_TYPES = { link: LinkView };

/** A box's accessible name: its words, marks and gaps. */
export function boxName(box: FlowBox, marks: BoxMarks, t: Translate): string {
  const { node } = box;
  const parts = [box.name];
  if (node.title !== null) {
    parts.push(node.title);
  }
  if (node.kind !== null) {
    parts.push(node.kind);
  }
  if (node.start) {
    parts.push(startWord(box.group !== null, t));
  }
  if (node.end) {
    parts.push("End");
  }
  if (box.form === "open") {
    parts.push(`open group, ${holdsText(node.holds, t)}`);
  } else if (box.form === "closed") {
    parts.push(`closed group, ${holdsText(node.holds, t)}`);
    if (node.counts !== null) {
      parts.push(countsText(node.counts, t));
    }
  } else if (node.rollup !== null) {
    parts.push(rollupLook(node.rollup, t).label);
  }
  if (marks.gap !== null) {
    parts.push(marks.gap.said);
  }
  if (marks.dimmed) {
    parts.push("outside the stage shown");
  }
  if (marks.opening) {
    parts.push("opening");
  }
  if (marks.marked) {
    parts.push("marked");
  }
  if (marks.selected) {
    parts.push("selected");
  }
  return parts.join(", ");
}

function handlesOf(box: FlowBox) {
  const y = (box.form === "open" ? HEAD_HEIGHT : box.height) / 2 - HANDLE_SIZE / 2;
  return [
    { id: "out", type: "source" as const, position: Position.Right, x: box.width - HANDLE_SIZE / 2, y, width: HANDLE_SIZE, height: HANDLE_SIZE },
    { id: "in", type: "target" as const, position: Position.Left, x: -HANDLE_SIZE / 2, y, width: HANDLE_SIZE, height: HANDLE_SIZE },
  ];
}

function nodeOf(box: FlowBox, marks: BoxMarks, tabStop: boolean, t: Translate): BoxNode {
  return {
    id: box.id,
    type: "box",
    className: box.form === "open" ? FRAME_CLASS : undefined,
    position: { x: box.x, y: box.y },
    width: box.width,
    height: box.height,
    handles: handlesOf(box),
    data: { box, ...marks },
    ariaLabel: boxName(box, marks, t),
    domAttributes: { tabIndex: tabStop ? 0 : -1 },
    zIndex: box.nesting * 2 + 2,
    draggable: false,
    selectable: false,
    connectable: false,
    deletable: false,
  };
}

function bandNode(lane: FlowLane, y: number): CanvasNode {
  return {
    id: `band:${lane.group ?? ""}`,
    type: "band",
    position: { x: lane.x + LANE_PAD, y },
    width: 200,
    height: 24,
    data: { lane },
    zIndex: lane.nesting * 2 + 2,
    draggable: false,
    selectable: false,
    connectable: false,
    deletable: false,
    focusable: false,
  };
}

function edgeOf(laid: LaidEdge, types: readonly string[], emphasis: boolean): LinkEdge {
  const colour = typeColour(laid.edge.type, types);
  return {
    id: laid.id,
    type: "link",
    source: laid.source,
    target: laid.target,
    sourceHandle: "out",
    targetHandle: "in",
    data: { laid, emphasis },
    ariaLabel: edgeName(laid.edge),
    className: emphasis ? "is-emphasis" : undefined,
    style: { stroke: colour, strokeDasharray: dashOf(laid.edge, types), ...(isRepeat(laid.edge) ? { strokeWidth: "var(--flow-repeat-width)" } : {}) },
    markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14, color: colour },
    zIndex: laid.nesting * 2 + 1,
    focusable: false,
    selectable: false,
    deletable: false,
    reconnectable: false,
  };
}

/** A strip in the row under the picture, beside the controls, so it covers no box of a short canvas. */
const MINIMAP_SIZE = { width: 160, height: 48 };

/** The canvas's commands for a box, decided by the view: a click selects, Enter also moves focus to the panel. */
export type BoxCommand = "select" | "details" | "toggle" | "tree" | "close";

const COMMANDS: Record<string, BoxCommand> = { Enter: "details", " ": "toggle", o: "tree", Escape: "close" };

/** A name to bring into view once it is drawn; a new object each time. `always`: to the centre, even if it is already in view. */
export interface Centring {
  name: string;
  always: boolean;
}

export interface FlowCanvasProps {
  layout: FlowLayout;
  /** The sorted types of the top read: each edge's colour and plain dash. */
  types: readonly string[];
  rovingId: string | null;
  selected: string | null;
  marked: ReadonlySet<string>;
  /** Groups whose read is on its way. */
  opening: ReadonlySet<string>;
  /** The names a gap row of the top read names. */
  named: ReadonlySet<string>;
  /** The stage filter's value: boxes outside it dim, the layout stays. */
  stage: string;
  centring: Centring | null;
  onRove: (id: string) => void;
  onCommand: (box: FlowBox, command: BoxCommand) => void;
  /** Receives the canvas's focus function, so the view can put focus back on a box. */
  focusRef: (focus: (id: string) => void) => void;
}

/** What has to show of a box: an open group's head, as much of it as the canvas holds. */
function headOf(box: FlowBox, room: number): Rect {
  return box.form === "open" ? { x: box.x, y: box.y, height: HEAD_HEIGHT, width: Math.min(box.width, room) } : box;
}

function useReadableFit(layout: FlowLayout): () => boolean {
  const store = useStoreApi();
  const { setViewport } = useReactFlow();
  return useCallback(() => {
    const { width, height, panZoom } = store.getState();
    const viewport = panZoom === null ? null : fitOf(layout, width, height);
    if (viewport === null) {
      return false;
    }
    void setViewport(viewport, { duration: durationOf("--duration-slow") });
    return true;
  }, [layout, setViewport, store]);
}

/**
 * Fits the view while the owner has not moved it, the canvas's size changing too. After that a
 * layout never moves the view on its own but to keep the box in hand where it was on the screen:
 * opening a group pushes its neighbours aside, not the group away.
 */
function KeepInView({ layout, fit, held, touched }: { layout: FlowLayout; fit: () => boolean; held: string | null; touched: { current: boolean } }) {
  const width = useStore((state) => state.width);
  const height = useStore((state) => state.height);
  const store = useStoreApi();
  const { setViewport } = useReactFlow();
  const fitted = useRef<FlowLayout | null>(null);
  const fittedWidth = useRef(width);
  const fittedHeight = useRef(height);
  useEffect(() => {
    const before = fitted.current;
    const resized = fittedWidth.current !== width || fittedHeight.current !== height;
    fittedWidth.current = width;
    fittedHeight.current = height;
    if ((before === layout && !resized) || width <= 0 || height <= 0) {
      return;
    }
    if (before === null || !touched.current) {
      if (fit()) {
        fitted.current = layout;
      }
      return;
    }
    if (before === layout) {
      return;
    }
    fitted.current = layout;
    const was = before.boxes.find((box) => box.id === held);
    const now = layout.boxes.find((box) => box.id === held);
    if (was !== undefined && now !== undefined && (was.x !== now.x || was.y !== now.y)) {
      const [x, y, zoom] = store.getState().transform;
      void setViewport({ x: x - (now.x - was.x) * zoom, y: y - (now.y - was.y) * zoom, zoom });
    }
  }, [layout, fit, width, height, held, touched, store, setViewport]);
  return null;
}

function Canvas({ layout, types, rovingId, selected, marked, opening, named, stage, centring, onRove, onCommand, focusRef }: FlowCanvasProps) {
  const t = useTranslate();
  const wrapper = useRef<HTMLDivElement>(null);
  const store = useStoreApi();
  const { setCenter } = useReactFlow();
  const hintId = useId();
  const fit = useReadableFit(layout);
  const touched = useRef(false);
  /** The box focus was on, kept when focus fell off it with the box. */
  const focused = useRef<string | null>(null);
  const previous = useRef(layout);
  const centred = useRef<Centring | null>(null);

  const selectedId = useMemo(() => layout.boxes.find((box) => box.name === selected)?.id ?? null, [layout, selected]);
  const nodes = useMemo<CanvasNode[]>(() => {
    const boxes = layout.boxes.map((box) =>
      nodeOf(
        box,
        {
          selected: box.name === selected,
          marked: marked.has(box.name),
          opening: opening.has(box.name),
          gap: gapMark(box.node, box.form === "open", named, t),
          dimmed: outsideStage(box.node, stage),
        },
        box.id === rovingId,
        t,
      ),
    );
    const bands = layout.lanes.flatMap((lane) => (lane.bandY === null ? [] : [bandNode(lane, lane.bandY)]));
    return [...boxes, ...bands];
  }, [layout, selected, marked, opening, named, stage, rovingId, t]);
  const edges = useMemo(
    () => layout.edges.map((laid) => edgeOf(laid, types, selectedId !== null && (laid.source === selectedId || laid.target === selectedId))),
    [layout, types, selectedId],
  );

  const boxElement = useCallback((id: string): HTMLElement | null => {
    const found = Array.from(wrapper.current?.querySelectorAll<HTMLElement>(".react-flow__node") ?? []);
    return found.find((element) => element.dataset.id === id) ?? null;
  }, []);

  const focusBox = useCallback(
    (id: string) => {
      boxElement(id)?.focus();
    },
    [boxElement],
  );

  useEffect(() => {
    focusRef(focusBox);
  }, [focusRef, focusBox]);

  const centreOn = useCallback(
    (box: FlowBox) => {
      const [, , zoom] = store.getState().transform;
      const y = box.form === "open" ? box.y + HEAD_HEIGHT / 2 : box.y + box.height / 2;
      void setCenter(box.x + Math.min(box.width, 400) / 2, y, { zoom, duration: durationOf("--duration-normal") });
    },
    [setCenter, store],
  );

  // A gap item's first member: centred once it is drawn, however many reads that took.
  useEffect(() => {
    if (centring === null || centred.current === centring) {
      return;
    }
    const box = layout.boxes.find((candidate) => candidate.name === centring.name);
    const { transform, width, height } = store.getState();
    if (box === undefined || width <= 0) {
      return;
    }
    centred.current = centring;
    const [x, y, zoom] = transform;
    if (centring.always || !inView(headOf(box, width / zoom), { x, y, zoom, width, height })) {
      touched.current = true;
      centreOn(box);
    }
  }, [centring, layout, centreOn, store]);

  // The canvas's width changed, a panel opening beside it: what was centred on the old width is
  // looked at again, against the view it will have.
  const canvasWidth = useStore((state) => state.width);
  const canvasHeight = fittedCanvasHeight(layout, canvasWidth);
  // The minimap only covers a picture already whole in view.
  const pictureWhole = useStore((state) => {
    const [x, y, zoom] = state.transform;
    return inView({ x: 0, y: 0, width: layout.width, height: layout.height }, { x, y, zoom, width: state.width, height: state.height });
  });
  const lookedAt = useRef(canvasWidth);
  useEffect(() => {
    if (lookedAt.current === canvasWidth) {
      return;
    }
    lookedAt.current = canvasWidth;
    const box = layout.boxes.find((candidate) => candidate.name === selected);
    const { transform, width, height } = store.getState();
    const [x, y, zoom] = transform;
    const view = (touched.current ? null : fitOf(layout, width, height)) ?? { x, y, zoom };
    if (box !== undefined && width > 0 && !inView(headOf(box, width / view.zoom), { ...view, width, height })) {
      touched.current = true;
      centreOn(box);
    }
  }, [canvasWidth, layout, selected, centreOn, store]);

  // A focused box leaving the picture hands focus to its enclosing head; at Top, to the tab stop.
  useEffect(() => {
    const before = previous.current;
    previous.current = layout;
    const id = focused.current;
    if (before === layout || id === null || layout.boxes.some((box) => box.id === id)) {
      return;
    }
    const active = document.activeElement;
    const onIt = active instanceof HTMLElement && active.closest<HTMLElement>(".react-flow__node")?.dataset.id === id;
    if (!onIt && !focusIsLost()) {
      return;
    }
    const next = enclosingHead(before, layout, id) ?? rovingId;
    if (next !== null) {
      onRove(next);
      focusBox(next);
    }
  }, [layout, rovingId, onRove, focusBox]);

  /** A focused box out of sight is centred at the current zoom. */
  function onFocus(event: FocusEvent<HTMLDivElement>) {
    const element = event.target instanceof HTMLElement ? event.target.closest<HTMLElement>(".react-flow__node") : null;
    const id = element?.dataset.id;
    const box = id === undefined ? undefined : layout.boxes.find((candidate) => candidate.id === id);
    if (id === undefined || box === undefined) {
      return;
    }
    focused.current = id;
    if (id !== rovingId) {
      onRove(id);
    }
    const { transform, width, height } = store.getState();
    const [x, y, zoom] = transform;
    if (!inView(headOf(box, width / zoom), { x, y, zoom, width, height })) {
      touched.current = true;
      centreOn(box);
    }
  }

  function onBlur(event: FocusEvent<HTMLDivElement>) {
    if (event.relatedTarget !== null && !(wrapper.current?.contains(event.relatedTarget) ?? false)) {
      focused.current = null;
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target) || !(event.target instanceof HTMLElement)) {
      return;
    }
    const id = event.target.closest<HTMLElement>(".react-flow__node")?.dataset.id;
    const box = id === undefined ? undefined : layout.boxes.find((candidate) => candidate.id === id);
    if (id === undefined || box === undefined) {
      return;
    }
    const key = hotkeyOf(event);
    if (letterOf(event) === "c") {
      event.preventDefault();
      touched.current = true;
      centreOn(box);
      return;
    }
    const command = Object.hasOwn(COMMANDS, key) ? COMMANDS[key] : undefined;
    if (command !== undefined) {
      event.preventDefault();
      touched.current = touched.current || command === "toggle";
      onCommand(box, command);
      return;
    }
    if (!["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"].includes(event.key)) {
      return;
    }
    event.preventDefault();
    const next = boxAfterKey(layout, id, event.key);
    if (next !== null) {
      onRove(next);
      focusBox(next);
    }
  }

  function onNodeClick(event: MouseEvent, id: string) {
    const box = layout.boxes.find((candidate) => candidate.id === id);
    if (box === undefined) {
      return;
    }
    onRove(id);
    const toggle = event.target instanceof Element && event.target.closest(`[${TOGGLE}]`) !== null;
    touched.current = touched.current || toggle;
    onCommand(box, toggle ? "toggle" : "select");
  }

  return (
    <div
      ref={wrapper}
      className="graph-canvas flow-canvas"
      style={canvasHeight === null ? undefined : ({ "--flow-canvas-fitted": `${String(canvasHeight)}px` } as CSSProperties)}
      onKeyDown={onKeyDown}
      onFocus={onFocus}
      onBlur={onBlur}
    >
      <p id={hintId} className="sr-only">
        The List view holds every box and edge as text.
      </p>
      <ReactFlow<CanvasNode, LinkEdge>
        nodes={nodes}
        edges={edges}
        nodeTypes={NODE_TYPES}
        edgeTypes={EDGE_TYPES}
        aria-label="Flow canvas"
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
        preventScrolling={false}
        disableKeyboardA11y
        deleteKeyCode={null}
        selectionKeyCode={null}
        multiSelectionKeyCode={null}
        panActivationKeyCode={null}
        zoomActivationKeyCode={null}
        proOptions={{ hideAttribution: true }}
        ariaLabelConfig={FLOW_ARIA_LABELS}
        defaultMarkerColor={null}
        colorMode="dark"
        minZoom={0.1}
        maxZoom={2}
        onMoveStart={(event) => {
          if (event !== null) {
            touched.current = true;
          }
        }}
        onNodeClick={(event, node) => {
          onNodeClick(event, node.id);
        }}
      >
        <Controls orientation="horizontal" showInteractive={false} showFitView={false}>
          <ControlButton
            className="react-flow__controls-fitview"
            title={FLOW_ARIA_LABELS["controls.fitView.ariaLabel"]}
            aria-label={FLOW_ARIA_LABELS["controls.fitView.ariaLabel"]}
            onClick={() => {
              fit();
            }}
          >
            <Icon name="fit" />
          </ControlButton>
        </Controls>
        {!pictureWhole && <MiniMap style={MINIMAP_SIZE} pannable={false} zoomable={false} nodeClassName={(node) => node.className ?? ""} />}
        <KeepInView layout={layout} fit={fit} held={rovingId} touched={touched} />
      </ReactFlow>
    </div>
  );
}

/** The read-only canvas of the reads in hand. */
export function FlowCanvas(props: FlowCanvasProps) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}

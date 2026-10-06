import {
  BaseEdge,
  ControlButton,
  Controls,
  EdgeLabelRenderer,
  getBezierPath,
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
  type EdgeMarker,
  type EdgeProps,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { memo, useCallback, useEffect, useId, useMemo, useRef, type FocusEvent, type KeyboardEvent } from "react";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { linkStateLook } from "../tree/labels";
import { arcOver, readableViewport } from "./geometry";
import { BOX_HANDLES, BOX_HEIGHT, BOX_WIDTH, HANDLE_SIZE, patternAt, type Layout, type LayoutBox, type LayoutEdge } from "./layout";
import { boxLabel, edgeLabel, isArchived, moreLabel } from "./lines";

// The read-only canvas (docs/features/ui-graph.md "Canvas"): React Flow draws the layout's boxes
// and edges; nothing drags, connects, selects or deletes. Every key prop is null (each default
// listens on the document and prevents its key), React Flow's own keyboard and words are off or
// replaced, and the boxes keep one tab stop that the arrow keys move. The wheel scrolls the page,
// never zooms it: pinch and the zoom buttons do.

/**
 * Every key of React Flow's `defaultAriaLabelConfig` (`@xyflow/system`), replaced: nothing here
 * can be removed, dragged, joined or edited, so no description says so.
 */
export const GRAPH_ARIA_LABELS = {
  "node.a11yDescription.default": "A box of the graph. Arrow keys go to other boxes; Enter opens its details.",
  "node.a11yDescription.keyboardDisabled": "A box of the graph. Arrow keys go to other boxes; Enter opens its details.",
  "node.a11yDescription.ariaLiveMessage": () => "",
  "edge.a11yDescription.default": "A link between two boxes; the List view names every link as text.",
  "controls.ariaLabel": "Canvas zoom",
  "controls.zoomIn.ariaLabel": "Zoom in",
  "controls.zoomOut.ariaLabel": "Zoom out",
  "controls.fitView.ariaLabel": "Fit the graph to the view, its text readable",
  "controls.interactive.ariaLabel": "Canvas lock",
  "minimap.ariaLabel": "Overview of the graph",
  "handle.ariaLabel": "Link end",
} satisfies AriaLabelConfig;

type BoxData = { box: LayoutBox; selected: boolean };
type BoxNode = Node<BoxData, "spec" | "stub" | "more">;
type LinkData = { layout: LayoutEdge; emphasis: boolean };
type LinkEdge = Edge<LinkData, "link">;

const SIDES = { left: Position.Left, right: Position.Right, top: Position.Top } as const;

/** The four handles of every box; invisible, there for the edges' ends. */
function Handles() {
  return (
    <>
      <Handle type="source" position={Position.Right} id="out" isConnectable={false} aria-hidden="true" />
      <Handle type="target" position={Position.Left} id="in" isConnectable={false} aria-hidden="true" />
      <Handle type="source" position={Position.Top} id="out-top" isConnectable={false} aria-hidden="true" />
      <Handle type="target" position={Position.Top} id="in-top" isConnectable={false} aria-hidden="true" />
    </>
  );
}

function SelectedMark() {
  return (
    <span className="graph-mark graph-mark-selected">
      <Icon name="selected" />
      Selected
    </span>
  );
}

/** A walked node: name, Focus and Selected as text and icon, title (clipped), kind, distance. */
const SpecBox = memo(function SpecBox({ data }: NodeProps<BoxNode>) {
  const content = data.box.content;
  if (content.type !== "walked") {
    return null;
  }
  const walked = content.walked;
  const first = walked.holders[0];
  const archived = isArchived(walked);
  const classes = ["graph-box", walked.distance === 0 && "is-focus", data.selected && "is-selected", archived && "is-archived"];
  return (
    <div className={classes.filter(Boolean).join(" ")} aria-hidden="true">
      <Handles />
      <p className="graph-box-top">
        <span className="graph-box-name mono">{walked.name}</span>
        {walked.distance === 0 && (
          <span className="graph-mark graph-mark-focus">
            <Icon name="focus" />
            Focus
          </span>
        )}
        {data.selected && <SelectedMark />}
      </p>
      <p className="graph-box-title">{first?.title ?? "-"}</p>
      <p className="graph-box-meta">
        <span className="graph-box-kind mono">{first?.kind ?? "-"}</span>
        {archived && (
          <span className="graph-box-flag">
            <Icon name="archived" />
            archived
          </span>
        )}
        {walked.holders.length > 1 && <span className="graph-box-flag">{walked.holders.length} holders</span>}
        <span className="graph-box-distance">d {walked.distance}</span>
      </p>
    </div>
  );
});

/** An end the walk did not reach: a section named by itself, or an unresolved end with its state. */
const StubBox = memo(function StubBox({ data }: NodeProps<BoxNode>) {
  const content = data.box.content;
  if (content.type !== "stub") {
    return null;
  }
  const stub = content.stub.stub;
  return (
    <div className={`graph-box graph-stub${data.selected ? " is-selected" : ""}`} aria-hidden="true">
      <Handles />
      <p className="graph-box-top">
        <span className="graph-box-name mono">{stub.type === "section" ? stub.name : stub.edge.written}</span>
        {data.selected && <SelectedMark />}
      </p>
      <p className="graph-box-meta">
        <span className="graph-box-flag">
          <Icon name="stub" />
          Not walked
        </span>
        {stub.type === "section" ? <span className="graph-box-flag">a section</span> : <Badge look={linkStateLook(stub.edge.state)} />}
      </p>
    </div>
  );
});

/** The rest of a collapsed column. */
const MoreBox = memo(function MoreBox({ data }: NodeProps<BoxNode>) {
  const content = data.box.content;
  if (content.type !== "more") {
    return null;
  }
  return (
    <div className={`graph-box graph-more${data.selected ? " is-selected" : ""}`} aria-hidden="true">
      <Handles />
      <p className="graph-box-top">
        <Icon name="more" />
        <span className="graph-more-count">{moreLabel(content.hidden, content.column)}</span>
      </p>
      <p className="graph-box-meta">Every node is in the List</p>
    </div>
  );
});

/** A link: a curve with the type's stroke pattern, its arrow at `dst`, the type as its label. */
const LinkView = memo(function LinkView({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  markerEnd,
  markerStart,
  style,
  data,
}: EdgeProps<LinkEdge>) {
  let path: string;
  let labelX: number;
  let labelY: number;
  if (sourcePosition === Position.Top && targetPosition === Position.Top) {
    ({ path, labelX, labelY } = arcOver(sourceX, sourceY, targetX, targetY));
  } else {
    [path, labelX, labelY] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
  }
  return (
    <>
      <BaseEdge id={id} path={path} markerEnd={markerEnd} markerStart={markerStart} style={style} interactionWidth={0} />
      {data !== undefined && (
        <EdgeLabelRenderer>
          <div
            className={`graph-edge-label mono${data.emphasis ? " is-emphasis" : ""}`}
            aria-hidden="true"
            style={{ transform: `translate(-50%, -50%) translate(${String(labelX)}px, ${String(labelY)}px)` }}
          >
            {data.layout.edge.type}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
});

const NODE_TYPES = { spec: SpecBox, stub: StubBox, more: MoreBox };
const EDGE_TYPES = { link: LinkView };

/** The arrowheads: the emphasis one takes its colour from the canvas's emphasis role (currentColor). */
const ARROW: EdgeMarker = { type: MarkerType.ArrowClosed, width: 14, height: 14 };
const ARROW_EMPHASIS: EdgeMarker = { ...ARROW, color: "currentColor" };

/** A box's accessible name. */
export function boxName(box: LayoutBox): string {
  const content = box.content;
  if (content.type === "walked") {
    return boxLabel(content.walked);
  }
  if (content.type === "more") {
    return `${moreLabel(content.hidden, content.column)}; every node is in the List`;
  }
  const stub = content.stub.stub;
  if (stub.type === "section") {
    return `${stub.name}, not walked: a section`;
  }
  return `${stub.edge.written}, not walked: ${linkStateLook(stub.edge.state).label}`;
}

function nodeOf(box: LayoutBox): BoxNode {
  return {
    id: box.id,
    type: box.content.type === "walked" ? "spec" : box.content.type,
    position: { x: box.x, y: box.y },
    width: BOX_WIDTH,
    height: BOX_HEIGHT,
    handles: BOX_HANDLES.map((handle) => ({
      id: handle.id,
      type: handle.type,
      position: SIDES[handle.side],
      x: handle.x,
      y: handle.y,
      width: HANDLE_SIZE,
      height: HANDLE_SIZE,
    })),
    data: { box, selected: false },
    ariaLabel: boxName(box),
    domAttributes: { tabIndex: -1 },
    draggable: false,
    selectable: false,
    connectable: false,
    deletable: false,
  };
}

function edgeOf(layoutEdge: LayoutEdge, emphasis: boolean): LinkEdge {
  const arrow = emphasis ? ARROW_EMPHASIS : ARROW;
  return {
    id: layoutEdge.id,
    type: "link",
    source: layoutEdge.source,
    target: layoutEdge.target,
    sourceHandle: layoutEdge.sourceHandle,
    targetHandle: layoutEdge.targetHandle,
    data: { layout: layoutEdge, emphasis },
    ariaLabel: edgeLabel(layoutEdge.edge),
    className: emphasis ? "is-emphasis" : undefined,
    style: { strokeDasharray: patternAt(layoutEdge.position) },
    markerEnd: layoutEdge.arrowAt === "end" ? arrow : undefined,
    markerStart: layoutEdge.arrowAt === "start" ? arrow : undefined,
    focusable: false,
    selectable: false,
    deletable: false,
    reconnectable: false,
  };
}

/** A duration token in milliseconds (0 under reduced motion, or where no style sheet applies). */
function durationOf(token: string): number {
  const value = getComputedStyle(document.documentElement).getPropertyValue(token).trim();
  const number = Number.parseFloat(value);
  if (!Number.isFinite(number)) {
    return 0;
  }
  return value.endsWith("ms") ? number : value.endsWith("s") ? number * 1000 : number;
}

/**
 * The canvas's fit (docs/features/ui-graph.md "Canvas"): the whole graph when its text stays
 * readable, else the focus and its neighbours at the smallest readable zoom. Smooth where motion is
 * allowed. False while the canvas has no pan-zoom or size yet.
 */
function useReadableFit(layout: Layout): () => boolean {
  const store = useStoreApi();
  const { setViewport } = useReactFlow();
  return useCallback(() => {
    const { width, height, panZoom } = store.getState();
    const focusId = layout.columns[0]?.boxes[0];
    const focus = layout.boxes.find((box) => box.id === focusId) ?? null;
    const viewport = panZoom === null ? null : readableViewport(layout.boxes, focus, width, height);
    if (viewport === null) {
      return false;
    }
    void setViewport(viewport, { duration: durationOf("--duration-slow") });
    return true;
  }, [layout, setViewport, store]);
}

/** Fits the view to each new answer, once the canvas has its size. */
function FitEachAnswer({ layout, fit }: { layout: Layout; fit: () => boolean }) {
  const width = useStore((state) => state.width);
  const height = useStore((state) => state.height);
  const fitted = useRef<Layout | null>(null);
  useEffect(() => {
    if (fitted.current !== layout && width > 0 && height > 0 && fit()) {
      fitted.current = layout;
    }
  }, [layout, fit, width, height]);
  return null;
}

/** The box a key moves to from `from`, or null to stay. */
export function boxAfterKey(layout: Layout, from: string, key: string): string | null {
  const at = layout.columns.findIndex((column) => column.boxes.includes(from));
  const column = layout.columns[at];
  if (column === undefined) {
    return null;
  }
  const row = column.boxes.indexOf(from);
  switch (key) {
    case "ArrowDown":
    case "j":
      return column.boxes[row + 1] ?? null;
    case "ArrowUp":
    case "k":
      return column.boxes[row - 1] ?? null;
    case "Home":
      return column.boxes[0] ?? null;
    case "End":
      return column.boxes[column.boxes.length - 1] ?? null;
    case "ArrowLeft":
    case "ArrowRight": {
      const next = layout.columns[key === "ArrowLeft" ? at - 1 : at + 1];
      if (next === undefined || next.boxes.length === 0) {
        return null;
      }
      // The nearest row; a tie goes to the upper one.
      const nearest = Math.min(row, next.boxes.length - 1);
      return next.boxes[nearest] ?? null;
    }
    default:
      return null;
  }
}

/** The canvas's commands for a box (Enter, `o`, `c`, Esc), decided by the view. */
export type BoxCommand = "details" | "tree" | "centre" | "close";

const COMMANDS: Record<string, BoxCommand> = { Enter: "details", o: "tree", c: "centre", Escape: "close" };

export interface GraphCanvasProps {
  layout: Layout;
  /** The box with the tab stop. */
  rovingId: string | null;
  selectedId: string | null;
  /** A box took the tab stop (a key, a click, Tab). */
  onRove: (id: string) => void;
  onSelect: (id: string) => void;
  onCommand: (id: string, command: BoxCommand) => void;
  /** Receives the canvas's focus function, so the view can put focus back on a box. */
  focusRef: (focus: (id: string) => void) => void;
}

function Canvas({ layout, rovingId, selectedId, onRove, onSelect, onCommand, focusRef }: GraphCanvasProps) {
  const wrapper = useRef<HTMLDivElement>(null);
  const store = useStoreApi();
  const { setCenter } = useReactFlow();
  const hintId = useId();
  const fit = useReadableFit(layout);

  const baseNodes = useMemo(() => layout.boxes.map(nodeOf), [layout]);
  const nodes = useMemo(
    () =>
      baseNodes.map((node) => {
        const roving = node.id === rovingId;
        const selected = node.id === selectedId;
        if (!roving && !selected) {
          return node;
        }
        return { ...node, data: { ...node.data, selected }, domAttributes: { tabIndex: roving ? 0 : -1 } };
      }),
    [baseNodes, rovingId, selectedId],
  );
  const edges = useMemo(
    () => layout.edges.map((edge) => edgeOf(edge, selectedId !== null && (edge.source === selectedId || edge.target === selectedId))),
    [layout, selectedId],
  );

  const boxElement = useCallback((id: string): HTMLElement | null => {
    const found = Array.from(wrapper.current?.querySelectorAll<HTMLElement>(".react-flow__node") ?? []);
    return found.find((element) => element.dataset.id === id) ?? null;
  }, []);

  // A box with tabindex -1 takes focus from a script; its tab stop follows in the next render.
  const focusBox = useCallback(
    (id: string) => {
      boxElement(id)?.focus();
    },
    [boxElement],
  );

  useEffect(() => {
    focusRef(focusBox);
  }, [focusRef, focusBox]);

  /** A focused box out of sight is centred at the current zoom. */
  function onFocus(event: FocusEvent<HTMLDivElement>) {
    const element = event.target instanceof HTMLElement ? event.target.closest<HTMLElement>(".react-flow__node") : null;
    const id = element?.dataset.id;
    const box = id === undefined ? undefined : layout.boxes.find((candidate) => candidate.id === id);
    if (id === undefined || box === undefined) {
      return;
    }
    if (id !== rovingId) {
      onRove(id);
    }
    const { transform, width, height } = store.getState();
    const [tx, ty, zoom] = transform;
    const left = box.x * zoom + tx;
    const top = box.y * zoom + ty;
    const inside = left >= 0 && top >= 0 && left + BOX_WIDTH * zoom <= width && top + BOX_HEIGHT * zoom <= height;
    if (!inside) {
      void setCenter(box.x + BOX_WIDTH / 2, box.y + BOX_HEIGHT / 2, { zoom, duration: durationOf("--duration-normal") });
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target) || !(event.target instanceof HTMLElement)) {
      return;
    }
    const id = event.target.closest<HTMLElement>(".react-flow__node")?.dataset.id;
    if (id === undefined) {
      return;
    }
    const command = Object.hasOwn(COMMANDS, event.key) ? COMMANDS[event.key] : undefined;
    if (command !== undefined) {
      event.preventDefault();
      onCommand(id, command);
      return;
    }
    const next = boxAfterKey(layout, id, event.key);
    if (next === null) {
      if (["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight", "Home", "End", "j", "k"].includes(event.key)) {
        event.preventDefault();
      }
      return;
    }
    event.preventDefault();
    onRove(next);
    focusBox(next);
  }

  return (
    <div ref={wrapper} className="graph-canvas" onKeyDown={onKeyDown} onFocus={onFocus}>
      <p id={hintId} className="sr-only">
        The List view holds every node and edge as text.
      </p>
      <ReactFlow<BoxNode, LinkEdge>
        nodes={nodes}
        edges={edges}
        nodeTypes={NODE_TYPES}
        edgeTypes={EDGE_TYPES}
        aria-label="Graph canvas"
        aria-describedby={hintId}
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
        ariaLabelConfig={GRAPH_ARIA_LABELS}
        defaultMarkerColor={null}
        colorMode="dark"
        minZoom={0.1}
        maxZoom={2}
        onNodeClick={(_event, node) => {
          onRove(node.id);
          onSelect(node.id);
        }}
      >
        <Controls showInteractive={false} showFitView={false}>
          <ControlButton
            className="react-flow__controls-fitview"
            title={GRAPH_ARIA_LABELS["controls.fitView.ariaLabel"]}
            aria-label={GRAPH_ARIA_LABELS["controls.fitView.ariaLabel"]}
            onClick={() => {
              fit();
            }}
          >
            <Icon name="fit" />
          </ControlButton>
        </Controls>
        <MiniMap pannable={false} zoomable={false} />
        <FitEachAnswer layout={layout} fit={fit} />
      </ReactFlow>
    </div>
  );
}

/** The read-only canvas of one answer's layout. */
export function GraphCanvas(props: GraphCanvasProps) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}

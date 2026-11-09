import {
  BaseEdge,
  ControlButton,
  Controls,
  getBezierPath,
  Handle,
  MarkerType,
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
import { memo, useCallback, useEffect, useId, useMemo, useRef } from "react";
import type { RoadmapDocument } from "../api/types";
import { arcOver, readableViewport } from "../graph/geometry";
import { BOX_HANDLES, BOX_HEIGHT, BOX_WIDTH, HANDLE_SIZE } from "../graph/layout";
import { taskStatusLook } from "../tasks/labels";
import { Icon } from "../ui/Icon";
import { chainOf, LANE_HEAD, type ChainBox, type ChainEdge, type ChainLane, type ChainLayout } from "./chain";
import { countsText, currentLook, cutRuleText, inACycleLook, laneName, mvpCutLook, notDrawnText, oneLine } from "./text";
import { useTranslate, type Translate } from "../i18n";

// The chain canvas (docs/features/roadmap-screen.md; ui/README.md "Screen rules", Canvas): the
// plan's listed items as boxes in their lanes and an edge per `waits_on` pair, read only. The
// Graph canvas's read-only props: nothing drags, connects, selects, deletes or takes a key; every
// key option null (each default listens on the document); boxes are not focusable (the Lanes tab is
// the keyboard's path); a click selects the item through the route. The wheel scrolls the page.

/** Every key of React Flow's `defaultAriaLabelConfig`, replaced in the catalog's words: nothing here can be changed. */
function chainAriaLabels(t: Translate) {
  return {
    "node.a11yDescription.default": t("roadmap.aria.item"),
    "node.a11yDescription.keyboardDisabled": t("roadmap.aria.item"),
    "node.a11yDescription.ariaLiveMessage": () => "",
    "edge.a11yDescription.default": t("roadmap.aria.wait"),
    "controls.ariaLabel": t("roadmap.aria.controls"),
    "controls.zoomIn.ariaLabel": t("roadmap.aria.zoomIn"),
    "controls.zoomOut.ariaLabel": t("roadmap.aria.zoomOut"),
    "controls.fitView.ariaLabel": t("roadmap.aria.fit"),
    "controls.interactive.ariaLabel": t("roadmap.aria.lock"),
    "minimap.ariaLabel": t("roadmap.aria.minimap"),
    "handle.ariaLabel": t("roadmap.aria.handle"),
  } satisfies AriaLabelConfig;
}

type ItemData = { box: ChainBox; selected: boolean };
type LaneData = { lane: ChainLane };
type CutData = { text: string; height: number };
type ChainNode = Node<ItemData, "item"> | Node<LaneData, "lane"> | Node<CutData, "cut">;
type WaitEdge = Edge<{ edge: ChainEdge }, "wait">;

const SIDES = { left: Position.Left, right: Position.Right, top: Position.Top } as const;

/** A handle no connection starts or ends at. */
const CLOSED = { isConnectable: false, isConnectableStart: false, isConnectableEnd: false, "aria-hidden": true } as const;

/** The four handles of a box, invisible, there for the edges' ends; none takes a connection. */
function Handles() {
  return (
    <>
      <Handle type="source" position={Position.Right} id="out" {...CLOSED} />
      <Handle type="target" position={Position.Left} id="in" {...CLOSED} />
      <Handle type="source" position={Position.Top} id="out-top" {...CLOSED} />
      <Handle type="target" position={Position.Top} id="in-top" {...CLOSED} />
    </>
  );
}

/** An item: its ID, title (clipped; whole in Lanes), "In a cycle" and its task cells, each as text and icon. */
const ItemBox = memo(function ItemBox({ data }: NodeProps<Node<ItemData, "item">>) {
  const t = useTranslate();
  const item = data.box.item;
  return (
    <div className={`graph-box chain-box${data.selected ? " is-selected" : ""}`} aria-hidden="true">
      <Handles />
      <p className="graph-box-top">
        <span className="graph-box-name mono">{item.id}</span>
        {item.cycle && (
          <span className="graph-mark chain-mark-cycle" data-cycle="">
            <Icon name={inACycleLook(t).icon} />
            {inACycleLook(t).label}
          </span>
        )}
        {data.selected && (
          <span className="graph-mark graph-mark-selected">
            <Icon name="selected" />
            {t("roadmap.selected")}
          </span>
        )}
      </p>
      <p className="graph-box-title">{item.title === null ? "-" : oneLine(item.title)}</p>
      {item.tasks.length > 0 && (
        <p className="graph-box-meta chain-box-tasks">
          {item.tasks.map((task) => {
            const look = taskStatusLook(task.status, t);
            return (
              <span key={task.id} className="graph-box-flag" data-task={task.id}>
                <Icon name={look.icon} />
                <span className="mono">{task.id}</span> {look.label}
              </span>
            );
          })}
        </p>
      )}
    </div>
  );
});

/** A lane's name, marks and counts, drawn as text above its boxes. */
const LaneLabel = memo(function LaneLabel({ data }: NodeProps<Node<LaneData, "lane">>) {
  const t = useTranslate();
  const group = data.lane.group;
  return (
    <div className="chain-lane" aria-hidden="true">
      <p className="chain-lane-name">{laneName(group, t)}</p>
      <p className="chain-lane-marks">
        {group.current && (
          <span className="graph-mark">
            <Icon name={currentLook(t).icon} />
            {currentLook(t).label}
          </span>
        )}
        {group.mvp_cut && (
          <span className="graph-mark">
            <Icon name={mvpCutLook(t).icon} />
            {mvpCutLook(t).label}
          </span>
        )}
        <span>{countsText(group, t)}</span>
      </p>
    </div>
  );
});

/** The cut's rule: a dashed line down the gap after the `mvp_cut` lane, its words at the top. */
const CutRule = memo(function CutRule({ data }: NodeProps<Node<CutData, "cut">>) {
  const t = useTranslate();
  return (
    <div className="chain-cut" aria-hidden="true" style={{ height: data.height }}>
      <span className="chain-cut-label">
        <Icon name={mvpCutLook(t).icon} />
        {data.text}
      </span>
    </div>
  );
});

/** A wait: from the predecessor to the waiting item, its arrow at the item. */
const WaitView = memo(function WaitView({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, markerEnd }: EdgeProps<WaitEdge>) {
  let path: string;
  if (sourcePosition === Position.Top && targetPosition === Position.Top) {
    ({ path } = arcOver(sourceX, sourceY, targetX, targetY));
  } else {
    [path] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
  }
  return <BaseEdge id={id} path={path} markerEnd={markerEnd} interactionWidth={0} />;
});

const NODE_TYPES = { item: ItemBox, lane: LaneLabel, cut: CutRule };
const EDGE_TYPES = { wait: WaitView };

const ARROW: EdgeMarker = { type: MarkerType.ArrowClosed, width: 14, height: 14 };

/** A box's accessible name: its ID, title, cycle mark, waits and task cells. */
export function itemBoxName(box: ChainBox, t: Translate): string {
  const item = box.item;
  const parts = [item.id, item.title === null ? t("roadmap.noTitle") : oneLine(item.title)];
  if (item.cycle) {
    parts.push(inACycleLook(t).label.toLowerCase());
  }
  if (item.waits_on.length > 0) {
    parts.push(t("roadmap.waitsOnIds", { ids: item.waits_on.join(", ") }));
  }
  for (const task of item.tasks) {
    parts.push(t("roadmap.task", { id: task.id, state: taskStatusLook(task.status, t).label }));
  }
  return parts.join(", ");
}

function itemNode(box: ChainBox, t: Translate): ChainNode {
  return {
    id: box.id,
    type: "item",
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
    ariaLabel: itemBoxName(box, t),
    ariaRole: "group",
    draggable: false,
    selectable: false,
    connectable: false,
    deletable: false,
    focusable: false,
  };
}

function laneNode(lane: ChainLane): ChainNode {
  return {
    id: lane.id,
    type: "lane",
    position: { x: lane.x, y: 0 },
    width: BOX_WIDTH,
    height: LANE_HEAD - 16,
    data: { lane },
    draggable: false,
    selectable: false,
    connectable: false,
    deletable: false,
    focusable: false,
  };
}

function waitEdge(edge: ChainEdge, t: Translate): WaitEdge {
  return {
    id: edge.id,
    type: "wait",
    source: edge.source,
    target: edge.target,
    sourceHandle: edge.sourceHandle,
    targetHandle: edge.targetHandle,
    data: { edge },
    ariaLabel: t("roadmap.waitEdge", { to: edge.to, from: edge.from }),
    markerEnd: ARROW,
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

/** The fit: the whole plan when its text stays readable, else the first lanes from the top left. */
function useReadableFit(layout: ChainLayout): () => boolean {
  const store = useStoreApi();
  const { setViewport } = useReactFlow();
  return useCallback(() => {
    const { width, height, panZoom } = store.getState();
    const placed = [...layout.lanes.map((lane) => ({ x: lane.x, y: 0 })), ...layout.boxes];
    const viewport = panZoom === null ? null : readableViewport(placed, placed[0] ?? null, width, height);
    if (viewport === null) {
      return false;
    }
    void setViewport(viewport, { duration: durationOf("--duration-slow") });
    return true;
  }, [layout, setViewport, store]);
}

/** Fits the view to each new plan, once the canvas has its size. */
function FitEachPlan({ layout, fit }: { layout: ChainLayout; fit: () => boolean }) {
  const width = useStore((state) => state.width);
  const height = useStore((state) => state.height);
  const fitted = useRef<ChainLayout | null>(null);
  useEffect(() => {
    if (fitted.current !== layout && width > 0 && height > 0 && fit()) {
      fitted.current = layout;
    }
  }, [layout, fit, width, height]);
  return null;
}

export interface RoadmapChainProps {
  plan: RoadmapDocument;
  /** The item the route names, outlined and marked; null for none. */
  selectedId: string | null;
  /** A box was clicked: select its item through the route. */
  onSelect: (id: string) => void;
}

function Canvas({ plan, layout, selectedId, onSelect }: RoadmapChainProps & { layout: ChainLayout }) {
  const t = useTranslate();
  const hintId = useId();
  const fit = useReadableFit(layout);
  const cutText = cutRuleText(plan, t);
  const labels = useMemo(() => chainAriaLabels(t), [t]);

  const nodes = useMemo<ChainNode[]>(() => {
    const placed: ChainNode[] = [...layout.lanes.map(laneNode)];
    if (layout.cutX !== null && cutText !== null) {
      placed.push({
        id: "cut",
        type: "cut",
        position: { x: layout.cutX, y: 0 },
        width: 2,
        height: layout.height,
        data: { text: cutText, height: layout.height },
        draggable: false,
        selectable: false,
        connectable: false,
        deletable: false,
        focusable: false,
      });
    }
    for (const box of layout.boxes) {
      const node = itemNode(box, t);
      placed.push(box.item.id === selectedId && node.type === "item" ? { ...node, data: { ...node.data, selected: true } } : node);
    }
    return placed;
  }, [layout, selectedId, cutText, t]);
  const edges = useMemo(() => layout.edges.map((edge) => waitEdge(edge, t)), [layout, t]);

  return (
    <div className="graph-canvas chain-canvas">
      <p id={hintId} className="sr-only">
        {t("roadmap.canvasDescription")}
      </p>
      <ReactFlow<ChainNode, WaitEdge>
        nodes={nodes}
        edges={edges}
        nodeTypes={NODE_TYPES}
        edgeTypes={EDGE_TYPES}
        aria-label={t("roadmap.canvasLabel")}
        aria-describedby={hintId}
        nodesDraggable={false}
        nodesConnectable={false}
        connectOnClick={false}
        nodesFocusable={false}
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
        ariaLabelConfig={labels}
        defaultMarkerColor={null}
        colorMode="dark"
        minZoom={0.1}
        maxZoom={2}
        onNodeClick={(_event, node) => {
          if (node.type === "item") {
            onSelect(node.data.box.item.id);
          }
        }}
      >
        <Controls showInteractive={false} showFitView={false}>
          <ControlButton
            className="react-flow__controls-fitview"
            title={labels["controls.fitView.ariaLabel"]}
            aria-label={labels["controls.fitView.ariaLabel"]}
            onClick={() => {
              fit();
            }}
          >
            <Icon name="fit" />
          </ControlButton>
        </Controls>
        <FitEachPlan layout={layout} fit={fit} />
      </ReactFlow>
    </div>
  );
}

/**
 * The read-only chain of one plan: its lanes as columns, its items as boxes, its waits as edges;
 * past CANVAS_LIMIT boxes, whole lanes only and the count of those not drawn named above it.
 */
export function RoadmapChain(props: RoadmapChainProps) {
  const t = useTranslate();
  const layout = useMemo(() => chainOf(props.plan), [props.plan]);
  const limit = layout.limit;
  return (
    <div className="graph-canvas-panel chain-panel">
      {limit !== null && (
        <p className="notice cut-notice chain-limit" role="note">
          <span className="notice-title">
            <Icon name="info" />
            <span>{notDrawnText(limit.hidden, limit.total, t)}</span>
          </span>
        </p>
      )}
      <ReactFlowProvider>
        <Canvas {...props} layout={layout} />
      </ReactFlowProvider>
      {/* React Flow's attribution is hidden on the canvas, so it is credited here as text (ui/README.md "Screen rules", Canvas). */}
      <p className="graph-legend-credit">{t("roadmap.credit")}</p>
    </div>
  );
}

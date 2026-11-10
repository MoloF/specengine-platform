import { memo, useEffect, useMemo, useRef, type MouseEvent } from "react";
import type { FlowEdge, FlowNode } from "../api/types";
import { replaceHash } from "../app/location";
import { flowHash, type FlowEdgeKey, type FlowPlace } from "../app/routes";
import { nodeName } from "../graph/layout";
import { useTranslate } from "../i18n";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { isPlainClick } from "./clicks";
import { boxWords, countsText, edgeKeyOf, edgeKeyText, edgeRowWords, gapMark, holdsText, rollupLook, startWord } from "./labels";
import { isGroup } from "./layout";

// The List (docs/features/writer-studio.md D10, "List"; docs/features/flow-scenario.md D12): the
// canvas's text twin, every node and arrow of the drawing read, past the cap too, in served order,
// each by its title. Each row is a link that selects what it names, one history entry replaced; IDs
// and `path:line` stand in the panel alone. A group adds what it holds and Open, which enters it. An
// arrow keeps its count here. In a stage view the boxes stand under their group.

/** Where the List's links go. */
export interface ListPlaces {
  project: string;
  box: (name: string) => FlowPlace;
  edge: (key: FlowEdgeKey) => FlowPlace;
  enter: (name: string) => FlowPlace;
}

/** A click on a selecting link: the hash replaced, the row's link kept as where the selection was made. */
function selecting(onSelect: (origin: HTMLElement) => void) {
  return (event: MouseEvent<HTMLAnchorElement>) => {
    if (!isPlainClick(event)) {
      return;
    }
    event.preventDefault();
    replaceHash(event.currentTarget.getAttribute("href") ?? "");
    onSelect(event.currentTarget);
  };
}

function EdgeRows({ edges, titleOf, places, selectedEdge, onSelect }: { edges: readonly FlowEdge[]; titleOf: (name: string) => string; places: ListPlaces; selectedEdge: string | null; onSelect: (origin: HTMLElement) => void }) {
  const t = useTranslate();
  if (edges.length === 0) {
    return null;
  }
  return (
    <ul className="flow-list-edges">
      {edges.map((edge) => {
        const key = edgeKeyText(edgeKeyOf(edge));
        return (
          <li key={key} data-flow-edge={key} aria-current={key === selectedEdge ? "true" : undefined}>
            <a href={flowHash(places.project, places.edge(edgeKeyOf(edge)))} onClick={selecting(onSelect)}>
              <Icon name="arrowRight" />
              <bdi>{edgeRowWords(edge, titleOf(edge.to), t)}</bdi>
            </a>
          </li>
        );
      })}
    </ul>
  );
}

function BoxRow({
  node,
  inLane,
  named,
  selected,
  marked,
  edges,
  titleOf,
  places,
  selectedEdge,
  onSelect,
}: {
  node: FlowNode;
  inLane: boolean;
  named: ReadonlySet<string>;
  selected: boolean;
  marked: boolean;
  edges: readonly FlowEdge[];
  titleOf: (name: string) => string;
  places: ListPlaces;
  selectedEdge: string | null;
  onSelect: (origin: HTMLElement) => void;
}) {
  const t = useTranslate();
  const name = nodeName(node);
  const gap = gapMark(node, named, t);
  const group = isGroup(node);
  return (
    <li className="flow-list-box" data-flow-row={name} aria-current={selected ? "true" : undefined}>
      <p className="flow-list-head">
        <a className="flow-list-name" href={flowHash(places.project, places.box(name))} onClick={selecting(onSelect)}>
          <bdi className="flow-list-title">{boxWords(node)}</bdi>
        </a>
        {!group && node.rollup !== null && <Badge look={rollupLook(node.rollup, t)} />}
        {group && <span className="flow-list-holds">{holdsText(node.holds, t)}</span>}
        {group && (
          <a className="flow-list-open" href={flowHash(places.project, places.enter(name))}>
            {t("flow.openGroup", { name: boxWords(node) })}
          </a>
        )}
      </p>
      <p className="flow-list-meta">
        {group && node.counts !== null && <span>{countsText(node.counts, t)}</span>}
        {node.start && (
          <span className="flow-mark flow-mark-start">
            <Icon name="flowStart" />
            {startWord(inLane, t)}
          </span>
        )}
        {node.end && (
          <span className="flow-mark flow-mark-end">
            <Icon name="flowEnd" />
            {t("flow.end")}
          </span>
        )}
        {gap !== null && (
          <span className="flow-mark flow-mark-gap">
            <Icon name="flowGap" />
            {gap.shown}
          </span>
        )}
        {marked && (
          <span className="flow-mark flow-mark-marked">
            <Icon name="flowMarked" />
            {t("flow.marked")}
          </span>
        )}
        {selected && (
          <span className="flow-mark flow-mark-selected">
            <Icon name="selected" />
            {t("flow.selected")}
          </span>
        )}
      </p>
      <EdgeRows edges={edges} titleOf={titleOf} places={places} selectedEdge={selectedEdge} onSelect={onSelect} />
    </li>
  );
}

/** One run of boxes: all of a scope's, or one group's in a stage view. */
interface Shelf {
  group: string | null;
  boxes: FlowNode[];
}

export const FlowList = memo(function FlowList({
  boxes,
  edges,
  scope,
  scopeTitle,
  stageView,
  inLane,
  named,
  selected,
  selectedEdge,
  marked,
  places,
  titleOf,
  scrollTo,
  onSelect,
}: {
  /** The drawing read's boxes, served order, the scope's own node left out. */
  boxes: readonly FlowNode[];
  edges: readonly FlowEdge[];
  /** The scope's name; null at the top. */
  scope: string | null;
  scopeTitle: string;
  /** A stage's view: boxes stand under their group. */
  stageView: boolean;
  inLane: boolean;
  named: ReadonlySet<string>;
  selected: string | null;
  /** The selected arrow's key text. */
  selectedEdge: string | null;
  marked: ReadonlySet<string>;
  places: ListPlaces;
  /** A name's title where a read gives one, else the name. */
  titleOf: (name: string) => string;
  /** A name to scroll to; a new object each time. */
  scrollTo: { name: string } | null;
  onSelect: (origin: HTMLElement) => void;
}) {
  const list = useRef<HTMLDivElement>(null);
  const scrolled = useRef<{ name: string } | null>(null);
  const outOf = useMemo(() => {
    const found = new Map<string, FlowEdge[]>();
    for (const edge of edges) {
      const held = found.get(edge.from);
      if (held === undefined) {
        found.set(edge.from, [edge]);
      } else {
        held.push(edge);
      }
    }
    return found;
  }, [edges]);
  const shelves = useMemo<Shelf[]>(() => {
    if (!stageView) {
      return [{ group: null, boxes: [...boxes] }];
    }
    const byGroup = new Map<string | null, FlowNode[]>();
    for (const node of boxes) {
      const held = byGroup.get(node.group);
      if (held === undefined) {
        byGroup.set(node.group, [node]);
      } else {
        held.push(node);
      }
    }
    return [...byGroup].map(([group, held]) => ({ group, boxes: held }));
  }, [boxes, stageView]);
  const own = scope === null ? [] : (outOf.get(scope) ?? []);

  useEffect(() => {
    if (scrollTo === null || scrolled.current === scrollTo) {
      return;
    }
    const row = Array.from(list.current?.querySelectorAll<HTMLElement>("[data-flow-row]") ?? []).find((element) => element.dataset.flowRow === scrollTo.name);
    if (row !== undefined) {
      scrolled.current = scrollTo;
      row.scrollIntoView({ block: "nearest" });
    }
  });

  return (
    <div ref={list} className="flow-list">
      {own.length > 0 && scope !== null && (
        <section className="flow-list-own">
          <h2 className="flow-list-group">
            <bdi>{scopeTitle}</bdi>
          </h2>
          <EdgeRows edges={own} titleOf={titleOf} places={places} selectedEdge={selectedEdge} onSelect={onSelect} />
        </section>
      )}
      {shelves.map((shelf) => (
        <section key={shelf.group ?? ""} className="flow-list-shelf">
          {stageView && shelf.group !== null && (
            <h2 className="flow-list-group">
              <bdi>{titleOf(shelf.group)}</bdi>
            </h2>
          )}
          <ul className="flow-list-boxes">
            {shelf.boxes.map((node) => (
              <BoxRow
                key={`${node.path}:${String(node.line)}`}
                node={node}
                inLane={inLane}
                named={named}
                selected={nodeName(node) === selected}
                marked={marked.has(nodeName(node))}
                edges={outOf.get(nodeName(node)) ?? []}
                titleOf={titleOf}
                places={places}
                selectedEdge={selectedEdge}
                onSelect={onSelect}
              />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
});

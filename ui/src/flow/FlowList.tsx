import { memo, useEffect, useId, useMemo, useRef } from "react";
import type { FlowNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { nodeName } from "../graph/layout";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { useTranslate } from "../i18n";
import { countsText, edgeLine, gapMark, holdsText, readCount, rollupLook, startWord } from "./labels";
import { isGroup, type DrawnRead } from "./layout";
import { keptInList } from "./timeline";

function BoxRow({
  project,
  node,
  inLane,
  open,
  named,
  selected,
  marked,
  onToggle,
}: {
  project: string;
  node: FlowNode;
  inLane: boolean;
  /** Drawn open: a group asked open with no room or no read yet reads as closed, as on the canvas. */
  open: boolean;
  named: ReadonlySet<string>;
  selected: boolean;
  marked: boolean;
  onToggle: (name: string) => void;
}) {
  const t = useTranslate();
  const name = nodeName(node);
  const gap = gapMark(node, open, named, t);
  const group = isGroup(node);
  return (
    <li className="graph-list-node flow-list-box" data-flow-row={name} aria-current={selected ? "true" : undefined}>
      <p className="graph-list-node-head">
        <a className="mono graph-list-name" href={sectionHash(project, "flow", name)}>
          {name}
        </a>
        <span className="kind-tag">
          <span className="sr-only">Kind: </span>
          {node.kind ?? "-"}
        </span>
        {node.title !== null && <span>{node.title}</span>}
        {!group && node.rollup !== null && <Badge look={rollupLook(node.rollup, t)} />}
      </p>
      {group && node.counts !== null && <p className="flow-list-counts">{countsText(node.counts, t)}</p>}
      <p className="graph-list-node-meta">
        <span className="mono">
          {node.path}:{node.line}
        </span>
        {node.stage !== null && (
          <span className="plain-tag">
            <span className="sr-only">Stage: </span>
            <bdi>{node.stage}</bdi>
          </span>
        )}
        <span>{node.distance === null ? "Not on the walk" : `Distance ${String(node.distance)}`}</span>
        {node.start && (
          <span className="flow-mark flow-mark-start">
            <Icon name="flowStart" />
            {startWord(inLane, t)}
          </span>
        )}
        {node.end && (
          <span className="flow-mark flow-mark-end">
            <Icon name="flowEnd" />
            End
          </span>
        )}
        {group && <span>{holdsText(node.holds, t)}</span>}
        {gap !== null && (
          <span className="flow-mark flow-mark-gap">
            <Icon name="flowGap" />
            {gap.shown}
          </span>
        )}
        {marked && (
          <span className="flow-mark flow-mark-marked">
            <Icon name="flowMarked" />
            Marked
          </span>
        )}
        {selected && (
          <span className="flow-mark flow-mark-selected">
            <Icon name="selected" />
            Selected
          </span>
        )}
        {group && (
          <button
            type="button"
            className="link-button"
            aria-expanded={open}
            onClick={() => {
              onToggle(name);
            }}
          >
            {open ? "Close" : "Open"} <span className="sr-only">{name}</span>
          </button>
        )}
      </p>
    </li>
  );
}

function ReadSection({
  project,
  read,
  open,
  named,
  selected,
  marked,
  stage,
  onToggle,
}: {
  project: string;
  read: DrawnRead;
  open: ReadonlySet<string>;
  named: ReadonlySet<string>;
  selected: string | null;
  marked: ReadonlySet<string>;
  /** The stage filter's value: other steps are hidden, and the edges they end; a group stays, with its Open. */
  stage: string;
  onToggle: (name: string) => void;
}) {
  const t = useTranslate();
  const headingId = useId();
  const boxes = useMemo(() => read.boxes.filter((node) => keptInList(node, stage)), [read.boxes, stage]);
  const edges = useMemo(() => {
    const hidden = new Set(read.view.nodes.filter((node) => nodeName(node) !== read.group && !keptInList(node, stage)).map(nodeName));
    return read.view.edges.filter((edge) => !hidden.has(edge.from) && !hidden.has(edge.to));
  }, [read.view, read.group, stage]);
  return (
    <section className="graph-list-group" aria-labelledby={headingId}>
      <h2 id={headingId} className="graph-list-heading">
        {read.group === null ? "Top" : read.group} ({readCount({ nodes: boxes, edges }, t)})
      </h2>
      <ul className="graph-list-nodes">
        {boxes.map((node) => (
          <BoxRow
            key={`${node.path}:${String(node.line)}`}
            project={project}
            node={node}
            inLane={read.group !== null}
            open={open.has(nodeName(node))}
            named={named}
            selected={nodeName(node) === selected}
            marked={marked.has(nodeName(node))}
            onToggle={onToggle}
          />
        ))}
      </ul>
      {edges.length > 0 && (
        <ul className="graph-edge-lines">
          {edges.map((edge) => (
            <li key={edgeLine(edge)} className="verbatim">
              {edgeLine(edge)}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/**
 * The List (docs/features/flow-canvas.md "Data"; ui/README.md "Screen rules", Canvas): the reads
 * drawn, past the cap too. A section per read, its boxes, a closed group with Open, then each edge
 * as `spec flow` prints it. A gap item scrolls it to the row.
 */
export const FlowList = memo(function FlowList({
  project,
  reads,
  open,
  named,
  selected,
  marked,
  stage,
  scrollTo,
  onToggle,
}: {
  project: string;
  reads: readonly DrawnRead[];
  /** The groups drawn open. */
  open: ReadonlySet<string>;
  /** The names a gap row of the top read names. */
  named: ReadonlySet<string>;
  selected: string | null;
  marked: ReadonlySet<string>;
  /** The stage filter's value. */
  stage: string;
  /** A name to scroll to; a new object each time. */
  scrollTo: { name: string } | null;
  onToggle: (name: string) => void;
}) {
  const list = useRef<HTMLDivElement>(null);
  const scrolled = useRef<{ name: string } | null>(null);
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
    <div ref={list} className="graph-list flow-list">
      {reads.map((read) => (
        <ReadSection key={read.group ?? ""} project={project} read={read} open={open} named={named} selected={selected} marked={marked} stage={stage} onToggle={onToggle} />
      ))}
    </div>
  );
});

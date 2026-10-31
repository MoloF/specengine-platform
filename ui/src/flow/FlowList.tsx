import { memo, useEffect, useId, useRef } from "react";
import type { FlowNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { nodeName } from "../graph/layout";
import { Icon } from "../ui/Icon";
import { edgeLine, gapMark, holdsText, readCount, startWord } from "./labels";
import { isGroup, type DrawnRead } from "./layout";

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
  const name = nodeName(node);
  const gap = gapMark(node, open, named);
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
      </p>
      <p className="graph-list-node-meta">
        <span className="mono">
          {node.path}:{node.line}
        </span>
        <span>{node.distance === null ? "Not on the walk" : `Distance ${String(node.distance)}`}</span>
        {node.start && (
          <span className="flow-mark flow-mark-start">
            <Icon name="flowStart" />
            {startWord(inLane)}
          </span>
        )}
        {node.end && (
          <span className="flow-mark flow-mark-end">
            <Icon name="flowEnd" />
            End
          </span>
        )}
        {isGroup(node) && <span>{holdsText(node.holds)}</span>}
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
        {isGroup(node) && (
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
  onToggle,
}: {
  project: string;
  read: DrawnRead;
  open: ReadonlySet<string>;
  named: ReadonlySet<string>;
  selected: string | null;
  marked: ReadonlySet<string>;
  onToggle: (name: string) => void;
}) {
  const headingId = useId();
  return (
    <section className="graph-list-group" aria-labelledby={headingId}>
      <h2 id={headingId} className="graph-list-heading">
        {read.group === null ? "Top" : read.group} ({readCount({ nodes: read.boxes, edges: read.view.edges })})
      </h2>
      <ul className="graph-list-nodes">
        {read.boxes.map((node) => (
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
      {read.view.edges.length > 0 && (
        <ul className="graph-edge-lines">
          {read.view.edges.map((edge) => (
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
        <ReadSection key={read.group ?? ""} project={project} read={read} open={open} named={named} selected={selected} marked={marked} onToggle={onToggle} />
      ))}
    </div>
  );
});

import { useId, useState, type KeyboardEvent, type Ref } from "react";
import { apiErrorOf } from "../api/client";
import { useNode } from "../api/queries";
import type { FlowNode } from "../api/types";
import { sectionHash } from "../app/routes";
import { nodeName } from "../graph/layout";
import { Prose } from "../markdown/Prose";
import { splitFrontMatter } from "../markdown/source";
import { LinksPanel } from "../tree/LinksPanel";
import type { LineTarget } from "../tree/TextView";
import { Icon } from "../ui/Icon";
import { hasModifier } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { boxWords, holdsText, startWord, withGapsText } from "./labels";
import { isGroup } from "./layout";

/**
 * The panel beside the canvas (docs/features/flow-canvas.md "Panel"): a box's words, its place on
 * the walk, a group's edges within and Open or Close, its text and links. Nothing writes here; Esc
 * closes it and puts focus back on the box.
 */
export function FlowDetails({
  project,
  node,
  inLane,
  open,
  noRoom,
  headingRef,
  onToggle,
  onClose,
}: {
  project: string;
  node: FlowNode;
  inLane: boolean;
  /** Drawn open: a group asked open with no room or no read yet reads as closed, as on the canvas. */
  open: boolean;
  noRoom: boolean;
  headingRef: Ref<HTMLHeadingElement>;
  onToggle: (name: string) => void;
  onClose: () => void;
}) {
  const titleId = useId();
  const name = nodeName(node);
  const read = useNode(project, name, { with: ["links"] });
  const [target, setTarget] = useState<LineTarget | null>(null);
  const holder = read.data?.nodes[0];
  const text = holder === undefined ? null : splitFrontMatter(holder.text);

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape" && !hasModifier(event)) {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  }

  return (
    <aside className="graph-details flow-details" aria-labelledby={titleId} onKeyDown={onKeyDown}>
      <header className="graph-details-head">
        <h2 id={titleId} ref={headingRef} tabIndex={-1} className="graph-details-title mono">
          {name}
        </h2>
        <button type="button" className="button button-quiet graph-details-close" onClick={onClose}>
          Close<span className="sr-only"> the panel</span>
        </button>
      </header>
      <dl className="facts">
        <div className="fact fact-wide">
          <dt>Title</dt>
          <dd>{boxWords(node)}</dd>
        </div>
        <div className="fact">
          <dt>Kind</dt>
          <dd className="mono">{node.kind ?? "-"}</dd>
        </div>
        <div className="fact">
          <dt>Distance</dt>
          <dd>{node.distance ?? "Not on the walk"}</dd>
        </div>
        <div className="fact fact-wide">
          <dt>Where</dt>
          <dd className="mono">
            {node.path}:{node.line}
          </dd>
        </div>
      </dl>
      {(node.start || node.end) && (
        <p className="graph-details-state">
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
        </p>
      )}
      {isGroup(node) && (
        <section className="graph-details-section" aria-label="Group">
          <h3 className="card-section-title">Group</h3>
          <p>
            {holdsText(node.holds)}
            {node.gaps > 0 && `, ${withGapsText(node.gaps)}`}
          </p>
          {node.inner.length > 0 && (
            <ul className="graph-edge-lines">
              {node.inner.map((inner) => (
                <li key={inner.type}>
                  <span className="mono">{inner.type}</span> x{inner.count} within
                </li>
              ))}
            </ul>
          )}
          {noRoom && <p className="muted">No room on the canvas: close a group, or read the List.</p>}
          <p>
            <button
              type="button"
              className="button"
              aria-expanded={open}
              onClick={() => {
                onToggle(name);
              }}
            >
              <Icon name={open ? "chevronDown" : "chevronRight"} />
              <span>{open ? "Close" : "Open"} the group</span>
            </button>
          </p>
        </section>
      )}
      {read.data === undefined && read.error === null && <Skeleton label={`Reading ${name}`} lines={3} />}
      {read.error !== null && (
        <ErrorPanel
          title={`${name} could not be read`}
          message={apiErrorOf(read.error).message}
          retrying={read.isFetching}
          attempt={read.errorUpdateCount}
          onRetry={() => {
            void read.refetch({ cancelRefetch: false });
          }}
        />
      )}
      {read.data?.reason !== null && read.data !== undefined && <p className="verbatim">{read.data.reason}</p>}
      {holder !== undefined && text !== null && (
        <section className="graph-details-section" aria-label="Text">
          <h3 className="card-section-title">Text</h3>
          <Prose
            className="flow-details-text"
            source={text.body}
            links={holder.links?.outgoing ?? null}
            project={project}
            baseLevel={3}
            frontMatter={false}
            firstLine={holder.line + text.lines}
            path={holder.path}
            target={target}
          />
        </section>
      )}
      <section className="graph-details-section" aria-label="Links">
        <LinksPanel
          project={project}
          nodeRef={name}
          archive={false}
          query={read}
          onShowInText={(_place, line) => {
            setTarget((last) => ({ line, nonce: (last?.nonce ?? 0) + 1 }));
          }}
          onFollow={() => undefined}
        />
      </section>
      <p className="graph-details-actions">
        <a className="button" href={sectionHash(project, "tree", name)}>
          Open in Spec tree
        </a>
        <a className="button" href={sectionHash(project, "graph", name)}>
          <Icon name="graph" />
          Open in Graph
        </a>
      </p>
      <p className="muted">Changes go through your AI, as proposals.</p>
    </aside>
  );
}

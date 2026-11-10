import { memo, useMemo } from "react";
import type { GraphNode, GraphView } from "../api/types";
import { sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { linkStateLook } from "../tree/labels";
import { endsOf, nodeName, type StubEnd } from "./layout";
import { edgeLine } from "./lines";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

function NodeRow({ project, node }: { project: string; node: GraphNode }) {
  const t = useTranslate();
  const name = nodeName(node);
  return (
    <li className="graph-list-node">
      <p className="graph-list-node-head">
        <DocLink className="mono graph-list-name" project={project} docRef={name}>
          {name}
        </DocLink>
        <span className="kind-tag">
          <span className="sr-only">{t("graph.list.kind")}</span>
          {node.kind ?? "-"}
        </span>
        {node.title !== null && <span>{node.title}</span>}
        {node.distance === 0 && (
          <span className="graph-mark graph-mark-focus">
            <Icon name="focus" />
            {t("graph.focus")}
          </span>
        )}
        {node.archived && (
          <span className="plain-tag">
            <Icon name="archived" />
            {t("graph.archivedWord")}
          </span>
        )}
      </p>
      <p className="graph-list-node-meta">
        <span className="mono">
          {node.path}:{node.line}
        </span>
        <a href={sectionHash(project, "graph", name)}>
          {fill(t("graph.list.centre", { on: slot("on") }), { on: <span className="sr-only">{t("graph.list.centreOn", { name })}</span> })}
        </a>
      </p>
    </li>
  );
}

function StubRow({ stub }: { stub: StubEnd }) {
  const t = useTranslate();
  const end = stub.stub;
  return (
    <li className="graph-list-node">
      <p className="graph-list-node-head">
        <span className="mono graph-list-name">{end.type === "section" ? end.name : end.edge.written}</span>
        <span className="graph-mark">
          <Icon name="stub" />
          {end.type === "section" ? t("graph.notWalkedSection") : t("graph.notWalked")}
        </span>
        {end.type === "unresolved" && <Badge name={t("graph.list.state")} look={linkStateLook(end.edge.state, t)} />}
      </p>
      <ul className="graph-edge-lines">
        {stub.edges.map((edge, index) => (
          <li key={`${String(index)}-${edge.path}:${String(edge.line)}`} className="verbatim">
            {edgeLine(edge)}
          </li>
        ))}
      </ul>
      {end.type === "unresolved" && end.edge.reason !== null && (
        <p className="verbatim link-reason" lang="en">
          {end.edge.reason}
        </p>
      )}
    </li>
  );
}

/**
 * The List (docs/features/ui-graph.md "Implementation", List; ui/README.md "Screen rules", Canvas):
 * the whole answer as text, never limited like the canvas: the nodes by distance in the answer's
 * order, the stubs, every edge line.
 */
export const GraphList = memo(function GraphList({ project, answer }: { project: string; answer: GraphView }) {
  // The shown language re-renders the List: `t` changes with it, the answer does not.
  const t = useTranslate();
  const groups = useMemo(() => {
    const byDistance = new Map<number, GraphNode[]>();
    for (const node of answer.nodes) {
      const nodes = byDistance.get(node.distance);
      if (nodes === undefined) {
        byDistance.set(node.distance, [node]);
      } else {
        nodes.push(node);
      }
    }
    return [...byDistance.entries()];
  }, [answer]);
  const stubs = useMemo(() => endsOf(answer.nodes, answer.edges).stubs, [answer]);
  return (
    <div className="graph-list">
      {groups.map(([distance, nodes]) => (
        <section key={distance} className="graph-list-group" aria-labelledby={`graph-distance-${String(distance)}`}>
          <h2 id={`graph-distance-${String(distance)}`} tabIndex={-1} className="graph-list-heading" data-distance={distance}>
            {t("graph.list.distance", { distance, count: nodes.length })}
          </h2>
          <ul className="graph-list-nodes">
            {nodes.map((node) => (
              <NodeRow key={`${node.path}:${String(node.line)}`} project={project} node={node} />
            ))}
          </ul>
        </section>
      ))}
      {stubs.length > 0 && (
        <section className="graph-list-group" aria-labelledby="graph-stubs">
          <h2 id="graph-stubs" tabIndex={-1} className="graph-list-heading">
            {t("graph.list.notWalked", { count: stubs.length })}
          </h2>
          <ul className="graph-list-nodes">
            {stubs.map((stub) => (
              <StubRow key={stub.id} stub={stub} />
            ))}
          </ul>
        </section>
      )}
      <section className="graph-list-group" aria-labelledby="graph-edges">
        <h2 id="graph-edges" className="graph-list-heading">
          {t("graph.list.edges", { count: answer.edges.length })}
        </h2>
        <ul className="graph-edge-lines">
          {answer.edges.map((edge, index) => (
            <li key={`${String(index)}-${edge.path}:${String(edge.line)}`} className="verbatim">
              {edgeLine(edge)}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
});

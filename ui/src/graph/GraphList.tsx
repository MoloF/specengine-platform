import { memo, useMemo } from "react";
import type { GraphNode, GraphView } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { linkStateLook } from "../tree/labels";
import { endsOf, nodeName, type StubEnd } from "./layout";
import { edgeLine } from "./lines";

function NodeRow({ project, node }: { project: string; node: GraphNode }) {
  const name = nodeName(node);
  return (
    <li className="graph-list-node">
      <p className="graph-list-node-head">
        <a className="mono graph-list-name" href={sectionHash(project, "tree", name)}>
          {name}
        </a>
        <span className="kind-tag">
          <span className="sr-only">Kind: </span>
          {node.kind ?? "-"}
        </span>
        {node.title !== null && <span>{node.title}</span>}
        {node.distance === 0 && (
          <span className="graph-mark graph-mark-focus">
            <Icon name="focus" />
            Focus
          </span>
        )}
        {node.archived && (
          <span className="plain-tag">
            <Icon name="archived" />
            archived
          </span>
        )}
      </p>
      <p className="graph-list-node-meta">
        <span className="mono">
          {node.path}:{node.line}
        </span>
        <a href={sectionHash(project, "graph", name)}>
          Centre here<span className="sr-only"> on {name}</span>
        </a>
      </p>
    </li>
  );
}

function StubRow({ stub }: { stub: StubEnd }) {
  const end = stub.stub;
  return (
    <li className="graph-list-node">
      <p className="graph-list-node-head">
        <span className="mono graph-list-name">{end.type === "section" ? end.name : end.edge.written}</span>
        <span className="graph-mark">
          <Icon name="stub" />
          {end.type === "section" ? "Not walked: a section" : "Not walked"}
        </span>
        {end.type === "unresolved" && <Badge name="State" look={linkStateLook(end.edge.state)} />}
      </p>
      <ul className="graph-edge-lines">
        {stub.edges.map((edge, index) => (
          <li key={`${String(index)}-${edge.path}:${String(edge.line)}`} className="verbatim">
            {edgeLine(edge)}
          </li>
        ))}
      </ul>
      {end.type === "unresolved" && end.edge.reason !== null && <p className="verbatim link-reason">{end.edge.reason}</p>}
    </li>
  );
}

/**
 * The List (docs/features/ui-graph.md "List"): the whole answer as text, never limited like the
 * canvas: the nodes by distance in the answer's order, the stubs, every edge line.
 */
export const GraphList = memo(function GraphList({ project, answer }: { project: string; answer: GraphView }) {
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
            Distance {distance} ({nodes.length})
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
            Not walked ({stubs.length})
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
          Edges ({answer.edges.length})
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

import { Icon } from "../ui/Icon";
import { FLOW_KINDS, dashOf, typeColour, typeDash } from "./labels";

/** A stroke as the canvas draws it. */
function Sample({ dash, colour }: { dash: string; colour: string }) {
  return (
    <svg className="flow-sample" viewBox="0 0 48 8" width="48" height="8" aria-hidden="true" focusable="false">
      <line x1="1" y1="4" x2="47" y2="4" style={{ stroke: colour, strokeDasharray: dash }} />
    </svg>
  );
}

/** An edge of a kind, as its dash is looked up: the colour stays the edge's own. */
const kindDashOf = (kind: string) => dashOf({ from: "", to: "", type: "", kind, when: null, count: 1, path: "", line: 0 }, []);

/**
 * The canvas's legend (docs/features/flow-canvas.md "Data"): each type and kind with its sample, in
 * the daemon's words; what an arc means; the marks; the drawing library's credit.
 */
export function FlowLegend({ types }: { types: readonly string[] }) {
  return (
    <section className="graph-legend flow-legend" aria-label="Legend">
      <h2 className="card-section-title">Legend</h2>
      <p className="graph-legend-about">Left to right is the walk from the start; each box sits by its first step. A line under the boxes runs against the walk.</p>
      <ul className="graph-legend-list">
        {types.map((type) => (
          <li key={type} className="graph-legend-item">
            <Sample dash={typeDash(type, types)} colour={typeColour(type, types)} />
            <span className="mono">{type}</span>
          </li>
        ))}
        {FLOW_KINDS.map((kind) => (
          <li key={kind} className="graph-legend-item">
            <Sample dash={kindDashOf(kind)} colour="var(--graph-edge)" />
            <span className="mono">{kind}</span>
          </li>
        ))}
        <li className="graph-legend-item">
          <svg className="flow-sample" viewBox="0 0 48 14" width="48" height="14" aria-hidden="true" focusable="false">
            <path d="M47,2 H42 Q38,2 38,6 V8 Q38,12 34,12 H14 Q10,12 10,8 V6 Q10,2 6,2 H1" style={{ stroke: "var(--graph-edge)" }} />
          </svg>
          <span>an arc: runs against the walk</span>
        </li>
      </ul>
      <ul className="graph-legend-list">
        <li className="graph-legend-item">
          <span className="flow-mark flow-mark-start">
            <Icon name="flowStart" />
            Start
          </span>
          <span>where the walk begins; in a group, Entry</span>
        </li>
        <li className="graph-legend-item">
          <span className="flow-mark flow-mark-end">
            <Icon name="flowEnd" />
            End
          </span>
          <span>where a way through finishes</span>
        </li>
        <li className="graph-legend-item">
          <span className="flow-mark flow-mark-gap">
            <Icon name="flowGap" />
            Gap
          </span>
          <span>a row of Gaps names it; a closed group counts what it holds</span>
        </li>
        <li className="graph-legend-item">
          <span className="flow-mark">
            <Icon name="chevronRight" />
            Group
          </span>
          <span>click the arrow, or press Space, to open or close it</span>
        </li>
      </ul>
      <p className="graph-legend-credit">Drawn with React Flow (MIT)</p>
    </section>
  );
}

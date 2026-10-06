import type { FollowedType } from "../api/types";
import { Icon } from "../ui/Icon";
import { patternAt } from "./layout";
import { directionIcon, followedLabel } from "./lines";

/** A type's stroke pattern, drawn as the canvas draws it. */
export function PatternSwatch({ position }: { position: number }) {
  return (
    <svg className="graph-pattern" viewBox="0 0 40 8" width="40" height="8" aria-hidden="true" focusable="false">
      <line x1="1" y1="4" x2="39" y2="4" style={{ strokeDasharray: patternAt(position) }} />
    </svg>
  );
}

/**
 * The canvas's legend (docs/features/ui-graph.md "Legend"): what an arrow and a column mean; each
 * followed type in the answer's order with its pattern (by its place in the mode's chip `order`, as
 * the canvas draws it), direction icon and words; the marks; the drawing library's credit.
 */
export function GraphLegend({ types, order }: { types: readonly FollowedType[]; order: readonly string[] }) {
  return (
    <section className="graph-legend" aria-label="Legend">
      <h2 className="card-section-title">Legend</h2>
      <p className="graph-legend-about">Arrows point as the link is written; columns are the distance.</p>
      <ul className="graph-legend-list">
        {types.map((followed) => (
          <li key={followed.type} className="graph-legend-item">
            <PatternSwatch position={order.indexOf(followed.type)} />
            <Icon name={directionIcon(followed.direction)} />
            <span className="mono">{followedLabel(followed.type, followed.direction)}</span>
          </li>
        ))}
      </ul>
      <ul className="graph-legend-list">
        <li className="graph-legend-item">
          <span className="graph-mark graph-mark-focus">
            <Icon name="focus" />
            Focus
          </span>
          <span>the REF, distance 0</span>
        </li>
        <li className="graph-legend-item">
          <span className="graph-mark graph-mark-selected">
            <Icon name="selected" />
            Selected
          </span>
          <span>its details are open, its links drawn bold</span>
        </li>
        <li className="graph-legend-item">
          <span className="graph-box-flag">
            <Icon name="archived" />
            archived
          </span>
          <span>in an archived file</span>
        </li>
        <li className="graph-legend-item">
          <span className="graph-box-flag graph-legend-stub">
            <Icon name="stub" />
            Not walked
          </span>
          <span>a nested section or an unresolved end, dashed; never followed</span>
        </li>
        <li className="graph-legend-item">
          <span className="graph-box-flag">
            <Icon name="more" />
            +k more
          </span>
          <span>the rest of a full column; every node is in the List</span>
        </li>
      </ul>
      <p className="graph-legend-credit">Drawn with React Flow (MIT)</p>
    </section>
  );
}

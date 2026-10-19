import type { HealthQuery } from "../api/queries";
import type { UnboundCounts, UnboundMarker, UnboundRow } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { CodePart, CodeRowList, counted, oneLine, RowNotMeasured } from "./codeRows";
import { bindingLabel, codeRowLook, idStateLabel, unboundReasonLook } from "./labels";

/**
 * A marker that names an unbound node yet binds it not: where it is, its relation raw, its ID
 * state's and binding's words, and why it does not bind (cannot_verify in cannot-verify's role).
 */
function MarkerLine({ marker }: { marker: UnboundMarker }) {
  return (
    <li className="health-code-marker">
      <span className="mono health-code-place">
        {oneLine(marker.path)}:{marker.line}:{marker.column}
      </span>
      <span className="kind-tag">{oneLine(marker.relation)}</span>
      <span className="health-code-fact">
        ID <span className="health-code-word">{oneLine(idStateLabel(marker.id_state))}</span>
      </span>
      <span className="health-code-fact">
        Binding <span className="health-code-word">{oneLine(bindingLabel(marker.binding))}</span>
      </span>
      <Badge name="Binds it not" look={unboundReasonLook(marker.reason)} />
    </li>
  );
}

/**
 * One unbound node: its ID linked to the node in the tree, kind and title raw, where it is (plain),
 * the rules it fails (1-based, as configured), how many mentions name it, then the markers that
 * name it without binding it.
 */
function UnboundLine({ project, row }: { project: string; row: UnboundRow }) {
  return (
    <li className="health-left-row health-code-row" data-unbound={row.id} tabIndex={-1}>
      <p className="health-code-head">
        <a className="health-code-id" href={sectionHash(project, "tree", row.id)}>
          <bdi className="mono">{oneLine(row.id)}</bdi>
        </a>
        {row.kind !== null && <span className="kind-tag">{oneLine(row.kind)}</span>}
        {row.title !== null && <bdi className="health-code-title">{oneLine(row.title)}</bdi>}
      </p>
      <p className="health-code-meta">
        <span className="mono health-code-place">
          {oneLine(row.path)}:{row.line}
        </span>
        <span className="health-code-rules">
          Rules failed: <span className="mono">{row.rules.join(", ")}</span>
        </span>
        <span className="health-code-mentions">{counted(row.mentions, "mention", "mentions")}</span>
      </p>
      {row.markers.length > 0 && (
        <ul className="health-code-markers" aria-label={`Markers naming ${oneLine(row.id)} without binding it`}>
          {row.markers.map((marker, index) => (
            <MarkerLine key={`${String(index)}-${marker.path}:${String(marker.line)}:${String(marker.column)}`} marker={marker} />
          ))}
        </ul>
      )}
    </li>
  );
}

/**
 * Row 5 measured: how many of the selected nodes are unbound, then those nodes in the daemon's
 * order (by path, then line; never sorted here). None: said as the rule, then over how many selected
 * nodes, never a bare 0.
 */
function Unbound({ project, counts, rows }: { project: string; counts: UnboundCounts; rows: UnboundRow[] }) {
  const selected = counted(counts.selected, "selected node", "selected nodes");
  if (rows.length === 0) {
    return (
      <>
        <p className="health-left-total health-code-none">
          <span className="health-value health-measured">None</span>: every node the rules select is bound.
        </p>
        <p className="health-amendments-scope">Measured over {selected}.</p>
      </>
    );
  }
  return (
    <>
      <p className="health-left-total">
        <strong className="health-value health-measured">{counts.unbound}</strong> of {selected}
      </p>
      <CodeRowList rows={rows} label="Unbound nodes, by path" order="by path" render={(row) => <UnboundLine project={project} row={row} />} />
    </>
  );
}

/**
 * Row 5 of What is left, "Unbound nodes" (docs/features/health-code-ui.md; the body:
 * docs/canon/health-code.md "Row 5: unbound"): the nodes the project's `[[health.bound]]` rules
 * select that no marker binds, from the view's health read. Four states kept apart: being measured,
 * not measured (off, not configured), could not be measured (the causes or the read's failure,
 * Retry), measured (zero too). Visibility only, never a gate (ADR-0047).
 */
export function UnboundPart({ project, health, failure, busy }: { project: string; health: HealthQuery; failure: Error | null; busy: boolean }) {
  return (
    <CodePart health={health} failure={failure} busy={busy} row="unbound" title="Unbound nodes" what="unbound nodes">
      {(report, retry) => {
        const row = report.unbound;
        if (codeRowLook(row.state).measured && row.counts !== null && row.rows !== null) {
          return <Unbound project={project} counts={row.counts} rows={row.rows} />;
        }
        return <RowNotMeasured row={row} retrying={health.isFetching} onRetry={retry} />;
      }}
    </CodePart>
  );
}

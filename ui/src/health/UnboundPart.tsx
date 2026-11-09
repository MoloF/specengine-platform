import type { HealthQuery } from "../api/queries";
import type { UnboundCounts, UnboundMarker, UnboundRow } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { CodePart, CodeRowList, oneLine, RowNotMeasured } from "./codeRows";
import { bindingLabel, idStateLabel, isMeasuredState, unboundReasonLook } from "./labels";
import { useTranslate } from "../i18n";
import { around, fill, slot } from "../i18n/fill";

/**
 * A marker that names an unbound node yet binds it not: where it is, its relation raw, its ID
 * state's and binding's words, and why it does not bind (cannot_verify in cannot-verify's role).
 */
function MarkerLine({ marker }: { marker: UnboundMarker }) {
  const t = useTranslate();
  return (
    <li className="health-code-marker">
      <span className="mono health-code-place">
        {oneLine(marker.path)}:{marker.line}:{marker.column}
      </span>
      <span className="kind-tag">{oneLine(marker.relation)}</span>
      <span className="health-code-fact">
        {fill(t("health.unboundRow.idFact", { word: slot("word") }), { word: <span className="health-code-word">{oneLine(idStateLabel(marker.id_state, t))}</span> })}
      </span>
      <span className="health-code-fact">
        {fill(t("health.unboundRow.bindingFact", { word: slot("word") }), { word: <span className="health-code-word">{oneLine(bindingLabel(marker.binding, t))}</span> })}
      </span>
      <Badge name={t("health.unboundRow.bindsNot")} look={unboundReasonLook(marker.reason, t)} />
    </li>
  );
}

/**
 * One unbound node: its ID linked to the node in the tree, kind and title raw, where it is (plain),
 * the rules it fails (1-based, as configured), how many mentions name it, then the markers that
 * name it without binding it.
 */
function UnboundLine({ project, row }: { project: string; row: UnboundRow }) {
  const t = useTranslate();
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
        <span className="health-code-rules">{fill(t("health.unboundRow.rules", { rules: slot("rules") }), { rules: <span className="mono">{row.rules.join(", ")}</span> })}</span>
        <span className="health-code-mentions">{t("health.unboundRow.mentions", { count: row.mentions })}</span>
      </p>
      {row.markers.length > 0 && (
        <ul className="health-code-markers" aria-label={t("health.unboundRow.markersLabel", { id: oneLine(row.id) })}>
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
  const t = useTranslate();
  if (rows.length === 0) {
    return (
      <>
        <p className="health-left-total health-code-none">
          {fill(t("health.unboundRow.none", { none: slot("none") }), { none: <span className="health-value health-measured">{t("health.amendments.noneWord")}</span> })}
        </p>
        <p className="health-amendments-scope">{t("health.unboundRow.measuredOver", { count: counts.selected })}</p>
      </>
    );
  }
  const [before, after] = around(t("health.unboundRow.total", { unbound: counts.unbound, count: counts.selected }), String(counts.unbound));
  return (
    <>
      <p className="health-left-total">
        {before}
        <strong className="health-value health-measured">{counts.unbound}</strong>
        {after}
      </p>
      <CodeRowList rows={rows} label={t("health.unboundRow.listLabel")} order={t("health.code.byPath")} render={(row) => <UnboundLine project={project} row={row} />} />
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
    <CodePart health={health} failure={failure} busy={busy} row="unbound">
      {(report, retry) => {
        const row = report.unbound;
        if (isMeasuredState(row.state) && row.counts !== null && row.rows !== null) {
          return <Unbound project={project} counts={row.counts} rows={row.rows} />;
        }
        return <RowNotMeasured row={row} retrying={health.isFetching} onRetry={retry} />;
      }}
    </CodePart>
  );
}

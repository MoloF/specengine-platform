import type { HealthQuery } from "../api/queries";
import type { AssumesCounts, AssumesRow, HealthCode } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { CodePart, CodeRowList, oneLine, RowNotMeasured } from "./codeRows";
import { assumedLook, isMeasuredState } from "./labels";
import { useTranslate, type Translate } from "../i18n";
import { around, fill, slot } from "../i18n/fill";

/**
 * The tail `spec health` prints after a row 4 line's relation (docs/canon/health-code.md "Text"):
 * the queue item's kind and status, its choice (the stored one-key JSON) and record when the state
 * carries them, a rejection's note; the node an `@assumes` names, where it is, then either the open
 * items holding it or its status and successor. Null when the row has neither (no such item).
 */
function tailOf(row: AssumesRow, record: boolean, under: boolean, t: Translate): string | null {
  const parts: string[] = [];
  if (row.proposal !== null) {
    const item = row.proposal;
    let said = `${item.kind} ${item.status}`;
    if (record && item.choice !== null) {
      said += ` ${JSON.stringify(item.choice)}`;
    }
    if (item.reason !== null) {
      said += `: ${item.reason}`;
    }
    parts.push(said);
    if (record && item.record_id !== null && item.record_path !== null) {
      parts.push(t("health.assumes.record", { id: item.record_id, path: item.record_path }));
    }
  }
  if (row.node !== null) {
    const node = row.node;
    parts.push(node.id, `${node.path}:${String(node.line)}`);
    if (under) {
      parts.push(t("health.assumes.under", { ids: node.proposals.join(", ") }));
    } else {
      if (node.status !== null) {
        parts.push(t("health.assumes.status", { status: node.status }));
      }
      if (node.successor !== null) {
        parts.push(t("health.assumes.successor", { id: node.successor }));
      }
    }
  }
  return parts.length === 0 ? null : oneLine(parts.join(" | "));
}

/**
 * One row 4 line: the state's label and icon; the ID as written, linked to the node it names in
 * the tree (an `@assumes` on an archived node or one in question), else to its inbox card (a queue
 * item the queue holds), else text (no item of that ID); where the marker is; its relation raw;
 * then the CLI's tail. Paths plain.
 */
function AssumesLine({ project, row }: { project: string; row: AssumesRow }) {
  const t = useTranslate();
  const { look, record, under } = assumedLook(row.state, t);
  const id = <bdi className="mono">{oneLine(row.id)}</bdi>;
  let named;
  if (row.node !== null) {
    named = (
      <a className="health-code-id" href={sectionHash(project, "tree", row.node.id)}>
        {id}
      </a>
    );
  } else if (row.proposal !== null) {
    named = (
      <a className="health-code-id" href={sectionHash(project, "inbox", row.id)}>
        {id}
      </a>
    );
  } else {
    named = <span className="health-code-id">{id}</span>;
  }
  const tail = tailOf(row, record, under, t);
  return (
    <li className="health-left-row health-code-row" data-assumes={row.id} tabIndex={-1}>
      <p className="health-code-head">
        <Badge name={t("health.assumes.state")} look={look} />
        {named}
      </p>
      <p className="health-code-meta">
        <span className="mono health-code-place">
          {oneLine(row.path)}:{row.line}:{row.column}
        </span>
        <span className="kind-tag">{oneLine(row.relation)}</span>
      </p>
      {tail !== null && (
        <p className="health-code-tail mono" dir="auto">
          {tail}
        </p>
      )}
    </li>
  );
}

/**
 * Row 4 measured: how many lines over how many markers, then the lines in the daemon's order (by
 * ID, then place; never sorted here). None: said as the rule, then what was measured over: the
 * markers judged and the code files read, never a bare 0.
 */
function Assumes({ project, counts, rows, code }: { project: string; counts: AssumesCounts; rows: AssumesRow[]; code: HealthCode | null }) {
  const t = useTranslate();
  const markers = t("health.code.markers", { count: counts.markers });
  if (rows.length === 0) {
    return (
      <>
        <p className="health-left-total health-code-none">
          {fill(t("health.assumes.none", { none: slot("none") }), { none: <span className="health-value health-measured">{t("health.amendments.noneWord")}</span> })}
        </p>
        <p className="health-amendments-scope">
          {code === null ? t("health.code.measuredOver", { markers }) : t("health.code.measuredOverIn", { markers, files: t("health.code.files", { count: code.files }) })}
        </p>
      </>
    );
  }
  const [before, after] = around(t("health.code.rowsOf", { count: counts.rows, markers }), String(counts.rows));
  return (
    <>
      <p className="health-left-total">
        {before}
        <strong className="health-value health-measured">{counts.rows}</strong>
        {after}
      </p>
      <CodeRowList rows={rows} label={t("health.assumes.listLabel")} order={t("health.assumes.order")} render={(row) => <AssumesLine project={project} row={row} />} />
    </>
  );
}

/**
 * Row 4 of What is left, "Code assuming a decision" (docs/features/health-code-ui.md; the body:
 * docs/canon/health-code.md "Row 4: assumes"): code resting on a queue item, or an `@assumes` on a
 * node archived or in question, from the view's health read. Four states kept apart: being
 * measured, not measured (off), could not be measured (the causes or the read's failure, Retry),
 * measured (zero too). Visibility only, never a gate (ADR-0047).
 */
export function AssumesPart({ project, health, failure, busy }: { project: string; health: HealthQuery; failure: Error | null; busy: boolean }) {
  return (
    <CodePart health={health} failure={failure} busy={busy} row="assumes">
      {(report, retry) => {
        const row = report.assumes;
        if (isMeasuredState(row.state) && row.counts !== null && row.rows !== null) {
          return <Assumes project={project} counts={row.counts} rows={row.rows} code={report.code} />;
        }
        return <RowNotMeasured row={row} retrying={health.isFetching} onRetry={retry} />;
      }}
    </CodePart>
  );
}

import type { HealthQuery } from "../api/queries";
import type { AssumesCounts, AssumesRow, HealthCode } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { CodePart, CodeRowList, counted, oneLine, RowNotMeasured } from "./codeRows";
import { assumedLook, codeRowLook } from "./labels";

/**
 * The tail `spec health` prints after a row 4 line's relation (docs/canon/health-code.md "Text"):
 * the queue item's kind and status, its choice (the stored one-key JSON) and record when the state
 * carries them, a rejection's note; the node an `@assumes` names, where it is, then either the open
 * items holding it or its status and successor. Null when the row has neither (no such item).
 */
function tailOf(row: AssumesRow, record: boolean, under: boolean): string | null {
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
      parts.push(`record ${item.record_id} ${item.record_path}`);
    }
  }
  if (row.node !== null) {
    const node = row.node;
    parts.push(node.id, `${node.path}:${String(node.line)}`);
    if (under) {
      parts.push(`under ${node.proposals.join(", ")}`);
    } else {
      if (node.status !== null) {
        parts.push(`status ${node.status}`);
      }
      if (node.successor !== null) {
        parts.push(`successor ${node.successor}`);
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
  const { look, record, under } = assumedLook(row.state);
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
  const tail = tailOf(row, record, under);
  return (
    <li className="health-left-row health-code-row" data-assumes={row.id} tabIndex={-1}>
      <p className="health-code-head">
        <Badge name="State" look={look} />
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
  if (rows.length === 0) {
    const files = code === null ? "" : ` in ${counted(code.files, "code file", "code files")}`;
    return (
      <>
        <p className="health-left-total health-code-none">
          <span className="health-value health-measured">None</span>: no code rests on a queue item, an archived node or a node in question.
        </p>
        <p className="health-amendments-scope">
          Measured over {counted(counts.markers, "marker", "markers")}
          {files}.
        </p>
      </>
    );
  }
  return (
    <>
      <p className="health-left-total">
        <strong className="health-value health-measured">{counts.rows}</strong> {counts.rows === 1 ? "row" : "rows"} of {counted(counts.markers, "marker", "markers")}
      </p>
      <CodeRowList rows={rows} label="Code assuming a decision, by ID" order="by ID" render={(row) => <AssumesLine project={project} row={row} />} />
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
    <CodePart health={health} failure={failure} busy={busy} row="assumes" title="Code assuming a decision" what="code assuming a decision">
      {(report, retry) => {
        const row = report.assumes;
        if (codeRowLook(row.state).measured && row.counts !== null && row.rows !== null) {
          return <Assumes project={project} counts={row.counts} rows={row.rows} code={report.code} />;
        }
        return <RowNotMeasured row={row} retrying={health.isFetching} onRetry={retry} />;
      }}
    </CodePart>
  );
}

import type { MouseEvent } from "react";
import type { HealthQuery } from "../api/queries";
import type { BindingPlace, DriftCounts, DriftRow, TestedRow } from "../api/types";
import { sectionHash } from "../app/routes";
import { Notes } from "../overview/parts";
import { syncLook } from "../tree/labels";
import { Badge } from "../ui/Badge";
import { CodePart, CodeRowList, counted, oneLine, RowNotMeasured } from "./codeRows";
import { codeRowLook, driftClassLook, driftTally, testedLook } from "./labels";

/** A binding as `spec verify` prints it: `<kind> <symbol>[ <item_kind>]` (`docs/canon/spec-verify-output.md` "Text"). */
function bindingOf(place: Pick<BindingPlace, "kind" | "symbol" | "item_kind">): string {
  return place.item_kind === null ? `${place.kind} ${place.symbol}` : `${place.kind} ${place.symbol} ${place.item_kind}`;
}

/** Where a binding was or might be, as verify's sub-line gives it: `<file> | <binding>`. */
function placeOf(place: BindingPlace): string {
  return oneLine(`${place.file} | ${bindingOf(place)}`);
}

/**
 * A row's node: linked to the tree as cited (an unresolved or gone ID opens the tree's "names
 * nothing"); a bare marker's `""` as text. `onFollow`: the tree view's, when the link sits in it.
 */
function NodeLink({ project, node, onFollow }: { project: string; node: string; onFollow?: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  if (node === "") {
    return <span className="health-code-id">No ID</span>;
  }
  return (
    <a className="health-code-id" href={sectionHash(project, "tree", node)} onClick={onFollow}>
      <bdi className="mono">{oneLine(node)}</bdi>
    </a>
  );
}

/**
 * One row 6 line, `spec verify`'s row (docs/canon/health-drift.md "Text"): its class's label and
 * icon (cannot verify in its own role, never ok's), the node linked to the tree, the relation raw;
 * verify's state through the tree's labels, its category and detail raw; the file and the binding
 * plain (a keyless row has none); then where a moved row was, or where an ambiguous one might be.
 */
function DriftLine({ project, row }: { project: string; row: DriftRow }) {
  const judgement = [row.category, row.detail].filter((word) => word !== null).join(" ");
  const from = row.anchor?.from ?? null;
  const candidates = row.anchor?.candidates ?? [];
  return (
    <li className="health-left-row health-code-row" data-drift={row.node} tabIndex={-1}>
      <p className="health-code-head">
        <Badge name="Class" look={driftClassLook(row.class)} />
        <NodeLink project={project} node={row.node} />
        <span className="kind-tag">{oneLine(row.relation)}</span>
      </p>
      <p className="health-code-meta">
        <Badge name="State" look={syncLook(row.state)} />
        {judgement !== "" && <span className="mono health-code-word">{oneLine(judgement)}</span>}
      </p>
      <p className="health-code-meta">
        <span className="mono health-code-place">{oneLine(row.file)}</span>
        {row.kind !== null && row.symbol !== null ? (
          <span className="mono health-code-place">{oneLine(bindingOf({ kind: row.kind, symbol: row.symbol, item_kind: row.item_kind }))}</span>
        ) : (
          <span>No symbol</span>
        )}
      </p>
      {from !== null && (
        <p className="health-code-tail">
          Moved from <span className="mono">{placeOf(from)}</span>
        </p>
      )}
      {candidates.map((candidate, index) => (
        <p key={`${String(index)}-${candidate.file}`} className="health-code-tail">
          Candidate <span className="mono">{placeOf(candidate)}</span>
        </p>
      ))}
    </li>
  );
}

/**
 * One `[[tested]]` entry: its state's label and icon (unverifiable in cannot-verify's role, never
 * verified's), the node linked to the tree, the category raw; the revision the run vouched for, the
 * node's now, the commit and the run's date, as stored. Row 6 lists those not verified; a node's
 * Bindings tab every one of its set (docs/features/ui-node-bindings.md), its links the tree view's
 * `onFollow`.
 */
export function TestedLine({ project, entry, onFollow }: { project: string; entry: TestedRow; onFollow?: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  return (
    <li className="health-left-row health-code-row" data-tested={entry.node} tabIndex={-1}>
      <p className="health-code-head">
        <Badge name="Evidence" look={testedLook(entry.state)} />
        <NodeLink project={project} node={entry.node} onFollow={onFollow} />
        {entry.category !== null && <span className="mono health-code-word">{oneLine(entry.category)}</span>}
      </p>
      <p className="health-code-meta">
        <span className="health-code-fact">
          Vouched for rev <span className="mono">{entry.rev}</span>
        </span>
        <span className="health-code-fact">
          {entry.node_rev === null ? (
            "No revision now"
          ) : (
            <>
              Now rev <span className="mono">{entry.node_rev}</span>
            </>
          )}
        </span>
        <span className="health-code-fact">
          Commit <span className="mono health-code-place">{oneLine(entry.commit)}</span>
        </span>
        <span className="health-code-fact">
          Tests passed <time dateTime={entry.verified_at}>{oneLine(entry.verified_at)}</time>
        </span>
      </p>
    </li>
  );
}

/**
 * The `[[tested]]` entries not verified, beneath the rows, in the daemon's order (by node): none
 * said as such; not judged (null) said so, the row's note giving why.
 */
function TestedPart({ project, tested }: { project: string; tested: TestedRow[] | null }) {
  let body;
  if (tested === null) {
    body = <p className="health-amendments-scope health-drift-evidence">Not judged: the test evidence could not be judged on this read.</p>;
  } else if (tested.length === 0) {
    body = (
      <p className="health-amendments-scope health-drift-evidence">
        None: no <code>[[tested]]</code> entry is stale, unverifiable or without its node.
      </p>
    );
  } else {
    body = <CodeRowList rows={tested} label="Test evidence not verified, by node" order="by node" render={(entry) => <TestedLine project={project} entry={entry} />} />;
  }
  return (
    <>
      <h4 className="health-subtitle">Test evidence</h4>
      {body}
    </>
  );
}

/**
 * Row 6 measured: the row's notes, then how many rows over how many bindings and verify's counts
 * by class, then the rows in the daemon's order (verify's key order; never sorted here), then the
 * test evidence. None: said as such, then what was measured over, never a bare 0.
 */
function Drift({ project, counts, rows, tested, notes }: { project: string; counts: DriftCounts; rows: DriftRow[]; tested: TestedRow[] | null; notes: string[] }) {
  const bindings = counted(counts.rows, "binding", "bindings");
  let measured;
  if (rows.length === 0) {
    measured = (
      <>
        <p className="health-left-total health-code-none">
          <span className="health-value health-measured">None</span>: every binding <code>spec verify</code> judges is in sync.
        </p>
        <p className="health-amendments-scope">
          Measured over {bindings}, {counts.skipped} skipped.
        </p>
      </>
    );
  } else {
    measured = (
      <>
        <p className="health-left-total">
          <strong className="health-value health-measured">{rows.length}</strong> of {bindings}, {counts.skipped} skipped
        </p>
        <ul className="health-tally" aria-label="Bindings by class, as spec verify counts them">
          {driftTally(counts).map(({ key, look, count }) => (
            <li key={key}>
              <Badge look={look} />
              <span className="sr-only">:</span> <span className="health-tally-count">{count}</span>
            </li>
          ))}
        </ul>
        <CodeRowList rows={rows} label="Drift, in spec verify's order" order="in spec verify's order" render={(row) => <DriftLine project={project} row={row} />} />
      </>
    );
  }
  return (
    <>
      <Notes notes={notes.map(oneLine)} label="Notes from the daemon on the drift" />
      {measured}
      <TestedPart project={project} tested={tested} />
    </>
  );
}

/**
 * Row 6 of What is left, "Drift" (docs/features/health-drift-ui.md; the body:
 * docs/canon/health-drift.md "Document"): plain `spec verify`'s rows but `ok`, its counts and the
 * `[[tested]]` entries not verified, from the view's health read. Four states kept apart, as rows 4
 * and 5: being measured, not measured (off), could not be measured (the causes or the read's
 * failure, Retry), measured (zero too). Visibility only, never a verdict or a gate (ADR-0047).
 */
export function DriftPart({ project, health, failure, busy }: { project: string; health: HealthQuery; failure: Error | null; busy: boolean }) {
  return (
    <CodePart health={health} failure={failure} busy={busy} row="drift" title="Drift" what="drift">
      {(report, retry) => {
        const row = report.drift;
        if (codeRowLook(row.state).measured && row.counts !== null && row.rows !== null) {
          return <Drift project={project} counts={row.counts} rows={row.rows} tested={row.tested} notes={row.notes} />;
        }
        return <RowNotMeasured row={row} retrying={health.isFetching} onRetry={retry} />;
      }}
    </CodePart>
  );
}

import type { MouseEvent, ReactNode } from "react";
import type { HealthQuery } from "../api/queries";
import type { BindingPlace, DriftCounts, DriftLevels, DriftLock, DriftMarker, DriftRow, StoredDriftRow, TestedRow } from "../api/types";
import { sectionHash } from "../app/routes";
import { Notes } from "../overview/parts";
import { bindingOf } from "../tree/BindingLine";
import { syncLook } from "../tree/labels";
import { Badge } from "../ui/Badge";
import { CodePart, CodeRowList, counted, oneLine, RowNotMeasured } from "./codeRows";
import { codeRowLook, driftClassLook, driftTally, isEditorialRow, testedLook } from "./labels";

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

/** A row's markers as verify keeps them: `line <n> @<N>`, or unversioned (`rev` null); none on a lock-only row. */
function markersOf(markers: readonly DriftMarker[]): string {
  if (markers.length === 0) {
    return "No marker";
  }
  const each = markers.map((marker) => `line ${String(marker.line)} ${marker.rev === null ? "unversioned" : `@${String(marker.rev)}`}`);
  return `${markers.length === 1 ? "Marker" : "Markers"} ${each.join(", ")}`;
}

/** A row's lock entry: the revision vouched for, the commit and the date, as stored; none without an entry. */
function LockFact({ lock }: { lock: DriftLock | null }) {
  if (lock === null) {
    return <span className="health-code-fact">Not locked</span>;
  }
  return (
    <span className="health-code-fact">
      Locked rev <span className="mono">{lock.rev}</span>, <span className="mono health-code-place">{oneLine(lock.commit)}</span>,{" "}
      <time dateTime={lock.verified_at}>{oneLine(lock.verified_at)}</time>
    </span>
  );
}

/** A level list's words in canonical order, as sent. */
function levelWords(words: readonly string[]): string {
  return words.length === 0 ? "none" : oneLine(words.join(", "));
}

/** A row's levels: the lock entry's, its markers' united, and those whose digest changed (only when compared by AST). */
function LevelsFact({ levels }: { levels: DriftLevels }) {
  return (
    <p className="health-code-meta health-drift-levels">
      <span>Levels</span>
      <span className="health-code-fact">
        lock <span className="mono">{levelWords(levels.lock)}</span>
      </span>
      <span className="health-code-fact">
        markers <span className="mono">{levelWords(levels.markers)}</span>
      </span>
      {levels.changed !== null && (
        <span className="health-code-fact">
          changed <span className="mono">{levelWords(levels.changed)}</span>
        </span>
      )}
    </p>
  );
}

/**
 * One drift row, `spec verify`'s row (docs/canon/health-drift.md "Text"), Health's row 6 line and a
 * drift item's stored row on the Inbox card alike (docs/canon/drift-screens.md): its class's label
 * and icon (cannot verify in its own role, never ok's), the node linked to the tree, the relation
 * raw, Pre-existing when so; verify's state through the tree's labels, its category and detail raw;
 * the file and the binding plain (a keyless row has none); the markers, the node's revision, the
 * lock entry, how the code was compared, the levels when sent; then where a moved row was, or where
 * an ambiguous one might be. Every field on one line (`oneLine`); no text says "now": the card's
 * rows are as of the commit judged, Health's as of the working tree. An absent `levels` or `anchor`
 * (an older build's stored row) draws as null. `markEditorial`: the card's, a `spec_ahead
 * same_revision` row marked Editorial (labels.ts says which); Health draws no mark. `onFollow`: the
 * tree view's, when the line sits in it; `children`: what the line carries after its own parts (the
 * Bindings tab's lock diff: the row's sides and commands, docs/features/lock-diff-ui.md).
 */
export function DriftLine({
  project,
  row,
  markEditorial = false,
  onFollow,
  children,
}: {
  project: string;
  row: StoredDriftRow;
  markEditorial?: boolean;
  onFollow?: (event: MouseEvent<HTMLAnchorElement>) => void;
  children?: ReactNode;
}) {
  const judgement = [row.category, row.detail].filter((word) => word !== null).join(" ");
  const levels = row.levels ?? null;
  const from = row.anchor?.from ?? null;
  const candidates = row.anchor?.candidates ?? [];
  return (
    <li className="health-left-row health-code-row" data-drift={row.node} tabIndex={-1}>
      <p className="health-code-head">
        <Badge name="Class" look={driftClassLook(row.class)} />
        <NodeLink project={project} node={row.node} onFollow={onFollow} />
        <span className="kind-tag">{oneLine(row.relation)}</span>
        {row.pre_existing && <span className="plain-tag">Pre-existing</span>}
        {markEditorial && isEditorialRow(row) && <span className="plain-tag drift-editorial">Editorial</span>}
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
      <p className="health-code-meta health-drift-facts">
        <span className="health-code-fact">{markersOf(row.markers)}</span>
        <span className="health-code-fact">
          {row.node_rev === null ? (
            "No node revision"
          ) : (
            <>
              Node rev <span className="mono">{row.node_rev}</span>
            </>
          )}
        </span>
        <LockFact lock={row.lock} />
        <span className="health-code-fact">
          {row.compared === null ? (
            "Not compared"
          ) : (
            <>
              Compared <span className="mono">{oneLine(row.compared)}</span>
            </>
          )}
        </span>
      </p>
      {levels !== null && <LevelsFact levels={levels} />}
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
      {children}
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

import type { MouseEvent, ReactNode } from "react";
import type { HealthQuery } from "../api/queries";
import type { BindingPlace, DriftCounts, DriftLevels, DriftLock, DriftMarker, DriftRow, StoredDriftRow, TestedRow } from "../api/types";
import { sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { Notes } from "../overview/parts";
import { bindingOf } from "../tree/BindingLine";
import { syncLook } from "../tree/labels";
import { Badge } from "../ui/Badge";
import { CodePart, CodeRowList, oneLine, RowNotMeasured } from "./codeRows";
import { driftClassLook, driftTally, isEditorialRow, isMeasuredState, testedLook } from "./labels";
import { useTranslate, type Translate } from "../i18n";
import { around, fill, slot } from "../i18n/fill";

/** The command and the evidence table these lines name, as code. */
const VERIFY_COMMAND = "spec verify";
const TESTED_TABLE = "[[tested]]";

/** Where a binding was or might be, as verify's sub-line gives it: `<file> | <binding>`. */
function placeOf(place: BindingPlace): string {
  return oneLine(`${place.file} | ${bindingOf(place)}`);
}

/**
 * A row's node as cited (an unresolved or gone ID says it names nothing): in the tree, with the tree
 * view's `onFollow`, a tree link; on Health a document link
 * (docs/features/doc-modal.md "Acceptance criteria", DM-8). A bare marker's `""` as text.
 */
function NodeLink({ project, node, onFollow }: { project: string; node: string; onFollow?: (event: MouseEvent<HTMLAnchorElement>) => void }) {
  const t = useTranslate();
  if (node === "") {
    return <span className="health-code-id">{t("health.drift.noId")}</span>;
  }
  return onFollow === undefined ? (
    <DocLink className="health-code-id" project={project} docRef={node}>
      <bdi className="mono">{oneLine(node)}</bdi>
    </DocLink>
  ) : (
    <a className="health-code-id" href={sectionHash(project, "tree", node)} onClick={onFollow}>
      <bdi className="mono">{oneLine(node)}</bdi>
    </a>
  );
}

/** A row's markers as verify keeps them: `line <n> @<N>`, or unversioned (`rev` null); none on a lock-only row. */
function markersOf(markers: readonly DriftMarker[], t: Translate): string {
  if (markers.length === 0) {
    return t("health.drift.noMarker");
  }
  const each = markers.map((marker) => t("health.drift.markerLine", { line: marker.line, rev: marker.rev === null ? t("health.drift.unversioned") : `@${String(marker.rev)}` }));
  return markers.length === 1 ? t("health.drift.marker", { list: each.join(", ") }) : t("health.drift.markers", { list: each.join(", ") });
}

/** A row's lock entry: the revision vouched for, the commit and the date, as stored; none without an entry. */
function LockFact({ lock }: { lock: DriftLock | null }) {
  const t = useTranslate();
  if (lock === null) {
    return <span className="health-code-fact">{t("health.drift.notLocked")}</span>;
  }
  return (
    <span className="health-code-fact">
      {fill(t("health.drift.locked", { rev: slot("rev"), commit: slot("commit"), at: slot("at") }), {
        rev: <span className="mono">{lock.rev}</span>,
        commit: <span className="mono health-code-place">{oneLine(lock.commit)}</span>,
        at: <time dateTime={lock.verified_at}>{oneLine(lock.verified_at)}</time>,
      })}
    </span>
  );
}

/** A level list's words in canonical order, as sent. */
function levelWords(words: readonly string[], t: Translate): string {
  return words.length === 0 ? t("health.drift.levelNone") : oneLine(words.join(", "));
}

/** A row's levels: the lock entry's, its markers' united, and those whose digest changed (only when compared by AST). */
function LevelsFact({ levels }: { levels: DriftLevels }) {
  const t = useTranslate();
  return (
    <p className="health-code-meta health-drift-levels">
      <span>{t("health.drift.levels")}</span>
      <span className="health-code-fact">{fill(t("health.drift.levelLock", { words: slot("words") }), { words: <span className="mono">{levelWords(levels.lock, t)}</span> })}</span>
      <span className="health-code-fact">{fill(t("health.drift.levelMarkers", { words: slot("words") }), { words: <span className="mono">{levelWords(levels.markers, t)}</span> })}</span>
      {levels.changed !== null && (
        <span className="health-code-fact">{fill(t("health.drift.levelChanged", { words: slot("words") }), { words: <span className="mono">{levelWords(levels.changed, t)}</span> })}</span>
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
  const t = useTranslate();
  const judgement = [row.category, row.detail].filter((word) => word !== null).join(" ");
  const levels = row.levels ?? null;
  const from = row.anchor?.from ?? null;
  const candidates = row.anchor?.candidates ?? [];
  return (
    <li className="health-left-row health-code-row" data-drift={row.node} tabIndex={-1}>
      <p className="health-code-head">
        <Badge name={t("health.drift.class")} look={driftClassLook(row.class, t)} />
        <NodeLink project={project} node={row.node} onFollow={onFollow} />
        <span className="kind-tag">{oneLine(row.relation)}</span>
        {row.pre_existing && <span className="plain-tag">{t("health.drift.preExisting")}</span>}
        {markEditorial && isEditorialRow(row) && <span className="plain-tag drift-editorial">{t("health.drift.editorial")}</span>}
      </p>
      <p className="health-code-meta">
        <Badge name={t("health.drift.state")} look={syncLook(row.state, t)} />
        {judgement !== "" && <span className="mono health-code-word">{oneLine(judgement)}</span>}
      </p>
      <p className="health-code-meta">
        <span className="mono health-code-place">{oneLine(row.file)}</span>
        {row.kind !== null && row.symbol !== null ? (
          <span className="mono health-code-place">{oneLine(bindingOf({ kind: row.kind, symbol: row.symbol, item_kind: row.item_kind }))}</span>
        ) : (
          <span>{t("health.drift.noSymbol")}</span>
        )}
      </p>
      <p className="health-code-meta health-drift-facts">
        <span className="health-code-fact">{markersOf(row.markers, t)}</span>
        <span className="health-code-fact">
          {row.node_rev === null
            ? t("health.drift.noNodeRev")
            : fill(t("health.drift.nodeRev", { rev: slot("rev") }), { rev: <span className="mono">{row.node_rev}</span> })}
        </span>
        <LockFact lock={row.lock} />
        <span className="health-code-fact">
          {row.compared === null
            ? t("health.drift.notCompared")
            : fill(t("health.drift.compared", { what: slot("what") }), { what: <span className="mono">{oneLine(row.compared)}</span> })}
        </span>
      </p>
      {levels !== null && <LevelsFact levels={levels} />}
      {from !== null && <p className="health-code-tail">{fill(t("health.drift.movedFrom", { place: slot("place") }), { place: <span className="mono">{placeOf(from)}</span> })}</p>}
      {candidates.map((candidate, index) => (
        <p key={`${String(index)}-${candidate.file}`} className="health-code-tail">
          {fill(t("health.drift.candidate", { place: slot("place") }), { place: <span className="mono">{placeOf(candidate)}</span> })}
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
  const t = useTranslate();
  return (
    <li className="health-left-row health-code-row" data-tested={entry.node} tabIndex={-1}>
      <p className="health-code-head">
        <Badge name={t("health.drift.evidence")} look={testedLook(entry.state, t)} />
        <NodeLink project={project} node={entry.node} onFollow={onFollow} />
        {entry.category !== null && <span className="mono health-code-word">{oneLine(entry.category)}</span>}
      </p>
      <p className="health-code-meta">
        <span className="health-code-fact">{fill(t("health.drift.vouched", { rev: slot("rev") }), { rev: <span className="mono">{entry.rev}</span> })}</span>
        <span className="health-code-fact">
          {entry.node_rev === null ? t("health.drift.noRevNow") : fill(t("health.drift.nowRev", { rev: slot("rev") }), { rev: <span className="mono">{entry.node_rev}</span> })}
        </span>
        <span className="health-code-fact">
          {fill(t("health.drift.commit", { commit: slot("commit") }), { commit: <span className="mono health-code-place">{oneLine(entry.commit)}</span> })}
        </span>
        <span className="health-code-fact">
          {fill(t("health.drift.testsPassed", { at: slot("at") }), { at: <time dateTime={entry.verified_at}>{oneLine(entry.verified_at)}</time> })}
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
  const t = useTranslate();
  let body;
  if (tested === null) {
    body = <p className="health-amendments-scope health-drift-evidence">{t("health.drift.testedNotJudged")}</p>;
  } else if (tested.length === 0) {
    body = (
      <p className="health-amendments-scope health-drift-evidence">
        {fill(t("health.drift.testedNone", { table: slot("table") }), { table: <code>{TESTED_TABLE}</code> })}
      </p>
    );
  } else {
    body = <CodeRowList rows={tested} label={t("health.drift.testedLabel")} order={t("health.drift.byNode")} render={(entry) => <TestedLine project={project} entry={entry} />} />;
  }
  return (
    <>
      <h4 className="health-subtitle">{t("health.drift.testedTitle")}</h4>
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
  const t = useTranslate();
  const bindings = t("health.drift.bindings", { count: counts.rows });
  let measured;
  if (rows.length === 0) {
    measured = (
      <>
        <p className="health-left-total health-code-none">
          {fill(t("health.drift.none", { none: slot("none"), command: slot("command") }), {
            none: <span className="health-value health-measured">{t("health.amendments.noneWord")}</span>,
            command: <code>{VERIFY_COMMAND}</code>,
          })}
        </p>
        <p className="health-amendments-scope">{t("health.drift.measuredOver", { count: counts.rows, skipped: counts.skipped })}</p>
      </>
    );
  } else {
    const [before, after] = around(t("health.drift.total", { shown: rows.length, bindings, skipped: counts.skipped }), String(rows.length));
    measured = (
      <>
        <p className="health-left-total">
          {before}
          <strong className="health-value health-measured">{rows.length}</strong>
          {after}
        </p>
        <ul className="health-tally" aria-label={t("health.drift.tallyLabel")}>
          {driftTally(counts, t).map(({ key, look, count }) => (
            <li key={key}>
              <Badge look={look} />
              <span className="sr-only">:</span> <span className="health-tally-count">{count}</span>
            </li>
          ))}
        </ul>
        <CodeRowList rows={rows} label={t("health.drift.listLabel")} order={t("health.drift.order")} render={(row) => <DriftLine project={project} row={row} />} />
      </>
    );
  }
  return (
    <>
      <Notes notes={notes.map(oneLine)} label={t("health.drift.notes")} />
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
    <CodePart health={health} failure={failure} busy={busy} row="drift">
      {(report, retry) => {
        const row = report.drift;
        if (isMeasuredState(row.state) && row.counts !== null && row.rows !== null) {
          return <Drift project={project} counts={row.counts} rows={row.rows} tested={row.tested} notes={row.notes} />;
        }
        return <RowNotMeasured row={row} retrying={health.isFetching} onRetry={retry} />;
      }}
    </CodePart>
  );
}

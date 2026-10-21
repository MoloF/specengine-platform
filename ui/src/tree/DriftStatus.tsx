import { apiErrorOf } from "../api/client";
import type { HealthQuery } from "../api/queries";
import { sectionHash } from "../app/routes";
import { codeRowLook, MEASURED_WITHOUT_COUNTS } from "../health/labels";
import { oneLine } from "../health/oneLine";
import { Icon } from "../ui/Icon";
import { ReadFailure } from "../ui/states";
import { driftOf, driftWords, measuredRows, type TreeDrift } from "./drift";

/** "<n> drift row(s) name(s)": the count and its verb. */
function rowsNaming(count: number): string {
  return count === 1 ? "1 drift row names" : `${String(count)} drift rows name`;
}

/**
 * What "Show drift" says after the toggles (docs/features/ui-tree-drift.md "Line"), drawn only once
 * the tree has answered (TreeView.tsx): the first health read running, "Measuring drift"; its
 * failure in the daemon's words with Retry; row 6 not measured (off, cannot, a state not listed, no
 * counts, a malformed answer), one line with its causes and no badge, never a 0; measured with no
 * row, said so; the rows naming no tree line, one summary linking to Health. A read again keeps the
 * last answer's line in place, busy (`aria-busy`, dimmed), adding nothing above it. Class and state
 * words are told apart in `health/labels.ts` alone.
 */
export function DriftStatus({
  project,
  health,
  failure,
  drift,
  treeShown,
}: {
  project: string;
  health: HealthQuery;
  failure: Error | null;
  drift: TreeDrift | null;
  /** The tree's lines are drawn: the summary of rows naming none of them is meaningful. */
  treeShown: boolean;
}) {
  const report = health.data;
  if (report === undefined) {
    if (failure !== null) {
      return (
        <div className="tree-drift">
          <ReadFailure
            title="Drift could not be read"
            failure={failure}
            retrying={health.isFetching}
            attempt={health.errorUpdateCount}
            onRetry={() => {
              void health.refetch({ cancelRefetch: false });
            }}
          />
        </div>
      );
    }
    return (
      <div className="tree-drift" aria-busy={health.isFetching}>
        <p className="note tree-drift-line" data-drift-state="measuring">
          Measuring drift
        </p>
      </div>
    );
  }
  const row = driftOf(report);
  const lines = measuredRows(row);
  let body = null;
  if (lines === null) {
    const { state, causes } = driftWords(row);
    const look = state === null ? null : codeRowLook(state);
    body = (
      <p className="note tree-drift-line" data-drift-state="not-measured">
        <Icon name={look?.retry === true ? "cannotCheck" : "info"} />
        <span>
          No drift badges: {look === null || look.measured ? MEASURED_WITHOUT_COUNTS : oneLine(look.label)}
          {causes.length > 0 && (
            <>
              :{" "}
              <span className="verbatim" dir="auto">
                {causes.map(oneLine).join("; ")}
              </span>
            </>
          )}
        </span>
      </p>
    );
  } else if (lines.length === 0) {
    body = (
      <p className="note tree-drift-line" data-drift-state="none">
        <Icon name="checkClean" />
        <span>
          No drift: every binding <code>spec verify</code> judges is in sync.
        </span>
      </p>
    );
  } else if (treeShown && drift !== null && drift.unmatched > 0) {
    body = (
      <p className="note tree-drift-line" data-drift-state="unmatched">
        <Icon name="info" />
        <span>
          {rowsNaming(drift.unmatched)} no line of this tree. <a href={sectionHash(project, "health")}>See them in Health</a>
        </span>
      </p>
    );
  }
  return (
    <div className="tree-drift" aria-busy={health.isFetching}>
      {body}
      {health.error !== null && !health.isFetching && (
        <p className="note tree-drift-line" role="alert">
          Drift could not be read again: {apiErrorOf(health.error).message}
        </p>
      )}
    </div>
  );
}

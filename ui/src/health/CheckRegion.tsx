import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import type { CheckQuery } from "../api/queries";
import type { CheckCounts, CheckReport } from "../api/types";
import { sectionHash } from "../app/routes";
import { useAnnounce } from "../ui/announcer";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ReadFailure, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { HealthRegion } from "./parts";
import { isCannotCheck, isClean, isKnownMode, unknownModeLook, verdictLook } from "./labels";

/** The counts in the report's order, each with its label; one the report omits is not shown (W-1). */
const COUNTS: readonly (readonly [keyof Omit<CheckCounts, "worst_w_bytes">, string])[] = [
  ["documents", "Documents"],
  ["errors", "Errors"],
  ["warnings", "Warnings"],
  ["debt", "In debt"],
  ["expired", "Expired debt"],
  ["stale", "Stale debt entries"],
  ["introduced", "Introduced"],
  ["new_debt", "New debt"],
];

/** A cause as the CLI prints it after `cannot`: `<path>: <message>`, `.` for no path (core `check/report.rs` `shown`). */
function causeText(path: string, message: string): string {
  return `${path === "" ? "." : path}: ${message}`;
}

function CheckAnswer({ project, report }: { project: string; report: CheckReport }) {
  const cannot = isCannotCheck(report.verdict);
  const verdict = verdictLook(report.verdict);
  const counts = COUNTS.flatMap(([key, label]) => {
    const value = report.counts[key];
    return value === undefined ? [] : [{ key, label, value }];
  });
  const clean = isClean(report.verdict) && report.findings.length === 0 && report.stale.length === 0;
  return (
    <>
      <p className="check-verdict">
        <Badge name="Verdict" look={verdict} />
      </p>
      {clean && (
        <div className="health-clean">
          <p>
            The check is clean: {report.counts.documents} {report.counts.documents === 1 ? "document" : "documents"}, no findings, no debt.
          </p>
          <p>
            Next: <a href={sectionHash(project, "inbox")}>see what waits in the Inbox</a>.
          </p>
        </div>
      )}
      <p className="check-line">
        <span className="check-line-label">Mode:</span>{" "}
        {isKnownMode(report.mode) ? <span className="mono check-mode">{report.mode}</span> : <Badge look={unknownModeLook(report.mode)} />}
      </p>
      <p className="check-line">
        <span className="check-line-label">Worst W:</span>{" "}
        <span className="health-value check-worst-w">{cannot ? "Not measured" : `${String(report.counts.worst_w_bytes)} B`}</span>
      </p>
      <p className="check-line">
        <span className="check-line-label">Task W:</span>{" "}
        <span className="health-value check-task-w">
          Not measured yet (the <code>bundles</code> log)
        </span>
      </p>
      {report.cannot_check.length > 0 && (
        <div className="health-part">
          <h3 className="health-part-title">Why the check could not vouch for the corpus</h3>
          <ul className="check-causes">
            {report.cannot_check.map((cause, index) => (
              <li key={`${String(index)}-${cause.path}`} className="verbatim">
                {causeText(cause.path, cause.message)}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="health-part">
        <h3 className="health-part-title">Counts</h3>
        {cannot && <p className="muted">Counts cover only what the check could read.</p>}
        <dl className="check-counts">
          {counts.map(({ key, label, value }) => (
            <div key={key} className="check-count" data-count={key}>
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
      </div>
    </>
  );
}

/**
 * Region 1, Check (docs/features/ui-health.md "Description and interactions"): the report's
 * verdict, mode, counts and W as the core gave them; "Check again" reads it once more. Nothing is
 * derived: an omitted count is not shown, W is "Not measured" when the check could not vouch.
 */
export function CheckRegion({ project, check }: { project: string; check: CheckQuery }) {
  const failure = useRetainedFailure(check.error, check.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(check.data, () => heading.current);
  const announce = useAnnounce();

  function checkAgain() {
    if (check.isFetching) {
      return;
    }
    void check.refetch({ cancelRefetch: false }).then((result) => {
      if (result.data !== undefined && result.error === null) {
        announce(`Checked again: ${verdictLook(result.data.verdict).label}.`);
      }
    });
  }

  let body;
  if (check.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={`Checking ${project}`} lines={4} />
      ) : (
        <ReadFailure
          title="The check could not be read"
          failure={failure}
          retrying={check.isFetching}
          attempt={check.errorUpdateCount}
          onRetry={() => {
            retried();
            void check.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    body = (
      <>
        <CheckAnswer project={project} report={check.data} />
        {check.error !== null && !check.isFetching && (
          <p className="note" role="alert">
            The check could not be read again: {apiErrorOf(check.error).message}
          </p>
        )}
      </>
    );
  }

  return (
    <HealthRegion
      title="Check"
      boundary="check"
      headingRef={heading}
      busy={check.data === undefined ? failure === null : check.isFetching}
      className="health-check"
      action={
        check.data === undefined ? null : (
          <button type="button" className="button" aria-disabled={check.isFetching} onClick={checkAgain}>
            <Icon name="retry" />
            <span>{check.isFetching ? "Checking" : "Check again"}</span>
          </button>
        )
      }
    >
      {body}
    </HealthRegion>
  );
}

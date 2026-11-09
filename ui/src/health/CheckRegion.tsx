import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import type { CheckQuery, HealthQuery } from "../api/queries";
import type { CheckReport } from "../api/types";
import { sectionHash } from "../app/routes";
import { useAnnounce } from "../ui/announcer";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ReadFailure, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { HealthRegion } from "./parts";
import { isCannotCheck, isClean, isKnownMode, unknownModeLook, verdictLook } from "./labels";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

/** The counts in the report's order, each with its label's key; one the report omits is not shown (W-1). */
const COUNTS = [
  ["documents", "health.check.count.documents"],
  ["errors", "health.check.count.errors"],
  ["warnings", "health.check.count.warnings"],
  ["debt", "health.check.count.debt"],
  ["expired", "health.check.count.expired"],
  ["stale", "health.check.count.stale"],
  ["introduced", "health.check.count.introduced"],
  ["new_debt", "health.check.count.newDebt"],
] as const;

/** The log Task W will be read from, named as code. */
const BUNDLES_LOG = "bundles";

/** A cause as the CLI prints it after `cannot`: `<path>: <message>`, `.` for no path (core `check/report.rs` `shown`). */
function causeText(path: string, message: string): string {
  return `${path === "" ? "." : path}: ${message}`;
}

function CheckAnswer({ project, report }: { project: string; report: CheckReport }) {
  const t = useTranslate();
  const cannot = isCannotCheck(report.verdict);
  const verdict = verdictLook(report.verdict, t);
  const counts = COUNTS.flatMap(([key, label]) => {
    const value = report.counts[key];
    return value === undefined ? [] : [{ key, label: t(label), value }];
  });
  const clean = isClean(report.verdict) && report.findings.length === 0 && report.stale.length === 0;
  return (
    <>
      <p className="check-verdict">
        <Badge name={t("health.check.verdict")} look={verdict} />
      </p>
      {clean && (
        <div className="health-clean">
          <p>{t("health.check.clean", { count: report.counts.documents })}</p>
          <p>{fill(t("health.check.cleanNext", { link: slot("link") }), { link: <a href={sectionHash(project, "inbox")}>{t("health.check.cleanNextLink")}</a> })}</p>
        </div>
      )}
      <p className="check-line">
        <span className="check-line-label">{t("health.check.mode")}</span>{" "}
        {isKnownMode(report.mode) ? <span className="mono check-mode">{report.mode}</span> : <Badge look={unknownModeLook(report.mode)} />}
      </p>
      <p className="check-line">
        <span className="check-line-label">{t("health.check.worstW")}</span>{" "}
        <span className="health-value check-worst-w">{cannot ? t("health.check.notMeasured") : t("health.check.bytes", { bytes: report.counts.worst_w_bytes })}</span>
      </p>
      <p className="check-line">
        <span className="check-line-label">{t("health.check.taskW")}</span>{" "}
        <span className="health-value check-task-w">{fill(t("health.check.taskWNot", { log: slot("log") }), { log: <code>{BUNDLES_LOG}</code> })}</span>
      </p>
      {report.cannot_check.length > 0 && (
        <div className="health-part">
          <h3 className="health-part-title">{t("health.check.causes")}</h3>
          <ul className="check-causes">
            {report.cannot_check.map((cause, index) => (
              <li key={`${String(index)}-${cause.path}`} className="verbatim" lang="en">
                {causeText(cause.path, cause.message)}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="health-part">
        <h3 className="health-part-title">{t("health.check.counts")}</h3>
        {cannot && <p className="muted">{t("health.check.countsCannot")}</p>}
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
 * verdict, mode, counts and W as the core gave them; "Check again" reads it once more, and the
 * unapplied amendments with it (`docs/features/health-left.md` "Description and interactions"), a
 * read of either already in flight kept, never a second walk. Nothing is derived: an omitted count
 * is not shown, W is "Not measured" when the check could not vouch.
 */
export function CheckRegion({ project, check, health }: { project: string; check: CheckQuery; health: HealthQuery }) {
  const t = useTranslate();
  const failure = useRetainedFailure(check.error, check.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(check.data, () => heading.current);
  const announce = useAnnounce();

  function checkAgain() {
    if (check.isFetching) {
      return;
    }
    void health.refetch({ cancelRefetch: false });
    void check.refetch({ cancelRefetch: false }).then((result) => {
      if (result.data !== undefined && result.error === null) {
        announce(t("health.check.checkedAgain", { verdict: verdictLook(result.data.verdict, t).label }));
      }
    });
  }

  let body;
  if (check.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("health.check.checking", { project })} lines={4} />
      ) : (
        <ReadFailure
          title={t("health.check.failed")}
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
            {fill(t("health.check.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(check.error).message}</span> })}
          </p>
        )}
      </>
    );
  }

  return (
    <HealthRegion
      title={t("health.check.title")}
      boundary={t("health.check.boundary")}
      headingRef={heading}
      busy={check.data === undefined ? failure === null : check.isFetching}
      className="health-check"
      action={
        check.data === undefined ? null : (
          <button type="button" className="button" aria-disabled={check.isFetching} onClick={checkAgain}>
            <Icon name="retry" />
            <span>{check.isFetching ? t("health.check.checkingButton") : t("health.check.again")}</span>
          </button>
        )
      }
    >
      {body}
    </HealthRegion>
  );
}

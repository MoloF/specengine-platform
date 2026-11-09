import { useMemo, useRef } from "react";
import type { CheckQuery } from "../api/queries";
import type { CheckReport, DebtEntry } from "../api/types";
import { Badge } from "../ui/Badge";
import { ReadFailure, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { budgetRowsOf, debtRowsOf, rowsOf } from "./findings";
import { findingSeverityLook, isCannotCheck } from "./labels";
import { DebtNote, FindingMessage, FindingPlace, FindingSubject, HealthRegion } from "./parts";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

/** The baseline's file, named as code. */
const DEBT_FILE = ".spec-debt.toml";

/**
 * A baseline entry that matched nothing, by its line in `.spec-debt.toml` (never a line of its
 * `path`): "`.spec-debt.toml` line <line>: <code> on <path> matches nothing".
 */
function StaleLine({ entry }: { entry: DebtEntry }) {
  const t = useTranslate();
  return (
    <li className="stale-row">
      {fill(t("health.debt.staleLine", { file: slot("file"), line: entry.line, code: slot("code"), path: slot("path") }), {
        file: <code>{DEBT_FILE}</code>,
        code: <code className="stale-code">{entry.code}</code>,
        path: <span className="mono stale-path">{entry.path === "" ? '""' : entry.path}</span>,
      })}
    </li>
  );
}

function DebtAndBudgets({ project, report }: { project: string; report: CheckReport }) {
  const t = useTranslate();
  // A check that could not vouch for the corpus may not have read the baseline or every file:
  // an empty list then says only that nothing was reported.
  const cannot = isCannotCheck(report.verdict);
  const rows = useMemo(() => rowsOf(report.findings), [report.findings]);
  const debt = useMemo(() => debtRowsOf(rows), [rows]);
  const budgets = useMemo(() => budgetRowsOf(rows), [rows]);
  return (
    <>
      <div className="health-part">
        <h3 className="health-part-title">{t("health.debt.debtTitle")}</h3>
        {debt.length === 0 ? (
          <p className="muted">
            {cannot ? t("health.debt.noDebtCannot") : fill(t("health.debt.noDebt", { file: slot("file") }), { file: <code>{DEBT_FILE}</code> })}
          </p>
        ) : (
          <ul className="debt-list">
            {debt.map(({ key, finding }) => (
              <li key={key} className="debt-row" data-debt={key}>
                <span className="finding-head">
                  <Badge name={t("health.severityName")} look={findingSeverityLook(finding.severity, t)} />
                  <code className="debt-code">{finding.code}</code>
                  <FindingPlace project={project} finding={finding} linked={false} />
                  <FindingSubject subject={finding.subject} />
                </span>
                {finding.debt !== undefined && <DebtNote debt={finding.debt} />}
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="health-part">
        <h3 className="health-part-title">{t("health.debt.staleTitle")}</h3>
        {report.stale.length === 0 ? (
          <p className="muted">{cannot ? t("health.debt.noStaleCannot") : t("health.debt.noStale")}</p>
        ) : (
          <ul className="stale-list">
            {report.stale.map((entry, index) => (
              <StaleLine key={`${String(index)}-${String(entry.line)}`} entry={entry} />
            ))}
          </ul>
        )}
      </div>
      <div className="health-part">
        <h3 className="health-part-title">{t("health.debt.budgetsTitle")}</h3>
        {budgets.length === 0 ? (
          <p className="muted">{cannot ? t("health.debt.noBudgetCannot") : t("health.debt.noBudget")}</p>
        ) : (
          <ul className="budget-list">
            {budgets.map(({ key, finding }) => (
              <li key={key} className="budget-row" data-budget={key}>
                <span className="finding-head">
                  <span className="budget-slot">
                    <span className="budget-slot-label">{t("health.debt.slot")}</span> <span className="finding-subject-text mono verbatim">{finding.subject}</span>
                  </span>
                  <FindingPlace project={project} finding={finding} linked={false} />
                </span>
                <FindingMessage message={finding.message} />
                {finding.debt !== undefined && <DebtNote debt={finding.debt} />}
              </li>
            ))}
          </ul>
        )}
      </div>
    </>
  );
}

/**
 * Region 4, Debt and budgets: the findings in debt by their stored expiry, expired first; each
 * stale baseline entry; exactly the `budget` findings, slot and message verbatim (no headroom: the
 * size and the cap live only in the message, never parsed).
 */
export function DebtRegion({ project, check }: { project: string; check: CheckQuery }) {
  const t = useTranslate();
  const failure = useRetainedFailure(check.error, check.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(check.data, () => heading.current);

  let body;
  if (check.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("health.debt.loading", { project })} lines={3} />
      ) : (
        <ReadFailure
          title={t("health.debt.failed")}
          failure={failure}
          retrying={check.isFetching}
          attempt={check.errorUpdateCount}
          announce={false}
          onRetry={() => {
            retried();
            void check.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    body = <DebtAndBudgets project={project} report={check.data} />;
  }

  return (
    <HealthRegion
      title={t("health.debt.title")}
      boundary={t("health.debt.boundary")}
      headingRef={heading}
      busy={check.data === undefined ? failure === null : check.isFetching}
      className="health-debt"
    >
      {body}
    </HealthRegion>
  );
}

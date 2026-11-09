import { apiErrorOf } from "../api/client";
import type { HealthQuery } from "../api/queries";
import { sectionHash } from "../app/routes";
import { codeRowLook, measuredWithoutCounts } from "../health/labels";
import { oneLine } from "../health/oneLine";
import { Icon } from "../ui/Icon";
import { ReadFailure } from "../ui/states";
import { driftOf, driftWords, measuredRows, type TreeDrift } from "./drift";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

/** The command whose rows the line counts, named as code. */
const VERIFY_COMMAND = "spec verify";

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
  const t = useTranslate();
  const report = health.data;
  if (report === undefined) {
    if (failure !== null) {
      return (
        <div className="tree-drift">
          <ReadFailure
            title={t("tree.drift.failed")}
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
          {t("tree.drift.measuring")}
        </p>
      </div>
    );
  }
  const row = driftOf(report);
  const lines = measuredRows(row);
  let body = null;
  if (lines === null) {
    const { state, causes } = driftWords(row);
    const look = state === null ? null : codeRowLook(state, t);
    body = (
      <p className="note tree-drift-line" data-drift-state="not-measured">
        <Icon name={look?.retry === true ? "cannotCheck" : "info"} />
        <span>
          {causes.length === 0
            ? t("tree.drift.noBadges", { state: look === null || look.measured ? measuredWithoutCounts(t) : oneLine(look.label) })
            : fill(t("tree.drift.noBadgesCauses", { state: look === null || look.measured ? measuredWithoutCounts(t) : oneLine(look.label), causes: slot("causes") }), {
                causes: (
                  <span className="verbatim" dir="auto" lang="en">
                    {causes.map(oneLine).join("; ")}
                  </span>
                ),
              })}
        </span>
      </p>
    );
  } else if (lines.length === 0) {
    body = (
      <p className="note tree-drift-line" data-drift-state="none">
        <Icon name="checkClean" />
        <span>{fill(t("tree.drift.none", { command: slot("command") }), { command: <code>{VERIFY_COMMAND}</code> })}</span>
      </p>
    );
  } else if (treeShown && drift !== null && drift.unmatched > 0) {
    body = (
      <p className="note tree-drift-line" data-drift-state="unmatched">
        <Icon name="info" />
        <span>
          {fill(t("tree.drift.unmatched", { count: drift.unmatched, link: slot("link") }), { link: <a href={sectionHash(project, "health")}>{t("tree.drift.seeHealth")}</a> })}
        </span>
      </p>
    );
  }
  return (
    <div className="tree-drift" aria-busy={health.isFetching}>
      {body}
      {health.error !== null && !health.isFetching && (
        <p className="note tree-drift-line" role="alert">
          {fill(t("tree.drift.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(health.error).message}</span> })}
        </p>
      )}
    </div>
  );
}

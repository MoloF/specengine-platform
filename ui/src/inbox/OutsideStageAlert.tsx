import type { OutsideStage } from "../api/queries";
import { sectionHash } from "../app/routes";
import { Icon } from "../ui/Icon";

/**
 * A stage changed outside this tab (`docs/canon/decision-staging.md` "UI", "Threat model"): the
 * live tail named a stage or an unstage no write of this tab made. Who made it is unknowable; the
 * owner checks the choice before confirming it on a terminal. Shown on every screen of the project
 * until dismissed or replaced by the next change.
 */
export function OutsideStageAlert({ change, onDismiss }: { change: OutsideStage; onDismiss: () => void }) {
  return (
    <div className="notice notice-outside-stage" role="alert">
      <p className="notice-title">
        <Icon name="alert" />
        <span>
          {change.stagedAt === null
            ? `The staged decision on ${change.id} was removed outside this tab`
            : `The staged decision on ${change.id} changed outside this tab`}
        </span>
      </p>
      {change.stagedAt === null ? (
        <p>Nothing is staged for it now.</p>
      ) : (
        <p>
          A choice was staged <time dateTime={change.stagedAt}>{change.stagedAt}</time> by another page or process. Read it
          before you confirm anything on a terminal.
        </p>
      )}
      <p className="notice-actions">
        <a href={sectionHash(change.project, "inbox", change.id)}>Show {change.id} in the Inbox</a>
        <button type="button" className="button button-quiet" onClick={onDismiss}>
          Dismiss
        </button>
      </p>
    </div>
  );
}

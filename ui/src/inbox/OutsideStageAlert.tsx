import type { OutsideStage } from "../api/queries";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";

/**
 * A stage changed outside this tab (`docs/canon/decision-staging.md` "UI", "Threat model"): the
 * live tail named a stage or an unstage no write of this tab made. Who made it is unknowable; the
 * owner checks the choice before confirming it on a terminal. Shown on every screen of the project
 * until dismissed or replaced by the next change.
 */
export function OutsideStageAlert({ change, onDismiss }: { change: OutsideStage; onDismiss: () => void }) {
  const t = useTranslate();
  return (
    <div className="notice notice-outside-stage" role="alert">
      <p className="notice-title">
        <Icon name="alert" />
        <span>{change.stagedAt === null ? t("inbox.outside.removed", { id: change.id }) : t("inbox.outside.changed", { id: change.id })}</span>
      </p>
      {change.stagedAt === null ? (
        <p>{t("inbox.outside.nothing")}</p>
      ) : (
        <p>{fill(t("inbox.outside.staged", { at: slot("at") }), { at: <time dateTime={change.stagedAt}>{change.stagedAt}</time> })}</p>
      )}
      <p className="notice-actions">
        <a href={sectionHash(change.project, "inbox", change.id)}>{t("inbox.outside.show", { id: change.id })}</a>
        <Button appearance="subtle" onClick={onDismiss}>
          {t("inbox.outside.dismiss")}
        </Button>
      </p>
    </div>
  );
}

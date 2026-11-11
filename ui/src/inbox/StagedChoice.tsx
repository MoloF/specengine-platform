import { useId } from "react";
import type { ProposalOption, Stage } from "../api/types";
import { CopyButton } from "../ui/CopyButton";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { confirmCommand, verbOf } from "./decisions";

/**
 * What a staged choice is read from: a review document (the Inbox's card) or a round sheet's item
 * (the Questions screen), both carrying the stage, its time, the options and the working answer.
 */
interface StagedOf {
  staged: Stage | null;
  staged_at: string | null;
  options: readonly ProposalOption[];
  working_answer: string | null;
}

/**
 * The staged option's label: agent text, written as the round sheet writes a slot (controls and
 * bidirectional marks as `\u{..}`, `docs/canon/round.md` "Sheet") and isolated from its neighbours.
 */
function stagedLabel(label: string | undefined, t: Translate) {
  return label === undefined ? t("inbox.staged.notAnOption") : <bdi>{shownSlot(label)}</bdi>;
}

/**
 * The owner's staged choice (`docs/canon/decision-staging.md` "UI"): when it was staged, the
 * terminal command that confirms it with Copy (fixed words and a checked ID, never free text), what
 * is staged, and Unstage. Nothing staged: nothing shown. The proposal stays open and listed. An
 * editorial approve copies the same bare command, which confirms the stage with its note, and shows
 * an Editorial row (`docs/features/rev-discipline.md` "UI"). On a hub, for a login `[team]` entitles,
 * Confirm stands beside Unstage (`docs/features/hub-confirm.md`): it opens a dialog on this one item,
 * and no local page has it.
 */
export function StagedChoice({
  id,
  review,
  busy,
  refusal,
  onUnstage,
  onConfirm,
  confirmBusy = false,
}: {
  id: string;
  review: StagedOf;
  /** A stage or an unstage is being sent: Unstage stays focusable but does nothing. */
  busy: boolean;
  /** The daemon's words when it refused the last unstage. */
  refusal: string | null;
  onUnstage: () => void;
  /** Opens the Confirm dialog on this item; absent: no control, a local daemon's page among them. */
  onConfirm?: (() => void) | undefined;
  /** A write is being sent: Confirm stays focusable but does nothing. */
  confirmBusy?: boolean;
}) {
  const t = useTranslate();
  const headingId = useId();
  const staged = review.staged;
  const at = review.staged_at;
  if (staged === null || at === null) {
    return null;
  }
  const command = confirmCommand(staged.decision, id);
  return (
    <section className="staged-choice" aria-labelledby={headingId}>
      <h3 id={headingId} className="staged-title">
        <Icon name="terminal" />
        <span>{t("inbox.staged.title")}</span>
      </h3>
      <p className="staged-line">
        {fill(t("inbox.staged.line", { at: slot("at"), command: slot("command") }), {
          // As every stored time shows (`formatUtc`), the stored value in `dateTime`.
          at: <time dateTime={at}>{formatUtc(at, t)}</time>,
          command: command === null ? <span>{t("inbox.staged.noCommand")}</span> : <code className="command-text">{command}</code>,
        })}
      </p>
      {command !== null && <CopyButton text={command} context={t("inbox.copyContext", { command })} failed={t("inbox.copyFailed")} />}
      <dl className="pairs">
        <dt>{t("inbox.staged.decision")}</dt>
        <dd>{verbOf(staged.decision === "approve" ? "accept" : "reject", t)}</dd>
        {staged.decision === "approve" ? (
          <>
            {staged.option !== null && (
              <>
                <dt>{t("inbox.staged.option")}</dt>
                <dd>
                  <span className="mono">[{staged.option}]</span> {stagedLabel(review.options[staged.option]?.label, t)}
                </dd>
              </>
            )}
            {staged.answer !== null && (
              <>
                <dt>{t("inbox.staged.answer")}</dt>
                <dd className="staged-text">{staged.answer}</dd>
              </>
            )}
            {staged.option === null && staged.answer === null && review.working_answer !== null && (
              <>
                <dt>{t("inbox.staged.choice")}</dt>
                <dd>{t("inbox.staged.workingAnswer")}</dd>
              </>
            )}
            {staged.canon !== null && (
              <>
                <dt>{t("inbox.staged.canon")}</dt>
                <dd className="mono">{staged.canon}</dd>
              </>
            )}
            {staged.editorial === true && (
              <>
                <dt>{t("inbox.staged.editorial")}</dt>
                <dd>{t("inbox.staged.keepRevision")}</dd>
              </>
            )}
            {staged.note !== null && (
              <>
                <dt>{t("inbox.staged.note")}</dt>
                <dd className="staged-text">{staged.note}</dd>
              </>
            )}
          </>
        ) : (
          <>
            <dt>{t("inbox.staged.reason")}</dt>
            <dd className="staged-text">{staged.reason}</dd>
          </>
        )}
      </dl>
      <div className="staged-actions">
        {onConfirm !== undefined && (
          <button
            type="button"
            className="button button-primary"
            aria-disabled={confirmBusy}
            onClick={() => {
              if (!confirmBusy) {
                onConfirm();
              }
            }}
          >
            {fill(t("inbox.staged.confirm", { id: slot("id") }), { id: <span className="sr-only">{id}</span> })}
          </button>
        )}
        <button
          type="button"
          className="button"
          aria-disabled={busy}
          onClick={() => {
            if (!busy) {
              onUnstage();
            }
          }}
        >
          {fill(t("inbox.staged.unstage", { id: slot("id") }), { id: <span className="sr-only">{id}</span> })}
        </button>
      </div>
      {refusal !== null && (
        <div className="refusal" role="alert">
          <p className="refusal-title">
            <Icon name="alert" />
            <span>{t("inbox.staged.refused")}</span>
          </p>
          <p className="verbatim" lang="en">
            {refusal}
          </p>
        </div>
      )}
    </section>
  );
}

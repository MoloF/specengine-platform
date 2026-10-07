import { useId } from "react";
import type { Proposal } from "../api/types";
import { CopyButton } from "../ui/CopyButton";
import { Icon } from "../ui/Icon";
import { confirmCommand, WORDING } from "./decisions";

const COPY_FAILED = "Copy failed: select the command and copy it";

/**
 * The owner's staged choice (`docs/canon/decision-staging.md` "UI"): when it was staged, the
 * terminal command that confirms it with Copy (fixed words and a checked ID, never free text), what
 * is staged, and Unstage. Nothing staged: nothing shown. The proposal stays open and listed.
 */
export function StagedChoice({
  id,
  review,
  busy,
  refusal,
  onUnstage,
}: {
  id: string;
  review: Proposal;
  /** A stage or an unstage is being sent: Unstage stays focusable but does nothing. */
  busy: boolean;
  /** The daemon's words when it refused the last unstage. */
  refusal: string | null;
  onUnstage: () => void;
}) {
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
        <span>Staged decision</span>
      </h3>
      <p className="staged-line">
        Staged <time dateTime={at}>{at}</time>. Confirm on a terminal:{" "}
        {command === null ? <span>no command is offered for this ID.</span> : <code className="command-text">{command}</code>}
      </p>
      {command !== null && <CopyButton text={command} context={`the command: ${command}`} failed={COPY_FAILED} />}
      <dl className="pairs">
        <dt>Decision</dt>
        <dd>{staged.decision === "approve" ? WORDING.accept.verb : WORDING.reject.verb}</dd>
        {staged.decision === "approve" ? (
          <>
            {staged.option !== null && (
              <>
                <dt>Option</dt>
                <dd>
                  <span className="mono">[{staged.option}]</span> {review.options[staged.option]?.label ?? "Not among this proposal's options"}
                </dd>
              </>
            )}
            {staged.answer !== null && (
              <>
                <dt>Answer</dt>
                <dd className="staged-text">{staged.answer}</dd>
              </>
            )}
            {staged.option === null && staged.answer === null && review.working_answer !== null && (
              <>
                <dt>Choice</dt>
                <dd>The working answer</dd>
              </>
            )}
            {staged.canon !== null && (
              <>
                <dt>Canon</dt>
                <dd className="mono">{staged.canon}</dd>
              </>
            )}
            {staged.note !== null && (
              <>
                <dt>Note</dt>
                <dd className="staged-text">{staged.note}</dd>
              </>
            )}
          </>
        ) : (
          <>
            <dt>Reason</dt>
            <dd className="staged-text">{staged.reason}</dd>
          </>
        )}
      </dl>
      <div className="staged-actions">
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
          Unstage <span className="sr-only">{id}</span>
        </button>
      </div>
      {refusal !== null && (
        <div className="refusal" role="alert">
          <p className="refusal-title">
            <Icon name="alert" />
            <span>The daemon refused to unstage it; nothing changed.</span>
          </p>
          <p className="verbatim">{refusal}</p>
        </div>
      )}
    </section>
  );
}

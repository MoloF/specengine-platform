import { useEffect, useId, useRef, useState, type SubmitEvent } from "react";
import type { Proposal, StageChoice } from "../api/types";
import { Dialog } from "../ui/Dialog";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { wordingOf, type DecisionKind } from "./decisions";
import { isRefused, revisionsToKeep } from "./revisions";
import { RevisionList } from "./RevisionList";

function initialOption(proposal: Proposal): number | null {
  if (proposal.options.length === 0) {
    return null;
  }
  const recommended = proposal.recommendation;
  return recommended !== null && recommended >= 0 && recommended < proposal.options.length ? recommended : 0;
}

/**
 * The stage a dialog sends (`docs/canon/decision-staging.md` "The stage"): an approve with the
 * chosen option when the proposal has options (else none: a question's working answer, an
 * update's change), `editorial` true when the owner keeps the revisions, else null
 * (`docs/features/rev-discipline.md` "UI"), no `--answer` or `--canon` from the UI; a reject with
 * its reason.
 */
function stageOf(kind: "accept" | "reject", option: number | null, editorial: boolean, text: string): StageChoice {
  return kind === "accept"
    ? { decision: "approve", option, answer: null, canon: null, editorial: editorial ? true : null, note: text === "" ? null : text }
    : { decision: "reject", reason: text };
}

/**
 * Needs clarification and Defer: the daemon stages neither (`docs/canon/decision-staging.md` "UI"),
 * so the dialog says what they would do, that nothing is sent, and closes.
 */
function NotStaged({ id, proposal, kind, onCancel }: { id: string; proposal: Proposal; kind: DecisionKind; onCancel: () => void }) {
  const t = useTranslate();
  const wording = wordingOf(kind, id, t);
  return (
    <Dialog title={wording.title} onClose={onCancel} className="decision-dialog">
      <p className="dialog-text">{proposal.summary ?? proposal.target_id ?? proposal.kind ?? id}</p>
      <p className="dialog-text dialog-effect">{wording.effect}</p>
      <div className="notice notice-not-served" role="status">
        <p className="notice-title">
          <Icon name="info" />
          <span>{t("inbox.dialog.notBuilt")}</span>
        </p>
        <p>{t("inbox.dialog.notSent")}</p>
      </div>
      <div className="dialog-actions">
        <button type="button" className="button" data-autofocus="" onClick={onCancel}>
          {t("inbox.dialog.close")}
        </button>
      </div>
    </Dialog>
  );
}

/**
 * One decision as an in-app dialog. Accept and Reject stage the owner's choice: a submit is one
 * call; while it is pending the dialog stays open: a second submit, Cancel and Esc do nothing, and
 * the buttons keep focus (aria-disabled, not disabled). A refusal keeps the dialog and the typed
 * text, shows the daemon's message verbatim and puts focus back in the text field; the alert speaks
 * it, so the field leaves it out of its description until focus leaves the field. A proposal
 * revised while the dialog is open (a new `updated_at`, a stage made elsewhere among them) replaces
 * the shown one: the dialog says so, keeps the typed text, picks the new version's recommended
 * option (the option is an index into the new list) and waits for a submit made on the new
 * version, which sends its `updated_at`. Accept on a change that alters a node's meaning (the
 * review's `revisions` a non-empty list) offers "Editorial: keep the revision", unchecked: approval
 * raises each revision unless the owner says the change is wording only; a revised version unchecks
 * it again, its revisions being another change's. Needs clarification and Defer send nothing.
 */
export function DecisionDialog({
  id,
  proposal,
  kind,
  onCancel,
  onSubmit,
}: {
  /** The proposal decided on. */
  id: string;
  /** Its review document as read now; the dialog shows the version it was opened on until it changes. */
  proposal: Proposal;
  kind: DecisionKind;
  onCancel: () => void;
  /** Stages the choice against the shown version's `updated_at`; resolves to the refusal to show, or null when the dialog is closed by the caller. */
  onSubmit: (stage: StageChoice, updatedAt: string) => Promise<string | null>;
}) {
  if (kind === "accept" || kind === "reject") {
    return <StageDialog id={id} proposal={proposal} kind={kind} onCancel={onCancel} onSubmit={onSubmit} />;
  }
  return <NotStaged id={id} proposal={proposal} kind={kind} onCancel={onCancel} />;
}

function StageDialog({
  id,
  proposal,
  kind,
  onCancel,
  onSubmit,
}: {
  id: string;
  proposal: Proposal;
  kind: "accept" | "reject";
  onCancel: () => void;
  onSubmit: (stage: StageChoice, updatedAt: string) => Promise<string | null>;
}) {
  const t = useTranslate();
  const wording = wordingOf(kind, id, t);
  const textId = useId();
  const editorialId = useId();
  const editorialHintId = useId();
  const missingId = useId();
  const refusalId = useId();
  /** The version on screen: the one opened, then each revision the inbox brings. */
  const [shown, setShown] = useState(proposal);
  const [changed, setChanged] = useState(false);
  const [option, setOption] = useState(() => initialOption(proposal));
  /** The owner keeps the revisions (`--editorial`); never the default. */
  const [editorial, setEditorial] = useState(false);
  const [text, setText] = useState("");
  const [missing, setMissing] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  /** Focus was just put in the field beside an alert that speaks for itself. */
  const [quiet, setQuiet] = useState(false);
  const inFlight = useRef(false);
  const form = useRef<HTMLFormElement>(null);
  const textArea = useRef<HTMLTextAreaElement>(null);

  if (!pending && proposal.updated_at !== shown.updated_at) {
    // Storing information from previous renders (react.dev, useState). While a stage is being
    // sent the shown version stays: the call went out with its `updated_at`, and its answer closes
    // the dialog or comes back as a refusal, after which the revision is taken.
    setShown(proposal);
    setChanged(true);
    setOption(initialOption(proposal));
    setEditorial(false);
  }

  // A revision can take away the radio that had focus: put focus on the new choice, not the body.
  useEffect(() => {
    if (changed && focusIsLost()) {
      form.current?.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    }
  }, [changed, shown]);

  const hasOptions = kind === "accept" && shown.options.length > 0;
  /** The nodes whose revisions Accept raises: only then is Editorial offered. */
  const raised = kind === "accept" ? revisionsToKeep(shown) : null;
  const showMissing = missing && text.trim() === "";
  const describedBy = quiet
    ? ""
    : [showMissing ? missingId : null, refusal !== null ? refusalId : null].filter((id) => id !== null).join(" ");

  function cancel() {
    if (!inFlight.current) {
      onCancel();
    }
  }

  async function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    const updatedAt = shown.updated_at;
    if (inFlight.current || updatedAt === null) {
      return;
    }
    const trimmed = text.trim();
    if (wording.required && trimmed === "") {
      setMissing(true);
      setQuiet(true);
      textArea.current?.focus();
      return;
    }
    inFlight.current = true;
    setPending(true);
    setRefusal(null);
    const outcome = await onSubmit(stageOf(kind, hasOptions ? option : null, raised !== null && editorial, trimmed), updatedAt);
    if (outcome !== null) {
      inFlight.current = false;
      setPending(false);
      setRefusal(outcome);
      setQuiet(true);
      textArea.current?.focus();
    }
  }

  return (
    <Dialog title={wording.title} onClose={cancel} className="decision-dialog">
      {changed && (
        // A new alert for each revision, so a second one is spoken too.
        <div key={shown.updated_at} className="notice" role="alert">
          <p className="notice-title">
            <Icon name="alert" />
            <span>{t("inbox.dialog.changed")}</span>
          </p>
          <p>{t("inbox.dialog.changedText")}</p>
        </div>
      )}
      <p className="dialog-text">{shown.summary ?? shown.target_id ?? shown.kind ?? id}</p>
      <p className="dialog-text dialog-effect">{wording.effect}</p>
      {shown.interpretation !== null && (
        // Whatever its kind: the key alone says the change carries a decision.
        <p className="dialog-text">
          {fill(t("inbox.dialog.interprets", { decision: slot("decision") }), {
            decision: (
              <span className="mono">
                <bdi>{shownSlot(shown.interpretation.decision)}</bdi>
              </span>
            ),
          })}
        </p>
      )}
      {shown.staged_at !== null && (
        <p className="dialog-text">
          {fill(t("inbox.dialog.replaces", { at: slot("at") }), { at: <time dateTime={shown.staged_at}>{shown.staged_at}</time> })}
        </p>
      )}
      <form
        ref={form}
        className="decision-form"
        noValidate
        aria-busy={pending}
        onSubmit={(event) => {
          void submit(event);
        }}
      >
        {hasOptions && (
          <fieldset className="option-choice">
            <legend>{t("inbox.dialog.option")}</legend>
            {shown.options.map((choice, index) => (
              <label key={`${String(index)}-${choice.label}`} className="option-radio">
                <input
                  type="radio"
                  name="option"
                  value={index}
                  checked={option === index}
                  onChange={() => {
                    setOption(index);
                  }}
                  data-autofocus={option === index ? "" : undefined}
                />
                <span className="option-radio-body">
                  <span className="option-radio-label">
                    {/* The index as `spec approve --option` and the terminal's review line write it. */}
                    <span className="mono">[{index}]</span> <bdi>{shownSlot(choice.label)}</bdi>
                    {index === shown.recommendation && (
                      <span className="recommended-mark">
                        <Icon name="recommended" />
                        {t("inbox.recommended")}
                      </span>
                    )}
                  </span>
                  <span className="option-radio-detail">{choice.effect}</span>
                  <span className="option-radio-detail">{t("inbox.price", { price: choice.price })}</span>
                </span>
              </label>
            ))}
          </fieldset>
        )}
        {kind === "accept" && !hasOptions && shown.working_answer !== null && (
          <div className="dialog-text">
            <p className="field-label">{t("inbox.dialog.workingRecorded")}</p>
            <p className="staged-text">{shown.working_answer}</p>
          </div>
        )}
        {kind === "accept" && !hasOptions && shown.working_answer === null && (
          <p className="dialog-text">{t("inbox.dialog.noOptions")}</p>
        )}
        {raised !== null && (
          <div className="editorial-choice">
            <input
              id={editorialId}
              type="checkbox"
              checked={editorial}
              aria-describedby={editorialHintId}
              onChange={(event) => {
                setEditorial(event.target.checked);
              }}
            />
            <div className="editorial-body">
              <label htmlFor={editorialId} className="option-radio-label">
                {t("inbox.dialog.editorial")}
              </label>
              <div id={editorialHintId} className="option-radio-detail">
                <p>{raised.some(isRefused) ? t("inbox.dialog.uncheckedRefused") : t("inbox.dialog.unchecked")}</p>
                <RevisionList entries={raised} />
                <p>{t("inbox.dialog.checked")}</p>
              </div>
            </div>
          </div>
        )}
        <label className="field-label" htmlFor={textId}>
          {wording.field}
        </label>
        <textarea
          id={textId}
          ref={textArea}
          className="field"
          rows={3}
          value={text}
          readOnly={pending}
          aria-required={wording.required}
          aria-invalid={showMissing}
          aria-describedby={describedBy === "" ? undefined : describedBy}
          data-autofocus={hasOptions ? undefined : ""}
          onChange={(event) => {
            setText(event.target.value);
          }}
          onBlur={() => {
            setQuiet(false);
          }}
        />
        {showMissing && (
          <p id={missingId} className="field-error" role="alert">
            {wording.missing}
          </p>
        )}
        {refusal !== null && (
          <div id={refusalId} className="refusal" role="alert">
            <p className="refusal-title">
              <Icon name="alert" />
              <span>{t("inbox.dialog.refused")}</span>
            </p>
            <p className="verbatim" lang="en">
              {refusal}
            </p>
          </div>
        )}
        <div className="dialog-actions">
          <button
            type="submit"
            className={kind === "reject" ? "button button-danger" : "button button-primary"}
            aria-disabled={pending}
          >
            {pending ? t("inbox.staging") : wording.submit}
          </button>
          <button type="button" className="button" aria-disabled={pending} onClick={cancel}>
            {t("inbox.dialog.cancel")}
          </button>
        </div>
      </form>
    </Dialog>
  );
}

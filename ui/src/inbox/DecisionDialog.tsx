import { useEffect, useId, useRef, useState, type SubmitEvent } from "react";
import type { Proposal, StageChoice } from "../api/types";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { shownSlot, shownText } from "../ui/text";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { answersOf } from "../round/drafts";
import { wordingOf, type DecisionKind } from "./decisions";
import { AskedFold, AskedText, ItemLead, RecommendedMark } from "./ItemLead";
import { isRefused, revisionsToKeep } from "./revisions";
import { RevisionList } from "./RevisionList";

/**
 * What Accept stages on an item with options: one of them by index, or its working answer where the
 * item holds a price of another answer; null until the owner picks: nothing is picked for them, a
 * discrepancy's recommendation neither (docs/features/plain-questions.md "Acceptance criteria", PQ12).
 */
type Chosen = number | "working" | null;

/**
 * The stage a dialog sends (`docs/canon/decision-staging.md` "The stage"): an approve with the
 * chosen option when one is chosen (else none: a question's working answer, picked or its only
 * answer, an update's change), `editorial` true when the owner keeps the revisions, else null
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
      {/* A gist, agent text, as the lead draws it; the summary's line as before. */}
      <p className="dialog-text">{proposal.gist === null ? (proposal.summary ?? proposal.target_id ?? proposal.kind ?? id) : <bdi>{shownSlot(proposal.gist)}</bdi>}</p>
      <p className="dialog-text dialog-effect">{wording.effect}</p>
      <div className="notice notice-not-served" role="status">
        <p className="notice-title">
          <Icon name="info" />
          <span>{t("inbox.dialog.notBuilt")}</span>
        </p>
        <p>{t("inbox.dialog.notSent")}</p>
      </div>
      <div className="dialog-actions">
        <Button data-autofocus="" onClick={onCancel}>
          {t("inbox.dialog.close")}
        </Button>
      </div>
    </Dialog>
  );
}

/**
 * One decision as an in-app dialog. Accept and Reject stage the owner's choice: a submit is one
 * call; while it is pending the dialog stays open: a second submit, Cancel and Esc do nothing, and
 * the buttons keep focus (aria-disabled, not disabled). Accept on an item with options offers each
 * and, where it holds a price of another answer, its working answer; nothing is picked, the
 * recommendation only marked with who made it, and Accept stays off, saying so, until the owner picks
 * (docs/features/plain-questions.md "Acceptance criteria", PQ12). An item holding a gist leads with it
 * and why it matters, its text folded under the choices. A refusal keeps the dialog and the typed
 * text, shows the daemon's message verbatim and puts focus back in the text field; the alert speaks
 * it, so the field leaves it out of its description until focus leaves the field. A proposal
 * revised while the dialog is open (a new `updated_at`, a stage made elsewhere among them) replaces
 * the shown one: the dialog says so, keeps the typed text, clears the pick (an option is an index
 * into the old list) and waits for a pick and a submit made on the new
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
  const pickFirstId = useId();
  /** The version on screen: the one opened, then each revision the inbox brings. */
  const [shown, setShown] = useState(proposal);
  const [changed, setChanged] = useState(false);
  const [chosen, setChosen] = useState<Chosen>(null);
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
    setChosen(null);
    setEditorial(false);
  }

  // A revision can take away the radio that had focus: put focus on the new choice, not the body.
  useEffect(() => {
    if (changed && focusIsLost()) {
      form.current?.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    }
  }, [changed, shown]);

  const hasOptions = kind === "accept" && shown.options.length > 0;
  /** By its structure, never its kind: its working answer where it holds a price of another answer; no own answer here (`docs/canon/decision-staging.md` "UI"). */
  const takesWorking = hasOptions && answersOf(shown).some((offered) => offered.pick === "working");
  /** Accept on options stays off until a pick. */
  const needsPick = hasOptions && chosen === null;
  /** Accept without options records the working answer, shown as the choice; the fold leaves it out then, so it shows once. */
  const recordsWorking = kind === "accept" && !hasOptions && shown.working_answer !== null;
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
    if (needsPick) {
      form.current?.querySelector<HTMLElement>("input[type=radio]")?.focus();
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
    const outcome = await onSubmit(stageOf(kind, hasOptions && typeof chosen === "number" ? chosen : null, raised !== null && editorial, trimmed), updatedAt);
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
      {shown.gist === null ? (
        <p className="dialog-text">{shown.summary ?? shown.target_id ?? shown.kind ?? id}</p>
      ) : (
        <ItemLead gist={shown.gist} matters={shown.matters} />
      )}
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
                  checked={chosen === index}
                  onChange={() => {
                    setChosen(index);
                  }}
                  data-autofocus={chosen === index || (chosen === null && index === 0) ? "" : undefined}
                />
                <span className="option-radio-body">
                  <span className="option-radio-label">
                    {/* The index as `spec approve --option` and the terminal's review line write it. */}
                    <span className="mono">[{index}]</span> <bdi>{shownSlot(choice.label)}</bdi>
                    {index === shown.recommendation && <RecommendedMark author={shown.author} />}
                  </span>
                  <span className="option-radio-detail">{choice.effect}</span>
                  <span className="option-radio-detail">{t("inbox.price", { price: choice.price })}</span>
                </span>
              </label>
            ))}
            {takesWorking && shown.working_answer !== null && (
              <label className="option-radio">
                <input
                  type="radio"
                  name="option"
                  value="working"
                  checked={chosen === "working"}
                  onChange={() => {
                    setChosen("working");
                  }}
                  data-autofocus={chosen === "working" ? "" : undefined}
                />
                <span className="option-radio-body">
                  <span className="option-radio-label">{t("round.takeWorking")}</span>
                  <span className="option-radio-detail">
                    <bdi>{shownText(shown.working_answer)}</bdi>
                  </span>
                </span>
              </label>
            )}
          </fieldset>
        )}
        {shown.gist !== null && (
          <AskedFold>
            {shown.summary !== null && <AskedText text={shown.summary} />}
            {shown.working_answer !== null && !recordsWorking && (
              <div className="round-part">
                <p className="round-label">{t("round.workingAnswer")}</p>
                <AskedText text={shown.working_answer} />
              </div>
            )}
            {shown.price_of_other !== null && (
              <div className="round-part">
                <p className="round-label">{t("round.priceOfOther")}</p>
                <AskedText text={shown.price_of_other} />
              </div>
            )}
          </AskedFold>
        )}
        {recordsWorking && (
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
        {needsPick && (
          <p id={pickFirstId} className="note">
            <Icon name="info" />
            <span>{t("inbox.lead.pickFirst")}</span>
          </p>
        )}
        <div className="dialog-actions">
          <Button type="submit" appearance={kind === "reject" ? "danger" : "primary"} aria-disabled={pending || needsPick} aria-describedby={needsPick ? pickFirstId : undefined}>
            {pending ? t("inbox.staging") : wording.submit}
          </Button>
          <Button aria-disabled={pending} onClick={cancel}>
            {t("inbox.dialog.cancel")}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

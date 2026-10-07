import { useEffect, useId, useRef, useState, type SubmitEvent } from "react";
import type { Proposal, StageChoice } from "../api/types";
import { Dialog } from "../ui/Dialog";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { WORDING, type DecisionKind } from "./decisions";

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
 * update's change), no `--answer` or `--canon` from the UI; a reject with its reason.
 */
function stageOf(kind: "accept" | "reject", option: number | null, text: string): StageChoice {
  return kind === "accept"
    ? { decision: "approve", option, answer: null, canon: null, note: text === "" ? null : text }
    : { decision: "reject", reason: text };
}

/**
 * Needs clarification and Defer: the daemon stages neither (`docs/canon/decision-staging.md` "UI"),
 * so the dialog says what they would do, that nothing is sent, and closes.
 */
function NotStaged({ id, proposal, kind, onCancel }: { id: string; proposal: Proposal; kind: DecisionKind; onCancel: () => void }) {
  const wording = WORDING[kind];
  return (
    <Dialog title={wording.title(id)} onClose={onCancel} className="decision-dialog">
      <p className="dialog-text">{proposal.summary ?? proposal.target_id ?? proposal.kind ?? id}</p>
      <p className="dialog-text dialog-effect">{wording.effect}</p>
      <div className="notice notice-not-served" role="status">
        <p className="notice-title">
          <Icon name="info" />
          <span>Not built yet: only Accept and Reject are staged</span>
        </p>
        <p>Nothing is sent. The proposal stays in the queue as it is, and nothing waits on it.</p>
      </div>
      <div className="dialog-actions">
        <button type="button" className="button" data-autofocus="" onClick={onCancel}>
          Close
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
 * version, which sends its `updated_at`. Needs clarification and Defer send nothing.
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
  const wording = WORDING[kind];
  const textId = useId();
  const missingId = useId();
  const refusalId = useId();
  /** The version on screen: the one opened, then each revision the inbox brings. */
  const [shown, setShown] = useState(proposal);
  const [changed, setChanged] = useState(false);
  const [option, setOption] = useState(() => initialOption(proposal));
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
  }

  // A revision can take away the radio that had focus: put focus on the new choice, not the body.
  useEffect(() => {
    if (changed && focusIsLost()) {
      form.current?.querySelector<HTMLElement>("[data-autofocus]")?.focus();
    }
  }, [changed, shown]);

  const hasOptions = kind === "accept" && shown.options.length > 0;
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
    const outcome = await onSubmit(stageOf(kind, hasOptions ? option : null, trimmed), updatedAt);
    if (outcome !== null) {
      inFlight.current = false;
      setPending(false);
      setRefusal(outcome);
      setQuiet(true);
      textArea.current?.focus();
    }
  }

  return (
    <Dialog title={wording.title(id)} onClose={cancel} className="decision-dialog">
      {changed && (
        // A new alert for each revision, so a second one is spoken too.
        <div key={shown.updated_at} className="notice" role="alert">
          <p className="notice-title">
            <Icon name="alert" />
            <span>This proposal changed since you opened it</span>
          </p>
          <p>The dialog now shows the new version and keeps your text. Check it, then stage your decision again.</p>
        </div>
      )}
      <p className="dialog-text">{shown.summary ?? shown.target_id ?? shown.kind ?? id}</p>
      <p className="dialog-text dialog-effect">{wording.effect}</p>
      {shown.staged_at !== null && (
        <p className="dialog-text">
          It replaces the choice staged <time dateTime={shown.staged_at}>{shown.staged_at}</time>.
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
            <legend>Option</legend>
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
                    <span className="mono">[{index}]</span> {choice.label}
                    {index === shown.recommendation && (
                      <span className="recommended-mark">
                        <Icon name="recommended" />
                        Recommended
                      </span>
                    )}
                  </span>
                  <span className="option-radio-detail">{choice.effect}</span>
                  <span className="option-radio-detail">Price: {choice.price}</span>
                </span>
              </label>
            ))}
          </fieldset>
        )}
        {kind === "accept" && !hasOptions && shown.working_answer !== null && (
          <div className="dialog-text">
            <p className="field-label">The working answer is recorded:</p>
            <p className="staged-text">{shown.working_answer}</p>
          </div>
        )}
        {kind === "accept" && !hasOptions && shown.working_answer === null && (
          <p className="dialog-text">This proposal has no options to choose from.</p>
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
              <span>The daemon refused to stage it; nothing changed.</span>
            </p>
            <p className="verbatim">{refusal}</p>
          </div>
        )}
        <div className="dialog-actions">
          <button
            type="submit"
            className={kind === "reject" ? "button button-danger" : "button button-primary"}
            aria-disabled={pending}
          >
            {pending ? "Staging" : wording.submit}
          </button>
          <button type="button" className="button" aria-disabled={pending} onClick={cancel}>
            Cancel
          </button>
        </div>
      </form>
    </Dialog>
  );
}

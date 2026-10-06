import { useEffect, useId, useRef, useState, type SubmitEvent } from "react";
import type { Decision, Proposal } from "../api/types";
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

function decisionOf(kind: DecisionKind, option: number | null, text: string): Decision {
  switch (kind) {
    case "accept":
      return { decision: "accept", option, note: text === "" ? null : text };
    case "reject":
      return { decision: "reject", reason: text };
    case "needs_clarification":
      return { decision: "needs_clarification", note: text };
    case "defer":
      return { decision: "defer", note: text === "" ? null : text };
  }
}

/**
 * One decision as an in-app dialog. A submit is one call; while it is pending the dialog stays
 * open: a second submit, Cancel and Esc do nothing, and the buttons keep focus (aria-disabled, not
 * disabled). A refusal keeps the dialog and the typed text, shows the daemon's message verbatim
 * and puts focus back in the text field; the alert speaks it, so the field leaves it out of its
 * description until focus leaves the field. A proposal revised while the dialog is open (a new
 * `updated_at`) replaces the shown one: the dialog says so, keeps the typed text, picks the new
 * version's recommended option (the option is an index into the new list) and waits for a submit
 * made on the new version.
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
  /** Resolves to the refusal to show, or null when the dialog is closed by the caller. */
  onSubmit: (decision: Decision) => Promise<string | null>;
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
    // Storing information from previous renders (react.dev, useState). While a decision is being
    // sent the shown version stays: the call went out with it, and its answer closes the dialog or
    // comes back as a refusal, after which the revision is taken.
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
    if (inFlight.current) {
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
    const outcome = await onSubmit(decisionOf(kind, option, trimmed));
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
          <p>The dialog now shows the new version and keeps your text. Check it, then send your decision.</p>
        </div>
      )}
      <p className="dialog-text">{shown.summary ?? shown.target_id ?? shown.kind ?? id}</p>
      <p className="dialog-text dialog-effect">{wording.effect}</p>
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
                    {index + 1}. {choice.label}
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
        {kind === "accept" && !hasOptions && <p className="dialog-text">This proposal has no options to choose from.</p>}
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
              <span>The daemon refused the decision; nothing changed.</span>
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
            {pending ? "Sending" : wording.verb}
          </button>
          <button type="button" className="button" aria-disabled={pending} onClick={cancel}>
            Cancel
          </button>
        </div>
      </form>
    </Dialog>
  );
}

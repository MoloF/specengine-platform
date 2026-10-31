import { createContext, use, useEffect, useId, useMemo, useRef, useState, type ReactNode, type SubmitEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useCardClearGroup, useCards, useCardWrite, type CardWriteRequest } from "../api/queries";
import type { Card, CardAcceptBody, CardClearBody, CardClearGroupWrite, CardCreateBody, CardEditBody, CardRow, CardVerdict, CardWrite } from "../api/types";
import { Dialog } from "../ui/Dialog";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";
import { actingName, isActingLine, useActing } from "./acting";
import { assigneeChoices, disciplineChoices } from "./filter";
import { EVIDENCE_KINDS, GAP_TYPES, NAME_IS_TYPED } from "./labels";

/** How `spec card edit` and `assign` write a cleared value (`docs/canon/cards.md` "Commands"). */
const CLEAR = "-";

/** While a submit is out the fields hold still. */
const Pending = createContext(false);

/** What a form writes as a list: one entry per line, blank lines dropped. */
function linesOf(text: string): string[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
}

/** What every write dialog is told. `onActingLine`: the daemon's line on the acting name, for the field that holds it. */
export interface WriteHandlers {
  onClose: () => void;
  onDone: (answer: CardWrite) => void;
  onActingLine?: (line: string) => void;
}

/** A write sent from a dialog: null once done (the caller closes it), else the words to show, the daemon's own. */
function useSend(project: string, { onDone, onActingLine }: WriteHandlers): (request: CardWriteRequest) => Promise<string | null> {
  const write = useCardWrite(project);
  return async (request) => {
    try {
      onDone(await write.mutateAsync(request));
      return null;
    } catch (error) {
      const answer = apiErrorOf(error);
      if (isActingLine(answer)) {
        onActingLine?.(answer.message);
      }
      return answer.message;
    }
  };
}

/** A one-line field. */
function TextField({
  label,
  value,
  onChange,
  hint,
  type = "text",
  required = false,
  autoFocus = false,
  list,
  invalid = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: string;
  type?: "text" | "date";
  required?: boolean;
  autoFocus?: boolean;
  list?: string;
  invalid?: boolean;
}) {
  const id = useId();
  const hintId = useId();
  const pending = use(Pending);
  return (
    <div className="card-field">
      <label className="field-label" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        className="field"
        type={type}
        value={value}
        readOnly={pending}
        list={list}
        autoComplete="off"
        spellCheck={false}
        aria-required={required}
        aria-invalid={invalid}
        aria-describedby={hint === undefined ? undefined : hintId}
        data-autofocus={autoFocus ? "" : undefined}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      {hint !== undefined && (
        <p id={hintId} className="card-field-hint">
          {hint}
        </p>
      )}
    </div>
  );
}

/** A text field of several lines. */
function TextArea({
  label,
  value,
  onChange,
  hint,
  required = false,
  autoFocus = false,
  invalid = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: string;
  required?: boolean;
  autoFocus?: boolean;
  invalid?: boolean;
}) {
  const id = useId();
  const hintId = useId();
  const pending = use(Pending);
  return (
    <div className="card-field">
      <label className="field-label" htmlFor={id}>
        {label}
      </label>
      <textarea
        id={id}
        className="field"
        rows={3}
        value={value}
        readOnly={pending}
        aria-required={required}
        aria-invalid={invalid}
        aria-describedby={hint === undefined ? undefined : hintId}
        data-autofocus={autoFocus ? "" : undefined}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      {hint !== undefined && (
        <p id={hintId} className="card-field-hint">
          {hint}
        </p>
      )}
    </div>
  );
}

/**
 * A card write as a dialog (`ui/README.md` "Screen rules", Dialogs): a submit is one call; while it
 * is out the dialog stays open, a second submit, Cancel and Esc doing nothing and the buttons keeping
 * focus. `check` stops a submit that cannot be right, sending nothing. A refusal keeps the dialog and
 * the typed text, says the daemon's words in an alert and puts focus back in the first field. A card
 * revised while it is open (`revision`, a refusal's read among them) says so; the next submit goes
 * on the new revision, its fields kept.
 */
function WriteDialog({
  title,
  action,
  sending,
  dismiss = "Cancel",
  danger = false,
  revision = null,
  canSubmit = true,
  check,
  onClose,
  onSubmit,
  children,
}: {
  title: ReactNode;
  action: string;
  sending: string;
  dismiss?: string;
  danger?: boolean;
  revision?: string | null;
  canSubmit?: boolean;
  /** Why this submit cannot be sent, else null. */
  check?: () => string | null;
  onClose: () => void;
  onSubmit: () => Promise<string | null>;
  children: ReactNode;
}) {
  const form = useRef<HTMLFormElement>(null);
  const inFlight = useRef(false);
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [shown, setShown] = useState(revision);
  const [changed, setChanged] = useState(false);

  if (!pending && revision !== shown) {
    // Storing information from previous renders (react.dev, useState). A submit out went on the
    // revision it read; its answer closes the dialog, or comes back and this runs then.
    setShown(revision);
    setChanged(true);
  }

  // A changed card can take away what had focus: the first field takes it, never the body.
  useEffect(() => {
    if (changed && focusIsLost()) {
      firstField()?.focus();
    }
  }, [changed]);

  function firstField(): HTMLElement | null {
    return form.current?.querySelector<HTMLElement>("[data-autofocus], input, textarea, select") ?? null;
  }

  function cancel() {
    if (!inFlight.current) {
      onClose();
    }
  }

  async function submit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault();
    if (inFlight.current || !canSubmit) {
      return;
    }
    const stopped = check?.() ?? null;
    setProblem(stopped);
    if (stopped !== null) {
      firstField()?.focus();
      return;
    }
    inFlight.current = true;
    setPending(true);
    setRefusal(null);
    const outcome = await onSubmit();
    if (outcome !== null) {
      inFlight.current = false;
      setPending(false);
      setRefusal(outcome);
      firstField()?.focus();
    }
  }

  return (
    <Dialog title={title} onClose={cancel} className="decision-dialog card-dialog">
      {changed && (
        // A new alert for each revision, so a second one is spoken too.
        <div key={shown} className="notice" role="alert">
          <p className="notice-title">
            <Icon name="alert" />
            <span>This card changed since you opened it</span>
          </p>
          <p>Fields you left alone keep the new value; what you changed is sent.</p>
        </div>
      )}
      <form
        ref={form}
        className="card-form"
        noValidate
        aria-busy={pending}
        onSubmit={(event) => {
          void submit(event);
        }}
      >
        <Pending value={pending}>{children}</Pending>
        {problem !== null && (
          <p className="field-error" role="alert">
            {problem}
          </p>
        )}
        {refusal !== null && (
          <div className="refusal" role="alert">
            <p className="refusal-title">
              <Icon name="alert" />
              <span>The daemon did not take it.</span>
            </p>
            <p className="verbatim">{refusal}</p>
          </div>
        )}
        <div className="dialog-actions">
          <button type="submit" className={danger ? "button button-danger" : "button button-primary"} aria-disabled={pending} disabled={!canSubmit}>
            {pending ? sending : action}
          </button>
          <button type="button" className="button" aria-disabled={pending} onClick={cancel}>
            {dismiss}
          </button>
        </div>
      </form>
    </Dialog>
  );
}

/** The typed text as an absent key: empty leaves it out of the body. */
function present<K extends string>(key: K, text: string): Partial<Record<K, string>> {
  const value = text.trim();
  return value === "" ? {} : ({ [key]: value } as Record<K, string>);
}

function presentList<K extends string>(key: K, text: string): Partial<Record<K, string[]>> {
  const lines = linesOf(text);
  return lines.length === 0 ? {} : ({ [key]: lines } as Record<K, string[]>);
}

/** The New card dialog's body, as `create_card` takes it. */
export function createBodyOf(form: NewCardForm, as: string | null): CardCreateBody {
  return {
    title: form.title.trim(),
    ...present("goal", form.goal),
    ...present("discipline", form.discipline),
    ...presentList("target", form.targets),
    ...presentList("criterion", form.criteria),
    ...present("assignee", form.assignee),
    ...present("estimate", form.estimate),
    ...present("due", form.due),
    ...present("parent", form.parent),
    ...present("corrects", form.corrects),
    as,
  };
}

/** The New card dialog's fields as typed. */
export interface NewCardForm {
  title: string;
  goal: string;
  discipline: string;
  targets: string;
  criteria: string;
  assignee: string;
  estimate: string;
  due: string;
  parent: string;
  corrects: string;
}

/**
 * New card: every field `create_card` takes, targets and criteria one per line. `target`: the node a
 * New card button stands on, already filled: how a board fills from the brief. Fields left empty are
 * no key: the daemon picks the flow by discipline, the first column.
 */
export function NewCardDialog({ project, target, ...handlers }: { project: string; target?: string } & WriteHandlers) {
  const disciplinesId = useId();
  const namesId = useId();
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const known = useCards(project).data;
  const [acting] = useActing();
  const [form, setForm] = useState<NewCardForm>({
    title: "",
    goal: "",
    discipline: "",
    targets: target ?? "",
    criteria: "",
    assignee: "",
    estimate: "",
    due: "",
    parent: "",
    corrects: "",
  });
  const [titleMissing, setTitleMissing] = useState(false);

  function field(key: keyof NewCardForm) {
    return (value: string) => {
      setForm((current) => ({ ...current, [key]: value }));
    };
  }

  const disciplines = useMemo(() => {
    const served = (known?.flows ?? []).flatMap((flow) => flow.disciplines);
    const read = disciplineChoices(known?.cards ?? [], "").map((choice) => choice.label);
    return [...new Set([...served, ...read])];
  }, [known]);
  const names = useMemo(() => assigneeChoices(known?.cards ?? [], "").slice(0, -1), [known]);

  return (
    <WriteDialog
      title="New card"
      action="Create card"
      sending="Creating"
      onClose={onClose}
      check={() => {
        const missing = form.title.trim() === "";
        setTitleMissing(missing);
        return missing ? "A card needs a title." : null;
      }}
      onSubmit={() => send({ write: "create", body: createBodyOf(form, actingName(acting)) })}
    >
      <TextField label="Title" value={form.title} onChange={field("title")} required autoFocus invalid={titleMissing && form.title.trim() === ""} />
      <TextArea label="Goal" value={form.goal} onChange={field("goal")} />
      <div className="card-form-row">
        <TextField label="Discipline" value={form.discipline} onChange={field("discipline")} list={disciplinesId} hint="Picks the flow; empty: the flow that lists none." />
        <TextField label="Assignee" value={form.assignee} onChange={field("assignee")} list={namesId} />
      </div>
      <datalist id={disciplinesId}>
        {disciplines.map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>
      <datalist id={namesId}>
        {names.map((choice) => (
          <option key={choice.key} value={choice.label} />
        ))}
      </datalist>
      <TextArea label="Source" value={form.targets} onChange={field("targets")} hint="Spec nodes, one per line; none: a card by hand." />
      <TextArea label="Criteria" value={form.criteria} onChange={field("criteria")} hint="One per line." />
      <div className="card-form-row">
        <TextField label="Estimate" value={form.estimate} onChange={field("estimate")} hint="N or N-M, no unit." />
        <TextField label="Due" type="date" value={form.due} onChange={field("due")} />
      </div>
      <div className="card-form-row">
        <TextField label="Parent" value={form.parent} onChange={field("parent")} hint="A card's ID." />
        <TextField label="Corrects" value={form.corrects} onChange={field("corrects")} hint="A card's ID." />
      </div>
    </WriteDialog>
  );
}

/** A field of the card as the Edit dialog types it. */
interface EditForm {
  title: string;
  goal: string;
  targets: string;
  criteria: string;
  estimate: string;
  due: string;
  parent: string;
  corrects: string;
}

function editFormOf(card: Card): EditForm {
  return {
    title: card.title,
    goal: card.goal ?? "",
    targets: card.targets.join("\n"),
    criteria: card.criteria.join("\n"),
    estimate: card.estimate ?? "",
    due: card.due ?? "",
    parent: card.parent ?? "",
    corrects: card.corrects ?? "",
  };
}

// What Edit and Assign send is what the person changed: each field is compared with its own text as
// the dialog opened, never with the card as read since. A revision arriving meanwhile moves the
// compare-and-set on; a field this person left alone stays out of the body, so a name another writer
// set is not cleared, and text no form can hold as stored (a goal ending in a line feed, a criterion
// holding one) is not rewritten.

/** A one-value field as sent: a changed value, `-` for one cleared; unchanged from `opened`, no key. */
function changedValue<K extends string>(key: K, typed: string, opened: string): Partial<Record<K, string>> {
  const value = typed.trim();
  if (value === opened.trim()) {
    return {};
  }
  return { [key]: value === "" ? CLEAR : value } as Record<K, string>;
}

/** A list as sent: replaced whole, `["-"]` to clear it; unchanged from `opened`, no key. */
function changedList<K extends string>(key: K, typed: string, opened: string): Partial<Record<K, string[]>> {
  const lines = linesOf(typed);
  const before = linesOf(opened);
  if (lines.length === before.length && lines.every((line, index) => line === before[index])) {
    return {};
  }
  return { [key]: lines.length === 0 ? [CLEAR] : lines } as Record<K, string[]>;
}

/** The Edit dialog's changes against the form as it opened; `evidence` the entry to add. */
export function editChanges(form: EditForm, opened: EditForm, evidence: string | null): Omit<CardEditBody, "revision" | "as"> {
  return {
    ...changedValue("title", form.title, opened.title),
    ...changedValue("goal", form.goal, opened.goal),
    ...changedList("target", form.targets, opened.targets),
    ...changedList("criterion", form.criteria, opened.criteria),
    ...changedValue("estimate", form.estimate, opened.estimate),
    ...changedValue("due", form.due, opened.due),
    ...changedValue("parent", form.parent, opened.parent),
    ...changedValue("corrects", form.corrects, opened.corrects),
    ...(evidence === null ? {} : { evidence: [evidence] }),
  };
}

/** Edit: New card's fields less discipline and assignee, changed fields only; Add evidence appends one. */
export function EditCardDialog({ project, card, ...handlers }: { project: string; card: Card } & WriteHandlers) {
  const kindId = useId();
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const [acting] = useActing();
  const [opened] = useState<EditForm>(() => editFormOf(card));
  const [form, setForm] = useState<EditForm>(opened);
  const [evidenceKind, setEvidenceKind] = useState(EVIDENCE_KINDS[0]?.word ?? "");
  const [value, setValue] = useState("");
  const valueHint = EVIDENCE_KINDS.find((entry) => entry.word === evidenceKind)?.hint;

  function field(key: keyof EditForm) {
    return (typed: string) => {
      setForm((current) => ({ ...current, [key]: typed }));
    };
  }

  const changes = editChanges(form, opened, value.trim() === "" ? null : `${evidenceKind}=${value.trim()}`);
  // A change of an estimate the card holds, a clear too, says why and stands alone: the daemon judges and words it.
  const reestimated = card.estimate !== null && changes.estimate !== undefined;
  const [reason, setReason] = useState("");

  return (
    <WriteDialog
      title={`Edit ${card.id}`}
      action="Save"
      sending="Saving"
      revision={card.revision}
      onClose={onClose}
      check={() => {
        if (form.title.trim() === "") {
          return "A card needs a title.";
        }
        return Object.keys(changes).length === 0 ? "Nothing to save: no field changed." : null;
      }}
      onSubmit={() =>
        send({ write: "edit", id: card.id, body: { ...changes, ...(reestimated && reason.trim() !== "" ? { reason: reason.trim() } : {}), revision: card.revision, as: actingName(acting) } })
      }
    >
      <TextField label="Title" value={form.title} onChange={field("title")} required autoFocus invalid={form.title.trim() === ""} />
      <TextArea label="Goal" value={form.goal} onChange={field("goal")} />
      <TextArea label="Source" value={form.targets} onChange={field("targets")} hint="Spec nodes, one per line; the list is replaced." />
      <TextArea label="Criteria" value={form.criteria} onChange={field("criteria")} hint="One per line; the list is replaced." />
      <div className="card-form-row">
        <TextField label="Estimate" value={form.estimate} onChange={field("estimate")} hint="N or N-M, no unit." />
        <TextField label="Due" type="date" value={form.due} onChange={field("due")} />
      </div>
      {reestimated && <TextArea label="Why the estimate changes" value={reason} onChange={setReason} required hint="A changed estimate is saved alone, with this reason." />}
      <div className="card-form-row">
        <TextField label="Parent" value={form.parent} onChange={field("parent")} hint="A card's ID." />
        <TextField label="Corrects" value={form.corrects} onChange={field("corrects")} hint="A card's ID." />
      </div>
      <fieldset className="card-evidence-field">
        <legend>Add evidence</legend>
        <div className="card-form-row">
          <div className="card-field">
            <label className="field-label" htmlFor={kindId}>
              Kind
            </label>
            <select
              id={kindId}
              className="field"
              value={evidenceKind}
              onChange={(event) => {
                setEvidenceKind(event.target.value);
              }}
            >
              {EVIDENCE_KINDS.map((entry) => (
                <option key={entry.word} value={entry.word}>
                  {entry.label}
                </option>
              ))}
            </select>
          </div>
          <TextField label="Value" value={value} onChange={setValue} hint={valueHint} />
        </div>
      </fieldset>
    </WriteDialog>
  );
}

/** Assign: the assignee and the reviewer, changed ones only, `-` for one cleared. */
export function AssignDialog({ project, card, ...handlers }: { project: string; card: Card } & WriteHandlers) {
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const [acting] = useActing();
  const [opened] = useState(() => ({ assignee: card.assignee ?? "", reviewer: card.reviewer ?? "" }));
  const [assignee, setAssignee] = useState(opened.assignee);
  const [reviewer, setReviewer] = useState(opened.reviewer);
  const changes = { ...changedValue("assignee", assignee, opened.assignee), ...changedValue("reviewer", reviewer, opened.reviewer) };

  return (
    <WriteDialog
      title={`Assign ${card.id}`}
      action="Save"
      sending="Saving"
      revision={card.revision}
      onClose={onClose}
      check={() => (Object.keys(changes).length === 0 ? "Nothing to save: no name changed." : null)}
      onSubmit={() => send({ write: "assign", id: card.id, body: { ...changes, revision: card.revision, as: actingName(acting) } })}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      <TextField label="Assignee" value={assignee} onChange={setAssignee} autoFocus hint="Who does the work; empty: nobody." />
      <TextField label="Reviewer" value={reviewer} onChange={setReviewer} hint="Who accepts the result; empty: nobody." />
    </WriteDialog>
  );
}

/** Cancel card: a required reason. */
export function CancelCardDialog({ project, card, ...handlers }: { project: string; card: Card } & WriteHandlers) {
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const [acting] = useActing();
  const [reason, setReason] = useState("");
  const [missing, setMissing] = useState(false);

  return (
    <WriteDialog
      title={`Cancel ${card.id}`}
      action="Cancel card"
      sending="Cancelling"
      dismiss="Keep card"
      danger
      revision={card.revision}
      onClose={onClose}
      check={() => {
        const empty = reason.trim() === "";
        setMissing(empty);
        return empty ? "Say why this card is cancelled." : null;
      }}
      onSubmit={() => send({ write: "cancel", id: card.id, body: { reason: reason.trim(), revision: card.revision, as: actingName(acting) } })}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      <p className="dialog-text dialog-effect">The card leaves its column and takes no further change. Show cancelled still lists it.</p>
      <TextArea label="Reason" value={reason} onChange={setReason} required autoFocus invalid={missing && reason.trim() === ""} />
    </WriteDialog>
  );
}

/**
 * Accept: the verdict, the acting name and a note, disabled while the name or a departure's text is
 * empty; here the page's dialog is the consent the CLI asks on a terminal. The rules are the
 * daemon's and it words its refusal: the card in the flow's accept column, the name its reviewer,
 * never its assignee. "Departs" names direct targets and raises a discrepancy, which holds nothing
 * up; a card with no targets has none to name. The name is the Board's one field, so typed here it
 * stays in its header too; it stands last, so a refusal of it is said right under it.
 */
export function AcceptCardDialog({ project, card, ...handlers }: { project: string; card: Pick<CardRow, "id" | "title" | "revision" | "assignee" | "reviewer"> & Partial<Pick<CardRow, "targets">> } & WriteHandlers) {
  const { onClose } = handlers;
  const targets = card.targets ?? [];
  const send = useSend(project, handlers);
  const verdictId = useId();
  const gapId = useId();
  const [acting, setActing] = useActing();
  const [note, setNote] = useState("");
  const [verdict, setVerdict] = useState<CardVerdict>("briefed");
  const [departs, setDeparts] = useState(targets.join("\n"));
  const [summary, setSummary] = useState("");
  const [observed, setObserved] = useState("");
  const [documented, setDocumented] = useState("");
  const [gapType, setGapType] = useState(GAP_TYPES[0]?.word ?? "");
  const name = actingName(acting);
  const departing = verdict === "departs";
  const departed = linesOf(departs);
  const complete = !departing || (departed.length > 0 && summary.trim() !== "" && observed.trim() !== "" && documented.trim() !== "");

  function bodyOf(as: string): CardAcceptBody {
    const common = { as, note: note.trim() === "" ? null : note.trim(), revision: card.revision };
    return departing ? { ...common, departs: departed, summary: summary.trim(), observed: observed.trim(), documented: documented.trim(), gap_type: gapType } : { ...common, briefed: true };
  }

  return (
    <WriteDialog
      title={`Accept ${card.id}`}
      action="Accept card"
      sending="Accepting"
      revision={card.revision}
      canSubmit={name !== null && complete}
      onClose={onClose}
      onSubmit={() => (name === null ? Promise.resolve("A name is needed.") : send({ write: "accept", id: card.id, body: bodyOf(name) }))}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      <dl className="facts">
        <div className="fact">
          <dt>Reviewer</dt>
          <dd>{card.reviewer === null ? "-" : <bdi>{shownSlot(card.reviewer)}</bdi>}</dd>
        </div>
        <div className="fact">
          <dt>Assignee</dt>
          <dd>{card.assignee === null ? "-" : <bdi>{shownSlot(card.assignee)}</bdi>}</dd>
        </div>
      </dl>
      <p className="dialog-text dialog-effect">{NAME_IS_TYPED}</p>
      <fieldset className="card-evidence-field">
        <legend>Against the brief</legend>
        <div className="card-field">
          <label htmlFor={`${verdictId}-briefed`}>
            <input
              id={`${verdictId}-briefed`}
              type="radio"
              name={verdictId}
              checked={!departing}
              onChange={() => {
                setVerdict("briefed");
              }}
            />{" "}
            As briefed
          </label>
          <label htmlFor={`${verdictId}-departs`}>
            <input
              id={`${verdictId}-departs`}
              type="radio"
              name={verdictId}
              checked={departing}
              disabled={targets.length === 0}
              onChange={() => {
                setVerdict("departs");
              }}
            />{" "}
            Departs from it
          </label>
        </div>
      </fieldset>
      {departing && (
        <>
          <TextArea label="Departs from" value={departs} onChange={setDeparts} required invalid={departed.length === 0} hint="Direct targets of this card, one per line." />
          <TextField label="Summary" value={summary} onChange={setSummary} required invalid={summary.trim() === ""} />
          <TextArea label="What the result does" value={observed} onChange={setObserved} required invalid={observed.trim() === ""} />
          <TextArea label="What the brief says" value={documented} onChange={setDocumented} required invalid={documented.trim() === ""} />
          <div className="card-field">
            <label className="field-label" htmlFor={gapId}>
              The gap
            </label>
            <select
              id={gapId}
              className="field"
              value={gapType}
              onChange={(event) => {
                setGapType(event.target.value);
              }}
            >
              {GAP_TYPES.map((entry) => (
                <option key={entry.word} value={entry.word}>
                  {entry.label}
                </option>
              ))}
            </select>
          </div>
          <p className="dialog-text dialog-effect">This raises a discrepancy for the owner; it stops nothing and the card is accepted.</p>
        </>
      )}
      <TextArea label="Note" value={note} onChange={setNote} autoFocus={name !== null} />
      <TextField label="Accepting as" value={acting} onChange={setActing} required autoFocus={name === null} hint="Its reviewer's name, never its assignee's." />
    </WriteDialog>
  );
}

/**
 * Clear: a person answers a suspect card (`docs/features/card-trace.md` "Clearing"), the page's dialog
 * being the consent the CLI asks on a terminal. "No effect" takes a reason and vouches for the card
 * against the brief as it stands; "Needs correction", for an accepted card alone, also takes an
 * estimate and opens a new card, the original staying accepted. The rules are the daemon's. The name
 * is the Board's one field and stands last, as in Accept, so a refusal of it is said right under it.
 */
export function ClearCardDialog({ project, card, on, ...handlers }: { project: string; card: Card; on: string | null } & WriteHandlers) {
  const outcomeId = useId();
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const [acting, setActing] = useActing();
  const [outcome, setOutcome] = useState<CardClearBody["outcome"]>("no_effect");
  const [reason, setReason] = useState("");
  const [estimate, setEstimate] = useState("");
  const [title, setTitle] = useState("");
  const [checked, setChecked] = useState(false);
  const name = actingName(acting);
  const correcting = outcome === "needs_correction";
  const accepted = card.acceptance !== null;

  function bodyOf(as: string): CardClearBody {
    const common = { outcome, reason: reason.trim(), revision: card.revision, as };
    return correcting ? { ...common, estimate: estimate.trim(), ...(title.trim() === "" ? {} : { title: title.trim() }) } : common;
  }

  return (
    <WriteDialog
      title={`Clear ${card.id}`}
      action="Clear card"
      sending="Clearing"
      revision={card.revision}
      canSubmit={name !== null}
      onClose={onClose}
      check={() => {
        setChecked(true);
        if (reason.trim() === "") {
          return "Say why the mark clears.";
        }
        return correcting && estimate.trim() === "" ? "A correction needs an estimate." : null;
      }}
      onSubmit={() => (name === null ? Promise.resolve("A name is needed.") : send({ write: "clear", id: card.id, body: bodyOf(name) }))}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      {on !== null && (
        <p className="dialog-text dialog-effect">
          The brief changed at <bdi className="mono">{shownSlot(on)}</bdi>.
        </p>
      )}
      <p className="dialog-text dialog-effect">{NAME_IS_TYPED}</p>
      <fieldset className="card-evidence-field">
        <legend>What the change means for this card</legend>
        <div className="card-field">
          <label htmlFor={`${outcomeId}-none`}>
            <input
              id={`${outcomeId}-none`}
              type="radio"
              name={outcomeId}
              checked={!correcting}
              onChange={() => {
                setOutcome("no_effect");
              }}
            />{" "}
            No effect
          </label>
          <label htmlFor={`${outcomeId}-correct`}>
            <input
              id={`${outcomeId}-correct`}
              type="radio"
              name={outcomeId}
              checked={correcting}
              disabled={!accepted}
              onChange={() => {
                setOutcome("needs_correction");
              }}
            />{" "}
            Needs correction
          </label>
          {!accepted && <p className="card-field-hint">A correction answers an accepted card.</p>}
        </div>
      </fieldset>
      <TextArea label="Reason" value={reason} onChange={setReason} required autoFocus={name !== null} invalid={checked && reason.trim() === ""} />
      {correcting && (
        <>
          <TextField label="Estimate" value={estimate} onChange={setEstimate} required invalid={checked && estimate.trim() === ""} hint="N or N-M, no unit." />
          <TextField label="Title of the correction" value={title} onChange={setTitle} hint={`Empty: Correct ${shownSlot(card.title)}`} />
          <p className="dialog-text dialog-effect">This opens a new card. This one stays accepted, and its mark stands until the new card is accepted.</p>
        </>
      )}
      <TextField label="Clearing as" value={acting} onChange={setActing} required autoFocus={name === null} hint="Your name: a person answers a mark." />
    </WriteDialog>
  );
}

/**
 * Clear all (docs/features/trace-views.md "A node's cards"): every suspect card of a group, no effect
 * only, one POST, the dialog the consent. A refusal keeps it open, in the daemon's words.
 */
export function ClearGroupDialog({ project, group, cards, onClose, onDone }: { project: string; group: string; cards: number; onClose: () => void; onDone: (answer: CardClearGroupWrite) => void }) {
  const clear = useCardClearGroup(project);
  const [acting, setActing] = useActing();
  const [reason, setReason] = useState("");
  const [checked, setChecked] = useState(false);
  const name = actingName(acting);

  async function send(as: string): Promise<string | null> {
    try {
      onDone(await clear.mutateAsync({ group, reason: reason.trim(), as }));
      return null;
    } catch (error) {
      return apiErrorOf(error).message;
    }
  }

  return (
    <WriteDialog
      title={`Clear all under ${group}`}
      action="Clear all"
      sending="Clearing"
      canSubmit={name !== null}
      onClose={onClose}
      check={() => {
        setChecked(true);
        return reason.trim() === "" ? "Say why the marks clear." : null;
      }}
      onSubmit={() => (name === null ? Promise.resolve("A name is needed.") : send(name))}
    >
      <p className="dialog-text">
        {cards === 1 ? "1 suspect card" : `${String(cards)} suspect cards`} of <bdi className="mono">{shownSlot(group)}</bdi> and what it holds clear as no effect. A card a correction holds is passed over.
      </p>
      <p className="dialog-text dialog-effect">{NAME_IS_TYPED}</p>
      <TextArea label="Reason" value={reason} onChange={setReason} required autoFocus={name !== null} invalid={checked && reason.trim() === ""} />
      <TextField label="Clearing as" value={acting} onChange={setActing} required autoFocus={name === null} hint="Your name: a person answers a mark." />
    </WriteDialog>
  );
}

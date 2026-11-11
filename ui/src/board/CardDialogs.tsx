import { createContext, use, useEffect, useId, useMemo, useRef, useState, type ReactNode, type SubmitEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useCard, useCardClearGroup, useCardRaise, useCards, useCardWrite, useHeldWorkspaces, type CardWriteRequest } from "../api/queries";
import type {
  Card,
  CardAcceptBody,
  CardClearBody,
  CardClearGroupWrite,
  CardCreateBody,
  CardEditBody,
  CardFlow,
  CardMoveBody,
  CardRaiseBody,
  CardRow,
  CardVerdict,
  CardWrite,
  IntakeDocument,
  IntakeMatch,
  Workspace,
  WorkspaceDocument,
} from "../api/types";
import { Dialog } from "../ui/Dialog";
import { Disclosure } from "../ui/Disclosure";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";
import { actingName, handOffReviewer, isActingLine, useActing, useHubLogin, useTeamRow } from "./acting";
import { assigneeChoices, disciplineChoices } from "./filter";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { isCard } from "./CardPanel";
import { evidenceKinds, gapTypes, nameIsLogin, nameIsTyped, RAISED_KINDS } from "./labels";

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
export function TextField({
  label,
  value,
  onChange,
  hint,
  type = "text",
  required = false,
  autoFocus = false,
  list,
  invalid = false,
  readOnly = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: ReactNode;
  type?: "text" | "date";
  required?: boolean;
  autoFocus?: boolean;
  list?: string;
  invalid?: boolean;
  readOnly?: boolean;
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
        className={readOnly ? "field field-fixed" : "field"}
        type={type}
        value={value}
        readOnly={pending || readOnly}
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

/** A text field of several lines; `readOnly` holds it still past a write that already took it (Return's reason once moved). */
export function TextArea({
  label,
  value,
  onChange,
  hint,
  required = false,
  autoFocus = false,
  invalid = false,
  readOnly = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: string;
  required?: boolean;
  autoFocus?: boolean;
  invalid?: boolean;
  readOnly?: boolean;
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
        readOnly={pending || readOnly}
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

/** A choice among served values: `placeholder`, when given, an empty first option that is no choice. */
function SelectField({
  label,
  value,
  onChange,
  options,
  placeholder,
  hint,
  required = false,
  autoFocus = false,
  invalid = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** Each option's value, sent or looked up, and its label, the project's words in any script. */
  options: readonly { value: string; label: string }[];
  placeholder?: string;
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
      <select
        id={id}
        className="field"
        value={value}
        aria-required={required}
        aria-invalid={invalid}
        aria-disabled={pending || undefined}
        aria-describedby={hint === undefined ? undefined : hintId}
        data-autofocus={autoFocus ? "" : undefined}
        onChange={(event) => {
          if (!pending) {
            onChange(event.target.value);
          }
        }}
      >
        {placeholder !== undefined && <option value="">{placeholder}</option>}
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      {hint !== undefined && (
        <p id={hintId} className="card-field-hint">
          {hint}
        </p>
      )}
    </div>
  );
}

/**
 * Who a person-answered act goes out as: the typed field locally, its `hint` what to type. On a hub
 * the login stands read-only in the same field, `[team]`'s `name` beside it when read, shown only and
 * never sent (`docs/features/hub-team.md` "Description and interactions"). It takes no `required`
 * there: nothing can be missing from it.
 */
function ActingField({
  project,
  label,
  value,
  onChange,
  autoFocus,
  hint,
}: {
  project: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoFocus: boolean;
  hint: string;
}) {
  const login = useHubLogin();
  const team = useTeamRow(project);
  if (login === null) {
    return <TextField label={label} value={value} onChange={onChange} required autoFocus={autoFocus} hint={hint} />;
  }
  const name = team?.name ?? null;
  return <TextField label={label} value={login} onChange={onChange} readOnly hint={name === null ? undefined : <bdi>{shownSlot(name)}</bdi>} />;
}

/** What a dialog says of the name it goes out under: typed locally, the login on a hub. */
export function ActingNote() {
  const t = useTranslate();
  const login = useHubLogin();
  return <p className="dialog-text dialog-effect">{login === null ? nameIsTyped(t) : nameIsLogin(t)}</p>;
}

/**
 * The field a dialog's focus goes back to: the first marked invalid, else the first autofocus field
 * that takes text, else the first that does; never one held read-only, nor one folded away.
 */
function firstFieldOf(root: HTMLFormElement | null): HTMLElement | null {
  if (root === null) {
    return null;
  }
  const takes = (field: HTMLElement) => field.closest("[hidden]") === null && !((field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) && field.readOnly);
  const fields = Array.from(root.querySelectorAll<HTMLElement>("input, textarea, select")).filter(takes);
  return fields.find((field) => field.getAttribute("aria-invalid") === "true") ?? Array.from(root.querySelectorAll<HTMLElement>("[data-autofocus]")).find(takes) ?? fields[0] ?? null;
}

/**
 * A card write as a dialog (`ui/README.md` "Screen rules", Dialogs): a submit is one call; while it
 * is out the dialog stays open, a second submit, Cancel and Esc doing nothing and the buttons keeping
 * focus. `check` stops a submit that cannot be right, sending nothing. A refusal keeps the dialog and
 * the typed text, says the daemon's words in an alert and puts focus back in the first field. A card
 * revised while it is open (`revision`, a refusal's read among them) says so; the next submit goes
 * on the new revision, its fields kept. A submit answering `false` keeps the dialog with no refusal:
 * what it says stands among the children (a raise not stored, its hits); `action` null offers no
 * submit at all, only the dismiss, which then takes the focus the submit had. Focus after a stop or a
 * refusal goes to the first field marked invalid, else the first autofocus field that takes text,
 * else the first that does: never a field held read-only, nor one folded away. `primary` false
 * draws the submit as a plain button, where another act of the dialog is its one primary.
 */
export function WriteDialog({
  title,
  action,
  sending,
  dismiss,
  danger = false,
  primary = true,
  revision = null,
  canSubmit = true,
  check,
  onClose,
  onSubmit,
  children,
}: {
  title: ReactNode;
  action: string | null;
  sending: string;
  dismiss?: string;
  danger?: boolean;
  primary?: boolean;
  revision?: string | null;
  canSubmit?: boolean;
  /** Why this submit cannot be sent, else null. */
  check?: () => string | null;
  onClose: () => void;
  /** Null: done, the caller closes; a string: the daemon's refusal; false: kept open, nothing refused. */
  onSubmit: () => Promise<string | null | false>;
  children: ReactNode;
}) {
  const t = useTranslate();
  const form = useRef<HTMLFormElement>(null);
  const inFlight = useRef(false);
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [shown, setShown] = useState(revision);
  const [changed, setChanged] = useState(false);
  // Each stop or refusal asks for the first field once the form shows it: its invalid marks set, the held fields read-only again.
  const [refocus, setRefocus] = useState(0);
  const dismissButton = useRef<HTMLButtonElement>(null);

  if (!pending && revision !== shown) {
    // Storing information from previous renders (react.dev, useState). A submit out went on the
    // revision it read; its answer closes the dialog, or comes back and this runs then.
    setShown(revision);
    setChanged(true);
  }

  // A changed card can take away what had focus: the first field takes it, never the body.
  useEffect(() => {
    if (changed && focusIsLost()) {
      firstFieldOf(form.current)?.focus();
    }
  }, [changed]);

  useEffect(() => {
    if (refocus > 0) {
      firstFieldOf(form.current)?.focus();
    }
  }, [refocus]);

  // No submit left to press (every hit named): focus would fall to the body; the dismiss takes it.
  useEffect(() => {
    if (action === null && focusIsLost()) {
      dismissButton.current?.focus();
    }
  }, [action]);


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
      setRefocus((count) => count + 1);
      return;
    }
    inFlight.current = true;
    setPending(true);
    setRefusal(null);
    const outcome = await onSubmit();
    if (outcome === false) {
      inFlight.current = false;
      setPending(false);
    } else if (outcome !== null) {
      inFlight.current = false;
      setPending(false);
      setRefusal(outcome);
      setRefocus((count) => count + 1);
    }
  }

  return (
    <Dialog title={title} onClose={cancel} className="decision-dialog card-dialog">
      {changed && (
        // A new alert for each revision, so a second one is spoken too.
        <div key={shown} className="notice" role="alert">
          <p className="notice-title">
            <Icon name="alert" />
            <span>{t("board.dialog.changed")}</span>
          </p>
          <p>{t("board.dialog.changedText")}</p>
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
              <span>{t("board.view.refused")}</span>
            </p>
            <p className="verbatim" lang="en">
              {refusal}
            </p>
          </div>
        )}
        <div className="dialog-actions">
          {action !== null && (
            <button type="submit" className={danger ? "button button-danger" : primary ? "button button-primary" : "button"} aria-disabled={pending} disabled={!canSubmit}>
              {pending ? sending : action}
            </button>
          )}
          <button ref={dismissButton} type="button" className="button" aria-disabled={pending} onClick={cancel}>
            {dismiss ?? t("board.dialog.cancel")}
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

/** What a New card opens with besides the node preset: a gap handed to the AI (docs/features/writer-studio.md D11). */
export interface NewCardPreset {
  title: string;
  goal: string;
  targets: readonly string[];
}

/**
 * New card: every field `create_card` takes, targets and criteria one per line. `target`: the node a
 * New card button stands on, already filled: how a board fills from the brief; `preset`: a gap's
 * title, goal and targets. Fields left empty are no key: the daemon picks the flow by discipline, the
 * first column.
 */
export function NewCardDialog({ project, target, preset, ...handlers }: { project: string; target?: string; preset?: NewCardPreset } & WriteHandlers) {
  const t = useTranslate();
  const disciplinesId = useId();
  const namesId = useId();
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const known = useCards(project).data;
  const [acting] = useActing();
  const [form, setForm] = useState<NewCardForm>({
    title: preset?.title ?? "",
    goal: preset?.goal ?? "",
    discipline: "",
    targets: preset === undefined ? (target ?? "") : preset.targets.join("\n"),
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
  const names = useMemo(() => assigneeChoices(known?.cards ?? [], "", t).slice(0, -1), [known, t]);

  return (
    <WriteDialog
      title={t("board.dialog.newTitle")}
      action={t("board.dialog.create")}
      sending={t("board.dialog.creating")}
      onClose={onClose}
      check={() => {
        const missing = form.title.trim() === "";
        setTitleMissing(missing);
        return missing ? t("board.dialog.needsTitle") : null;
      }}
      onSubmit={() => send({ write: "create", body: createBodyOf(form, actingName(acting)) })}
    >
      <TextField label={t("board.field.title")} value={form.title} onChange={field("title")} required autoFocus invalid={titleMissing && form.title.trim() === ""} />
      <TextArea label={t("board.field.goal")} value={form.goal} onChange={field("goal")} />
      <div className="card-form-row">
        <TextField label={t("board.field.discipline")} value={form.discipline} onChange={field("discipline")} list={disciplinesId} hint={t("board.hint.discipline")} />
        <TextField label={t("board.field.assignee")} value={form.assignee} onChange={field("assignee")} list={namesId} />
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
      <TextArea label={t("board.field.source")} value={form.targets} onChange={field("targets")} hint={t("board.hint.sourceNew")} />
      <TextArea label={t("board.field.criteria")} value={form.criteria} onChange={field("criteria")} hint={t("board.hint.perLine")} />
      <div className="card-form-row">
        <TextField label={t("board.field.estimate")} value={form.estimate} onChange={field("estimate")} hint={t("board.hint.estimate")} />
        <TextField label={t("board.field.due")} type="date" value={form.due} onChange={field("due")} />
      </div>
      <div className="card-form-row">
        <TextField label={t("board.field.parent")} value={form.parent} onChange={field("parent")} hint={t("board.hint.cardId")} />
        <TextField label={t("board.field.corrects")} value={form.corrects} onChange={field("corrects")} hint={t("board.hint.cardId")} />
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
  const t = useTranslate();
  const kinds = evidenceKinds(t);
  const kindId = useId();
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const [acting] = useActing();
  const [opened] = useState<EditForm>(() => editFormOf(card));
  const [form, setForm] = useState<EditForm>(opened);
  const [evidenceKind, setEvidenceKind] = useState(kinds[0]?.word ?? "");
  const [value, setValue] = useState("");
  const valueHint = kinds.find((entry) => entry.word === evidenceKind)?.hint;

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
      title={t("board.dialog.editTitle", { id: card.id })}
      action={t("board.dialog.save")}
      sending={t("board.dialog.saving")}
      revision={card.revision}
      onClose={onClose}
      check={() => {
        if (form.title.trim() === "") {
          return t("board.dialog.needsTitle");
        }
        return Object.keys(changes).length === 0 ? t("board.dialog.noFieldChanged") : null;
      }}
      onSubmit={() =>
        send({ write: "edit", id: card.id, body: { ...changes, ...(reestimated && reason.trim() !== "" ? { reason: reason.trim() } : {}), revision: card.revision, as: actingName(acting) } })
      }
    >
      <TextField label={t("board.field.title")} value={form.title} onChange={field("title")} required autoFocus invalid={form.title.trim() === ""} />
      <TextArea label={t("board.field.goal")} value={form.goal} onChange={field("goal")} />
      <TextArea label={t("board.field.source")} value={form.targets} onChange={field("targets")} hint={t("board.hint.sourceEdit")} />
      <TextArea label={t("board.field.criteria")} value={form.criteria} onChange={field("criteria")} hint={t("board.hint.perLineReplaced")} />
      <div className="card-form-row">
        <TextField label={t("board.field.estimate")} value={form.estimate} onChange={field("estimate")} hint={t("board.hint.estimate")} />
        <TextField label={t("board.field.due")} type="date" value={form.due} onChange={field("due")} />
      </div>
      {reestimated && <TextArea label={t("board.field.whyEstimate")} value={reason} onChange={setReason} required hint={t("board.hint.whyEstimate")} />}
      <div className="card-form-row">
        <TextField label={t("board.field.parent")} value={form.parent} onChange={field("parent")} hint={t("board.hint.cardId")} />
        <TextField label={t("board.field.corrects")} value={form.corrects} onChange={field("corrects")} hint={t("board.hint.cardId")} />
      </div>
      <fieldset className="card-evidence-field">
        <legend>{t("board.dialog.addEvidence")}</legend>
        <div className="card-form-row">
          <div className="card-field">
            <label className="field-label" htmlFor={kindId}>
              {t("board.dialog.kind")}
            </label>
            <select
              id={kindId}
              className="field"
              value={evidenceKind}
              onChange={(event) => {
                setEvidenceKind(event.target.value);
              }}
            >
              {kinds.map((entry) => (
                <option key={entry.word} value={entry.word}>
                  {entry.label}
                </option>
              ))}
            </select>
          </div>
          <TextField label={t("board.field.value")} value={value} onChange={setValue} hint={valueHint} />
        </div>
      </fieldset>
    </WriteDialog>
  );
}

/** Assign: the assignee and the reviewer, changed ones only, `-` for one cleared. */
export function AssignDialog({ project, card, ...handlers }: { project: string; card: Card } & WriteHandlers) {
  const t = useTranslate();
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const [acting] = useActing();
  const [opened] = useState(() => ({ assignee: card.assignee ?? "", reviewer: card.reviewer ?? "" }));
  const [assignee, setAssignee] = useState(opened.assignee);
  const [reviewer, setReviewer] = useState(opened.reviewer);
  const changes = { ...changedValue("assignee", assignee, opened.assignee), ...changedValue("reviewer", reviewer, opened.reviewer) };

  return (
    <WriteDialog
      title={t("board.dialog.assignTitle", { id: card.id })}
      action={t("board.dialog.save")}
      sending={t("board.dialog.saving")}
      revision={card.revision}
      onClose={onClose}
      check={() => (Object.keys(changes).length === 0 ? t("board.dialog.noNameChanged") : null)}
      onSubmit={() => send({ write: "assign", id: card.id, body: { ...changes, revision: card.revision, as: actingName(acting) } })}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      <TextField label={t("board.field.assignee")} value={assignee} onChange={setAssignee} autoFocus hint={t("board.hint.assignee")} />
      <TextField label={t("board.field.reviewer")} value={reviewer} onChange={setReviewer} hint={t("board.hint.reviewer")} />
    </WriteDialog>
  );
}

/** Cancel card: a required reason. */
export function CancelCardDialog({ project, card, ...handlers }: { project: string; card: Card } & WriteHandlers) {
  const t = useTranslate();
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const [acting] = useActing();
  const [reason, setReason] = useState("");
  const [missing, setMissing] = useState(false);

  return (
    <WriteDialog
      title={t("board.dialog.cancelTitle", { id: card.id })}
      action={t("board.dialog.cancelCard")}
      sending={t("board.dialog.cancelling")}
      dismiss={t("board.dialog.keepCard")}
      danger
      revision={card.revision}
      onClose={onClose}
      check={() => {
        const empty = reason.trim() === "";
        setMissing(empty);
        return empty ? t("board.dialog.cancelWhy") : null;
      }}
      onSubmit={() => send({ write: "cancel", id: card.id, body: { reason: reason.trim(), revision: card.revision, as: actingName(acting) } })}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      <p className="dialog-text dialog-effect">{t("board.dialog.cancelEffect")}</p>
      <TextArea label={t("board.field.reason")} value={reason} onChange={setReason} required autoFocus invalid={missing && reason.trim() === ""} />
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
  const t = useTranslate();
  const gaps = gapTypes(t);
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
  const [gapType, setGapType] = useState(gaps[0]?.word ?? "");
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
      title={t("board.dialog.acceptTitle", { id: card.id })}
      action={t("board.dialog.acceptCard")}
      sending={t("board.dialog.accepting")}
      revision={card.revision}
      canSubmit={name !== null && complete}
      onClose={onClose}
      onSubmit={() => (name === null ? Promise.resolve(t("board.dialog.nameNeeded")) : send({ write: "accept", id: card.id, body: bodyOf(name) }))}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      <dl className="facts">
        <div className="fact">
          <dt>{t("board.field.reviewer")}</dt>
          <dd>{card.reviewer === null ? "-" : <bdi>{shownSlot(card.reviewer)}</bdi>}</dd>
        </div>
        <div className="fact">
          <dt>{t("board.field.assignee")}</dt>
          <dd>{card.assignee === null ? "-" : <bdi>{shownSlot(card.assignee)}</bdi>}</dd>
        </div>
      </dl>
      <ActingNote />
      <fieldset className="card-evidence-field">
        <legend>{t("board.panel.brief")}</legend>
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
            {t("board.dialog.asBriefed")}
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
            {t("board.dialog.departsFromIt")}
          </label>
        </div>
      </fieldset>
      {departing && (
        <>
          <TextArea label={t("board.field.departsFrom")} value={departs} onChange={setDeparts} required invalid={departed.length === 0} hint={t("board.hint.departsFrom")} />
          <TextField label={t("board.field.summary")} value={summary} onChange={setSummary} required invalid={summary.trim() === ""} />
          <TextArea label={t("board.field.observed")} value={observed} onChange={setObserved} required invalid={observed.trim() === ""} />
          <TextArea label={t("board.field.documented")} value={documented} onChange={setDocumented} required invalid={documented.trim() === ""} />
          <div className="card-field">
            <label className="field-label" htmlFor={gapId}>
              {t("board.dialog.gap")}
            </label>
            <select
              id={gapId}
              className="field"
              value={gapType}
              onChange={(event) => {
                setGapType(event.target.value);
              }}
            >
              {gaps.map((entry) => (
                <option key={entry.word} value={entry.word}>
                  {entry.label}
                </option>
              ))}
            </select>
          </div>
          <p className="dialog-text dialog-effect">{t("board.dialog.raisesDiscrepancy")}</p>
        </>
      )}
      <TextArea label={t("board.field.note")} value={note} onChange={setNote} autoFocus={name !== null} />
      <ActingField project={project} label={t("board.field.acceptingAs")} value={acting} onChange={setActing} autoFocus={name === null} hint={t("board.hint.acceptingAs")} />
    </WriteDialog>
  );
}

/** What a hand-off is made from: the card as read, or a tile's row, whose goal is read before the form opens. */
export type HandOffSource = Pick<Card, "id" | "title" | "targets" | "reviewer"> & { goal?: string | null };

/** The hand-off dialog's fields as typed. */
interface HandOffForm {
  /** The chosen department's name, `""` before a choice; unused with today's Discipline field. */
  department: string;
  discipline: string;
  title: string;
  goal: string;
  targets: string;
  reviewer: string;
}

/**
 * The departments a hand-off may go to: those whose served `disciplines` is not empty, in served
 * order; null, today's typed Discipline field, with fewer than two or with the workspaces not read
 * (docs/features/departments.md "Acceptance criteria", DP-14). The daemon computed each list.
 */
export function handOffDepartments(document: WorkspaceDocument | undefined): Workspace[] | null {
  const offered = (document?.workspaces ?? []).filter((workspace) => workspace.disciplines.length > 0);
  return offered.length < 2 ? null : offered;
}

/**
 * Hand off (`docs/canon/card-flows.md` "The rule"): a child card in another trade's flow, which its
 * discipline picks as for any new card. With two or more departments serving a discipline, a
 * required Department select, then a Discipline select defaulting to its first when it serves
 * several; the form is chosen once, as the dialog opens, from the workspaces read the Shell holds,
 * and the write still sends only `discipline`. Otherwise the dialog offers every served flow's
 * disciplines, whatever the workspace shows, from the flows the caller's list read serves. Title,
 * goal and targets come from the card, all editable; criteria, estimate, due date and evidence stay
 * with it, and no assignee is named: the receiving trade takes the card with a move. The reviewer is
 * the asking side, required: the parent's reviewer by default, else the acting name when it names a
 * person; on a hub the login, read-only. Submit is one `POST .../cards`, nothing else: no assign follows.
 */
export function HandOffDialog({ project, card, flows, ...handlers }: { project: string; card: HandOffSource; flows: readonly CardFlow[] } & WriteHandlers) {
  const t = useTranslate();
  const disciplinesId = useId();
  const { onClose } = handlers;
  const send = useSend(project, handlers);
  const login = useHubLogin();
  const [acting] = useActing();
  const held = useHeldWorkspaces(project);
  // Chosen once: a read landing while the dialog is open changes no field under the person's hands.
  const [departments] = useState(() => handOffDepartments(held.data));
  // A tile's row holds no goal: the card is read, and its goal fills the field once, unless typed in first.
  const read = useCard(project, card.goal === undefined ? card.id : null);
  const readGoal = read.data !== undefined && isCard(read.data) ? read.data.goal : undefined;
  // The reviewer the daemon takes: the parent's, else a person's acting name, never `agent` (docs/canon/card-flows.md "Hand-off").
  const [form, setForm] = useState<HandOffForm>({ department: "", discipline: "", title: card.title, goal: card.goal ?? "", targets: card.targets.join("\n"), reviewer: handOffReviewer(card.reviewer, acting) });
  const [goalFilled, setGoalFilled] = useState(card.goal !== undefined);
  const [goalTyped, setGoalTyped] = useState(false);
  const [checked, setChecked] = useState(false);
  if (!goalFilled && readGoal !== undefined) {
    setGoalFilled(true);
    if (!goalTyped) {
      setForm((current) => ({ ...current, goal: readGoal ?? "" }));
    }
  }
  const reviewer = login ?? form.reviewer;

  function field(key: keyof HandOffForm) {
    return (value: string) => {
      if (key === "goal") {
        setGoalTyped(true);
      }
      setForm((current) => ({ ...current, [key]: value }));
    };
  }

  const disciplines = useMemo(() => [...new Set(flows.flatMap((flow) => flow.disciplines))], [flows]);
  const department = departments?.find((candidate) => candidate.name === form.department) ?? null;

  function chooseDepartment(name: string) {
    const chosen = departments?.find((candidate) => candidate.name === name);
    // Its first discipline until another is chosen: the second select never starts empty.
    setForm((current) => ({ ...current, department: name, discipline: chosen?.disciplines[0] ?? "" }));
  }

  function bodyOf(): CardCreateBody {
    return {
      title: form.title.trim(),
      ...present("goal", form.goal),
      discipline: form.discipline.trim(),
      ...presentList("target", form.targets),
      parent: card.id,
      reviewer: reviewer.trim(),
      as: actingName(acting),
    };
  }

  return (
    <WriteDialog
      title={t("board.handoff.title", { id: card.id })}
      action={t("board.handoff.submit")}
      sending={t("board.handoff.sending")}
      onClose={onClose}
      check={() => {
        setChecked(true);
        if (departments !== null && department === null) {
          return t("board.handoff.needsDepartment");
        }
        if (form.discipline.trim() === "") {
          return t("board.handoff.needsDiscipline");
        }
        if (form.title.trim() === "") {
          return t("board.dialog.needsTitle");
        }
        return reviewer.trim() === "" ? t("board.handoff.reviewerNeeded") : null;
      }}
      onSubmit={() => send({ write: "create", body: bodyOf() })}
    >
      <p className="dialog-text dialog-effect">{t("board.handoff.effect")}</p>
      <dl className="facts">
        <div className="fact">
          <dt>{t("board.field.parent")}</dt>
          <dd className="mono">{card.id}</dd>
        </div>
      </dl>
      {departments === null ? (
        <>
          <TextField
            label={t("board.field.discipline")}
            value={form.discipline}
            onChange={field("discipline")}
            list={disciplinesId}
            required
            autoFocus
            invalid={checked && form.discipline.trim() === ""}
            hint={t("board.handoff.disciplineHint")}
          />
          <datalist id={disciplinesId}>
            {disciplines.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
        </>
      ) : (
        <>
          <SelectField
            label={t("board.field.department")}
            value={form.department}
            onChange={chooseDepartment}
            options={departments.map((offered) => ({ value: offered.name, label: offered.title }))}
            placeholder={t("board.handoff.chooseDepartment")}
            required
            autoFocus
            invalid={checked && department === null}
            hint={t("board.handoff.departmentHint")}
          />
          {department !== null && department.disciplines.length > 1 && (
            <SelectField
              label={t("board.field.discipline")}
              value={form.discipline}
              onChange={field("discipline")}
              options={department.disciplines.map((name) => ({ value: name, label: name }))}
              required
              hint={t("board.handoff.disciplineHint")}
            />
          )}
        </>
      )}
      <TextField label={t("board.field.title")} value={form.title} onChange={field("title")} required invalid={checked && form.title.trim() === ""} />
      <TextArea label={t("board.field.goal")} value={form.goal} onChange={field("goal")} />
      <TextArea label={t("board.field.source")} value={form.targets} onChange={field("targets")} hint={t("board.hint.sourceNew")} />
      {login === null ? (
        <TextField label={t("board.field.reviewer")} value={form.reviewer} onChange={field("reviewer")} required invalid={checked && reviewer.trim() === ""} hint={t("board.handoff.reviewerHint")} />
      ) : (
        <TextField label={t("board.field.reviewer")} value={login} onChange={field("reviewer")} readOnly hint={t("board.handoff.reviewerHint")} />
      )}
    </WriteDialog>
  );
}

// A question or a decision raised on a card (docs/canon/card-questions.md "The rule";
// docs/features/card-questions.md "Acceptance criteria", U2, U3): the page sends what was typed and the
// targets chosen, and shows what the daemon answered. A hit is no refusal: nothing is stored, the dialog
// lists the hits, and Ask anyway sends the raise again naming each listed one; with every listed hit
// named and still nothing stored, it says so and offers no Ask anyway. No answer, or a refusal, offers
// Ask again, which resends the raise alone. Nothing here judges a hit, a cap or a state.

/** A question's or a decision's texts as typed, `why` a decision's, and the card's targets it is about. */
export interface RaiseForm {
  text: string;
  working: string;
  price: string;
  why: string;
  chosen: readonly string[];
}

/** A new form over the card's targets, each chosen. */
export function raiseFormOf(targets: readonly string[]): RaiseForm {
  return { text: "", working: "", price: "", why: "", chosen: [...targets] };
}

/** What the last raise left: nothing yet; not stored, with what it named; failed or unanswered, with what it named. */
export type RaiseOutcome = { state: "none" } | { state: "kept"; answer: IntakeDocument; sent: readonly string[] } | { state: "failed"; sent: readonly string[] };

export const NOTHING_RAISED: RaiseOutcome = { state: "none" };

/** How a hit is named in `distinct_from`: its ID, else its path (`docs/canon/agent-intake.md` "Dedup"). */
function hitName(hit: IntakeMatch): string {
  return hit.id ?? hit.path ?? "";
}

/** Whether every hit listed was named already: Ask anyway would send nothing new (U2 of docs/features/card-questions.md "Acceptance criteria"). */
export function allNamed(outcome: RaiseOutcome): boolean {
  return outcome.state === "kept" && outcome.answer.hits.every((hit) => outcome.sent.includes(hitName(hit)));
}

/** What the next raise names: none at first; after hits, those named before and each listed; after a failure, what it named. */
function nextNamed(outcome: RaiseOutcome): string[] {
  if (outcome.state === "none") {
    return [];
  }
  if (outcome.state === "failed") {
    return [...outcome.sent];
  }
  return [...new Set([...outcome.sent, ...outcome.answer.hits.map(hitName)])];
}

/** The submit's words after an outcome: the first ones, Ask anyway past hits, Ask again past a failure, none when every hit is named. */
export function raiseAction(outcome: RaiseOutcome, first: string, anyway: string, again: string): string | null {
  if (outcome.state === "none") {
    return first;
  }
  if (outcome.state === "failed") {
    return again;
  }
  return allNamed(outcome) ? null : anyway;
}

/** A raise's body less what each act adds: the targets and the three texts. */
type RaiseTexts = Omit<CardRaiseBody, "kind" | "why" | "severity" | "distinct_from" | "as">;

/** What every raise sends of a form, trimmed: the targets chosen, in the card's order, and the three texts. */
export function raiseTextsOf(form: RaiseForm, targets: readonly string[]): RaiseTexts {
  return {
    node_ids: targets.filter((target) => form.chosen.includes(target)),
    text: form.text.trim(),
    working_answer: form.working.trim(),
    price_of_other: form.price.trim(),
  };
}

/** A question's body: its texts and targets and the acting name; no `why`, which a question never holds. */
export function questionBodyOf(form: RaiseForm, targets: readonly string[], as: string | null): CardRaiseBody {
  return { kind: RAISED_KINDS.question, ...raiseTextsOf(form, targets), as };
}

/** What one raise came to: stored, with its ID; not stored, its hits listed; refused or unanswered, the daemon's words. */
export type Raised = { stored: string } | { kept: true } | { refusal: string };

/**
 * The raise of one card and its last outcome. `send` posts the body, naming the hits the outcome
 * says, and keeps what came back; a refusal naming `--as` goes to the acting name's field too.
 */
export function useRaise(project: string, id: string, onActingLine?: (line: string) => void): { outcome: RaiseOutcome; send: (body: CardRaiseBody) => Promise<Raised> } {
  const raise = useCardRaise(project);
  const [outcome, setOutcome] = useState<RaiseOutcome>(NOTHING_RAISED);
  async function send(body: CardRaiseBody): Promise<Raised> {
    const named = nextNamed(outcome);
    try {
      const answer = await raise.mutateAsync({ id, body: named.length === 0 ? body : { ...body, distinct_from: named } });
      if (answer.created && answer.id !== null) {
        return { stored: answer.id };
      }
      setOutcome({ state: "kept", answer, sent: named });
      return { kept: true };
    } catch (error) {
      const failure = apiErrorOf(error);
      if (isActingLine(failure)) {
        onActingLine?.(failure.message);
      }
      setOutcome({ state: "failed", sent: named });
      return { refusal: failure.message };
    }
  }
  return { outcome, send };
}

/** Why a form cannot be sent: a text left blank, or no target chosen; null when it can. */
export function raiseProblem(form: RaiseForm, texts: string, target: string): string | null {
  if ([form.text, form.working, form.price].some((text) => text.trim() === "")) {
    return texts;
  }
  return form.chosen.length === 0 ? target : null;
}

/** The card's targets a raise is about, each a checkbox, every one checked at first. */
export function TargetChoice({ targets, chosen, onChange }: { targets: readonly string[]; chosen: readonly string[]; onChange: (chosen: string[]) => void }) {
  const t = useTranslate();
  const pending = use(Pending);
  return (
    <fieldset className="card-evidence-field raise-targets">
      <legend>{t("board.ask.about")}</legend>
      {targets.map((target) => (
        <label key={target} className="raise-target">
          <input
            type="checkbox"
            checked={chosen.includes(target)}
            aria-disabled={pending || undefined}
            onChange={(event) => {
              if (!pending) {
                onChange(event.target.checked ? targets.filter((one) => one === target || chosen.includes(one)) : chosen.filter((one) => one !== target));
              }
            }}
          />{" "}
          <bdi className="mono">{shownSlot(target)}</bdi>
        </label>
      ))}
    </fieldset>
  );
}

/** A question's three required texts, each its own field; once a submit was tried, a blank one says so. */
export function QuestionFields({
  form,
  labels,
  checked,
  autoFocus = false,
  onChange,
}: {
  form: RaiseForm;
  labels: { text: string; working: string; price: string };
  checked: boolean;
  autoFocus?: boolean;
  onChange: (form: RaiseForm) => void;
}) {
  const field = (key: "text" | "working" | "price") => (value: string) => {
    onChange({ ...form, [key]: value });
  };
  return (
    <>
      <TextArea label={labels.text} value={form.text} onChange={field("text")} required autoFocus={autoFocus} invalid={checked && form.text.trim() === ""} />
      <TextArea label={labels.working} value={form.working} onChange={field("working")} required invalid={checked && form.working.trim() === ""} />
      <TextArea label={labels.price} value={form.price} onChange={field("price")} required invalid={checked && form.price.trim() === ""} />
    </>
  );
}

/**
 * What a raise not stored leaves in its dialog, a polite region that is always there: the move's
 * line once a return went and its question did not; the hits as the daemon listed them, ID or path,
 * state and answer, its notes verbatim; and, with every listed hit named, that this dialog can name
 * no more.
 */
export function RaiseNotice({ outcome, sentBack = null }: { outcome: RaiseOutcome; sentBack?: string | null }) {
  const t = useTranslate();
  return (
    <div className="raise-notice" role="status">
      {sentBack !== null && outcome.state !== "none" && (
        <p className="note">
          <Icon name="cardReturned" />
          <span>{t("board.return.notAsked", { id: sentBack })}</span>
        </p>
      )}
      {outcome.state === "kept" && (
        <>
          <p className="raise-hits-title">{t("board.ask.hits")}</p>
          <ul className="raise-hits">
            {outcome.answer.hits.map((hit, index) => (
              <li key={`${String(index)}-${hitName(hit)}`} className="raise-hit">
                <bdi className="mono">{shownSlot(hitName(hit))}</bdi>
                <span className="muted" lang="en">
                  {shownSlot(hit.status)}
                </span>
                {hit.answer !== null && <bdi className="board-text">{shownSlot(hit.answer)}</bdi>}
              </li>
            ))}
          </ul>
          {outcome.answer.notes.length > 0 && (
            <ul className="note-list" aria-label={t("board.view.writeNotes")}>
              {outcome.answer.notes.map((note, index) => (
                <li key={`${String(index)}-${note}`} className="verbatim-note" lang="en">
                  {note}
                </li>
              ))}
            </ul>
          )}
          {allNamed(outcome) && (
            <p className="note">
              <Icon name="info" />
              <span>{t("board.ask.tooMany")}</span>
            </p>
          )}
        </>
      )}
    </div>
  );
}

/** What a return moves: the card as the list or its read has it now, so a submit goes on its current revision; its targets, which a question may be about. */
export type ReturnTarget = Pick<CardRow, "id" | "title" | "revision" | "targets">;

/**
 * Return (`docs/canon/card-flows.md` "The rule"): a move into the flow's `return` column with a
 * reason, which the move's event keeps. One required field; submit is one `POST .../move` with the
 * column, the note and, from a drop between two cards, its place. Cancel moves nothing: the card
 * stays where it stood. The daemon judges the move as any other and words a refusal. A card with a
 * target may ask a question with it (docs/features/card-questions.md "Acceptance criteria", U2): its
 * part, closed at first, takes the three texts and the targets; submit then sends the move, and only
 * once it is taken the question, `POST .../items`. A refused move asks nothing. A question not stored
 * or refused leaves the card sent back and the dialog open on its fields, the reason held; Ask anyway
 * or Ask again then resends the question alone, and closing it reports the move.
 */
export function ReturnDialog({
  project,
  card,
  to,
  toTitle,
  before = null,
  ...handlers
}: { project: string; card: ReturnTarget; to: string; toTitle: string; before?: string | null } & Omit<WriteHandlers, "onDone"> & {
  /** The move went; `asked` the question stored with it, else null. */
  onDone: (answer: CardWrite, asked: string | null) => void;
}) {
  const t = useTranslate();
  const { onClose, onDone, onActingLine } = handlers;
  const write = useCardWrite(project);
  const [acting] = useActing();
  const [reason, setReason] = useState("");
  const [missing, setMissing] = useState(false);
  const [asking, setAsking] = useState(false);
  const [form, setForm] = useState<RaiseForm>(() => raiseFormOf(card.targets));
  const [checked, setChecked] = useState(false);
  // The move once taken, and the revision it went on: the dialog never reads its own move as a change made elsewhere.
  const [moved, setMoved] = useState<{ answer: CardWrite; on: string } | null>(null);
  const raise = useRaise(project, card.id, onActingLine);

  function bodyOf(): CardMoveBody {
    return { to, ...(before === null ? {} : { before }), note: reason.trim(), revision: card.revision, as: actingName(acting) };
  }

  async function ask(answer: CardWrite): Promise<string | null | false> {
    const raised = await raise.send(questionBodyOf(form, card.targets, actingName(acting)));
    if ("stored" in raised) {
      onDone(answer, raised.stored);
      return null;
    }
    return "kept" in raised ? false : raised.refusal;
  }

  async function submit(): Promise<string | null | false> {
    if (moved !== null) {
      return ask(moved.answer);
    }
    let answer: CardWrite;
    try {
      answer = await write.mutateAsync({ write: "move", id: card.id, body: bodyOf() });
    } catch (error) {
      const refusal = apiErrorOf(error);
      if (isActingLine(refusal)) {
        onActingLine?.(refusal.message);
      }
      return refusal.message;
    }
    if (!asking) {
      onDone(answer, null);
      return null;
    }
    setMoved({ answer, on: card.revision });
    return ask(answer);
  }

  const action = moved === null ? t("board.return.submit") : raiseAction(raise.outcome, t("board.ask.submit"), t("board.ask.anyway"), t("board.ask.again"));
  return (
    <WriteDialog
      title={t("board.return.title", { id: card.id })}
      action={action}
      sending={moved === null ? t("board.return.sending") : t("board.ask.sending")}
      dismiss={moved === null ? undefined : t("inbox.dialog.close")}
      revision={moved === null ? card.revision : moved.on}
      onClose={() => {
        // Sent back already: closing reports the move, whatever became of the question.
        if (moved === null) {
          onClose();
        } else {
          onDone(moved.answer, null);
        }
      }}
      check={() => {
        const empty = reason.trim() === "";
        setMissing(empty);
        if (empty) {
          return t("board.return.why");
        }
        if (!asking) {
          return null;
        }
        setChecked(true);
        return raiseProblem(form, t("board.ask.needsTexts"), t("board.ask.needsTarget"));
      }}
      onSubmit={submit}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      <p className="dialog-text dialog-effect">{fill(t("board.return.effect", { column: slot("column") }), { column: <bdi>{shownSlot(toTitle)}</bdi> })}</p>
      <TextArea label={t("board.field.reason")} value={reason} onChange={setReason} required autoFocus invalid={missing && reason.trim() === ""} readOnly={moved !== null} />
      {card.targets.length > 0 && (
        <Disclosure
          summary={t("board.return.ask")}
          className="raise-part"
          open={asking}
          onToggle={(open) => {
            // Once sent back, the question is what is left to send: its part stays open.
            if (moved === null) {
              setAsking(open);
            }
          }}
        >
          <QuestionFields form={form} labels={{ text: t("board.ask.question"), working: t("board.ask.workingAnswer"), price: t("board.ask.price") }} checked={checked} onChange={setForm} />
          <TargetChoice
            targets={card.targets}
            chosen={form.chosen}
            onChange={(chosen) => {
              setForm((current) => ({ ...current, chosen }));
            }}
          />
        </Disclosure>
      )}
      <RaiseNotice outcome={raise.outcome} sentBack={moved === null ? null : card.id} />
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
  const t = useTranslate();
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
      title={t("board.dialog.clearTitle", { id: card.id })}
      action={t("board.dialog.clearCard")}
      sending={t("board.dialog.clearing")}
      revision={card.revision}
      canSubmit={name !== null}
      onClose={onClose}
      check={() => {
        setChecked(true);
        if (reason.trim() === "") {
          return t("board.dialog.clearWhy");
        }
        return correcting && estimate.trim() === "" ? t("board.dialog.correctionEstimate") : null;
      }}
      onSubmit={() => (name === null ? Promise.resolve(t("board.dialog.nameNeeded")) : send({ write: "clear", id: card.id, body: bodyOf(name) }))}
    >
      <p className="dialog-text">
        <bdi>{shownSlot(card.title)}</bdi>
      </p>
      {on !== null && (
        <p className="dialog-text dialog-effect">{fill(t("board.dialog.briefChangedAt", { on: slot("on") }), { on: <bdi className="mono">{shownSlot(on)}</bdi> })}</p>
      )}
      <ActingNote />
      <fieldset className="card-evidence-field">
        <legend>{t("board.dialog.meaning")}</legend>
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
            {t("board.dialog.noEffect")}
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
            {t("board.dialog.needsCorrection")}
          </label>
          {!accepted && <p className="card-field-hint">{t("board.dialog.correctionAccepted")}</p>}
        </div>
      </fieldset>
      <TextArea label={t("board.field.reason")} value={reason} onChange={setReason} required autoFocus={name !== null} invalid={checked && reason.trim() === ""} />
      {correcting && (
        <>
          <TextField label={t("board.field.estimate")} value={estimate} onChange={setEstimate} required invalid={checked && estimate.trim() === ""} hint={t("board.hint.estimate")} />
          <TextField label={t("board.field.correctionTitle")} value={title} onChange={setTitle} hint={t("board.hint.correctionTitle", { title: shownSlot(card.title) })} />
          <p className="dialog-text dialog-effect">{t("board.dialog.correctionEffect")}</p>
        </>
      )}
      <ActingField project={project} label={t("board.field.clearingAs")} value={acting} onChange={setActing} autoFocus={name === null} hint={t("board.hint.clearingAs")} />
    </WriteDialog>
  );
}

/**
 * Clear all (docs/features/trace-views.md "A node's cards"): every suspect card of a group, no effect
 * only, one POST, the dialog the consent. A refusal keeps it open, in the daemon's words. On a hub the
 * POST carries `count`, the number the dialog shows, and the daemon refuses any other
 * (`docs/features/hub-team.md` "Data"); locally there is no such key.
 */
export function ClearGroupDialog({ project, group, cards, onClose, onDone }: { project: string; group: string; cards: number; onClose: () => void; onDone: (answer: CardClearGroupWrite) => void }) {
  const t = useTranslate();
  const clear = useCardClearGroup(project);
  const login = useHubLogin();
  const [acting, setActing] = useActing();
  const [reason, setReason] = useState("");
  const [checked, setChecked] = useState(false);
  const name = actingName(acting);

  async function send(as: string): Promise<string | null> {
    try {
      onDone(await clear.mutateAsync({ group, reason: reason.trim(), as, ...(login === null ? {} : { count: cards }) }));
      return null;
    } catch (error) {
      return apiErrorOf(error).message;
    }
  }

  return (
    <WriteDialog
      title={t("board.dialog.clearAllTitle", { group })}
      action={t("board.dialog.clearAll")}
      sending={t("board.dialog.clearing")}
      canSubmit={name !== null}
      onClose={onClose}
      check={() => {
        setChecked(true);
        return reason.trim() === "" ? t("board.dialog.clearAllWhy") : null;
      }}
      onSubmit={() => (name === null ? Promise.resolve(t("board.dialog.nameNeeded")) : send(name))}
    >
      <p className="dialog-text">{fill(t("board.dialog.clearAllText", { count: cards, group: slot("group") }), { group: <bdi className="mono">{shownSlot(group)}</bdi> })}</p>
      <ActingNote />
      <TextArea label={t("board.field.reason")} value={reason} onChange={setReason} required autoFocus={name !== null} invalid={checked && reason.trim() === ""} />
      <ActingField project={project} label={t("board.field.clearingAs")} value={acting} onChange={setActing} autoFocus={name === null} hint={t("board.hint.clearingAs")} />
    </WriteDialog>
  );
}

import { useMemo, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useHeldWorkspaces } from "../api/queries";
import { useCardSplit, useHeldTrace, useSplitPlan } from "../api/splitQueries";
import type { CardSplit, CardSplitBody, CardSplitChild, TraceCard, Workspace, WorkspaceDocument } from "../api/types";
import { sectionHash } from "../app/routes";
import { DocLink } from "../doc/DocLink";
import { useTranslate } from "../i18n";
import { Disclosure } from "../ui/Disclosure";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot } from "../ui/text";
import { storedWorkspace } from "../workspace/choice";
import { actingName, handOffReviewer, isActingLine, useActing, useHubLogin } from "./acting";
import { SelectField, TextArea, TextField, WriteDialog } from "./CardDialogs";
import { splitFromWords } from "./labels";

// The Split dialog (docs/canon/card-split.md "The rule"; docs/features/department-split.md
// "Acceptance criteria", U1, U2): a parent task and one subtask per chosen department in one write,
// all or none. Its own chunk, loaded by `lazy()` at each opener once pressed, never preloaded. It
// reads the daemon's plan once as it opens, a dry run planning every discipline the flows list, and
// shows it: each subtask's flow, its targets and where they come from, the project's split templates'
// suggestions checked. Nothing here computes a target, a suggestion or a place: the page sends the
// person's choice and the daemon judges it. The only comparison of served names is the grouping of
// the rows under the departments whose flows name theirs.

/** What a split is made from: a document, the parent made new on it; or a stored task, the parent itself. */
export type SplitSource = { target: string; title: string } | { parent: string; reviewer: string | null };

/** The dialog's fields as typed: the task part a new parent's alone. */
interface SplitForm {
  title: string;
  goal: string;
  discipline: string;
  reviewer: string;
}

/** The rows under one department, or the rows no department's flows name (`title` null with no department above them). */
interface SplitGroup {
  key: string;
  title: string | null;
  rows: CardSplitChild[];
}

/** Every discipline the departments serve, each once, in served order: the Discipline select's options. */
function servedDisciplines(read: WorkspaceDocument | undefined): string[] {
  return [...new Set((read?.workspaces ?? []).flatMap((workspace) => workspace.disciplines))];
}

/**
 * The plan's rows under the first department, in served order, whose `flows` names each row's flow;
 * the rest last, under Other flows when a department stands above them, else under no heading. A
 * department serving every flow (`flows` null) names none, so it takes no row.
 */
function groupsOf(children: readonly CardSplitChild[], departments: readonly Workspace[], other: string): SplitGroup[] {
  const held = departments.map((department): SplitGroup => ({ key: department.name, title: department.title, rows: [] }));
  const rest: CardSplitChild[] = [];
  for (const child of children) {
    const at = departments.findIndex((department) => department.flows?.includes(child.flow) === true);
    const group = held[at];
    if (group === undefined) {
      rest.push(child);
    } else {
      group.rows.push(child);
    }
  }
  const named = held.filter((group) => group.rows.length > 0);
  return rest.length === 0 ? named : [...named, { key: "", title: named.length === 0 ? null : other, rows: rest }];
}

/** The typed text as sent: trimmed, empty as null. */
function given(text: string): string | null {
  const value = text.trim();
  return value === "" ? null : value;
}

/** What the source puts in a body: a new parent's keys, or the stored parent's ID. */
function sourceKeys(source: SplitSource, form: Pick<SplitForm, "title" | "goal" | "discipline">): Pick<CardSplitBody, "target" | "title" | "goal" | "discipline" | "parent"> {
  return "parent" in source
    ? { target: null, title: null, goal: null, discipline: null, parent: source.parent }
    : { target: [source.target], title: form.title.trim(), goal: given(form.goal), discipline: given(form.discipline), parent: null };
}

/** One subtask: its checkbox labelled by its flow's title, the raw discipline under it when another, then its targets' line and the targets folded. */
function SplitRow({ project, child, checked, pending, onToggle }: { project: string; child: CardSplitChild; checked: boolean; pending: boolean; onToggle: (discipline: string) => void }) {
  const t = useTranslate();
  const words = splitFromWords(child.from, child.targets.length, t);
  return (
    <li className="split-row">
      <label className="split-choice">
        <input
          type="checkbox"
          checked={checked}
          aria-disabled={pending || undefined}
          onChange={() => {
            if (!pending) {
              onToggle(child.discipline);
            }
          }}
        />
        <span className="split-choice-text">
          <bdi className="split-flow">{shownSlot(child.flow_title)}</bdi>{" "}
          {child.discipline !== child.flow && (
            <span className="split-discipline mono" lang="en">
              {child.discipline}
            </span>
          )}
        </span>
      </label>
      {child.targets.length === 0 ? (
        <p className="split-targets-line">{t("board.split.noTargets")}</p>
      ) : (
        <Disclosure summary={<span lang={words.served ? "en" : undefined}>{words.text}</span>} className="split-targets">
          <ul className="split-target-list">
            {child.targets.map((target) => (
              <li key={target}>
                <DocLink project={project} docRef={target}>
                  <span className="mono">{target}</span>
                </DocLink>
              </li>
            ))}
          </ul>
        </Disclosure>
      )}
    </li>
  );
}

/**
 * The document's tasks the view around holds: its trace read under `heldRef`, the REF that view read
 * it by, the document's row the one named `target`, as the parent's target names it; each a link to
 * its work item. None held, nothing. No duplicate is judged.
 */
function ExistingTasks({ project, heldRef, target }: { project: string; heldRef: string; target: string }) {
  const t = useTranslate();
  const held = useHeldTrace(project, heldRef).data;
  const cards: TraceCard[] = held?.nodes.find((node) => node.id === target)?.cards ?? [];
  if (cards.length === 0) {
    return null;
  }
  return (
    <div className="split-existing">
      <p className="dialog-text">{t("board.split.existing")}</p>
      <ul className="split-existing-list">
        {cards.map((card) => (
          <li key={card.id}>
            <a href={sectionHash(project, "board", card.id)}>
              <span className="mono">{card.id}</span> <bdi>{shownSlot(card.title)}</bdi>
            </a>
          </li>
        ))}
      </ul>
    </div>
  );
}

/**
 * Split to departments. `source` a document (its REF and title, the parent made new on it) or a
 * stored task (the parent; only subtasks are made). On open, one plan read; a skeleton while it is
 * out; a failure in the daemon's words and Retry, which reads again with the form's discipline. A
 * new parent's Title, Goal and Discipline (every discipline the departments serve; the chosen
 * department's first by default); Reviewer, Hand off's; a checkbox per planned subtask, the
 * suggested ones checked; from a document view, its tasks, read from the trace that view holds under
 * `heldTraceRef` (none given, as from Flow or a task: none listed). Split is one POST of the checked
 * disciplines in row order; `onDone` hears the answer, the caller closing the dialog and saying it.
 */
export function SplitDialog({
  project,
  source,
  heldTraceRef,
  onClose,
  onDone,
  onActingLine,
}: {
  project: string;
  source: SplitSource;
  /** The REF the document view around holds the document's trace read under, the route's. */
  heldTraceRef?: string;
  onClose: () => void;
  onDone: (answer: CardSplit) => void;
  onActingLine?: (line: string) => void;
}) {
  const t = useTranslate();
  const login = useHubLogin();
  const [acting] = useActing();
  const held = useHeldWorkspaces(project);
  const write = useCardSplit(project);
  // Chosen once, as Hand off chooses its form: a read landing while the dialog is open changes no field.
  const [departments] = useState(() => held.data?.workspaces ?? []);
  const [disciplines] = useState(() => servedDisciplines(held.data));
  const doc = "parent" in source ? null : source;
  const [form, setForm] = useState<SplitForm>(() => ({
    title: doc?.title ?? "",
    goal: "",
    discipline: doc === null ? "" : (storedWorkspace(project, held.data)?.disciplines[0] ?? ""),
    reviewer: handOffReviewer("parent" in source ? source.reviewer : null, acting),
  }));
  // The discipline the plan was read with: the default as it opens, the form's on Retry.
  const [asked, setAsked] = useState(form.discipline);
  const plan = useSplitPlan(project, {
    ...sourceKeys(source, { title: doc?.title ?? "", goal: "", discipline: asked }),
    child: null,
    reviewer: null,
    dry_run: true,
    as: actingName(acting),
  });
  // The rows checked, by discipline: the plan's suggestions once it lands, then the person's.
  const [checked, setChecked] = useState<ReadonlySet<string>>(new Set());
  const [checkedFrom, setCheckedFrom] = useState<CardSplit | undefined>(undefined);
  const [stopped, setStopped] = useState(false);
  const pending = write.isPending;
  if (plan.data !== undefined && plan.data !== checkedFrom) {
    // Storing information from previous renders (react.dev, useState): a plan landing checks its suggestions.
    setCheckedFrom(plan.data);
    setChecked(new Set(plan.data.children.filter((child) => child.suggested).map((child) => child.discipline)));
  }
  const groups = useMemo(() => groupsOf(plan.data?.children ?? [], departments, t("board.split.otherFlows")), [plan.data, departments, t]);
  const rows = groups.flatMap((group) => group.rows);
  const chosen = rows.filter((child) => checked.has(child.discipline)).map((child) => child.discipline);
  const reviewer = login ?? form.reviewer;

  function field(key: keyof SplitForm) {
    return (value: string) => {
      setForm((current) => ({ ...current, [key]: value }));
    };
  }

  function toggle(discipline: string) {
    setChecked((current) => {
      const next = new Set(current);
      if (!next.delete(discipline)) {
        next.add(discipline);
      }
      return next;
    });
  }

  function retry() {
    if (form.discipline === asked) {
      void plan.refetch({ cancelRefetch: false });
    } else {
      setAsked(form.discipline);
    }
  }

  async function submit(): Promise<string | null> {
    try {
      onDone(await write.mutateAsync({ ...sourceKeys(source, form), child: chosen, reviewer: reviewer.trim(), dry_run: false, as: actingName(acting) }));
      return null;
    } catch (error) {
      const answer = apiErrorOf(error);
      if (isActingLine(answer)) {
        onActingLine?.(answer.message);
      }
      return answer.message;
    }
  }

  let subtasks;
  if (plan.data === undefined) {
    subtasks =
      plan.error === null ? (
        <Skeleton label={t("board.split.reading")} lines={3} />
      ) : (
        <ErrorPanel title={t("board.split.failed")} message={apiErrorOf(plan.error).message} retrying={plan.isFetching} attempt={plan.errorUpdateCount} onRetry={retry} />
      );
  } else {
    subtasks = (
      <>
        {rows.some((child) => child.suggested) && <p className="dialog-text split-suggested">{t("board.split.suggested")}</p>}
        {groups.map((group) =>
          group.title === null ? (
            <ul key={group.key} className="split-rows">
              {group.rows.map((child) => (
                <SplitRow key={child.discipline} project={project} child={child} checked={checked.has(child.discipline)} pending={pending} onToggle={toggle} />
              ))}
            </ul>
          ) : (
            <fieldset key={group.key} className="split-department">
              <legend>
                <bdi>{shownSlot(group.title)}</bdi>
              </legend>
              <ul className="split-rows">
                {group.rows.map((child) => (
                  <SplitRow key={child.discipline} project={project} child={child} checked={checked.has(child.discipline)} pending={pending} onToggle={toggle} />
                ))}
              </ul>
            </fieldset>
          ),
        )}
        {chosen.length === 0 && (
          <p className="card-field-hint" role="status">
            {t("board.split.none")}
          </p>
        )}
        {plan.data.notes.length > 0 && (
          <ul className="note-list">
            {plan.data.notes.map((note, index) => (
              <li key={`${String(index)}-${note}`} className="verbatim-note" lang="en">
                {note}
              </li>
            ))}
          </ul>
        )}
      </>
    );
  }

  return (
    <WriteDialog
      title={doc === null ? t("board.split.headingCard", { id: "parent" in source ? source.parent : "" }) : t("board.split.heading")}
      action={t("board.split.submit")}
      sending={t("board.split.pending")}
      className="split-dialog"
      canSubmit={plan.data !== undefined && chosen.length > 0}
      check={() => {
        setStopped(true);
        if (doc !== null && form.title.trim() === "") {
          return t("board.dialog.needsTitle");
        }
        return reviewer.trim() === "" ? t("board.handoff.reviewerNeeded") : null;
      }}
      onClose={onClose}
      onSubmit={submit}
    >
      {doc !== null && (
        <>
          <TextField label={t("board.split.title")} value={form.title} onChange={field("title")} required autoFocus invalid={stopped && form.title.trim() === ""} />
          <TextArea label={t("board.split.goal")} value={form.goal} onChange={field("goal")} />
          <SelectField
            label={t("board.split.discipline")}
            value={form.discipline}
            onChange={field("discipline")}
            options={disciplines.map((name) => ({ value: name, label: name }))}
            placeholder={t("ui.emptyValue")}
            hint={t("board.hint.discipline")}
          />
        </>
      )}
      {login === null ? (
        <TextField
          label={t("board.split.reviewer")}
          value={form.reviewer}
          onChange={field("reviewer")}
          required
          autoFocus={doc === null}
          invalid={stopped && reviewer.trim() === ""}
          hint={t("board.handoff.reviewerHint")}
        />
      ) : (
        <TextField label={t("board.split.reviewer")} value={login} onChange={field("reviewer")} readOnly hint={t("board.handoff.reviewerHint")} />
      )}
      <fieldset className="split-subtasks" aria-busy={plan.data === undefined && plan.error === null}>
        <legend>{t("board.split.subtasks")}</legend>
        {subtasks}
      </fieldset>
      {doc !== null && heldTraceRef !== undefined && <ExistingTasks project={project} heldRef={heldTraceRef} target={doc.target} />}
    </WriteDialog>
  );
}

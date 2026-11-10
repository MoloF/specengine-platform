import type { Card, CardTask } from "../api/types";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { approveCommand, awaitsOwner, TASK_ID } from "../tasks/actions";
import { specChanged, taskStatusLook } from "../tasks/labels";
import { Badge, type Look } from "../ui/Badge";
import { CopyButton } from "../ui/CopyButton";
import { EmptyState } from "../ui/EmptyState";
import { Icon } from "../ui/Icon";
import { shownSlot } from "../ui/text";
import { useHubLogin } from "./acting";

// A card's Tasks part (`docs/canon/card-tasks.md` "The rule";
// `docs/canon/board-screen.md` "The card modal and page"), under the work item's Development, folded
// (docs/features/design-foundation.md "The work item: the task modal and page"): the `T-` tasks the card links, in link
// order, each with its state as the card's read served it. It reads nothing of its own and writes
// nothing: no link, no task (`ui/README.md` "Screen rules"). A task's state is shown beside its card
// and decides nothing; the one command that waits on the owner, a plan's approval, is a command to
// copy, fixed words and a checked ID. On a hub no task is read: each row is its ID, and the state
// lives on the terminal that holds it.

/** A card's ID as `spec card` mints it: only such an ID enters a command. */
export const CARD_ID = /^C-[0-9]{4,}$/;

/** A target that may stand in a command as written: a letter or digit, then letters, digits, `.`, `_`, `/`, `-`. */
export const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._/-]*$/;

/** What stands for the nodes when a card names none, or one that is not safe to paste as written. */
const ANY_REF = "REF";

/** The prefix the daemon starts a linked task's note with. */
function notePrefix(id: string): string {
  return `${id}: `;
}

/** The card's notes on one linked task, as served: those starting `<id>: `. */
export function notesOn(notes: readonly string[], id: string): string[] {
  const prefix = notePrefix(id);
  return notes.filter((note) => note.startsWith(prefix));
}

/** The card's notes no linked task's row shows: its own, each shown once. */
export function ownNotes(card: Pick<Card, "notes" | "tasks">): string[] {
  const prefixes = card.tasks.map((task) => notePrefix(task.id));
  return card.notes.filter((note) => !prefixes.some((prefix) => note.startsWith(prefix)));
}

/** `spec task new` on the card's targets when each is safe to paste, else on `REF`; the title never filled in. */
export function newTaskCommand(targets: readonly string[]): string {
  const refs = targets.length > 0 && targets.every((target) => SAFE_REF.test(target)) ? targets.join(" ") : ANY_REF;
  return `spec task new --nodes ${refs} --title "..."`;
}

/** `spec card edit` linking a task to the card; null when the ID is not a card ID: no command is offered. */
export function linkCommand(id: string): string | null {
  return CARD_ID.test(id) ? `spec card edit ${id} --task T-NNNN` : null;
}

/** A command to copy: what it does, the command as copied, and Copy. */
function Command({ meaning, text }: { meaning: string; text: string }) {
  const t = useTranslate();
  return (
    <li className="command">
      <span className="command-meaning">{meaning}</span>
      <code className="command-text">{text}</code>
      <CopyButton text={text} context={t("tasks.copyContext", { command: text })} failed={t("tasks.copyFailed")} />
    </li>
  );
}

/** A served note, verbatim, the daemon's English. */
function Notes({ notes, label }: { notes: readonly string[]; label: string }) {
  if (notes.length === 0) {
    return null;
  }
  return (
    <ul className="note-list" aria-label={label}>
      {notes.map((note, index) => (
        <li key={`${String(index)}-${note}`} lang="en">
          {note}
        </li>
      ))}
    </ul>
  );
}

/** One linked task: its ID (a link to it on Tasks when served and a task ID), title, state, a spec change, and the approval when it waits on the owner. */
function TaskRow({ project, task, notes }: { project: string; task: CardTask; notes: readonly string[] }) {
  const t = useTranslate();
  const read = task.status !== null;
  const approve = awaitsOwner(task.status) ? approveCommand(task.id) : null;
  const notRead: Look = { label: t("board.tasks.notRead"), tone: "task-unknown", icon: "unknown" };
  return (
    <li className="card-task" data-task={task.id}>
      <div className="card-task-head">
        {read && TASK_ID.test(task.id) ? (
          <a className="mono" href={sectionHash(project, "tasks", task.id)}>
            {shownSlot(task.id)}
          </a>
        ) : (
          <span className="mono">{shownSlot(task.id)}</span>
        )}
        {read && (task.title === null ? <span className="muted">{t("tasks.untitled")}</span> : <bdi className="card-task-title">{shownSlot(task.title)}</bdi>)}
        {task.status === null ? <Badge look={notRead} /> : <Badge look={taskStatusLook(task.status, t)} />}
        {task.stale === true && <Badge look={specChanged(t)} />}
      </div>
      {approve !== null && (
        <ul className="command-list card-task-commands">
          <Command meaning={t("board.tasks.awaiting")} text={approve} />
        </ul>
      )}
      <Notes notes={notes} label={t("board.tasks.notes", { id: task.id })} />
    </li>
  );
}

/**
 * The part's body: a row per link, in link order; none, the commands that make a task and link it,
 * or on a hub, where a terminal's command would write the terminal's own queue, a sentence alone.
 */
export function CardTasks({ project, card }: { project: string; card: Card }) {
  const t = useTranslate();
  const onHub = useHubLogin() !== null;
  const tasks = card.tasks;
  if (tasks.length === 0) {
    // The empty state: no task yet, and how to make one and link it, or on a hub a sentence alone.
    const link = linkCommand(card.id);
    return (
      <EmptyState
        title={t("board.tasks.none")}
        level={3}
        description={
          onHub ? (
            <p className="note">
              <Icon name="terminal" />
              <span>{t("board.tasks.onHub")}</span>
            </p>
          ) : (
            <>
              <p className="note">
                <Icon name="terminal" />
                <span>{t("board.tasks.howTo")}</span>
              </p>
              <ul className="command-list card-task-commands">
                <Command meaning={t("board.tasks.make")} text={newTaskCommand(card.targets)} />
                {link !== null && <Command meaning={t("board.tasks.link")} text={link} />}
              </ul>
            </>
          )
        }
      />
    );
  }
  return (
    <>
      <ul className="board-panel-list card-tasks" aria-label={t("board.tasks.list", { id: card.id })}>
        {tasks.map((task, index) => (
          <TaskRow key={`${String(index)}-${task.id}`} project={project} task={task} notes={notesOn(card.notes, task.id)} />
        ))}
      </ul>
      {onHub && (
        <p className="note">
          <Icon name="terminal" />
          <span>{t("board.tasks.onHub")}</span>
        </p>
      )}
    </>
  );
}

import { useId, useRef, useState, type KeyboardEvent } from "react";
import { apiErrorOf } from "../api/client";
import type { TasksQuery } from "../api/queries";
import type { TaskListEntry } from "../api/types";
import { pushHash } from "../app/location";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { formatAge, formatUtc } from "../ui/time";
import { useFocusLater } from "../ui/useFocusLater";
import { useNow } from "../ui/useNow";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { chipsOf, DEFAULT_FILTERS, isFiltered, isPressed, matchesFilters, toggleChip, type TaskFilters } from "./filter";
import { groupsOf, waitsForYou } from "./groups";
import { SPEC_CHANGED, taskStatusLook } from "./labels";

/** How many targets a row names before "+k". */
const ROW_TARGETS = 3;

/** A row's targets: the first three, then "+k" for the rest. */
export function targetsText(targets: readonly string[]): string {
  if (targets.length === 0) {
    return "No target";
  }
  const shown = targets.slice(0, ROW_TARGETS).join(", ");
  return targets.length > ROW_TARGETS ? `${shown} +${String(targets.length - ROW_TARGETS)}` : shown;
}

function TaskRow({
  entry,
  open,
  tabbable,
  now,
  onOpen,
  onFocus,
}: {
  entry: TaskListEntry;
  open: boolean;
  tabbable: boolean;
  now: number;
  onOpen: (entry: TaskListEntry) => void;
  onFocus: (entry: TaskListEntry) => void;
}) {
  return (
    <div
      role="option"
      aria-selected={open}
      tabIndex={tabbable ? 0 : -1}
      data-task={entry.id}
      className="task-row"
      onClick={() => {
        onOpen(entry);
      }}
      onFocus={() => {
        onFocus(entry);
      }}
    >
      <span className="task-row-top">
        <span className="mono task-row-id">{entry.id}</span>
        <Badge name="State" look={taskStatusLook(entry.status)} />
        {entry.stale === true && <Badge look={SPEC_CHANGED} />}
        <time className="task-row-age" dateTime={entry.updated_at} title={formatUtc(entry.updated_at)}>
          <span className="sr-only">Updated </span>
          {formatAge(entry.updated_at, now)}
        </time>
      </span>
      <span className={entry.title === null ? "task-row-title muted" : "task-row-title"}>{entry.title ?? "Untitled"}</span>
      <span className="task-row-targets mono">
        <span className="sr-only">Targets: </span>
        {targetsText(entry.targets)}
      </span>
    </div>
  );
}

/**
 * The task list (`spec task list`): one read, the daemon's notes above, the filters over that one
 * answer, and the tasks in groups, what waits for the owner first. A listbox with one Tab stop: Up
 * and Down (k, j), Home and End move it; Enter or a click opens the task beside the list (one
 * history entry) and keeps focus on the row; `onOpened` hears each. Keys act only on a row, never
 * with Ctrl, Alt or Cmd.
 */
export function TaskList({
  project,
  taskId,
  tasks,
  onOpened,
}: {
  project: string;
  taskId: string | null;
  tasks: TasksQuery;
  /** Enter or a click on its row opened this task. */
  onOpened: (id: string) => void;
}) {
  const now = useNow();
  const failure = useRetainedFailure(tasks.error, tasks.isFetching);
  const focusLater = useFocusLater();
  const fieldId = useId();
  const listbox = useRef<HTMLDivElement>(null);
  const field = useRef<HTMLInputElement>(null);
  const region = useRef<HTMLDivElement>(null);
  const [filters, setFilters] = useState<TaskFilters>(DEFAULT_FILTERS);
  const [active, setActive] = useState<string | null>(taskId);
  const [shownTask, setShownTask] = useState<string | null>(taskId);

  // The route's task takes the Tab stop when the route changes (Back, a link): state from the
  // previous render (react.dev, useState).
  if (shownTask !== taskId) {
    setShownTask(taskId);
    if (taskId !== null) {
      setActive(taskId);
    }
  }

  function rowElement(id: string): HTMLElement | null {
    const rows = listbox.current?.querySelectorAll<HTMLElement>("[data-task]") ?? [];
    return Array.from(rows).find((row) => row.dataset.task === id) ?? null;
  }

  function rovingRow(): HTMLElement | null {
    return listbox.current?.querySelector<HTMLElement>('[role="option"][tabindex="0"]') ?? null;
  }

  // After a successful Retry the Retry button is gone: the list's Tab stop takes the focus.
  const retried = useRetryFocus(tasks.data, () => rovingRow() ?? field.current ?? region.current);

  if (tasks.data === undefined) {
    return failure === null ? (
      <div aria-busy="true" className="tasks-loading">
        <Skeleton label={`Loading the tasks of ${project}`} lines={6} />
      </div>
    ) : (
      <ErrorPanel
        title="The tasks could not be loaded"
        message={apiErrorOf(failure).message}
        retrying={tasks.isFetching}
        attempt={tasks.errorUpdateCount}
        onRetry={() => {
          retried();
          void tasks.refetch({ cancelRefetch: false });
        }}
      />
    );
  }

  const answer = tasks.data;
  const notes =
    answer.notes.length > 0 ? (
      <ul className="note-list" aria-label="Notes from the daemon on the list">
        {answer.notes.map((note, index) => (
          <li key={`${String(index)}-${note}`} className="verbatim-note">
            {note}
          </li>
        ))}
      </ul>
    ) : null;

  if (answer.tasks.length === 0) {
    return (
      <div ref={region} tabIndex={-1} className="tasks-list-region">
        {notes}
        <div className="empty-state">
          <p>
            <strong>No tasks yet.</strong> Create one: <code>spec task new --nodes REF...</code>
          </p>
          <p className="muted">A task names the spec nodes to work on; you approve it on a terminal, and agents work from its package.</p>
        </div>
      </div>
    );
  }

  const chips = chipsOf(answer.tasks);
  const changedCount = answer.tasks.filter((entry) => entry.stale === true).length;
  const waiting = answer.tasks.filter(waitsForYou).length;
  const visible = answer.tasks.filter((entry) => matchesFilters(entry, filters));
  const groups = groupsOf(visible);
  const order = groups.flatMap((group) => group.entries);
  const roving = order.find((entry) => entry.id === active) ?? order.find((entry) => entry.id === taskId) ?? order[0];

  function moveTo(entry: TaskListEntry | undefined) {
    if (entry === undefined) {
      return;
    }
    setActive(entry.id);
    focusLater(() => rowElement(entry.id));
  }

  function open(entry: TaskListEntry) {
    onOpened(entry.id);
    setActive(entry.id);
    pushHash(sectionHash(project, "tasks", entry.id));
    focusLater(() => rowElement(entry.id));
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (hasModifier(event) || isTextField(event.target) || !(event.target instanceof Element)) {
      return;
    }
    const id = event.target.closest<HTMLElement>('[role="option"]')?.dataset.task;
    const at = order.findIndex((entry) => entry.id === id);
    const here = order[at];
    if (here === undefined) {
      return;
    }
    switch (event.key) {
      case "ArrowDown":
      case "j":
        moveTo(order[at + 1]);
        break;
      case "ArrowUp":
      case "k":
        moveTo(order[at - 1]);
        break;
      case "Home":
        moveTo(order[0]);
        break;
      case "End":
        moveTo(order[order.length - 1]);
        break;
      case "Enter":
        open(here);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  function clearFilters() {
    setFilters(DEFAULT_FILTERS);
    // The button goes with the message: focus the list's Tab stop instead of the page's body.
    focusLater(() => rovingRow() ?? field.current);
  }

  return (
    <div ref={region} tabIndex={-1} className="tasks-list-region">
      {notes}
      <p className="tasks-summary">
        <Icon name="waiting" />
        <span>
          <strong>{waiting}</strong> waiting for you, {answer.tasks.length} {answer.tasks.length === 1 ? "task" : "tasks"} in all
        </span>
      </p>
      <div className="task-filters" role="group" aria-label="Filter the tasks">
        <div className="task-chips">
          {chips.map((chip) => {
            const pressed = isPressed(filters, chip.key);
            return (
              <button
                key={chip.key}
                type="button"
                className="chip"
                aria-pressed={pressed}
                onClick={() => {
                  setFilters((current) => toggleChip(current, chip.key));
                }}
              >
                <span className="chip-check" aria-hidden="true">
                  {pressed && <Icon name="selected" />}
                </span>
                <span>{chip.label}</span> <span className="chip-count">{chip.count}</span>
              </button>
            );
          })}
          <button
            type="button"
            className="chip chip-changed"
            aria-pressed={filters.changedOnly}
            onClick={() => {
              setFilters((current) => ({ ...current, changedOnly: !current.changedOnly }));
            }}
          >
            <span className="chip-check" aria-hidden="true">
              {filters.changedOnly && <Icon name="selected" />}
            </span>
            <Icon name="specChanged" />
            <span>Spec changed</span> <span className="chip-count">{changedCount}</span>
          </button>
        </div>
        <label className="field-label" htmlFor={fieldId}>
          Filter by ID, title or target
        </label>
        <input
          id={fieldId}
          ref={field}
          className="field"
          type="search"
          autoComplete="off"
          spellCheck={false}
          value={filters.query}
          onChange={(event) => {
            const query = event.target.value;
            setFilters((current) => ({ ...current, query }));
          }}
        />
      </div>
      {order.length === 0 ? (
        <div className="empty-filter">
          <p>No task matches the filters.</p>
          {isFiltered(
            filters,
            chips.map((chip) => chip.key),
          ) ? (
            <button type="button" className="button" onClick={clearFilters}>
              Clear filters
            </button>
          ) : (
            <p className="muted">Every task is closed: press Closed to list them.</p>
          )}
        </div>
      ) : (
        <div ref={listbox} role="listbox" aria-label="Tasks, what waits for you first" className="task-list" onKeyDown={onKeyDown}>
          {groups.map((group) => (
            <div
              key={group.key}
              role="group"
              aria-labelledby={`${fieldId}-${group.key}`}
              className={group.waiting ? "task-group task-group-waiting" : "task-group"}
            >
              <div id={`${fieldId}-${group.key}`} role="presentation" className="task-group-title">
                {group.waiting && <Icon name="waiting" />}
                <span>{group.title}</span>
                <span className="task-group-count">
                  <span className="sr-only">, </span>
                  {group.entries.length}
                  <span className="sr-only">{group.entries.length === 1 ? " task" : " tasks"}</span>
                </span>
              </div>
              {group.entries.map((entry) => (
                <TaskRow
                  key={entry.id}
                  entry={entry}
                  open={entry.id === taskId}
                  tabbable={entry === roving}
                  now={now}
                  onOpen={open}
                  onFocus={(focused) => {
                    if (focused.id !== active) {
                      setActive(focused.id);
                    }
                  }}
                />
              ))}
            </div>
          ))}
        </div>
      )}
      {tasks.error !== null && (
        <p className="note" role="alert">
          The tasks could not be read again: {apiErrorOf(tasks.error).message}
        </p>
      )}
    </div>
  );
}

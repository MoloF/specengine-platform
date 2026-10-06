import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTasks } from "../api/queries";
import { movesHere } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { SECTIONS } from "../app/sections";
import { useOpenShortcuts } from "../app/shortcuts";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { waitsForYou } from "./groups";
import { TaskDetail, type TaskTab } from "./TaskDetail";
import { TaskList } from "./TaskList";

const ABOUT = SECTIONS.find((section) => section.id === "tasks")?.about ?? "";

/** Where app.css stacks the task under the list (`.tasks-view`): narrow screens, 200 % zoom. */
const STACKED = "(max-width: 48em)";

/** Whether the task shows under the list rather than beside it. jsdom has no `matchMedia`: never there. */
function isStacked(): boolean {
  return "matchMedia" in window && window.matchMedia(STACKED).matches;
}

/** Scrolls the open task's heading to the top of the view; focus stays where it is. */
function bringIntoSight(column: HTMLElement | null) {
  (column?.querySelector<HTMLElement>("h1") ?? column)?.scrollIntoView({ block: "start" });
}

/** The detail column with no task chosen: the view's h1 and what waits for the owner. */
function NoTaskPane({ waiting }: { waiting: number | null }) {
  return (
    <section className="task-pane" aria-labelledby="tasks-title">
      <header className="task-head">
        <h1 id="tasks-title" tabIndex={-1}>
          Tasks
        </h1>
        <p className="view-about">{ABOUT}</p>
      </header>
      <div className="empty-state">
        <h2>No task open</h2>
        {waiting !== null && waiting > 0 && (
          <p className="waiting-note">
            <Icon name="waiting" />
            <span>
              {waiting} {waiting === 1 ? "task waits" : "tasks wait"} for you: the list shows them first.
            </span>
          </p>
        )}
        <p>Pick a task in the list to read its goal, plan, spec changes, proposals and runs, and the terminal commands for what you decide.</p>
        <p className="muted">In the list: the arrow keys move, Enter opens, ? lists every key.</p>
      </div>
    </section>
  );
}

/**
 * The Tasks view (`#/<project>/tasks[/<T>]`): the task list, what waits for the owner first, and
 * beside it the task the route names; stacked on narrow screens. Nothing is opened by itself (a
 * package read builds a bundle). Each region reads, fails and retries on its own. Read-only: an
 * owner action is a command for a terminal (docs/canon/architecture.md#control).
 */
export function TasksView({ project, taskId }: { project: string; taskId: string | null }) {
  const openShortcuts = useOpenShortcuts();
  const tasks = useTasks(project);
  const listRegion = useRef<HTMLDivElement>(null);
  const taskColumn = useRef<HTMLDivElement>(null);
  /** The task opened from the list while stacked, whose heading is brought into sight once it shows. */
  const sightFor = useRef<string | null>(null);
  const [tab, setTab] = useState<TaskTab>("overview");

  // The next task shown takes the request once; any other route change drops it.
  useEffect(() => {
    const wanted = sightFor.current;
    sightFor.current = null;
    if (wanted !== null && wanted === taskId) {
      bringIntoSight(taskColumn.current);
    }
  }, [taskId]);

  /**
   * Enter or a click opened `id` and kept focus on its row: stacked, the task is below the list and
   * out of sight, so its heading is scrolled into view (now if it is open already, else once it
   * shows). Side by side it is in sight: nothing moves.
   */
  function onOpened(id: string) {
    if (!isStacked()) {
      return;
    }
    if (id === taskId) {
      bringIntoSight(taskColumn.current);
    } else {
      sightFor.current = id;
    }
  }

  /** The list's place for focus: the task's row, else the list's Tab stop, else its field or itself. */
  function listTarget(id: string | null): HTMLElement | null {
    const region = listRegion.current;
    if (region === null) {
      return null;
    }
    const rows = Array.from(region.querySelectorAll<HTMLElement>("[data-task]"));
    return (
      rows.find((row) => row.dataset.task === id) ??
      region.querySelector<HTMLElement>('[role="option"][tabindex="0"]') ??
      region.querySelector<HTMLElement>('input[type="search"]') ??
      region
    );
  }

  function onViewKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  const waiting = tasks.data === undefined ? null : tasks.data.tasks.filter(waitsForYou).length;

  return (
    <div className="tasks-view" onKeyDown={onViewKeyDown}>
      <section className="tasks-pane" aria-label="Task list">
        <div
          ref={listRegion}
          tabIndex={-1}
          className="tasks-region"
          aria-busy={tasks.data === undefined ? tasks.error === null : tasks.isFetching}
        >
          <RegionBoundary name="task list">
            <TaskList project={project} taskId={taskId} tasks={tasks} onOpened={onOpened} />
          </RegionBoundary>
        </div>
      </section>
      <div ref={taskColumn} className="task-column">
        <RegionBoundary key={taskId ?? ""} name="task" heading={taskId ?? "Tasks"}>
          {taskId === null ? (
            <NoTaskPane waiting={waiting} />
          ) : (
            <TaskDetail
              key={`${project}/${taskId}`}
              project={project}
              taskId={taskId}
              tab={tab}
              onTab={setTab}
              onBack={() => {
                listTarget(taskId)?.focus();
              }}
              onBackToList={(event) => {
                // Only a link the page follows: Cmd-, Ctrl-, Shift- or Alt-click opens it elsewhere.
                if (movesHere(event)) {
                  listTarget(null)?.focus();
                }
              }}
            />
          )}
        </RegionBoundary>
      </div>
    </div>
  );
}

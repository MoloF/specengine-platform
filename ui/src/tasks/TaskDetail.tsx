import { useId, useRef, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useTask } from "../api/queries";
import type { TaskPackage } from "../api/types";
import { sectionHash } from "../app/routes";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { ReadFailure, Skeleton } from "../ui/states";
import { TabPanel, Tabs, type TabSpec } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { isTaskNotFound, READ_SCHEMA_VERSION } from "./answer";
import { waitsForYou } from "./groups";
import { FROZEN_AT_APPROVAL, stalenessLook, taskStatusLook } from "./labels";
import { OverviewPanel } from "./OverviewPanel";
import { OwnerCommands } from "./OwnerCommands";
import { PackagePanel } from "./PackagePanel";
import { When } from "./parts";
import { RunsPanel } from "./RunsPanel";
import { SpecChangesPanel } from "./SpecChangesPanel";
import { TaskProposalsPanel } from "./TaskProposalsPanel";

export type TaskTab = "overview" | "plan" | "changes" | "proposals" | "runs" | "package";

function withCount(label: string, count: number | null): ReactNode {
  if (count === null) {
    return label;
  }
  return (
    <>
      {label} <span className="tab-count">{count}</span>
    </>
  );
}

/** Why a task waits for the owner, in the list's terms (`status`, `stale`): nothing else is read. */
function WaitingNote({ task }: { task: TaskPackage }) {
  if (!waitsForYou(task)) {
    return null;
  }
  return (
    <p className="waiting-note">
      <Icon name="waiting" />
      <span>
        {task.status === "review"
          ? "Waiting for you: the plan is ready for your review."
          : "Waiting for you: the spec changed since you approved this task. Spec changes shows how; approving again re-freezes it."}
      </span>
    </p>
  );
}

/** The header facts of a package: state, staleness (four displays), times, the daemon's notes. */
function Facts({ task }: { task: TaskPackage }) {
  const staleness = stalenessLook(task.stale, task.spec_snapshot !== null);
  return (
    <>
      <div className="task-badges">
        <Badge name="State" look={taskStatusLook(task.status)} />
        {staleness === null ? (
          <span className="muted frozen-note">{FROZEN_AT_APPROVAL}</span>
        ) : (
          <Badge name="Spec since approval" look={staleness} />
        )}
      </div>
      {task.notes.length > 0 && (
        <ul className="note-list" aria-label="Notes from the daemon on this task">
          {task.notes.map((note, index) => (
            <li key={`${String(index)}-${note}`} className="verbatim-note">
              {note}
            </li>
          ))}
        </ul>
      )}
      <dl className="facts">
        <div className="fact">
          <dt>Created</dt>
          <dd>
            <When at={task.created_at} />
          </dd>
        </div>
        <div className="fact">
          <dt>Updated</dt>
          <dd>
            <When at={task.updated_at} />
          </dd>
        </div>
      </dl>
      <WaitingNote task={task} />
    </>
  );
}

/** The content of the shown tab; only the Package tab for a package this screen does not read. */
function panelOf(id: TaskTab, project: string, taskId: string, task: TaskPackage, readable: boolean): ReactNode {
  if (id === "package") {
    return <PackagePanel id={taskId} answer={task} />;
  }
  if (!readable) {
    return null;
  }
  switch (id) {
    case "overview":
      return <OverviewPanel project={project} task={task} />;
    case "plan":
      return (
        <div className="task-panel">
          {task.plan === null ? <p className="muted">No plan yet</p> : <pre className="task-plan">{task.plan}</pre>}
        </div>
      );
    case "changes":
      return <SpecChangesPanel project={project} task={task} />;
    case "proposals":
      return <TaskProposalsPanel project={project} task={task} />;
    case "runs":
      return <RunsPanel task={task} />;
  }
}

function tabsOf(task: TaskPackage, readable: boolean): TabSpec<TaskTab>[] {
  if (!readable) {
    return [{ id: "package", label: "Package" }];
  }
  return [
    { id: "overview", label: "Overview" },
    { id: "plan", label: "Plan" },
    { id: "changes", label: withCount("Spec changes", task.snapshot_diff?.length ?? null) },
    { id: "proposals", label: withCount("Proposals", task.open_proposals.length) },
    { id: "runs", label: withCount("Runs", task.runs.length) },
    { id: "package", label: "Package" },
  ];
}

/**
 * One task (`spec task show T --json`), beside the list: the h1 its ID and title, the state and
 * staleness badges, the times, the owner's commands and six tabs. Read once per T, never with the
 * previous task's package standing in. A package of another `schema_version` shows only the
 * Package tab. Esc anywhere in it goes back to its row in the list. Read-only.
 */
export function TaskDetail({
  project,
  taskId,
  tab,
  onTab,
  onBack,
  onBackToList,
}: {
  project: string;
  taskId: string;
  /** The tab chosen last, kept from task to task. */
  tab: TaskTab;
  onTab: (tab: TaskTab) => void;
  /** Esc: focus this task's row in the list. */
  onBack: () => void;
  /** "Back to tasks" was followed: focus the list. */
  onBackToList: (event: MouseEvent<HTMLAnchorElement>) => void;
}) {
  const query = useTask(project, taskId);
  const failure = useRetainedFailure(query.error, query.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(query.data, () => heading.current);
  const base = useId();
  const titleId = `${base}-title`;

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key !== "Escape" || event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    event.preventDefault();
    onBack();
  }

  const answer = query.data;
  const found = answer === undefined || isTaskNotFound(answer) ? null : answer;
  const readable = found !== null && found.schema_version === READ_SCHEMA_VERSION;
  const title = readable ? found.title : null;

  let head: ReactNode = null;
  let body: ReactNode;
  if (answer === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={`Loading ${taskId}`} lines={6} />
        </div>
      ) : (
        <ReadFailure
          title={`${taskId} could not be read`}
          failure={failure}
          retrying={query.isFetching}
          attempt={query.errorUpdateCount}
          onRetry={() => {
            retried();
            void query.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (found === null) {
    body = (
      <div className="empty-state">
        <h2>No such task</h2>
        <p className="verbatim">{isTaskNotFound(answer) ? answer.reason : ""}</p>
        <p>
          <a className="back-link" href={sectionHash(project, "tasks")} onClick={onBackToList}>
            <Icon name="back" />
            Back to tasks
          </a>
        </p>
      </div>
    );
  } else {
    const tabs = tabsOf(found, readable);
    const shown: TaskTab = readable ? tab : "package";
    head = readable ? (
      <>
        <Facts task={found} />
        <OwnerCommands id={found.id} status={found.status} />
      </>
    ) : (
      <>
        <div className="notice notice-version" role="note">
          <p className="notice-title">
            <Icon name="info" />
            <span>
              This package has schema_version {String(found.schema_version)}; this screen reads version {READ_SCHEMA_VERSION}.
            </span>
          </p>
          <p>Only the Package tab is shown, exactly as the daemon sent it.</p>
        </div>
        <OwnerCommands id={taskId} status={null} />
      </>
    );
    body = (
      <>
        <Tabs label={`Views of ${taskId}`} base={base} tabs={tabs} selected={shown} onSelect={onTab} />
        {tabs.map(({ id }) => (
          <TabPanel key={id} base={base} id={id} selected={id === shown}>
            {id === shown && panelOf(id, project, taskId, found, readable)}
          </TabPanel>
        ))}
      </>
    );
  }

  return (
    <article className="task-pane" aria-labelledby={titleId} aria-busy={answer === undefined && failure === null} onKeyDown={onKeyDown}>
      <header className="task-head">
        <h1 id={titleId} ref={heading} tabIndex={-1} className="task-heading">
          <span className="task-name mono">{taskId}</span>
          {readable && (
            <>
              {/* Drawn, the flex gap parts them; read aloud, a colon and a space. */}
              <span className="sr-only">:</span>{" "}
              <span className={title === null ? "task-title-text muted" : "task-title-text"}>{title ?? "Untitled"}</span>
            </>
          )}
        </h1>
        {head}
        {answer !== undefined && query.error !== null && (
          <p className="note" role="alert">
            {taskId} could not be read again: {apiErrorOf(query.error).message}
          </p>
        )}
      </header>
      {body}
    </article>
  );
}

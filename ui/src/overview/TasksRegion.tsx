import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useTasks } from "../api/queries";
import type { TaskList } from "../api/types";
import { sectionHash } from "../app/routes";
import { groupsOf } from "../tasks/groups";
import { SPEC_CHANGED, taskStatusLook } from "../tasks/labels";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ReadFailure, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { Count, Notes, Region, Stored } from "./parts";
import { HOME_ROWS, otherOpenOf, waitingOf } from "./tally";

/** One answer of `spec task list`: what waits for the owner first, then the other open groups' sizes. */
function TasksSummary({ project, answer }: { project: string; answer: TaskList }) {
  const notes = <Notes notes={answer.notes} label="Notes from the daemon on the task list" />;
  if (answer.tasks.length === 0) {
    return (
      <>
        {notes}
        <div className="home-empty">
          <p>No tasks yet.</p>
          <p>
            Create one on a terminal: <code>spec task new --nodes REF...</code>
          </p>
        </div>
      </>
    );
  }
  const groups = groupsOf(answer.tasks);
  const waiting = waitingOf(groups);
  const shown = waiting.slice(0, HOME_ROWS);
  const others = otherOpenOf(groups);
  return (
    <>
      {notes}
      {/* Emphasised only while something waits: nothing to do is no call to act. */}
      <div className={waiting.length > 0 ? "home-block home-block-waiting" : "home-block"}>
        <h3 className="home-block-title">
          <Icon name="waiting" />
          <span>Waiting for you</span>
          <Count value={waiting.length} />
        </h3>
        {waiting.length === 0 ? (
          <p className="muted">Nothing waits for you: no plan to review, no approved spec changed.</p>
        ) : (
          <ul className="home-list">
            {shown.map((entry) => (
              <li key={entry.id} className="home-row">
                <a className="home-row-link" href={sectionHash(project, "tasks", entry.id)}>
                  <span className="mono">{entry.id}</span>: <span className={entry.title === null ? "muted" : undefined}>{entry.title ?? "Untitled"}</span>
                </a>
                <span className="home-row-meta">
                  <Badge name="State" look={taskStatusLook(entry.status)} />
                  {entry.stale === true && <Badge look={SPEC_CHANGED} />}
                  <Stored label="Updated" at={entry.updated_at} />
                </span>
              </li>
            ))}
          </ul>
        )}
        {waiting.length > shown.length && (
          <p className="home-more muted">
            {shown.length} of {waiting.length} shown
          </p>
        )}
      </div>
      <div className="home-block">
        <h3 className="home-block-title">Other open tasks</h3>
        {others.length === 0 ? (
          <p className="muted">No other open task.</p>
        ) : (
          <ul className="home-tally">
            {others.map((group) => (
              <li key={group.key}>
                <span>{group.title}</span>
                <Count value={group.count} />
              </li>
            ))}
          </ul>
        )}
      </div>
    </>
  );
}

/**
 * The home's Tasks region: one `getTasks` per visit, its own loading, empty and error states, and a
 * Retry that reads the tasks again and nothing else.
 */
export function TasksRegion({ project }: { project: string }) {
  const tasks = useTasks(project);
  const failure = useRetainedFailure(tasks.error, tasks.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(tasks.data, () => heading.current);

  let body;
  if (tasks.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={`Loading the tasks of ${project}`} lines={4} />
      ) : (
        <ReadFailure
          title="The tasks could not be loaded"
          failure={failure}
          retrying={tasks.isFetching}
          attempt={tasks.errorUpdateCount}
          onRetry={() => {
            retried();
            void tasks.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else {
    body = (
      <>
        <TasksSummary project={project} answer={tasks.data} />
        {tasks.error !== null && (
          <p className="note" role="alert">
            The tasks could not be read again: {apiErrorOf(tasks.error).message}
          </p>
        )}
      </>
    );
  }

  return (
    <Region
      title="Tasks"
      boundary="tasks overview"
      headingRef={heading}
      busy={tasks.data === undefined && failure === null}
      link={
        <a className="home-open" href={sectionHash(project, "tasks")}>
          Open Tasks
          <Icon name="arrowRight" />
        </a>
      }
    >
      {body}
    </Region>
  );
}

import { useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useTasks } from "../api/queries";
import type { TaskList } from "../api/types";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { groupsOf } from "../tasks/groups";
import { specChanged, taskStatusLook } from "../tasks/labels";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ReadFailure, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { Count, Notes, Region, Stored } from "./parts";
import { HOME_ROWS, otherOpenOf, waitingOf } from "./tally";

/** The command a first task starts with: code, never a catalog value. */
const NEW_TASK_COMMAND = "spec task new --nodes REF...";

/** One answer of `spec task list`: what waits for the owner first, then the other open groups' sizes. */
function TasksSummary({ project, answer }: { project: string; answer: TaskList }) {
  const t = useTranslate();
  const notes = <Notes notes={answer.notes} label={t("home.tasks.notes")} />;
  if (answer.tasks.length === 0) {
    return (
      <>
        {notes}
        <div className="home-empty">
          <p>{t("home.tasks.none")}</p>
          <p>{fill(t("home.tasks.create", { command: slot("command") }), { command: <code>{NEW_TASK_COMMAND}</code> })}</p>
        </div>
      </>
    );
  }
  const groups = groupsOf(answer.tasks, t);
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
          <span>{t("home.tasks.waiting")}</span>
          <Count value={waiting.length} />
        </h3>
        {waiting.length === 0 ? (
          <p className="muted">{t("home.tasks.nothingWaits")}</p>
        ) : (
          <ul className="home-list">
            {shown.map((entry) => (
              <li key={entry.id} className="home-row">
                <a className="home-row-link" href={sectionHash(project, "tasks", entry.id)}>
                  <span className="mono">{entry.id}</span>: <span className={entry.title === null ? "muted" : undefined}>{entry.title ?? t("home.tasks.untitled")}</span>
                </a>
                <span className="home-row-meta">
                  <Badge name={t("home.tasks.state")} look={taskStatusLook(entry.status, t)} />
                  {entry.stale === true && <Badge look={specChanged(t)} />}
                  <Stored label={t("home.tasks.updated")} at={entry.updated_at} />
                </span>
              </li>
            ))}
          </ul>
        )}
        {waiting.length > shown.length && (
          <p className="home-more muted">{t("home.tasks.moreShown", { shown: shown.length, count: waiting.length })}</p>
        )}
      </div>
      <div className="home-block">
        <h3 className="home-block-title">{t("home.tasks.others")}</h3>
        {others.length === 0 ? (
          <p className="muted">{t("home.tasks.noOthers")}</p>
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
  const t = useTranslate();
  const tasks = useTasks(project);
  const failure = useRetainedFailure(tasks.error, tasks.isFetching);
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(tasks.data, () => heading.current);

  let body;
  if (tasks.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={t("home.tasks.loading", { project })} lines={4} />
      ) : (
        <ReadFailure
          title={t("home.tasks.failed")}
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
            {fill(t("home.tasks.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(tasks.error).message}</span> })}
          </p>
        )}
      </>
    );
  }

  return (
    <Region
      title={t("home.tasks.title")}
      boundary={t("home.tasks.boundary")}
      headingRef={heading}
      busy={tasks.data === undefined && failure === null}
      link={
        <a className="home-open" href={sectionHash(project, "tasks")}>
          {t("home.tasks.open")}
          <Icon name="arrowRight" />
        </a>
      }
    >
      {body}
    </Region>
  );
}

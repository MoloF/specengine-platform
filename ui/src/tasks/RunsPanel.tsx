import type { TaskPackage, TaskRun } from "../api/types";
import { Badge } from "../ui/Badge";
import { Prose } from "../markdown/Prose";
import { Icon } from "../ui/Icon";
import { runOutcomeLook } from "./labels";
import { Part, When } from "./parts";
import { useTranslate } from "../i18n";

/** Lists up to this long show open; longer ones open on demand. */
const SHOWN_OPEN = 10;

function RunItem({ project, run }: { project: string; run: TaskRun }) {
  const t = useTranslate();
  return (
    <li className="task-run">
      <h3 className="task-run-title">
        <span>{t("tasks.runs.run", { run: run.run })}</span>
        {run.ended_at === null ? (
          <span className="plain-tag running-tag">
            <Icon name="taskInProgress" />
            {t("tasks.runs.running")}
          </span>
        ) : run.outcome === null ? (
          <span className="plain-tag">{t("tasks.runs.noOutcome")}</span>
        ) : (
          <Badge name={t("tasks.runs.outcome")} look={runOutcomeLook(run.outcome, t)} />
        )}
      </h3>
      <dl className="pairs">
        <dt>{t("tasks.runs.role")}</dt>
        <dd className="mono run-role">{run.role}</dd>
        <dt>{t("tasks.runs.started")}</dt>
        <dd>
          <When at={run.started_at} />
        </dd>
        <dt>{t("tasks.runs.ended")}</dt>
        <dd>{run.ended_at === null ? t("tasks.runs.running") : <When at={run.ended_at} />}</dd>
      </dl>
      {run.summary !== null && <Prose className="task-text run-summary" source={run.summary} links={null} project={project} baseLevel={3} frontMatter={false} />}
      {run.changed_files.length === 0 ? (
        <p className="muted">{t("tasks.runs.noFiles")}</p>
      ) : (
        <details className="run-files" open={run.changed_files.length <= SHOWN_OPEN}>
          <summary>{t("tasks.runs.files", { count: run.changed_files.length })}</summary>
          <ul className="run-file-list mono">
            {run.changed_files.map((file, index) => (
              <li key={`${String(index)}-${file}`}>{file}</li>
            ))}
          </ul>
        </details>
      )}
    </li>
  );
}

/** The Runs tab: the claim, else "Not claimed"; each run by number, an open one "Running". */
export function RunsPanel({ project, task }: { project: string; task: TaskPackage }) {
  const t = useTranslate();
  const claim = task.claim;
  const runs = [...task.runs].sort((a, b) => a.run - b.run);
  return (
    <div className="task-panel">
      <Part title={t("tasks.runs.claim")}>
        {claim === null ? (
          <p className="muted">{t("tasks.runs.notClaimed")}</p>
        ) : (
          <dl className="pairs">
            <dt>{t("tasks.runs.claimed")}</dt>
            <dd>
              <When at={claim.at} />
            </dd>
            <dt>{t("tasks.runs.role")}</dt>
            <dd className="mono claim-role">{claim.role}</dd>
            <dt>{t("tasks.runs.worktree")}</dt>
            <dd className="mono">{claim.worktree}</dd>
            <dt>{t("tasks.runs.branch")}</dt>
            <dd className="mono">{claim.branch}</dd>
          </dl>
        )}
      </Part>
      <Part title={t("tasks.runs.title")} count={runs.length}>
        {runs.length === 0 ? (
          <p className="muted">{t("tasks.runs.none")}</p>
        ) : (
          <ol className="task-runs">
            {runs.map((run) => (
              <RunItem key={run.run} project={project} run={run} />
            ))}
          </ol>
        )}
      </Part>
    </div>
  );
}

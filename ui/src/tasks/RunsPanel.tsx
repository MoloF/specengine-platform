import type { TaskPackage, TaskRun } from "../api/types";
import { Badge } from "../ui/Badge";
import { Prose } from "../markdown/Prose";
import { Icon } from "../ui/Icon";
import { runOutcomeLook } from "./labels";
import { Part, When } from "./parts";

/** Lists up to this long show open; longer ones open on demand. */
const SHOWN_OPEN = 10;

function RunItem({ project, run }: { project: string; run: TaskRun }) {
  return (
    <li className="task-run">
      <h3 className="task-run-title">
        <span>Run {run.run}</span>
        {run.ended_at === null ? (
          <span className="plain-tag running-tag">
            <Icon name="taskInProgress" />
            Running
          </span>
        ) : run.outcome === null ? (
          <span className="plain-tag">No outcome reported</span>
        ) : (
          <Badge name="Outcome" look={runOutcomeLook(run.outcome)} />
        )}
      </h3>
      <dl className="pairs">
        <dt>Role</dt>
        <dd className="mono run-role">{run.role}</dd>
        <dt>Started</dt>
        <dd>
          <When at={run.started_at} />
        </dd>
        <dt>Ended</dt>
        <dd>{run.ended_at === null ? "Running" : <When at={run.ended_at} />}</dd>
      </dl>
      {run.summary !== null && <Prose className="task-text run-summary" source={run.summary} links={null} project={project} baseLevel={3} frontMatter={false} />}
      {run.changed_files.length === 0 ? (
        <p className="muted">No changed file reported.</p>
      ) : (
        <details className="run-files" open={run.changed_files.length <= SHOWN_OPEN}>
          <summary>
            {run.changed_files.length} changed {run.changed_files.length === 1 ? "file" : "files"}
          </summary>
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
  const claim = task.claim;
  const runs = [...task.runs].sort((a, b) => a.run - b.run);
  return (
    <div className="task-panel">
      <Part title="Claim">
        {claim === null ? (
          <p className="muted">Not claimed</p>
        ) : (
          <dl className="pairs">
            <dt>Claimed</dt>
            <dd>
              <When at={claim.at} />
            </dd>
            <dt>Role</dt>
            <dd className="mono claim-role">{claim.role}</dd>
            <dt>Worktree</dt>
            <dd className="mono">{claim.worktree}</dd>
            <dt>Branch</dt>
            <dd className="mono">{claim.branch}</dd>
          </dl>
        )}
      </Part>
      <Part title="Runs" count={runs.length}>
        {runs.length === 0 ? (
          <p className="muted">No run yet.</p>
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

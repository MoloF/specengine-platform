import type { TaskPackage, TaskProposal } from "../api/types";
import { sectionHash } from "../app/routes";
import { statusLook } from "../inbox/labels";
import { Badge } from "../ui/Badge";
import { Part } from "./parts";

function ProposalItems({ project, proposals }: { project: string; proposals: readonly TaskProposal[] }) {
  if (proposals.length === 0) {
    return <p className="muted">None.</p>;
  }
  return (
    <ul className="task-proposals">
      {proposals.map((proposal) => (
        <li key={proposal.id} className="task-proposal">
          <p className="task-proposal-head">
            <a className="mono" href={sectionHash(project, "inbox", proposal.id)}>
              {proposal.id}
            </a>
            <span className="kind-tag">{proposal.kind}</span>
            <Badge name="Status" look={statusLook(proposal.status)} />
            <span className="mono muted">
              <span className="sr-only">Targets: </span>
              {proposal.target_ids.join(", ")}
            </span>
          </p>
          <p className="task-text task-proposal-summary">{proposal.summary}</p>
        </li>
      ))}
    </ul>
  );
}

/**
 * The Proposals tab: the package's open proposals split by `task_id` alone, each linked to its
 * Inbox card. Information only: none of them holds the task (ADR-0012).
 */
export function TaskProposalsPanel({ project, task }: { project: string; task: TaskPackage }) {
  if (task.open_proposals.length === 0) {
    return (
      <div className="task-panel">
        <p className="muted">No open proposal.</p>
      </div>
    );
  }
  const raised = task.open_proposals.filter((proposal) => proposal.task_id === task.id);
  const others = task.open_proposals.filter((proposal) => proposal.task_id !== task.id);
  return (
    <div className="task-panel">
      <p className="muted">Information only: nothing waits on these. Each is decided on its own in the Inbox.</p>
      <Part title="Raised by this task" count={raised.length}>
        <ProposalItems project={project} proposals={raised} />
      </Part>
      <Part title="On its nodes" count={others.length}>
        <ProposalItems project={project} proposals={others} />
      </Part>
    </div>
  );
}

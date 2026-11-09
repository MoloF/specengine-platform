import type { TaskPackage, TaskProposal } from "../api/types";
import { sectionHash } from "../app/routes";
import { statusLook } from "../inbox/labels";
import { Badge } from "../ui/Badge";
import { Part } from "./parts";
import { useTranslate } from "../i18n";

function ProposalItems({ project, proposals }: { project: string; proposals: readonly TaskProposal[] }) {
  const t = useTranslate();
  if (proposals.length === 0) {
    return <p className="muted">{t("tasks.proposals.none")}</p>;
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
            <Badge name={t("tasks.proposals.status")} look={statusLook(proposal.status, t)} />
            <span className="mono muted">
              <span className="sr-only">{t("tasks.targetsSaid")}</span>
              {proposal.target_ids.join(", ")}
            </span>
          </p>
          {proposal.summary !== null && <p className="task-text task-proposal-summary">{proposal.summary}</p>}
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
  const t = useTranslate();
  if (task.open_proposals.length === 0) {
    return (
      <div className="task-panel">
        <p className="muted">{t("tasks.proposals.noneOpen")}</p>
      </div>
    );
  }
  const raised = task.open_proposals.filter((proposal) => proposal.task_id === task.id);
  const others = task.open_proposals.filter((proposal) => proposal.task_id !== task.id);
  return (
    <div className="task-panel">
      <p className="muted">{t("tasks.proposals.note")}</p>
      <Part title={t("tasks.proposals.raised")} count={raised.length}>
        <ProposalItems project={project} proposals={raised} />
      </Part>
      <Part title={t("tasks.proposals.others")} count={others.length}>
        <ProposalItems project={project} proposals={others} />
      </Part>
    </div>
  );
}

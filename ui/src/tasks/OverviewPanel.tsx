import type { TaskPackage, TaskTarget } from "../api/types";
import { sectionHash } from "../app/routes";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Prose } from "../markdown/Prose";
import { Part, When } from "./parts";

/** A target's name: its ID, else its path (an ID-less document). */
function nameOf(target: TaskTarget): string | null {
  return target.id ?? target.path;
}

/**
 * A target and where it opens. A null `kind` is a node the compared place no longer holds: its
 * stored name as text, no link.
 */
function Target({ project, target }: { project: string; target: TaskTarget }) {
  const t = useTranslate();
  const name = nameOf(target);
  return (
    <li className="task-target">
      <p className="task-target-head">
        <span className="mono">{name ?? t("tasks.overview.unnamed")}</span>
        {target.kind !== null && <span className="kind-tag">{target.kind}</span>}
        {target.title !== null && <span className="task-target-title">{target.title}</span>}
      </p>
      {target.id !== null && target.path !== null && <p className="mono muted task-target-path">{target.path}</p>}
      {target.kind !== null && name !== null && (
        <p className="task-target-links">
          <a href={sectionHash(project, "tree", name)}>
            {fill(t("tasks.overview.openInTree", { name: slot("name") }), { name: <span className="sr-only">{t("tasks.overview.nameSaid", { name })}</span> })}
          </a>
          <a href={sectionHash(project, "graph", name)}>
            {fill(t("tasks.overview.showInGraph", { name: slot("name") }), { name: <span className="sr-only">{t("tasks.overview.nameSaid", { name })}</span> })}
          </a>
        </p>
      )}
    </li>
  );
}

/**
 * The Overview tab: goal, criteria, targets, affected nodes, assumptions, owner notes, profile.
 * Goal, criterion, assumption and note texts render as markdown; names and paths stay verbatim.
 */
export function OverviewPanel({ project, task }: { project: string; task: TaskPackage }) {
  const t = useTranslate();
  return (
    <div className="task-panel">
      <Part title={t("tasks.overview.goal")}>
        {task.goal === null ? (
          <p className="muted">{t("tasks.overview.noGoal")}</p>
        ) : (
          <Prose className="task-text task-goal" source={task.goal} links={null} project={project} baseLevel={2} frontMatter={false} />
        )}
      </Part>

      <Part title={t("tasks.overview.criteria")} count={task.criteria.length}>
        {task.criteria.length === 0 ? (
          <p className="muted">{t("tasks.overview.noCriteria")}</p>
        ) : (
          <ol className="task-criteria">
            {task.criteria.map((criterion, index) => (
              <li key={`${String(index)}-${criterion.ref ?? ""}`} className="task-criterion">
                <p className="task-criterion-ref">
                  {criterion.ref === null ? (
                    <span className="plain-tag">{t("tasks.overview.freeText")}</span>
                  ) : (
                    <a className="mono" href={sectionHash(project, "tree", criterion.ref)}>
                      {criterion.ref}
                    </a>
                  )}
                </p>
                {criterion.text === null ? (
                  <p className="muted">{t("tasks.overview.notFound")}</p>
                ) : (
                  <Prose className="task-text criterion-text" source={criterion.text} links={null} project={project} baseLevel={2} frontMatter={false} />
                )}
              </li>
            ))}
          </ol>
        )}
      </Part>

      <Part title={t("tasks.overview.targets")} count={task.targets.length}>
        {task.targets.length === 0 ? (
          <p className="muted">{t("tasks.overview.noTargets")}</p>
        ) : (
          <ul className="task-targets">
            {task.targets.map((target, index) => (
              <Target key={`${String(index)}-${nameOf(target) ?? ""}`} project={project} target={target} />
            ))}
          </ul>
        )}
      </Part>

      <Part title={t("tasks.overview.affected")} count={task.affected_nodes.length}>
        {task.affected_nodes.length === 0 ? (
          <p className="muted">{t("tasks.overview.noneNamed")}</p>
        ) : (
          <ul className="task-names">
            {task.affected_nodes.map((id) => (
              <li key={id}>
                <a className="mono" href={sectionHash(project, "tree", id)}>
                  {id}
                </a>
              </li>
            ))}
          </ul>
        )}
      </Part>

      <Part title={t("tasks.overview.assumptions")} count={task.assumptions.length}>
        {task.assumptions.length === 0 ? (
          <p className="muted">{t("tasks.overview.noAssumptions")}</p>
        ) : (
          <ul className="task-assumptions">
            {task.assumptions.map((assumption) => (
              <li key={assumption.proposal} className="task-assumption">
                <a className="mono" href={sectionHash(project, "inbox", assumption.proposal)}>
                  {assumption.proposal}
                </a>
                <Prose className="task-text assumption-text" source={assumption.text} links={null} project={project} baseLevel={2} frontMatter={false} />
              </li>
            ))}
          </ul>
        )}
      </Part>

      <Part title={t("tasks.overview.ownerNotes")} count={task.owner_notes.length}>
        {task.owner_notes.length === 0 ? (
          <p className="muted">{t("tasks.overview.noNotes")}</p>
        ) : (
          <ol className="task-notes">
            {task.owner_notes.map((note, index) => (
              <li key={`${String(index)}-${note.at}`} className="task-note">
                <p className="task-note-at">
                  <When at={note.at} />
                </p>
                <Prose className="task-text owner-note-text" source={note.note} links={null} project={project} baseLevel={2} frontMatter={false} />
              </li>
            ))}
          </ol>
        )}
      </Part>

      <Part title={t("tasks.overview.profile")}>
        {task.profile === null ? <p className="muted">{t("tasks.overview.noProfile")}</p> : <p className="mono task-profile">{task.profile}</p>}
      </Part>
    </div>
  );
}

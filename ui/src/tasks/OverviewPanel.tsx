import type { TaskPackage, TaskTarget } from "../api/types";
import { sectionHash } from "../app/routes";
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
  const name = nameOf(target);
  return (
    <li className="task-target">
      <p className="task-target-head">
        <span className="mono">{name ?? "Unnamed target"}</span>
        {target.kind !== null && <span className="kind-tag">{target.kind}</span>}
        {target.title !== null && <span className="task-target-title">{target.title}</span>}
      </p>
      {target.id !== null && target.path !== null && <p className="mono muted task-target-path">{target.path}</p>}
      {target.kind !== null && name !== null && (
        <p className="task-target-links">
          <a href={sectionHash(project, "tree", name)}>
            Open in spec tree<span className="sr-only">: {name}</span>
          </a>
          <a href={sectionHash(project, "graph", name)}>
            Show in graph<span className="sr-only">: {name}</span>
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
  return (
    <div className="task-panel">
      <Part title="Goal">
        {task.goal === null ? (
          <p className="muted">No goal given.</p>
        ) : (
          <Prose className="task-text task-goal" source={task.goal} links={null} project={project} baseLevel={2} frontMatter={false} />
        )}
      </Part>

      <Part title="Criteria" count={task.criteria.length}>
        {task.criteria.length === 0 ? (
          <p className="muted">No criteria yet.</p>
        ) : (
          <ol className="task-criteria">
            {task.criteria.map((criterion, index) => (
              <li key={`${String(index)}-${criterion.ref ?? ""}`} className="task-criterion">
                <p className="task-criterion-ref">
                  {criterion.ref === null ? (
                    <span className="plain-tag">Free text</span>
                  ) : (
                    <a className="mono" href={sectionHash(project, "tree", criterion.ref)}>
                      {criterion.ref}
                    </a>
                  )}
                </p>
                {criterion.text === null ? (
                  <p className="muted">Not found in the compared place</p>
                ) : (
                  <Prose className="task-text criterion-text" source={criterion.text} links={null} project={project} baseLevel={2} frontMatter={false} />
                )}
              </li>
            ))}
          </ol>
        )}
      </Part>

      <Part title="Targets" count={task.targets.length}>
        {task.targets.length === 0 ? (
          <p className="muted">No target named.</p>
        ) : (
          <ul className="task-targets">
            {task.targets.map((target, index) => (
              <Target key={`${String(index)}-${nameOf(target) ?? ""}`} project={project} target={target} />
            ))}
          </ul>
        )}
      </Part>

      <Part title="Affected nodes" count={task.affected_nodes.length}>
        {task.affected_nodes.length === 0 ? (
          <p className="muted">None named.</p>
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

      <Part title="Assumptions" count={task.assumptions.length}>
        {task.assumptions.length === 0 ? (
          <p className="muted">No working assumption.</p>
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

      <Part title="Owner notes" count={task.owner_notes.length}>
        {task.owner_notes.length === 0 ? (
          <p className="muted">No note from you yet.</p>
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

      <Part title="Profile">
        {task.profile === null ? <p className="muted">No profile</p> : <p className="mono task-profile">{task.profile}</p>}
      </Part>
    </div>
  );
}

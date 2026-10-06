import type { TaskPackage } from "../api/types";
import { sectionHash } from "../app/routes";
import { removesAll } from "../ui/diff";
import { DiffView } from "../ui/DiffView";
import { Icon } from "../ui/Icon";
import { FROZEN_AT_APPROVAL } from "./labels";
import { Part, When } from "./parts";

/** The cut the core makes in a diff (`docs/features/task-package.md` "Data"). */
export const CUT_NOTICE = "Diff cut by SpecEngine at 8 192 bytes; the file in the worktree holds the rest.";

/** What a node whose diff removes it says in place of a link. */
export const REMOVED = "Removed since approval";

/** Node lists up to this long show open; longer ones open on demand. */
const SHOWN_OPEN = 10;

/**
 * A node the spec tree can open, linked; one its diff removes (gone from the compared place,
 * task-package "Data") as text with what happened to it, never a link to nothing.
 */
function NodeName({ project, id, removed }: { project: string; id: string; removed: boolean }) {
  if (removed) {
    return (
      <>
        <span className="mono">{id}</span> <span className="plain-tag">{REMOVED}</span>
      </>
    );
  }
  return (
    <a className="mono" href={sectionHash(project, "tree", id)}>
      {id}
    </a>
  );
}

/**
 * The Spec changes tab: the snapshot's place and nodes, then each `snapshot_diff` entry in the
 * daemon's order, its hunks as sent. Nothing is compared here: the diffs and `stale` are the core's.
 */
export function SpecChangesPanel({ project, task }: { project: string; task: TaskPackage }) {
  const snapshot = task.spec_snapshot;
  if (snapshot === null) {
    return (
      <div className="task-panel">
        <div className="empty-state">
          <p>{FROZEN_AT_APPROVAL}</p>
          <p className="muted">This task has no approval yet, so there is nothing to compare.</p>
        </div>
      </div>
    );
  }
  const diffs = task.snapshot_diff;
  const removed = new Set((diffs ?? []).filter((entry) => removesAll(entry.diff)).map((entry) => entry.id));
  return (
    <div className="task-panel">
      <Part title="Frozen at approval">
        <dl className="pairs">
          <dt>When</dt>
          <dd>
            <When at={snapshot.at} />
          </dd>
          <dt>Worktree</dt>
          <dd className="mono">{snapshot.place.worktree}</dd>
          <dt>Spec root</dt>
          <dd className="mono">{snapshot.place.root_rel === "" ? "(the worktree's root)" : snapshot.place.root_rel}</dd>
          <dt>Branch</dt>
          <dd className="mono">{snapshot.place.branch}</dd>
          <dt>Commit</dt>
          <dd className="mono">{snapshot.place.commit}</dd>
        </dl>
        <details className="snapshot-nodes" open={snapshot.nodes.length <= SHOWN_OPEN}>
          <summary>
            {snapshot.nodes.length} {snapshot.nodes.length === 1 ? "node" : "nodes"} frozen
          </summary>
          <ul className="snapshot-node-list">
            {snapshot.nodes.map((node) => (
              <li key={`${node.id}:${node.path}`}>
                <NodeName project={project} id={node.id} removed={removed.has(node.id)} />{" "}
                <span className="mono muted">{node.path}</span> <span className="mono muted hash">{node.span_hash}</span>
              </li>
            ))}
          </ul>
        </details>
      </Part>

      <Part title="Changes since approval" count={diffs?.length}>
        {diffs === null ? (
          <div className="notice notice-unknown">
            <p className="notice-title">
              <Icon name="unknown" />
              <span>Unknown: the spec could not be compared.</span>
            </p>
            {task.notes.map((note, index) => (
              <p key={`${String(index)}-${note}`} className="verbatim">
                {note}
              </p>
            ))}
          </div>
        ) : diffs.length === 0 ? (
          <p className="muted">No change since approval.</p>
        ) : (
          <ol className="snapshot-diffs">
            {diffs.map((entry, index) => (
              <li key={`${String(index)}-${entry.id}`} className="snapshot-diff" data-node={entry.id}>
                <p className="snapshot-diff-head">
                  <NodeName project={project} id={entry.id} removed={removesAll(entry.diff)} />
                  <span className="mono muted">{entry.path}</span>
                </p>
                <p className="mono muted hash">At approval: {entry.span_hash}</p>
                <DiffView diff={entry.diff} label={`Changes to ${entry.id} since approval`} />
                {entry.cut && (
                  <p className="note cut-note">
                    <Icon name="info" />
                    <span>{CUT_NOTICE}</span>
                  </p>
                )}
              </li>
            ))}
          </ol>
        )}
      </Part>
    </div>
  );
}

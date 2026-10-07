import type { SnapshotDiff, TaskPackage } from "../api/types";
import { sectionHash } from "../app/routes";
import { removesAll } from "../ui/diff";
import { DiffView } from "../ui/DiffView";
import { Icon } from "../ui/Icon";
import { FROZEN_AT_APPROVAL } from "./labels";
import { Part, When } from "./parts";

/** The cut the core makes in a diff (`docs/features/task-package.md` "Data"). */
export const CUT_NOTICE = "Diff cut by SpecEngine at 8 192 bytes; the file in the worktree holds the rest.";

/** An entry past the package's total: `diff` null, `cut` true (`docs/features/task-package.md` "Data"). */
export const TOTAL_CUT_NOTICE = "Diff cut: the package's diffs reached their size cap (262 144 bytes in all); the file in the worktree holds this change.";

/** An entry with no diff and no cut: the daemon's notes on the task say why. */
export const NO_DIFF_NOTICE = "No diff sent for this node; the daemon's notes on this task say why.";

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

/** Whether the entry's diff, as sent, removes the node's whole text; an entry with no diff tells nothing. */
function removes(entry: SnapshotDiff): boolean {
  return entry.diff !== null && removesAll(entry.diff);
}

/** A quiet line in place of hunks or under them. */
function DiffNote({ text }: { text: string }) {
  return (
    <p className="note cut-note">
      <Icon name="info" />
      <span>{text}</span>
    </p>
  );
}

/** An entry's hunks as sent with the core's cut under them, or, with none sent, why. */
function EntryDiff({ entry }: { entry: SnapshotDiff }) {
  if (entry.diff === null) {
    return <DiffNote text={entry.cut ? TOTAL_CUT_NOTICE : NO_DIFF_NOTICE} />;
  }
  return (
    <>
      <DiffView diff={entry.diff} label={`Changes to ${entry.id} since approval`} />
      {entry.cut && <DiffNote text={CUT_NOTICE} />}
    </>
  );
}

/**
 * The Spec changes tab: the snapshot's place and nodes, then each `snapshot_diff` entry in the
 * daemon's order, its hunks as sent (past the package's total, a line saying so). Nothing is
 * compared here: the diffs and `stale` are the core's.
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
  const removed = new Set((diffs ?? []).filter(removes).map((entry) => entry.id));
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
                  <NodeName project={project} id={entry.id} removed={removes(entry)} />
                  <span className="mono muted">{entry.path}</span>
                </p>
                <p className="mono muted hash">At approval: {entry.span_hash}</p>
                <EntryDiff entry={entry} />
              </li>
            ))}
          </ol>
        )}
      </Part>
    </div>
  );
}

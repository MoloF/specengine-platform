import type { SnapshotDiff, TaskPackage } from "../api/types";
import { DocLink } from "../doc/DocLink";
import { removesAll } from "../ui/diff";
import { DiffView } from "../ui/DiffView";
import { Icon } from "../ui/Icon";
import { frozenAtApproval } from "./labels";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Part, When } from "./parts";

/** Node lists up to this long show open; longer ones open on demand. */
const SHOWN_OPEN = 10;

/**
 * A node the spec tree can open, linked; one its diff removes (gone from the compared place,
 * `docs/canon/task-package.md` "Staleness") as text with what happened to it, never a link to nothing.
 */
function NodeName({ project, id, removed }: { project: string; id: string; removed: boolean }) {
  const t = useTranslate();
  if (removed) {
    return (
      <>
        <span className="mono">{id}</span> <span className="plain-tag">{t("tasks.changes.removed")}</span>
      </>
    );
  }
  return (
    <DocLink className="mono" project={project} docRef={id}>
      {id}
    </DocLink>
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
  const t = useTranslate();
  if (entry.diff === null) {
    return <DiffNote text={entry.cut ? t("tasks.changes.totalCut") : t("tasks.changes.noDiff")} />;
  }
  return (
    <>
      <DiffView diff={entry.diff} label={t("tasks.changes.diffLabel", { id: entry.id })} />
      {entry.cut && <DiffNote text={t("tasks.changes.cut")} />}
    </>
  );
}

/**
 * The Spec changes tab: the snapshot's place and nodes, then each `snapshot_diff` entry in the
 * daemon's order, its hunks as sent (past the package's total, a line saying so). Nothing is
 * compared here: the diffs and `stale` are the core's.
 */
export function SpecChangesPanel({ project, task }: { project: string; task: TaskPackage }) {
  const t = useTranslate();
  const snapshot = task.spec_snapshot;
  if (snapshot === null) {
    return (
      <div className="task-panel">
        <div className="empty-state">
          <p>{frozenAtApproval(t)}</p>
          <p className="muted">{t("tasks.changes.noApproval")}</p>
        </div>
      </div>
    );
  }
  const diffs = task.snapshot_diff;
  const removed = new Set((diffs ?? []).filter(removes).map((entry) => entry.id));
  return (
    <div className="task-panel">
      <Part title={t("tasks.changes.frozen")}>
        <dl className="pairs">
          <dt>{t("tasks.changes.when")}</dt>
          <dd>
            <When at={snapshot.at} />
          </dd>
          <dt>{t("tasks.runs.worktree")}</dt>
          <dd className="mono">{snapshot.place.worktree}</dd>
          <dt>{t("tasks.changes.root")}</dt>
          <dd className="mono">{snapshot.place.root_rel === "" ? t("tasks.changes.worktreeRoot") : snapshot.place.root_rel}</dd>
          <dt>{t("tasks.runs.branch")}</dt>
          <dd className="mono">{snapshot.place.branch}</dd>
          <dt>{t("tasks.changes.commit")}</dt>
          <dd className="mono">{snapshot.place.commit}</dd>
        </dl>
        <details className="snapshot-nodes" open={snapshot.nodes.length <= SHOWN_OPEN}>
          <summary>{t("tasks.changes.nodesFrozen", { count: snapshot.nodes.length })}</summary>
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

      <Part title={t("tasks.changes.since")} count={diffs?.length}>
        {diffs === null ? (
          <div className="notice notice-unknown">
            <p className="notice-title">
              <Icon name="unknown" />
              <span>{t("tasks.changes.unknown")}</span>
            </p>
            {task.notes.map((note, index) => (
              <p key={`${String(index)}-${note}`} className="verbatim" lang="en">
                {note}
              </p>
            ))}
          </div>
        ) : diffs.length === 0 ? (
          <p className="muted">{t("tasks.changes.none")}</p>
        ) : (
          <ol className="snapshot-diffs">
            {diffs.map((entry, index) => (
              <li key={`${String(index)}-${entry.id}`} className="snapshot-diff" data-node={entry.id}>
                <p className="snapshot-diff-head">
                  <NodeName project={project} id={entry.id} removed={removes(entry)} />
                  <span className="mono muted">{entry.path}</span>
                </p>
                <p className="mono muted hash">{fill(t("tasks.changes.atApproval", { hash: slot("hash") }), { hash: entry.span_hash })}</p>
                <EntryDiff entry={entry} />
              </li>
            ))}
          </ol>
        )}
      </Part>
    </div>
  );
}

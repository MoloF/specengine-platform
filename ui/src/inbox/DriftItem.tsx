import { useId } from "react";
import type { DriftEvidence } from "../api/types";
import { CopyButton } from "../ui/CopyButton";
import { Icon } from "../ui/Icon";
import { Section } from "../ui/states";
import { driftCommands, driftRowsText, driftRowText } from "./drift";

const COPY_FAILED = "Copy failed: select the command and copy it";

/**
 * A drift item's decision, in place of the four decision buttons: it is decided on a terminal
 * only, in the worktree where it was raised, and nothing is staged for it
 * (`docs/features/drift-queue.md` "Review, inbox, round, stage", "Approve"). Its commands, each with
 * Copy; nothing waits on the item.
 */
export function TerminalDecision({ id, worktree }: { id: string; worktree: string | null }) {
  const headingId = useId();
  const commands = driftCommands(id);
  return (
    <section className="owner-commands terminal-decision" aria-labelledby={headingId}>
      <h3 id={headingId} className="owner-commands-title">
        <Icon name="terminal" />
        <span>Decided on a terminal</span>
      </h3>
      <p className="owner-commands-note">
        A drift item is decided only on a terminal, in the worktree where it was raised
        {worktree !== null && (
          <>
            {" ("}
            <span className="mono">{worktree}</span>)
          </>
        )}
        , which asks [y/N]. Nothing is staged here, and nothing waits on it.
      </p>
      {commands === null ? (
        <p className="muted">No command for this ID</p>
      ) : (
        <ul className="command-list">
          {commands.map((command) => (
            <li key={command.action} className="command" data-action={command.action}>
              <span className="command-meaning">{command.meaning}</span>
              <code className="command-text">{command.text}</code>
              <CopyButton text={command.text} context={`the command: ${command.text}`} failed={COPY_FAILED} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/**
 * A drift item's evidence as the review document sends it: how many rows, the commit judged, the
 * evidence hash, and each stored row as its compact JSON, by key (the drift screen is Phase 4's).
 */
export function DriftSection({ drift, commit }: { drift: DriftEvidence; commit: string | null }) {
  return (
    <Section title="Drift evidence">
      <p>Bindings of the node that spec verify finds out of step with spec.lock, as judged when the item was raised.</p>
      <dl className="pairs">
        <dt>Rows</dt>
        <dd>{driftRowsText(drift)}</dd>
        <dt>Commit judged</dt>
        <dd className="mono">{commit ?? "Unknown"}</dd>
        <dt>Evidence hash</dt>
        <dd className="mono">{drift.hash}</dd>
      </dl>
      {drift.rows.length > 0 && (
        <ol className="drift-rows" aria-label="Rows as spec verify --json writes them">
          {drift.rows.map((row, index) => (
            <li key={index}>
              <code>{driftRowText(row)}</code>
            </li>
          ))}
        </ol>
      )}
    </Section>
  );
}

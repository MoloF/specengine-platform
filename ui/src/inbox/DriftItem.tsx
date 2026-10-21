import { useId } from "react";
import type { DriftEvidence } from "../api/types";
import { DriftLine } from "../health/DriftPart";
import { CopyButton } from "../ui/CopyButton";
import { Icon } from "../ui/Icon";
import { Section } from "../ui/states";
import { shownText } from "../ui/text";
import { driftCommands, driftRowsText, driftRowText, storedRowOf } from "./drift";

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
 * A stored row not in the shape `storedRowOf` reads: its compact JSON, as sent, still counted; a
 * control or bidirectional mark JSON leaves as is written `\u{..}` (`shownText`), as every field.
 */
function RawRow({ row }: { row: unknown }) {
  return (
    <li className="drift-raw" data-raw="">
      <span className="drift-raw-note">Shown as sent: not in the shape this screen reads.</span>
      <code>{shownText(driftRowText(row))}</code>
    </li>
  );
}

/**
 * A drift item's evidence as the review document sends it (docs/canon/drift-screens.md): how many
 * rows, the commit judged, the evidence hash, then each stored row in the stored order, never
 * sorted: typed through Health's row component with its markers, revisions, lock, comparison and
 * levels, a `spec_ahead same_revision` row marked Editorial; a row of another shape as its compact
 * JSON, still counted.
 */
export function DriftSection({ project, drift, commit }: { project: string; drift: DriftEvidence; commit: string | null }) {
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
        <ol className="drift-rows" aria-label="Stored rows, in spec verify's order">
          {drift.rows.map((row, index) => {
            const typed = storedRowOf(row);
            // The stored order is the rows' identity: two rows may share a node and a place.
            return typed === null ? <RawRow key={index} row={row} /> : <DriftLine key={index} project={project} row={typed} markEditorial />;
          })}
        </ol>
      )}
    </Section>
  );
}

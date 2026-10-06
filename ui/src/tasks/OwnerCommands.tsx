import { useId } from "react";
import type { TaskStatus } from "../api/types";
import { CopyButton } from "../ui/CopyButton";
import { Icon } from "../ui/Icon";
import { ownerCommands } from "./actions";

/** Worded as the daemon's refusal of a browser write (docs/features/daemon-read.md). */
export const COMMANDS_NOTE =
  "Owner actions run on a terminal, which asks [y/N]; nothing changes here. Approval freezes the spec in the worktree where you run it.";

const COPY_FAILED = "Copy failed: select the command and copy it";

/**
 * The owner's commands for a task: its state's actions (src/tasks/actions.ts), then `spec task
 * show`, each to copy into a terminal. The browser changes nothing. `status` null: only the read.
 */
export function OwnerCommands({ id, status }: { id: string; status: TaskStatus | null }) {
  const headingId = useId();
  const commands = ownerCommands(id, status);
  return (
    <section className="owner-commands" aria-labelledby={headingId}>
      <h2 id={headingId} className="owner-commands-title">
        <Icon name="terminal" />
        <span>Owner commands</span>
      </h2>
      <p className="owner-commands-note">{COMMANDS_NOTE}</p>
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

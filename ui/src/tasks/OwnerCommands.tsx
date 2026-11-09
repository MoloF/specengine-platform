import { useId } from "react";
import type { TaskStatus } from "../api/types";
import { CopyButton } from "../ui/CopyButton";
import { Icon } from "../ui/Icon";
import { useTranslate } from "../i18n";
import { ownerCommands } from "./actions";

/**
 * The owner's commands for a task: its state's actions (src/tasks/actions.ts), then `spec task
 * show`, each to copy into a terminal. The browser changes nothing. `status` null: only the read.
 */
export function OwnerCommands({ id, status }: { id: string; status: TaskStatus | null }) {
  const t = useTranslate();
  const headingId = useId();
  const commands = ownerCommands(id, status, t);
  return (
    <section className="owner-commands" aria-labelledby={headingId}>
      <h2 id={headingId} className="owner-commands-title">
        <Icon name="terminal" />
        <span>{t("tasks.command.title")}</span>
      </h2>
      {/* Worded as the daemon's refusal of a browser write (docs/features/daemon-read.md). */}
      <p className="owner-commands-note">{t("tasks.command.note")}</p>
      {commands === null ? (
        <p className="muted">{t("tasks.command.none")}</p>
      ) : (
        <ul className="command-list">
          {commands.map((command) => (
            <li key={command.action} className="command" data-action={command.action}>
              <span className="command-meaning">{command.meaning}</span>
              <code className="command-text">{command.text}</code>
              <CopyButton text={command.text} context={t("tasks.copyContext", { command: command.text })} failed={t("tasks.copyFailed")} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

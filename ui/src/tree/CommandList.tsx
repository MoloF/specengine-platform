import { useTranslate } from "../i18n";
import { CopyButton } from "../ui/CopyButton";
import type { NodeCommand } from "./commands";

/**
 * Commands for a terminal, in the order given: each its meaning, the command exactly as copied and
 * Copy (`tree/commands.ts`: fixed words and a checked ID). The Bindings tab's commands and the lock
 * diff's rows alike; nothing runs from here.
 */
export function CommandList({ commands, label }: { commands: readonly NodeCommand[]; label: string }) {
  const t = useTranslate();
  return (
    <ul className="command-list" aria-label={label}>
      {commands.map((command) => (
        <li key={command.action} className="command" data-action={command.action}>
          <span className="command-meaning">{command.meaning}</span>
          <code className="command-text">{command.text}</code>
          <CopyButton text={command.text} context={t("tree.commands.copyContext", { command: command.text })} failed={t("tree.commands.copyFailed")} />
        </li>
      ))}
    </ul>
  );
}

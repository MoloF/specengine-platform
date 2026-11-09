import { useTranslate, type Translate } from "../i18n";
import { diffLines, type DiffLineType } from "./diff";

/** What a screen reader hears before a changed line: its sign in words. */
function spokenOf(type: DiffLineType, t: Translate): string | undefined {
  if (type === "added") {
    return t("ui.diff.added");
  }
  return type === "removed" ? t("ui.diff.removed") : undefined;
}

/**
 * A read-only diff exactly as the daemon sent it: signs kept, lines styled, wrapped. The label sits
 * on a wrapping figure (ARIA 1.2 forbids naming a bare `pre`). The Inbox's section diff and a
 * task's spec changes since approval.
 */
export function DiffView({ diff, label }: { diff: string | null; label?: string }) {
  const t = useTranslate();
  if (diff === null) {
    return <p className="muted">{t("ui.diff.none")}</p>;
  }
  return (
    <figure className="diff-figure" aria-label={label ?? t("ui.diff.label")}>
      <pre className="diff">
        <code>
          {diffLines(diff).map((line, index) => {
            const spoken = spokenOf(line.type, t);
            return (
              <span key={index} className={`diff-line diff-${line.type}`} data-line={line.type}>
                {spoken !== undefined && <span className="sr-only">{spoken}</span>}
                {line.text}
                {"\n"}
              </span>
            );
          })}
        </code>
      </pre>
    </figure>
  );
}

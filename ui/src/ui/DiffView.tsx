import { diffLines, type DiffLineType } from "./diff";

const SPOKEN: Partial<Record<DiffLineType, string>> = { added: "Added: ", removed: "Removed: " };

/**
 * A read-only diff exactly as the daemon sent it: signs kept, lines styled, wrapped. The label sits
 * on a wrapping figure (ARIA 1.2 forbids naming a bare `pre`). The Inbox's section diff and a
 * task's spec changes since approval.
 */
export function DiffView({ diff, label = "Section diff" }: { diff: string | null; label?: string }) {
  if (diff === null) {
    return <p className="muted">No section diff attached.</p>;
  }
  return (
    <figure className="diff-figure" aria-label={label}>
      <pre className="diff">
        <code>
          {diffLines(diff).map((line, index) => {
            const spoken = SPOKEN[line.type];
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

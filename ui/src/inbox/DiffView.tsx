import { diffLines, type DiffLineType } from "./diff";

const SPOKEN: Partial<Record<DiffLineType, string>> = { added: "Added: ", removed: "Removed: " };

/**
 * The read-only section diff exactly as the daemon sent it: signs kept, lines styled, wrapped. The
 * label sits on a wrapping figure (ARIA 1.2 forbids naming a bare `pre`).
 */
export function DiffView({ diff }: { diff: string | null }) {
  if (diff === null) {
    return <p className="muted">No section diff attached.</p>;
  }
  return (
    <figure className="diff-figure" aria-label="Section diff">
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

import { memo, useEffect, useMemo, useRef, type CSSProperties } from "react";

/** Where a jump lands: a line of the text; a new `nonce` jumps again to the same line. */
export interface LineTarget {
  line: number;
  nonce: number;
}

/**
 * A text's lines, each keeping its line end, so that joined they are the text exactly: a final
 * line end opens no extra line, an empty text is one empty line.
 */
export function splitLines(text: string): string[] {
  const parts = text.split("\n");
  const lines = parts.map((part, index) => (index < parts.length - 1 ? `${part}\n` : part));
  if (lines.length > 1 && lines[lines.length - 1] === "") {
    lines.pop();
  }
  return lines;
}

/**
 * A node's text verbatim: one row per line, numbered from `firstLine`, wrapped, never parsed as
 * markdown and never trimmed. The numbers sit in their own cells, out of a copy and of the text.
 * A jump focuses its line and brings it into view. Memoised: a long text renders again only when
 * it, its numbering or its jump changes.
 */
export const TextView = memo(function TextView({
  text,
  firstLine,
  label,
  target,
}: {
  text: string;
  firstLine: number;
  label: string;
  target: LineTarget | null;
}) {
  const container = useRef<HTMLPreElement>(null);
  const lines = useMemo(() => splitLines(text), [text]);
  const last = firstLine + lines.length - 1;

  useEffect(() => {
    if (target === null) {
      return;
    }
    const row = container.current?.querySelector<HTMLElement>(`[data-line="${String(target.line)}"]`);
    row?.scrollIntoView({ block: "center" });
    row?.focus();
  }, [target]);

  return (
    <figure className="text-figure">
      <figcaption className="sr-only">{label}</figcaption>
      <pre ref={container} className="text-view" style={{ "--line-digits": String(String(last).length) } as CSSProperties}>
        {lines.map((content, index) => {
          const line = firstLine + index;
          const isTarget = target?.line === line;
          return (
            <span
              key={line}
              className="text-line"
              data-line={line}
              data-target={isTarget ? "" : undefined}
              tabIndex={isTarget ? -1 : undefined}
            >
              <span className="text-line-number" aria-hidden="true">
                {line}
              </span>
              <span className="text-line-content">{content}</span>
            </span>
          );
        })}
      </pre>
    </figure>
  );
});

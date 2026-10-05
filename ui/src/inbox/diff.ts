/** How a line of pre-computed unified hunks is styled; its text, sign included, is kept verbatim. */
export type DiffLineType = "file" | "hunk" | "added" | "removed" | "context" | "note";

export interface DiffLine {
  type: DiffLineType;
  text: string;
}

/**
 * Classifies the lines of a unified diff as the daemon sent it. File headers only before the
 * first `@@`; nothing is computed or reordered. A final newline ends the last line: it is not a
 * line of its own (one is dropped, so a real trailing empty context line survives).
 */
export function diffLines(diff: string): DiffLine[] {
  let inHunk = false;
  const texts = diff.split("\n");
  if (texts.length > 1 && texts[texts.length - 1] === "") {
    texts.pop();
  }
  return texts.map((text) => {
    if (text.startsWith("@@")) {
      inHunk = true;
      return { type: "hunk", text };
    }
    if (!inHunk && (text.startsWith("--- ") || text.startsWith("+++ "))) {
      return { type: "file", text };
    }
    if (text.startsWith("+")) {
      return { type: "added", text };
    }
    if (text.startsWith("-")) {
      return { type: "removed", text };
    }
    if (text.startsWith("\\")) {
      return { type: "note", text };
    }
    return { type: "context", text };
  });
}

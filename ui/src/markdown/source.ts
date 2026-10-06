// Pure readings of a text the renderer needs before and beside the markdown parse
// (docs/features/ui-markdown.md "Description and interactions"):
// - the front-matter block, split as the core splits it (`crates/specengine-core/src/front_matter.rs`
//   `split`): the first line after an optional BOM exactly `---`, closed by the next line exactly
//   `---`, `\n` or `\r\n` endings; unclosed, the whole text is body;
// - where a link's destination is written, scanned as the core scans it
//   (`crates/specengine-core/src/markdown.rs` `inline_destination`, `definition_destination`,
//   `destination_at`), so a link is matched to the daemon's links read by the bytes it holds as
//   `written`, never by a path, slug or decoding of the UI's own.

/** A text split at its front-matter block. */
export interface SplitText {
  /** The block verbatim, from the opening `---` through the closing line's ending; null when none. */
  block: string | null;
  /** How many lines the block takes: the body's first line is the text's line `lines + 1`. */
  lines: number;
  /** What follows the block (the whole text when there is none). */
  body: string;
}

/** The line starting at `at` without its ending, and where the next starts; null past the end. */
function lineAt(text: string, at: number): { line: string; next: number } | null {
  if (at >= text.length) {
    return null;
  }
  const newline = text.indexOf("\n", at);
  const end = newline < 0 ? text.length : newline;
  const line = text.slice(at, end);
  return { line: line.endsWith("\r") ? line.slice(0, -1) : line, next: newline < 0 ? text.length : newline + 1 };
}

/** The front-matter block of a document's text, if it opens with one that closes. */
export function splitFrontMatter(text: string): SplitText {
  const start = text.startsWith("\uFEFF") ? 1 : 0;
  const whole: SplitText = { block: null, lines: 0, body: text };
  const first = lineAt(text, start);
  if (first?.line !== "---") {
    return whole;
  }
  let lines = 1;
  let next = first.next;
  for (let current = lineAt(text, next); current !== null; current = lineAt(text, next)) {
    lines += 1;
    next = current.next;
    if (current.line === "---") {
      return { block: text.slice(start, next), lines, body: text.slice(next) };
    }
  }
  return whole;
}

/** A destination's place in the source: `[start, end)`, `<` and `>` excluded. */
export interface Destination {
  start: number;
  end: number;
}

/** Space, tab, vertical tab, form feed: the blanks passed before a destination on its line. */
function isBlankNoEol(unit: string | undefined): boolean {
  return unit === " " || unit === "\t" || unit === "\v" || unit === "\f";
}

const ASCII_PUNCTUATION = /^[!-/:-@[-`{-~]$/;

/**
 * The destination after optional blanks and at most one line ending (container `>` markers passed
 * after it) at `from`, before `end`: inside `<...>`, else the bare run up to a character at or
 * below U+0020 or, in a link, an unbalanced `)`.
 */
function destinationAt(source: string, from: number, end: number, inLink: boolean): Destination | null {
  let at = from;
  while (at < end && isBlankNoEol(source[at])) {
    at += 1;
  }
  if (at < end && (source[at] === "\r" || source[at] === "\n")) {
    if (source[at] === "\r" && source[at + 1] === "\n") {
      at += 1;
    }
    at += 1;
    while (at < end && (isBlankNoEol(source[at]) || source[at] === ">")) {
      at += 1;
    }
  }
  if (at >= end) {
    return null;
  }
  if (source[at] === "<") {
    const start = at + 1;
    let close = start;
    while (close < end) {
      const unit = source[close];
      if (unit === "\\") {
        close += 2;
      } else if (unit === ">") {
        return { start, end: close };
      } else if (unit === "<" || unit === "\r" || unit === "\n") {
        return null;
      } else {
        close += 1;
      }
    }
    return null;
  }
  const start = at;
  let depth = 0;
  while (at < end) {
    const unit = source[at] ?? "";
    if (unit === "\\" && ASCII_PUNCTUATION.test(source[at + 1] ?? "")) {
      at += 2;
      continue;
    }
    if (unit.charCodeAt(0) <= 0x20) {
      break;
    }
    if (inLink) {
      if (unit === "(") {
        depth += 1;
      } else if (unit === ")") {
        if (depth === 0) {
          break;
        }
        depth -= 1;
      }
    }
    at += 1;
  }
  return at > start ? { start, end: Math.min(at, end) } : null;
}

/**
 * The destination of the inline link spanning `[start, end)` (`[` to `)`): after the first `](` at
 * or past `textEnd`, where the link text's last part ends (brackets and code spans in the text are
 * passed over).
 */
export function inlineDestination(source: string, start: number, end: number, textEnd: number): Destination | null {
  const stop = Math.min(end, source.length);
  for (let at = Math.max(textEnd, start + 1); at + 1 < stop; at += 1) {
    if (source[at] === "]" && source[at + 1] === "(") {
      return destinationAt(source, at + 2, stop, true);
    }
  }
  return null;
}

/** The destination of the reference definition spanning `[start, end)`: after its label's `]:`. */
export function definitionDestination(source: string, start: number, end: number): Destination | null {
  const stop = Math.min(end, source.length);
  let at = start + 1;
  while (at < stop) {
    const unit = source[at];
    if (unit === "\\") {
      at += 2;
    } else if (unit === "]") {
      return source[at + 1] === ":" ? destinationAt(source, at + 2, stop, false) : null;
    } else {
      at += 1;
    }
  }
  return null;
}

/** Whether a destination names a place in the project: not empty, not `//`-led, no URI scheme. */
export function isLocalDestination(destination: string): boolean {
  const trimmed = destination.trim();
  if (trimmed === "" || trimmed.startsWith("//") || /^[A-Za-z][A-Za-z0-9+.-]*:/.test(trimmed)) {
    return false;
  }
  // `#` alone or `?...` alone names no path and no anchor.
  const hash = destination.indexOf("#");
  const before = hash < 0 ? destination : destination.slice(0, hash);
  const anchor = hash < 0 ? "" : destination.slice(hash + 1);
  return (before.split("?")[0] ?? "") !== "" || anchor !== "";
}

/** Line numbers of offsets in one text, 1-based. */
export function lineIndex(text: string): (offset: number) => number {
  const starts = [0];
  for (let at = text.indexOf("\n"); at >= 0; at = text.indexOf("\n", at + 1)) {
    starts.push(at + 1);
  }
  return (offset) => {
    let low = 0;
    let high = starts.length - 1;
    while (low < high) {
      const middle = (low + high + 1) >> 1;
      if ((starts[middle] ?? 0) <= offset) {
        low = middle;
      } else {
        high = middle - 1;
      }
    }
    return low + 1;
  };
}

/**
 * Raw HTML without its comments, scanned once (linear in the text): `<!--` up to the next `-->`,
 * looked for after `<!` as the core does (`<!-->` and `<!--->` are whole comments, CommonMark 0.31).
 * An unclosed comment runs to the end.
 */
export function withoutComments(html: string): string {
  let out = "";
  let at = 0;
  for (let open = html.indexOf("<!--", at); open >= 0; open = html.indexOf("<!--", at)) {
    out += html.slice(at, open);
    const close = html.indexOf("-->", open + 2);
    if (close < 0) {
      return out;
    }
    at = close + 3;
  }
  return out + html.slice(at);
}

function isAsciiSpace(unit: string | undefined): boolean {
  return unit === " " || unit === "\t" || unit === "\n" || unit === "\r" || unit === "\f";
}

/**
 * An `a` start tag at the start of `tag`, read as the core's `a_tag` (`markdown.rs`): its non-empty
 * `id` and `name` values and its length through `>` (or `/>`, then `selfClosing`); null when `tag`
 * opens no `a` start tag.
 */
export function anchorTag(tag: string): { names: string[]; length: number; selfClosing: boolean } | null {
  if (tag[0] !== "<" || (tag[1] !== "a" && tag[1] !== "A") || !isAsciiSpace(tag[2])) {
    return null;
  }
  const names: string[] = [];
  let at = 2;
  for (;;) {
    while (isAsciiSpace(tag[at])) {
      at += 1;
    }
    if (at >= tag.length) {
      return null;
    }
    if (tag[at] === ">") {
      return { names, length: at + 1, selfClosing: false };
    }
    if (tag[at] === "/" && tag[at + 1] === ">") {
      return { names, length: at + 2, selfClosing: true };
    }
    const nameStart = at;
    while (at < tag.length && tag[at] !== "=" && tag[at] !== ">" && tag[at] !== "/" && !isAsciiSpace(tag[at])) {
      at += 1;
    }
    if (at >= tag.length) {
      return null;
    }
    if (at === nameStart) {
      // A stray `/`: no attribute.
      at += 1;
      continue;
    }
    const key = tag.slice(nameStart, at).toLowerCase();
    let after = at;
    while (isAsciiSpace(tag[after])) {
      after += 1;
    }
    if (after >= tag.length) {
      return null;
    }
    if (tag[after] !== "=") {
      continue;
    }
    at = after + 1;
    while (isAsciiSpace(tag[at])) {
      at += 1;
    }
    if (at >= tag.length) {
      return null;
    }
    let value: string;
    const quote = tag[at];
    if (quote === '"' || quote === "'") {
      const close = tag.indexOf(quote, at + 1);
      if (close < 0) {
        return null;
      }
      value = tag.slice(at + 1, close);
      at = close + 1;
    } else {
      const valueStart = at;
      while (at < tag.length && tag[at] !== ">" && !isAsciiSpace(tag[at])) {
        at += 1;
      }
      if (at >= tag.length) {
        return null;
      }
      value = tag.slice(valueStart, at);
    }
    if (value !== "" && (key === "id" || key === "name")) {
      names.push(value);
    }
  }
}

const ANCHOR_CLOSE = /^<\/a\s*>$/i;

/** An end tag `</a>` alone. */
export function isAnchorClose(html: string): boolean {
  return ANCHOR_CLOSE.test(html.trim());
}

/** An `a` start tag alone that names an anchor (`id` or `name`), not self-closed: it opens an empty anchor when `</a>` follows. */
export function isNamedAnchorOpen(html: string): boolean {
  const trimmed = html.trim();
  const tag = anchorTag(trimmed);
  return tag !== null && tag.names.length > 0 && !tag.selfClosing && tag.length === trimmed.length;
}

/** An empty named anchor in one piece: `<a id="x"/>`, or `<a name="x" class="y">` then only whitespace and `</a>`. */
export function isEmptyNamedAnchor(html: string): boolean {
  const trimmed = html.trim();
  const tag = anchorTag(trimmed);
  if (tag === null || tag.names.length === 0) {
    return false;
  }
  const rest = trimmed.slice(tag.length);
  return tag.selfClosing ? rest === "" : isAnchorClose(rest);
}

/**
 * A heading text's trailing `{...}` attribute block (no `{` or `}` inside), found from the end in
 * one pass: the text before it (trailing blanks cut) and what it holds; null when there is none.
 */
export function attributeBlock(text: string): { before: string; inner: string } | null {
  let end = text.length;
  while (end > 0 && (text[end - 1] === " " || text[end - 1] === "\t")) {
    end -= 1;
  }
  if (end === 0 || text[end - 1] !== "}") {
    return null;
  }
  const open = text.lastIndexOf("{", end - 2);
  if (open < 0) {
    return null;
  }
  const inner = text.slice(open + 1, end - 1);
  if (inner.includes("}")) {
    return null;
  }
  let before = open;
  while (before > 0 && (text[before - 1] === " " || text[before - 1] === "\t")) {
    before -= 1;
  }
  return { before: text.slice(0, before), inner };
}

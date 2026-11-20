import type { Cited } from "../api/types";

// The link kind `cited` (docs/features/department-practices.md "Acceptance criteria", DP21; ADR-0084): in a text the
// daemon scanned, each verbatim occurrence of a served `cited` entry's `written` that stands alone,
// outside code, links to the entry's `name`, and nothing else does. The UI never looks for an ID:
// it matches only the strings the daemon resolved, so an ID-shaped token the daemon left out (or never
// served) stays text. An occurrence stands alone when no ID character runs on into it from either side
// (a longer ID, another project's `slug/ID`, a section's `ID#X`), so a served ID never links part of
// another one. The longest served string wins at a place. Shared by the plain texts (`CitedText`) and
// the markdown renderer (`./plugin`), whose text nodes are never code; there a node's text is decoded
// (`R\-12`, `R&#45;12` read `R-12`), so a match links only where the source holds it verbatim
// (`verbatimPlaces`).

/** One run of a text: as written, or a served string and the name its link opens. */
export interface CitedRun {
  text: string;
  /** The document view's REF; null for plain text. */
  name: string | null;
}

/** A character that runs on into an ID: a letter or digit of any script, `_` or `-`. */
const JOINS = /^[\p{L}\p{N}_-]$/u;

/** A letter or digit. */
const ALNUM = /^[\p{L}\p{N}]$/u;

/** What qualifies an ID next to it: `slug/ID`, `ID#SECTION`, `ID@rev`. */
const QUALIFIERS = new Set(["/", "#", "@"]);

/** The character (a whole code point) ending just before `at`, or null at the start. */
function charBefore(text: string, at: number): string | null {
  if (at <= 0) {
    return null;
  }
  const low = text.charCodeAt(at - 1);
  const wide = at >= 2 && low >= 0xdc00 && low <= 0xdfff && text.charCodeAt(at - 2) >= 0xd800 && text.charCodeAt(at - 2) <= 0xdbff;
  return text.slice(wide ? at - 2 : at - 1, at);
}

/** The character (a whole code point) starting at `at`, or null at the end. */
function charAt(text: string, at: number): string | null {
  const point = text.codePointAt(at);
  return point === undefined ? null : String.fromCodePoint(point);
}

/** Whether the occurrence at `start..end` stands alone: nothing of an ID or its qualifier runs on into it. */
export function standsAlone(text: string, start: number, end: number): boolean {
  const before = charBefore(text, start);
  if (before !== null && (JOINS.test(before) || QUALIFIERS.has(before))) {
    return false;
  }
  const after = charAt(text, end);
  if (after === null) {
    return true;
  }
  if (JOINS.test(after)) {
    return false;
  }
  const next = charAt(text, end + after.length);
  return !(QUALIFIERS.has(after) && next !== null && ALNUM.test(next));
}

/**
 * The code spans of a plain text, as CommonMark reads them: a run of backticks to the next run of
 * exactly as many, both runs inside; a run with no match is text.
 */
export function codeSpans(text: string): (readonly [number, number])[] {
  const runs: (readonly [number, number])[] = [];
  for (let at = text.indexOf("`"); at >= 0; ) {
    let end = at;
    while (text[end] === "`") {
      end += 1;
    }
    runs.push([at, end - at]);
    at = text.indexOf("`", end);
  }
  const spans: (readonly [number, number])[] = [];
  for (let open = 0; open < runs.length; ) {
    const [start, length] = runs[open] ?? [0, 0];
    let close = open + 1;
    while (close < runs.length && runs[close]?.[1] !== length) {
      close += 1;
    }
    const closing = runs[close];
    if (closing === undefined) {
      open += 1;
      continue;
    }
    spans.push([start, closing[0] + closing[1]]);
    open = close + 1;
  }
  return spans;
}

/** Markdown's backslash escapes: a backslash before ASCII punctuation stands for the punctuation alone. */
const ESCAPABLE = /^[!-/:-@[-`{-~]$/;

/** A character reference at the start: decimal, hexadecimal or named (`&#45;`, `&#x2d;`, `&hyphen;`). */
const REFERENCE = /^&(?:#([0-9]{1,7})|#[xX]([0-9A-Fa-f]{1,6})|[A-Za-z][A-Za-z0-9]{1,31});/;

/** What a numeric reference decodes to: the code point, U+FFFD for none, a surrogate or zero; null for a named one. */
function numericValue(decimal: string | undefined, hex: string | undefined): string | null {
  if (decimal === undefined && hex === undefined) {
    return null;
  }
  const point = decimal === undefined ? Number.parseInt(hex ?? "", 16) : Number.parseInt(decimal, 10);
  return point === 0 || point > 0x10ffff || (point >= 0xd800 && point <= 0xdfff) ? "\ufffd" : String.fromCodePoint(point);
}

/**
 * Where each UTF-16 unit of a decoded text stands in its source: its index in `raw` when copied as is,
 * -1 when it came from an escape or a character reference; null when the two cannot be aligned. A
 * source character the text drops (a continuation line's indent, a block quote's marker, a heading's
 * cut attribute block) is passed over.
 */
export function verbatimPlaces(raw: string, value: string): Int32Array | null {
  const places = new Int32Array(value.length).fill(-1);
  let at = 0;
  let from = 0;
  while (at < value.length) {
    if (from >= raw.length) {
      return null;
    }
    const next = raw[from + 1];
    if (raw[from] === "\\" && next !== undefined && ESCAPABLE.test(next) && next === value[at]) {
      at += 1;
      from += 2;
      continue;
    }
    const reference = raw[from] === "&" ? REFERENCE.exec(raw.slice(from, from + 40)) : null;
    if (reference !== null && !value.startsWith(reference[0], at)) {
      // A named reference decodes to one code point here, the common case.
      const decoded = numericValue(reference[1], reference[2]) ?? String.fromCodePoint(value.codePointAt(at) ?? 0xfffd);
      if (!value.startsWith(decoded, at)) {
        return null;
      }
      at += decoded.length;
      from += reference[0].length;
      continue;
    }
    if (raw[from] === value[at]) {
      places[at] = from;
      at += 1;
    }
    from += 1;
  }
  return places;
}

/** Whether `value[start..end)` is copied from its source unchanged, character for character, side by side. */
export function verbatimIn(places: Int32Array | null): (start: number, end: number) => boolean {
  return (start, end) => {
    if (places === null) {
      return false;
    }
    const first = places[start] ?? -1;
    for (let at = start; at < end; at += 1) {
      if (first < 0 || places[at] !== first + (at - start)) {
        return false;
      }
    }
    return true;
  };
}

/** The served strings, each once (its first entry's name), longest first; an empty one never matches. */
function entriesOf(cited: readonly Cited[]): (readonly [string, string])[] {
  const names = new Map<string, string>();
  for (const entry of cited) {
    if (entry.written !== "" && !names.has(entry.written)) {
      names.set(entry.written, entry.name);
    }
  }
  return [...names].sort((a, b) => b[0].length - a[0].length);
}

/**
 * A text cut into runs: each standing-alone occurrence of a served `written` outside `code` (and, given
 * `accept`, one it accepts) a link run, everything else text, in order; joined, the runs are the text.
 */
export function citedRuns(text: string, cited: readonly Cited[], code: readonly (readonly [number, number])[] = [], accept?: (start: number, end: number) => boolean): CitedRun[] {
  return runsOf(text, entriesOf(cited), code, accept);
}

/** The served strings as `citedRuns` matches them, built once for a holder's texts. */
export type CitedEntries = readonly (readonly [string, string])[];

/** The served strings, each once, longest first: a holder's texts share them. */
export function citedEntries(cited: readonly Cited[]): CitedEntries {
  return entriesOf(cited);
}

/** `citedRuns` over entries already built. */
export function runsOf(text: string, entries: CitedEntries, code: readonly (readonly [number, number])[] = [], accept?: (start: number, end: number) => boolean): CitedRun[] {
  if (entries.length === 0 || text === "") {
    return [{ text, name: null }];
  }
  const runs: CitedRun[] = [];
  let plain = 0;
  let span = 0;
  for (let at = 0; at < text.length; ) {
    const inside = code[span];
    if (inside !== undefined && at >= inside[0]) {
      at = Math.max(at, inside[1]);
      span += 1;
      continue;
    }
    const found = entries.find(
      ([written]) =>
        text.startsWith(written, at) &&
        (inside === undefined || at + written.length <= inside[0]) &&
        standsAlone(text, at, at + written.length) &&
        (accept === undefined || accept(at, at + written.length)),
    );
    if (found === undefined) {
      at += 1;
      continue;
    }
    if (at > plain) {
      runs.push({ text: text.slice(plain, at), name: null });
    }
    runs.push({ text: found[0], name: found[1] });
    at += found[0].length;
    plain = at;
  }
  if (plain < text.length || runs.length === 0) {
    runs.push({ text: text.slice(plain), name: null });
  }
  return runs;
}

import type { DriftRow } from "../api/types";

// Words the lock diff's panel finds in the daemon's notes (docs/features/lock-diff-ui.md "Data",
// docs/canon/spec-lock-diff.md "Notes"): each side the daemon did not show (`diff` null) has one
// note, `lock diff: <key>: no <spec|code> diff: <why>`, its <why> said beside that side. Only
// text is matched; no state, class or category is read here.

/**
 * A kept row's key as verify's text prints it, which the lock diff's notes name:
 * `<node> | <relation> | <file>[ | <kind> <symbol>[ <item_kind>]]` (`docs/canon/spec-lock-diff.md` "Notes").
 */
export function rowKeyOf(row: DriftRow): string {
  // A kept row has a lock entry, so a key; a keyless row is never kept.
  const binding = row.kind === null ? "" : ` | ${row.kind} ${row.symbol ?? ""}${row.item_kind === null ? "" : ` ${row.item_kind}`}`;
  return `${row.node} | ${row.relation} | ${row.file}${binding}`;
}

/**
 * Why `side` of `row` has no hunks: the rest of the first note that starts
 * `lock diff: <key>: no <side> diff: `, raw; null when none does (a key verify's text escaped, an
 * older daemon), when the notes above say why.
 */
export function sideNoteOf(notes: readonly string[], row: DriftRow, side: "spec" | "code"): string | null {
  const start = `lock diff: ${rowKeyOf(row)}: no ${side} diff: `;
  const note = notes.find((each) => each.startsWith(start));
  return note === undefined ? null : note.slice(start.length);
}

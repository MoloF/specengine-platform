import type { DriftRow, LockDiffNode, LockDiffRow, LockDiffView, NodeView } from "../api/types";
import { mockCode } from "./bindings";
import { DRAFT_FILE, TIDE_FILE } from "./harbor-sim/fixtures";
import { harborDrift, LOCKED_AT } from "./health";
import type { Scenario } from "./scenario";

// The mock's `spec lock diff REF --json` (docs/features/lock-diff.md "Data"; the panel that draws
// it comes with docs/features/lock-diff-ui.md). REF is resolved as the node read resolves it; each
// holder's set is itself and the sections within it, as `show --bindings` groups. harbor-sim's
// kept rows are Health's row 6 (src/mocks/health.ts `harborDrift`) by key, those with a lock entry
// whose state raises drift: MEC-TIDES's `conflict` with both sides, MEC-MOORING's `code_ahead`
// (moved from berths.rs) with a code side, RULE-BERTH-DRAFT's `marker_behind` with a spec side the
// daemon could not read at the entry's commit (`diff` null, its one note), RULE-HIGH-WATER's
// `symbol_gone` with a code side of `-` lines only. Under a run cause of the code layer (ledger-api
// with no [code]; either project in `code-cannot`) every holder's `rows` is null with the one
// `bindings:` note `show --bindings` gives; ledger-api in `not-configured` has [code] and no
// drifted row, so `rows` is []. Hunks are git's, as the daemon would send them, never made here.

/** A kept row's key as verify's text prints it, which the notes name: `<node> | <relation> | <file> | <kind> <symbol>[ <item_kind>]`. */
export function rowKeyOf(row: DriftRow): string {
  // A kept row has a lock entry, so a key; a keyless row is never kept.
  const binding = row.kind === null ? "" : ` | ${row.kind} ${row.symbol ?? ""}${row.item_kind === null ? "" : ` ${row.item_kind}`}`;
  return `${row.node} | ${row.relation} | ${row.file}${binding}`;
}

/** The entries' `file_blob`s: the files as the owner accepted them (invented OIDs). */
const TIDE_BLOB = "b6e2d94a1f0c38e75a2d6b9c4e1f7a0358c2d6e9";
const BERTHS_BLOB = "3d9a7c1e5b2f8046a1c3e5d7f9b2a4c6e8d0f135";
const TIDE_TABLE_BLOB = "e1c4a7d0b3f6295c8e1a4d7b0c3f6a9d2e5b8c17";

/** MEC-TIDES's document since its entry: its revision raised and its first sentence reworded (hunk lines count from the span's first line). */
const TIDES_SPEC_DIFF = [
  "@@ -7,12 +7,12 @@",
  " constrains: [RULE-BERTH-DRAFT]",
  " uses_term: [RULE-HIGH-WATER, TERM-SLACK-WATER]",
  " canon: docs/canon/tides.md",
  "-rev: 2",
  "+rev: 3",
  " ---",
  " ",
  " # Tide cycle",
  " ",
  "-Water level follows a 12 h 25 min cycle read from one fixed table.",
  "+Water level follows a 12 h 25 min cycle read from the tide table of the scenario (MEC-TIDE-TABLES).",
  " ",
  " ## RULE-TIDE-WINDOW: Entry only inside the tide window",
  " ",
  "",
].join("\n");

/** `tide_window` since its entry: the table no longer a parameter, so its signature changed. */
const TIDES_CODE_DIFF = [
  "@@ -1,5 +1,5 @@",
  "-pub fn tide_window(at: SimTime, table: &TideTable) -> Window {",
  "-    let high = table.next_high_water(at);",
  "+pub fn tide_window(at: SimTime) -> Window {",
  "+    let high = TIDE_TABLE.with(|table| table.next_high_water(at));",
  "     let margin = Minutes(90);",
  "     Window {",
  "         opens: high - margin,",
  "",
].join("\n");

/** `moor` as accepted in berths.rs against the one in crew.rs now: its body changed (a night takes longer). */
const MOOR_CODE_DIFF = [
  "@@ -3,6 +3,7 @@",
  "     if vessel.draft() > berth_limit(berth) {",
  "         return Err(MoorError::TooDeep);",
  "     }",
  "-    crew.start(vessel, Minutes(20));",
  "+    let minutes = if is_night() { Minutes(30) } else { Minutes(20) };",
  "+    crew.start(vessel, minutes);",
  "     Ok(Moored { vessel: vessel.id(), berth })",
  " }",
  "",
].join("\n");

/** The test RULE-HIGH-WATER's entry names, gone now: its old lines against nothing. */
const HIGH_WATER_CODE_DIFF = [
  "@@ -1,5 +0,0 @@",
  "-    fn high_water_holds() {",
  "-        let table = TideTable::sample();",
  "-        let high = table.high_water(Day(1));",
  "-        assert!(high.level > table.low_water(Day(1)).level);",
  "-    }",
  "",
].join("\n");

const BERTH_DRAFT_KEY = "RULE-BERTH-DRAFT | assumes | data/berths.ron | ron berths[4].draft_m";

/** harbor-sim's kept rows' sides by key, and the notes each gives (one per side not shown, N1 here), in output order. */
const HARBOR_SIDES: Readonly<Record<string, Pick<LockDiffRow, "spec" | "code"> & { notes: readonly string[] }>> = {
  "MEC-MOORING | implements | src/sim/crew.rs | item harbor_sim::crew::moor function_item": {
    spec: null,
    code: { blob: BERTHS_BLOB, file: "src/sim/berths.rs", diff: MOOR_CODE_DIFF },
    notes: [],
  },
  "MEC-TIDES | implements | src/sim/tide.rs | item harbor_sim::tide::tide_window function_item": {
    spec: { commit: LOCKED_AT, path: TIDE_FILE, diff: TIDES_SPEC_DIFF },
    code: { blob: TIDE_BLOB, file: "src/sim/tide.rs", diff: TIDES_CODE_DIFF },
    notes: [],
  },
  [BERTH_DRAFT_KEY]: {
    spec: { commit: LOCKED_AT, path: DRAFT_FILE, diff: null },
    code: null,
    notes: [`lock diff: ${BERTH_DRAFT_KEY}: no spec diff: ${DRAFT_FILE} cannot be read at ${LOCKED_AT}`],
  },
  "RULE-HIGH-WATER | verifies | src/sim/tide_table.rs | item harbor_sim::tide_table::tests::high_water_holds function_item": {
    spec: null,
    code: { blob: TIDE_TABLE_BLOB, file: "src/sim/tide_table.rs", diff: HIGH_WATER_CODE_DIFF },
    notes: [],
  },
};

/** harbor-sim's kept rows in verify's order: Health's rows with a side table entry, each verbatim, with its sides. */
export function harborLockDiffRows(): LockDiffRow[] {
  return harborDrift().flatMap((row) => {
    const sides = HARBOR_SIDES[rowKeyOf(row)];
    return sides === undefined ? [] : [{ row, spec: structuredClone(sides.spec), code: structuredClone(sides.code) }];
  });
}

/** The notes a kept row's sides give, in output order. */
export function sideNotesOf(row: DriftRow): readonly string[] {
  return HARBOR_SIDES[rowKeyOf(row)]?.notes ?? [];
}

/**
 * The lock diff document for `project` in `scenario`, over `shown`, the REF's `spec show` read: its
 * exit-1 document when the REF names nothing (`notes` show's, `nodes` []); else each holder with
 * its kept rows, or with `rows` null and the code layer's one note under a run cause.
 */
export function lockDiffOf(project: string, scenario: Scenario, shown: NodeView): LockDiffView {
  if (shown.reason !== null) {
    return { ref: shown.ref, reason: shown.reason, notes: [...shown.notes], nodes: [] };
  }
  const cause = mockCode(project, scenario).notes;
  const kept = cause.length > 0 || project !== "harbor-sim" ? [] : harborLockDiffRows();
  const notes = [...shown.notes, ...cause];
  const nodes = shown.nodes.map((node): LockDiffNode => {
    if (cause.length > 0) {
      return { id: node.id, path: node.path, line: node.line, rows: null };
    }
    const set = new Set([...(node.id === null ? [] : [node.id]), ...node.sections]);
    const rows = kept.filter(({ row }) => set.has(row.node));
    notes.push(...rows.flatMap(({ row }) => sideNotesOf(row)));
    return { id: node.id, path: node.path, line: node.line, rows };
  });
  return { ref: shown.ref, reason: null, notes, nodes };
}

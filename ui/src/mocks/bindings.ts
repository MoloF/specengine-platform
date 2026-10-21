import type { BindingRow, DriftRow, ShownBindings, TestedRow } from "../api/types";
import { CODE_GAP_CAUSE, CODE_OFF_CAUSE, harborDrift, harborTested, LOCK_CAUSE, TESTED_AT } from "./health";
import type { Scenario } from "./scenario";

// The mock's `spec show --bindings`, for `docs/canon/drift-screens.md` "Bindings tab"; the data:
// `docs/canon/code-reads.md` "spec show --bindings" and "Degrade". Each project's binding rows in
// verify's key order and its `[[tested]]` entries, which a shown node filters to its set (itself and
// the sections within it: src/mocks/corpus.ts). normal: harbor-sim's rows are Health's twelve (row 6,
// src/mocks/health.ts) as binding rows plus its nine in sync (HARBOR_OK), so a node shows every
// state, a moved anchor, an ambiguous one, a look-alike filed under its Latin fix, two keyless rows,
// a row not in any file now; its entries Health's but the gone node's, plus a verified one.
// ledger-api has no [code] table: null and the read's one `bindings:` note. large: harbor-sim as
// normal plus DOM-GEN-01's 3 000 unlocked rows, a stress case of its own. code-cannot: harbor-sim's
// code walk incomplete, null and the gap's note; ledger-api's spec.lock invalid, its rows listed but
// never judged (`sync` null) and the lock's note. not-configured: ledger-api has [code], its two
// bindings in sync (Health's row 6 there: none, 2 ok). The other scenarios read as normal.

/** A project's code layer as the mock's read finds it. */
export interface MockCode {
  /** The read's notes: one `bindings: <cause>` under a degrade cause, else none. */
  notes: readonly string[];
  /** Every row of the project in verify's key order; null under a cause listing no row (no [code], a walk gap). */
  rows: readonly BindingRow[] | null;
  /** The project's `[[tested]]` entries by node bytes; null when not judged (a listed cause). */
  tested: readonly TestedRow[] | null;
}

/** A shown node's symbol as the code index stores it: its lines, signature and layer. */
type Stored = [line: number | null, endLine: number | null, signature: string | null, layer: string | null];

/** harbor-sim's stored symbols by `<file>|<symbol>`; a file module, a RON value and a gone or ambiguous item have no lines but a RON's first marker's. */
const HARBOR_SYMBOLS: Readonly<Record<string, Stored>> = {
  "src/sim/crew.rs|harbor_sim::crew::moor": [31, 48, "pub fn moor(vessel: &mut Vessel, berth: BerthId) -> Result<Moored, MoorError>", "a"],
  "src/sim/tide.rs|harbor_sim::tide::tide_window": [10, 27, "pub fn tide_window(at: SimTime) -> Window", "a"],
  "src/sim/tide.rs|harbor_sim::tide::in_window": [22, 30, "pub fn in_window(at: SimTime, window: &Window) -> bool", "a"],
  "src/sim/tide.rs|harbor_sim::tide::tests::window_opens_at_flood": [88, 97, "fn window_opens_at_flood()", "a"],
  "src/sim/berths.rs|harbor_sim::berths::basin_name": [31, 35, "pub fn basin_name(basin: &Basin) -> &str", "a"],
  "src/sim/berths.rs|harbor_sim::berths::basin_label": [37, 41, "pub fn basin_label(basin: &Basin) -> String", "a"],
  "src/sim/tide_table.rs|harbor_sim::tide_table::spring_window": [59, 74, "pub fn spring_window(table: &TideTable, day: Day) -> Option<Window>", "a"],
  "src/sim/tide_table.rs|harbor_sim::tide_table::load": [12, 40, "pub fn load(path: &Path) -> Result<TideTable, TableError>", "a"],
  "src/sim/fairway.rs|harbor_sim::fairway::speed_limit": [8, 19, "pub fn speed_limit(vessel: &Vessel, night: bool) -> Knots", "a"],
  "src/sim/clock.rs|harbor_sim::clock::now": [14, 16, "pub fn now(&self) -> SimTime", "a"],
  "src/sim/pilot.rs|harbor_sim::pilot::pilot_required": [5, 11, "pub fn pilot_required(vessel: &Vessel) -> bool", "a"],
  "tests/mooring.rs|mooring::moors_at_slack_water": [6, 21, "fn moors_at_slack_water()", "a"],
  "data/fairway.ron|fairway.speed_knots": [2, null, null, null],
  "data/berths.ron|berths[4].draft_m": [3, null, null, null],
  "data/cranes.ron|cranes.max_lift_t": [4, null, null, null],
  "data/tides.ron|tides.spring_window_h": [7, null, null, null],
};

/** A row as `show --bindings` gives it: its key, the stored symbol's lines, signature and layer, verify's judgement. */
function bindingRow(row: Pick<DriftRow, "node" | "relation" | "file" | "kind" | "symbol" | "item_kind" | "category" | "detail" | "anchor"> & { sync: string | null }): BindingRow {
  const [line, endLine, signature, layer] = (row.symbol === null ? undefined : HARBOR_SYMBOLS[`${row.file}|${row.symbol}`]) ?? [null, null, null, null];
  return {
    node: row.node,
    relation: row.relation,
    file: row.file,
    kind: row.kind,
    symbol: row.symbol,
    item_kind: row.item_kind,
    line,
    end_line: endLine,
    signature,
    layer,
    sync: row.sync,
    category: row.category,
    detail: row.detail,
    anchor: row.anchor,
  };
}

const EXACT = { state: "exact", from: null, candidates: null };

/** A binding in sync: exactly where the lock has it. */
function inSync(node: string, relation: string, file: string, kind: string, symbol: string, itemKind: string | null): BindingRow {
  return bindingRow({ node, relation, file, kind, symbol, item_kind: itemKind, sync: "ok", category: null, detail: null, anchor: EXACT });
}

/** harbor-sim's nine bindings in sync, Health's HARBOR_OK, none of them a row 6 line. */
function harborInSync(): BindingRow[] {
  return [
    inSync("MEC-CARGO-CRANES", "configures", "data/cranes.ron", "ron", "cranes.max_lift_t", null),
    inSync("MEC-MOORING", "verifies", "tests/mooring.rs", "item", "mooring::moors_at_slack_water", "function_item"),
    inSync("MEC-TIDE-TABLES", "implements", "src/sim/tide_table.rs", "item", "harbor_sim::tide_table::load", "function_item"),
    inSync("MEC-TIDES", "verifies", "src/sim/tide.rs", "item", "harbor_sim::tide::tests::window_opens_at_flood", "function_item"),
    inSync("RULE-BASIN-NAME", "implements", "src/sim/berths.rs", "item", "harbor_sim::berths::basin_label", "function_item"),
    inSync("RULE-FAIRWAY-SPEED", "implements", "src/sim/fairway.rs", "item", "harbor_sim::fairway::speed_limit", "function_item"),
    inSync("RULE-HARBOR-CLOCK", "implements", "src/sim/clock.rs", "item", "harbor_sim::clock::now", "function_item"),
    inSync("RULE-PILOT-REQ", "implements", "src/sim/pilot.rs", "item", "harbor_sim::pilot::pilot_required", "function_item"),
    inSync("RULE-SPRING-WINDOW", "configures", "data/tides.ron", "ron", "tides.spring_window_h", null),
  ];
}

/** The relations in the lock's order (core `LockRelation`), which verify's key order follows. */
const RELATIONS = ["implements", "verifies", "configures", "assumes"];

function byBytes(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** verify's key order (core `RowKey`): node bytes, relation, file bytes, a keyless row first (by state, category), kind, symbol, item kind. */
export function byRowKey(a: BindingRow, b: BindingRow): number {
  const keyless = (row: BindingRow) => (row.kind === null ? 0 : 1);
  const keylessOrder = keyless(a) === 0 && keyless(b) === 0 ? byBytes(a.sync ?? "", b.sync ?? "") || byBytes(a.category ?? "", b.category ?? "") : 0;
  return (
    byBytes(a.node, b.node) ||
    RELATIONS.indexOf(a.relation) - RELATIONS.indexOf(b.relation) ||
    byBytes(a.file, b.file) ||
    keyless(a) - keyless(b) ||
    keylessOrder ||
    byBytes(a.kind ?? "", b.kind ?? "") ||
    byBytes(a.symbol ?? "", b.symbol ?? "") ||
    byBytes(a.item_kind ?? "", b.item_kind ?? "")
  );
}

/** Every row of harbor-sim: Health's twelve as binding rows and the nine in sync, in verify's key order. */
export function harborBindingRows(): BindingRow[] {
  const drift = harborDrift().map((row) => bindingRow({ ...row, sync: row.state }));
  return [...drift, ...harborInSync()].sort(byRowKey);
}

/** harbor-sim's entries as a read gives them: Health's but the gone node's (a read's set holds present nodes only), and a verified one. */
function harborEntries(): TestedRow[] {
  const verified: TestedRow = { node: "MEC-TIDE-TABLES", state: "verified", category: null, rev: 1, node_rev: 1, commit: TESTED_AT, verified_at: "2026-10-02" };
  return [verified, ...harborTested().filter((entry) => entry.state !== "node_gone")].sort((a, b) => byBytes(a.node, b.node));
}

/** How many rows DOM-GEN-01 holds in the large scenario. */
export const LARGE_BINDINGS = 3000;

/** The large scenario's stress case: DOM-GEN-01 implemented in 3 000 generated files, each unlocked (before a first accept). */
function largeRows(): BindingRow[] {
  return Array.from({ length: LARGE_BINDINGS }, (_, index) => {
    const part = `part_${String(index + 1).padStart(4, "0")}`;
    return bindingRow({
      node: "DOM-GEN-01",
      relation: "implements",
      file: `src/gen/d01/${part}.rs`,
      kind: "item",
      symbol: `harbor_sim::gen::d01::${part}::run`,
      item_kind: "function_item",
      sync: "unlocked",
      category: null,
      detail: null,
      anchor: null,
    });
  });
}

/** ledger-api's two bindings once it has a [code] table: judged in sync, or listed under a cause (judgement keys null). */
function ledgerRows(judged: boolean): BindingRow[] {
  const row = (node: string, file: string, symbol: string) =>
    bindingRow({
      node,
      relation: "implements",
      file,
      kind: "item",
      symbol,
      item_kind: "function_item",
      sync: judged ? "ok" : null,
      category: null,
      detail: null,
      anchor: judged ? EXACT : null,
    });
  return [
    row("POL-REFUND-WINDOW", "src/refunds/window.rs", "refunds::window::is_refundable"),
    row("SVC-REFUNDS", "src/refunds/service.rs", "refunds::service::refund_payment"),
  ];
}

/** The read's note under a degrade cause, as the CLI words it: `bindings: <cause>`. */
export function bindingsNote(cause: string): string {
  return `bindings: ${cause}`;
}

/** `project`'s code layer in `scenario`, as the mock's `show --bindings` reads it. */
export function mockCode(project: string, scenario: Scenario): MockCode {
  if (project === "harbor-sim") {
    if (scenario === "code-cannot") {
      return { notes: [bindingsNote(CODE_GAP_CAUSE)], rows: null, tested: null };
    }
    const rows = scenario === "large" ? [...harborBindingRows(), ...largeRows()].sort(byRowKey) : harborBindingRows();
    return { notes: [], rows, tested: harborEntries() };
  }
  if (scenario === "not-configured") {
    return { notes: [], rows: ledgerRows(true), tested: [] };
  }
  if (scenario === "code-cannot") {
    return { notes: [bindingsNote(LOCK_CAUSE)], rows: ledgerRows(false), tested: null };
  }
  return { notes: [bindingsNote(CODE_OFF_CAUSE)], rows: null, tested: null };
}

/** The Latin letters a Cyrillic look-alike stands for (the mock's own table): a look-alike row is filed under its fix. */
const LOOKALIKES: Readonly<Record<string, string>> = {
  "\u0410": "A",
  "\u0412": "B",
  "\u0415": "E",
  "\u041a": "K",
  "\u041c": "M",
  "\u041d": "H",
  "\u041e": "O",
  "\u0420": "P",
  "\u0421": "C",
  "\u0422": "T",
  "\u0425": "X",
};

/** The ID a row's node is filed under: as written, a look-alike under its Latin fix. */
function filedUnder(node: string): string {
  return Array.from(node, (char) => LOOKALIKES[char] ?? char).join("");
}

/**
 * A shown node's bindings: the rows and entries whose node is in its set (the IDs of itself and
 * the sections within it), in the project's order; null when the code layer listed no row.
 */
export function bindingsOf(code: MockCode, ids: ReadonlySet<string>): ShownBindings | null {
  if (code.rows === null) {
    return null;
  }
  return {
    rows: code.rows.filter((row) => ids.has(filedUnder(row.node))),
    omitted: 0,
    tested: code.tested === null ? null : code.tested.filter((entry) => ids.has(entry.node)),
  };
}

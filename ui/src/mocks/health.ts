import type {
  AssumedNode,
  AssumedProposal,
  AssumedState,
  AssumesCounts,
  AssumesRow,
  BindingPlace,
  DriftCounts,
  DriftLock,
  DriftRow,
  HealthAssumes,
  HealthCode,
  HealthDrift,
  HealthReport,
  HealthUnbound,
  ShownLink,
  TestedRow,
  UnappliedAmendment,
  UnboundMarker,
  UnboundReason,
  UnboundRow,
} from "../api/types";
import type { MockDocument, MockNode } from "./build";
import { DEEP_BERTH_FILE, harborSim, QUAY_DECISION_FILE, WEATHER_DECISION } from "./harbor-sim/fixtures";
import { largeDocuments } from "./harbor-sim/large";
import type { Scenario } from "./scenario";

// The mock's `spec health --json` (docs/features/health-left.md "Data", Mocks): one builder, no
// clock, the core's order (amendments by path, byte order). normal: harbor-sim two, the spec's
// decision with one dangling link and a draft spec amending RULE-BERTH-DRAFT, resolved, both
// documents of its corpus (src/mocks/harbor-sim/fixtures.ts), their links as its `show --links`
// lists them (src/mocks/health.test.ts holds the two to that); ledger-api none. empty: none. large: harbor-sim 3 000 by path. cannot-check: a 503, the walk incomplete
// (both reads walk one tree, so the first gap of the check's causes). `documents` is the check's
// `counts.documents` for the same project and scenario (src/mocks/check.ts). `error` and `slow`
// act in the client, as for every read.
//
// Rows 4-5 (docs/features/health-code-ui.md "Data", Mocks; docs/canon/health-code.md "Document"):
// harbor-sim measures both, row 4 a line in each of its eight states, row 5 five unbound nodes
// whose markers carry each reason, every ID and place from its corpus; ledger-api has no [code]
// table, both rows off. empty: both measured, none, for either project. large: harbor-sim's row 5
// 3 000 unbound nodes, the generated corpus's among them. code-cannot: rows 4-5 could not be
// measured (harbor-sim a gap of the code walk, ledger-api a queue below the schema and a bad rule),
// row 3 as normal. not-configured: row 5 without a rule, row 4 measured. Texts stored raw, as the
// JSON carries them: a control character in a rejection's note and in a cause, which the UI writes
// `\u{..}` as the CLI's text does.
//
// Row 6 (docs/features/health-drift-ui.md "Data"; docs/canon/health-drift.md "Document"): normal,
// harbor-sim measured: twelve rows of every class in verify's key order (node bytes, the relation in
// the lock's order, file bytes, keyless first, kind, symbol, item kind), never by class, state or
// file, so a sort in the UI would show: a moved anchor, an ambiguous one with two candidates, a
// look-alike ID in its script, two keyless rows, a lock-only row; the six markers naming its queue
// items skipped; a stale, an unverifiable and a node_gone entry. ledger-api has no [code] table:
// off. empty: both measured, none. large: harbor-sim 3 000 rows, its twelve among the generated
// corpus's, each of those `unlocked` (the flood before a first accept). code-cannot: row 6 could not
// be measured: harbor-sim the code walk's gap (rows 4-6 alike), ledger-api an invalid spec.lock (row
// 6 alone; the queue and the rule its rows 4-5 fail on never touch row 6). not-configured:
// harbor-sim as normal, ledger-api measured, none.

const HARBOR = "harbor-sim";

/** The link type both mock amendments carry: the mocks' data, never compared by app code. */
const AMENDS = "amends";

/** An `amends` link as `spec show --links` lists it in the document's out links: written in its front matter. */
function amendsLink(fields: Pick<ShownLink, "written" | "path" | "line"> & Partial<ShownLink>): ShownLink {
  return {
    type: AMENDS,
    origin: "frontmatter",
    at: null,
    name: null,
    state: "dangling",
    reason: null,
    ...fields,
  };
}

/** The spec's decision ("Data"): accepted, amending an ID the corpus no longer holds. */
const QUAY_DECISION = QUAY_DECISION_FILE;

/** A feature spec with no `id:`, a draft amending the berth's draft limit. */
const DEEP_BERTH = DEEP_BERTH_FILE;

function harborAmendments(): UnappliedAmendment[] {
  return [
    {
      path: QUAY_DECISION,
      id: "DEC-0014",
      title: "Quay lights stay amber",
      class: "decision",
      status: "accepted",
      links: [
        amendsLink({
          written: "RULE-QUAY-LIGHTS",
          path: QUAY_DECISION,
          line: 8,
          reason: "`RULE-QUAY-LIGHTS` resolves to no ID and no alias",
        }),
      ],
    },
    {
      path: DEEP_BERTH,
      id: null,
      title: "Berth 4 takes 13.0 m draft after the dredging",
      class: "spec",
      status: "draft",
      links: [amendsLink({ written: "RULE-BERTH-DRAFT", name: "RULE-BERTH-DRAFT", state: "resolved", path: DEEP_BERTH, line: 6 })],
    },
  ];
}

/** How many amendments the large scenario lists. */
export const LARGE_AMENDMENTS = 3000;

const pad = (value: number, width = 2) => String(value).padStart(width, "0");

/**
 * The large scenario's amendments, by path: each amends one rule of the generated corpus
 * (src/mocks/harbor-sim/large.ts), every seventh written as an ID that names nothing; every third
 * a decision with an ID, the rest draft specs without one. Their own files are not in the corpus:
 * a path opens the tree's "names nothing"; a resolved link's rule is there.
 */
function largeAmendments(): UnappliedAmendment[] {
  return Array.from({ length: LARGE_AMENDMENTS }, (_, index): UnappliedAmendment => {
    const number = index + 1;
    const path = `docs/features/gen/amend-${pad(number, 4)}.md`;
    const decision = number % 3 === 0;
    const rule = `RULE-GEN-${pad((index % 12) + 1)}-${pad((index % 6) + 1)}-${pad((Math.floor(index / 6) % 6) + 1)}-${String((index % 2) + 1)}`;
    const dangling = number % 7 === 0;
    const written = dangling ? `RULE-GEN-GONE-${pad(number, 4)}` : rule;
    return {
      path,
      id: decision ? `DEC-${pad(number + 1000, 4)}` : null,
      title: `Generated amendment ${String(number)}`,
      class: decision ? "decision" : "spec",
      status: decision ? "accepted" : "draft",
      links: [
        dangling
          ? amendsLink({ written, path, line: 6, reason: `\`${written}\` resolves to no ID and no alias` })
          : amendsLink({ written, name: rule, state: "resolved", path, line: 6 }),
      ],
    };
  });
}

/** The check's document count for the project and scenario (src/mocks/check.ts). */
function documentsOf(project: string, scenario: Scenario): number {
  if (scenario === "empty") {
    return 4;
  }
  if (project !== HARBOR) {
    return 12;
  }
  return scenario === "large" ? 3149 : 41;
}

/**
 * Why the read cannot run in the `cannot-check` scenario: the CLI's line for the first gap of the
 * walk by path, as `spec export index` words it (`docs/features/health-left.md` "Data"); the
 * check's causes for the same project hold it.
 */
export function walkGapMessage(project: string): string {
  const gap = project === HARBOR ? "`docs/spec/cranes.md` cannot be read (Permission denied (os error 13))" : "the directory `docs/spec/api` cannot be listed";
  return `spec: the walk is incomplete: ${gap}: unapplied amendments not measured`;
}

/** Why rows 4-5 are `off`: code-refs' sentence for a config with no `[code]` table, less `spec: ` (docs/canon/health-code.md "Reading"). */
export const CODE_OFF_CAUSE = "specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/** Why row 5 is `not_configured` (docs/canon/health-code.md "Row 5: unbound"). */
export const NO_RULE_CAUSE = "no [[health.bound]] rule: which nodes must be bound is the project's to declare";

/** harbor-sim's code walk gap in `code-cannot`, a file name holding an escape sequence, stored raw. */
export const CODE_GAP_CAUSE = "the code walk is incomplete: `src/sim/tide\u001b[2J.rs` cannot be read (Permission denied (os error 13))";

/** The causes of rows 4 and 5 in `code-cannot`, by project: the code's gap, a queue below the schema, a bad `[[health.bound]]`. */
export const CODE_CANNOT_CAUSES: Readonly<Record<string, { assumes: readonly string[]; unbound: readonly string[] }>> = {
  "harbor-sim": {
    assumes: [CODE_GAP_CAUSE],
    unbound: ["specengine.toml:44: unknown field `relation`, expected one of `kinds`, `classes`, `paths`, `when`, `relations`", CODE_GAP_CAUSE],
  },
  "ledger-api": {
    assumes: ["the queue stands at schema 6; a queue command upgrades it"],
    unbound: ["specengine.toml:31: `[[health.bound]]` without a selector: give `kinds`, `classes`, `paths` or `when`"],
  },
};

/** harbor-sim's rejected item's note: a line break and a right-to-left override, stored raw. */
export const REJECTED_NOTE = "Not a discrepancy: the pilot rule\nwas right;\u202e keep 15 minutes";

/** How many unbound nodes the large scenario lists. */
export const LARGE_UNBOUND = 3000;

/** Row 4 or 5 not measured: its state and causes, no counts, no rows (both rows share the shape). */
function notMeasured(state: "off" | "not_configured" | "cannot", causes: readonly string[]): HealthAssumes & HealthUnbound {
  return { state, causes: [...causes], counts: null, rows: null };
}

/** The code files each project's rows read, `spec refs`' count. */
const CODE: Readonly<Record<string, HealthCode>> = {
  "harbor-sim": { files: 9, rs: 7, ron: 2, skipped: 0 },
  "ledger-api": { files: 5, rs: 5, ron: 0, skipped: 0 },
};

function codeOf(project: string): HealthCode {
  return { ...(CODE[project] ?? { files: 0, rs: 0, ron: 0, skipped: 0 }) };
}

/** A node of a mock corpus by its ID, and its document: the first holder in file order. */
function held(documents: readonly MockDocument[], id: string): { node: MockNode; document: MockDocument } {
  for (const document of documents) {
    for (const node of [document.node, ...document.sections]) {
      if (node.id === id) {
        return { node, document };
      }
    }
  }
  throw new Error(`the mock corpus holds no ${id}`);
}

let harborDocuments: readonly MockDocument[] | null = null;

/** harbor-sim's spec files, built once (the corpus reads no clock). */
function harborFiles(): readonly MockDocument[] {
  harborDocuments ??= harborSim(0).corpus.documents;
  return harborDocuments;
}

/** The queue item a row 4 line names, keys in the JSON's order; absent ones null. */
function item(kind: string, status: string, fields: Partial<AssumedProposal> = {}): AssumedProposal {
  return { kind, status, choice: null, record_id: null, record_path: null, reason: null, ...fields };
}

/** The node an `@assumes` names, from the corpus: its document's raw status, the open items holding it. */
function namedNode(id: string, proposals: string[]): AssumedNode {
  const { node, document } = held(harborFiles(), id);
  return { id, path: node.path, line: node.line, status: document.node.status, successor: null, proposals };
}

/** One row 4 line, keys in the JSON's order. */
function line(id: string, state: AssumedState, [path, at, column]: [string, number, number], relation: string, proposal: AssumedProposal | null, node: AssumedNode | null): AssumesRow {
  return { id, state, path, line: at, column, relation, proposal, node };
}

/**
 * harbor-sim's row 4, by ID bytes, then path, line, column: one line in each state. PR-0041 is the
 * open discrepancy whose working answer the code carries; RULE-BERTH-DRAFT is held by the open
 * PR-0042 and PR-0044 of its queue; MEC-OLD-QUAYS is its archived document; the decided items'
 * records are documents of its corpus; PR-7 names no item.
 */
function harborAssumes(): AssumesRow[] {
  const weather = held(harborFiles(), WEATHER_DECISION).node.path;
  return [
    line("MEC-OLD-QUAYS", "archived", ["src/sim/berths.rs", 12, 4], "assumes", null, namedNode("MEC-OLD-QUAYS", [])),
    line("PR-0012", "applied", ["src/sim/tide_table.rs", 31, 4], "implements", item("update", "applied"), null),
    line(
      "PR-0019",
      "as_assumed",
      ["src/sim/weather.rs", 8, 4],
      "assumes",
      item("question", "applied", { choice: { working_answer: true }, record_id: WEATHER_DECISION, record_path: weather }),
      null,
    ),
    line(
      "PR-0023",
      "otherwise",
      ["data/berths.ron", 6, 17],
      "configures",
      item("discrepancy", "applied", { choice: { option: 1 }, record_id: "DEC-0014", record_path: QUAY_DECISION_FILE }),
      null,
    ),
    line("PR-0027", "rejected", ["src/sim/pilotage.rs", 40, 8], "implements", item("discrepancy", "rejected", { reason: REJECTED_NOTE }), null),
    line("PR-0041", "pending", ["src/sim/tide.rs", 17, 4], "assumes", item("discrepancy", "open"), null),
    line("PR-7", "unknown", ["src/sim/crew.rs", 13, 4], "implements", null, null),
    line("RULE-BERTH-DRAFT", "in_question", ["data/berths.ron", 3, 5], "assumes", null, namedNode("RULE-BERTH-DRAFT", ["PR-0042", "PR-0044"])),
  ];
}

/** Row 4's counts for `rows` over `markers` judged markers: one count per state, as listed. */
function assumesCounts(rows: readonly AssumesRow[], markers: number): AssumesCounts {
  const of = (state: string) => rows.filter((row) => row.state === state).length;
  return {
    markers,
    rows: rows.length,
    pending: of("pending"),
    as_assumed: of("as_assumed"),
    otherwise: of("otherwise"),
    applied: of("applied"),
    rejected: of("rejected"),
    unknown: of("unknown"),
    archived: of("archived"),
    in_question: of("in_question"),
  };
}

function measuredAssumes(rows: AssumesRow[], markers: number): HealthAssumes {
  return { state: "measured", causes: [], counts: assumesCounts(rows, markers), rows };
}

/** A marker naming an unbound node without binding it, keys in the JSON's order. */
function marker([path, at, column]: [string, number, number], relation: string, id_state: string, binding: string, reason: UnboundReason): UnboundMarker {
  return { path, line: at, column, relation, id_state, binding, reason };
}

/** One unbound node, its ID, kind, title and place from the corpus. */
function unboundOf(node: MockNode, rules: number[], mentions: number, markers: UnboundMarker[]): UnboundRow {
  if (node.id === null) {
    throw new Error(`${node.path}: a node without an ID is never selected`);
  }
  return { id: node.id, kind: node.kind, title: node.title, path: node.path, line: node.line, rules, mentions, markers };
}

/**
 * harbor-sim's row 5 under two rules (1: every rule; 2: accepted mechanics, by `implements`), by path
 * bytes, then line: a look-alike marker, an unanchored one, one that cannot be verified, one of
 * another relation, and a node no marker names. Its titles as its corpus has them, the Cyrillic
 * basin name and the pasted markup among them.
 */
function harborUnbound(): UnboundRow[] {
  const node = (id: string) => held(harborFiles(), id).node;
  return [
    unboundOf(node("RULE-BASIN-NAME"), [1], 1, [marker(["src/sim/berths.rs", 30, 4], "implements", "homoglyph", "item", "homoglyph")]),
    unboundOf(node("RULE-MOOR-CREW"), [1], 0, [marker(["src/sim/crew.rs", 21, 4], "implements", "resolved", "unanchored", "unanchored")]),
    unboundOf(node("RULE-MOOR-NIGHT"), [1], 0, [marker(["src/sim/crew.rs", 44, 8], "implements", "resolved", "cannot_verify", "cannot_verify")]),
    unboundOf(node("MEC-FAIRWAY"), [2], 2, [marker(["data/fairway.ron", 2, 5], "configures", "resolved", "ron", "relation")]),
    unboundOf(node("RULE-PILOT-REQ"), [1], 0, []),
  ];
}

/** Byte order of two strings, as the core sorts paths. */
function byBytes(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * The large scenario's row 5: harbor-sim's five and, under a third rule (`paths = ["docs/spec/gen/**"]`),
 * the generated corpus's nodes in path order but the last of them, bound; rules 1 and 3 for a rule.
 */
function largeUnbound(): UnboundRow[] {
  const generated = [...largeDocuments()]
    .sort((a, b) => byBytes(a.path, b.path))
    .flatMap((document) => [document.node, ...document.sections])
    .slice(0, LARGE_UNBOUND - harborUnbound().length)
    .map((node) => unboundOf(node, node.kind === "rule" ? [1, 3] : [3], 0, []));
  return [...harborUnbound(), ...generated].sort((a, b) => byBytes(a.path, b.path) || a.line - b.line || byBytes(a.id, b.id));
}

/** Row 5 measured over `selected` nodes and `rules` rules: the rest of the selected are bound. */
function measuredUnbound(rows: UnboundRow[], selected: number, rules: number): HealthUnbound {
  return { state: "measured", causes: [], counts: { rules, selected, bound: selected - rows.length, unbound: rows.length }, rows };
}

/** harbor-sim's live nodes the two rules select, the five above among them. */
const HARBOR_SELECTED = 14;

/** The generated corpus's nodes, all selected by the large scenario's third rule (src/mocks/harbor-sim/large.ts). */
const LARGE_SELECTED = 3108;

/** Why ledger-api's row 6 could not be measured in `code-cannot`: its spec.lock does not parse, as plain verify words it. */
export const LOCK_CAUSE = "spec.lock:1: unclosed array, expected `]`";

/** Row 6's causes in `code-cannot`, by project: harbor-sim's the code walk's gap, ledger-api's its lock. */
export const DRIFT_CANNOT_CAUSES: Readonly<Record<string, readonly string[]>> = {
  "harbor-sim": [CODE_GAP_CAUSE],
  "ledger-api": [LOCK_CAUSE],
};

/** The ID a look-alike marker of harbor-sim writes: RULE-BASIN-NAME with a Cyrillic capital A, keyed as written. */
export const LOOKALIKE_BASIN = "RULE-BASIN-N\u0410ME";

/** How many rows the large scenario's row 6 lists. */
export const LARGE_DRIFT = 3000;

/** The relations in the lock's order (core `LockRelation`), which verify's key order follows. */
const RELATIONS = ["implements", "verifies", "configures", "assumes"];

/** verify's key order (core `RowKey`): node bytes, relation, file bytes, a keyless row first (by state, category), kind, symbol, item kind. */
function byKey(a: DriftRow, b: DriftRow): number {
  const keyless = (row: DriftRow) => (row.kind === null ? 0 : 1);
  const keylessOrder = keyless(a) === 0 && keyless(b) === 0 ? byBytes(a.state, b.state) || byBytes(a.category ?? "", b.category ?? "") : 0;
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

/** A lock entry of harbor-sim: the revision vouched for, the commit, the UTC date. */
function entry(rev: number, commit: string, verified_at: string): DriftLock {
  return { rev, commit, verified_at };
}

/** The two commits harbor-sim's lock entries and tests vouched at. */
const LOCKED_AT = "4f1c9a2e7b3d5c8e0a6f1b2d3c4e5f60718293a4";
const TESTED_AT = "9e8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3a291807";

/** An item of harbor-sim's code as an anchor names it. */
function itemPlace(file: string, symbol: string): BindingPlace {
  return { file, kind: "item", symbol, item_kind: "function_item" };
}

/** One row 6 line, verify's 17 keys in their order: a keyed binding, unlocked unless given, no lock entry. */
function driftRow(node: string, relation: string, file: string, binding: [kind: string, symbol: string, itemKind: string | null] | null, fields: Partial<DriftRow>): DriftRow {
  return {
    node,
    relation,
    file,
    kind: binding?.[0] ?? null,
    symbol: binding?.[1] ?? null,
    item_kind: binding?.[2] ?? null,
    state: "unlocked",
    category: null,
    detail: null,
    class: "check",
    pre_existing: false,
    markers: [],
    node_rev: null,
    lock: null,
    compared: null,
    levels: null,
    anchor: null,
    ...fields,
  };
}

/**
 * harbor-sim's row 6, in verify's key order: every class and the eight non-ok states. MEC-MOORING's
 * `moor` moved from berths.rs to crew.rs and changed; RULE-HIGH-WATER's test is gone, two of the
 * same name its candidates; the look-alike, unanchored and cannot-verify markers are row 5's.
 */
function harborDrift(): DriftRow[] {
  const exact = { state: "exact", from: null, candidates: null };
  return [
    driftRow("MEC-CARGO-CRANES", "implements", "src/sim/berths.rs", ["module", "harbor_sim::berths", null], { markers: [{ line: 1, rev: 1 }], node_rev: 1 }),
    driftRow("MEC-FAIRWAY", "configures", "data/fairway.ron", ["ron", "fairway.speed_knots", null], { markers: [{ line: 2, rev: 1 }], node_rev: 1 }),
    driftRow("MEC-MOORING", "implements", "src/sim/crew.rs", ["item", "harbor_sim::crew::moor", "function_item"], {
      state: "code_ahead",
      category: "code_changed",
      detail: "body",
      markers: [{ line: 30, rev: 2 }],
      node_rev: 2,
      lock: entry(2, LOCKED_AT, "2026-10-02"),
      compared: "ast",
      levels: { lock: ["sig", "body"], markers: ["sig", "body"], changed: ["body"] },
      anchor: { state: "moved", from: itemPlace("src/sim/berths.rs", "harbor_sim::berths::moor"), candidates: null },
    }),
    driftRow("MEC-TIDES", "implements", "src/sim/tide.rs", ["item", "harbor_sim::tide::tide_window", "function_item"], {
      state: "conflict",
      class: "fix",
      markers: [{ line: 9, rev: 2 }],
      node_rev: 3,
      lock: entry(2, LOCKED_AT, "2026-10-02"),
      compared: "ast",
      levels: { lock: ["sig", "body"], markers: ["sig", "body"], changed: ["sig"] },
      anchor: exact,
    }),
    driftRow(LOOKALIKE_BASIN, "implements", "src/sim/berths.rs", ["item", "harbor_sim::berths::basin_name", "function_item"], {
      state: "bad_marker",
      category: "homoglyph",
      class: "fix",
      markers: [{ line: 30, rev: 1 }],
    }),
    driftRow("RULE-BERTH-DRAFT", "assumes", "data/berths.ron", ["ron", "berths[4].draft_m", null], {
      state: "spec_ahead",
      category: "marker_behind",
      markers: [{ line: 3, rev: 3 }],
      node_rev: 4,
      lock: entry(4, LOCKED_AT, "2026-10-02"),
      compared: "blob",
      anchor: exact,
    }),
    driftRow("RULE-HARBOR-CLOCK", "implements", "src/sim/weather.rs", ["item", "harbor_sim::weather::tick", "function_item"], {
      state: "cannot_verify",
      category: "ambiguous_symbol",
      detail: "duplicate",
      class: "cannot",
      markers: [{ line: 52, rev: 4 }],
      node_rev: 4,
      lock: entry(4, LOCKED_AT, "2026-10-02"),
    }),
    driftRow("RULE-HIGH-WATER", "verifies", "src/sim/tide_table.rs", ["item", "harbor_sim::tide_table::tests::high_water_holds", "function_item"], {
      state: "broken_binding",
      category: "symbol_gone",
      class: "fix",
      node_rev: 1,
      lock: entry(1, LOCKED_AT, "2026-10-02"),
      anchor: {
        state: "ambiguous",
        from: null,
        candidates: [
          itemPlace("src/sim/tide_table.rs", "harbor_sim::tide_table::tests::high_water_holds_at_neap"),
          itemPlace("src/sim/tide_table.rs", "harbor_sim::tide_table::tests::high_water_holds_at_spring"),
        ],
      },
    }),
    driftRow("RULE-MOOR-CREW", "implements", "src/sim/crew.rs", null, { state: "bad_marker", category: "unanchored", class: "fix", markers: [{ line: 21, rev: 2 }], node_rev: 2 }),
    driftRow("RULE-MOOR-NIGHT", "implements", "src/sim/crew.rs", null, { state: "cannot_verify", category: "item_error", class: "cannot", markers: [{ line: 44, rev: 2 }], node_rev: 2 }),
    driftRow("RULE-SPRING-WINDOW", "implements", "src/sim/tide_table.rs", ["item", "harbor_sim::tide_table::spring_window", "function_item"], { markers: [{ line: 58, rev: 1 }], node_rev: 1 }),
    driftRow("RULE-TIDE-WINDOW", "implements", "src/sim/tide.rs", ["item", "harbor_sim::tide::in_window", "function_item"], {
      state: "predated",
      class: "fix",
      markers: [{ line: 21, rev: 4 }],
      node_rev: 3,
      lock: entry(3, LOCKED_AT, "2026-10-02"),
      levels: { lock: ["sig", "body"], markers: ["sig", "body"], changed: null },
      anchor: exact,
    }),
  ];
}

/** harbor-sim's bindings in sync, and the markers naming its queue items (row 4's six), skipped. */
const HARBOR_OK = 9;
const HARBOR_SKIPPED = 6;

/** harbor-sim's `[[tested]]` entries not verified, by node bytes: MEC-TIDES's spec changed, RULE-QUAY-LIGHTS gone (DEC-0014 amends it), RULE-TIDE-WINDOW's marker predated. */
function harborTested(): TestedRow[] {
  return [
    { node: "MEC-TIDES", state: "stale", category: "spec_changed", rev: 2, node_rev: 3, commit: TESTED_AT, verified_at: "2026-10-02" },
    { node: "RULE-QUAY-LIGHTS", state: "node_gone", category: null, rev: 1, node_rev: null, commit: TESTED_AT, verified_at: "2026-10-02" },
    { node: "RULE-TIDE-WINDOW", state: "unverifiable", category: "predated", rev: 3, node_rev: 3, commit: TESTED_AT, verified_at: "2026-10-02" },
  ];
}

/**
 * The large scenario's row 6: harbor-sim's twelve and, before a first accept, the generated
 * corpus's nodes by ID but the last of them, each one `implements` marker `unlocked`; in verify's key order.
 */
function largeDrift(): DriftRow[] {
  const generated = largeDocuments()
    .flatMap((document) => [document.node, ...document.sections])
    .flatMap((node) => (node.id === null ? [] : [node.id]))
    .sort(byBytes)
    .slice(0, LARGE_DRIFT - harborDrift().length)
    .map((id, index) => {
      const domain = /-(\d\d)/.exec(id)?.[1] ?? "00";
      const symbol = `harbor_sim::gen::d${domain}::${id.toLowerCase().replaceAll("-", "_")}`;
      return driftRow(id, "implements", `src/gen/d${domain}.rs`, ["item", symbol, "function_item"], { markers: [{ line: (index % 400) + 1, rev: 1 }], node_rev: 1 });
    });
  return [...harborDrift(), ...generated].sort(byKey);
}

/** verify's counts of `rows` listed, `ok` in sync and `skipped` markers: one per class, as listed. */
function driftCounts(rows: readonly DriftRow[], ok: number, skipped: number): DriftCounts {
  const of = (value: string) => rows.filter((row) => row.class === value).length;
  return { rows: rows.length + ok, ok, fix: of("fix"), check: of("check"), cannot: of("cannot"), pre_existing: 0, skipped };
}

function measuredDrift(rows: DriftRow[], ok: number, skipped: number, tested: TestedRow[]): HealthDrift {
  return { state: "measured", causes: [], counts: driftCounts(rows, ok, skipped), rows, tested, notes: [] };
}

/** Row 6 not measured: its state and causes; no counts, rows or test evidence, never a 0. */
function driftNotMeasured(state: "off" | "cannot", causes: readonly string[]): HealthDrift {
  return { state, causes: [...causes], counts: null, rows: null, tested: null, notes: [] };
}

/** Row 6 of `project` in `scenario`, as `spec health --json` gives it. */
function driftOf(project: string, scenario: Scenario): HealthDrift {
  const harbor = project === HARBOR;
  const none = measuredDrift([], harbor ? 4 : 2, 0, []);
  if (scenario === "empty") {
    return none;
  }
  if (scenario === "code-cannot") {
    return driftNotMeasured("cannot", DRIFT_CANNOT_CAUSES[project] ?? [CODE_GAP_CAUSE]);
  }
  if (!harbor) {
    return scenario === "not-configured" ? none : driftNotMeasured("off", [CODE_OFF_CAUSE]);
  }
  return measuredDrift(scenario === "large" ? largeDrift() : harborDrift(), HARBOR_OK, HARBOR_SKIPPED, harborTested());
}

/** Rows 4-5 and the code count of `project` in `scenario`, as `spec health --json` gives them. */
function codeRowsOf(project: string, scenario: Scenario): Pick<HealthReport, "code" | "assumes" | "unbound"> {
  const harbor = project === HARBOR;
  if (scenario === "empty") {
    return {
      code: codeOf(project),
      assumes: measuredAssumes([], harbor ? 4 : 2),
      unbound: measuredUnbound([], harbor ? 4 : 2, 1),
    };
  }
  if (scenario === "code-cannot") {
    const causes = CODE_CANNOT_CAUSES[project] ?? { assumes: [CODE_GAP_CAUSE], unbound: [CODE_GAP_CAUSE] };
    return { code: harbor ? null : codeOf(project), assumes: notMeasured("cannot", causes.assumes), unbound: notMeasured("cannot", causes.unbound) };
  }
  if (scenario === "not-configured") {
    return {
      code: codeOf(project),
      assumes: harbor ? measuredAssumes(harborAssumes(), 11) : measuredAssumes([], 2),
      unbound: notMeasured("not_configured", [NO_RULE_CAUSE]),
    };
  }
  if (!harbor) {
    return { code: null, assumes: notMeasured("off", [CODE_OFF_CAUSE]), unbound: notMeasured("off", [CODE_OFF_CAUSE]) };
  }
  return {
    code: codeOf(project),
    assumes: measuredAssumes(harborAssumes(), 11),
    unbound: scenario === "large" ? measuredUnbound(largeUnbound(), HARBOR_SELECTED + LARGE_SELECTED, 3) : measuredUnbound(harborUnbound(), HARBOR_SELECTED, 2),
  };
}

/** The report `spec health --json` gives for `project` in `scenario` (`cannot-check`, `error` and `slow` act in the client). */
export function healthReportOf(project: string, scenario: Scenario): HealthReport {
  let amendments: UnappliedAmendment[] = [];
  if (project === HARBOR && scenario !== "empty") {
    amendments = scenario === "large" ? largeAmendments() : harborAmendments();
  }
  const { code, assumes, unbound } = codeRowsOf(project, scenario);
  return { documents: documentsOf(project, scenario), amendments, notes: [], code, assumes, unbound, drift: driftOf(project, scenario) };
}

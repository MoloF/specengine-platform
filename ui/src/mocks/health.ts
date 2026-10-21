import type {
  AssumedNode,
  AssumedProposal,
  AssumedState,
  AssumesCounts,
  AssumesRow,
  HealthAssumes,
  HealthCode,
  HealthDrift,
  HealthReport,
  HealthUnbound,
  ShownLink,
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
// Row 6 (docs/features/health-drift.md "Data"): `off` for either project in every scenario, the
// code layer's sentence its cause, no counts, rows or tested, no note, until 9b-2 draws the row and
// its mocks measure it; nothing reads it yet.

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

/** Row 6, not measured until 9b-2: `off`, the code layer's sentence its cause, never a 0. */
function driftOff(): HealthDrift {
  return { state: "off", causes: [CODE_OFF_CAUSE], counts: null, rows: null, tested: null, notes: [] };
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
  return { documents: documentsOf(project, scenario), amendments, notes: [], code, assumes, unbound, drift: driftOff() };
}

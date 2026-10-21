import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { DriftRow, HealthReport } from "../api/types";
import { checkReportOf } from "./check";
import { nodeViewOf } from "./corpus";
import { harborSim } from "./harbor-sim/fixtures";
import { ASSUMED_STATES, DRIFT_CLASSES, TESTED_STATES, UNBOUND_REASONS } from "../health/labels";
import {
  CODE_CANNOT_CAUSES,
  CODE_GAP_CAUSE,
  CODE_OFF_CAUSE,
  DRIFT_CANNOT_CAUSES,
  healthReportOf,
  LARGE_AMENDMENTS,
  LARGE_DRIFT,
  LARGE_UNBOUND,
  LOCK_CAUSE,
  LOOKALIKE_BASIN,
  NO_RULE_CAUSE,
  REJECTED_NOTE,
  walkGapMessage,
} from "./health";
import { MockClient } from "./MockClient";
import { SCENARIOS } from "./scenario";

// The mock's `health` (docs/features/health-left.md "Data", Mocks, W13): one builder, no clock, the
// core's order; normal, empty, large and cannot-check as the spec lists them, `error` and `slow`
// as every read. Rows 4-5 (docs/features/health-code-ui.md "Data", Mocks, UI-4): each object on its
// type (docs/canon/health-code.md "Document"), every state of either row, code-cannot and
// not-configured. Row 6 (docs/features/health-drift-ui.md "Data", UI-5): each object on its type
// (docs/canon/health-drift.md "Document"), every class, a moved and an ambiguous anchor, the test
// evidence not verified, off, empty, large and code-cannot.

const PROJECTS = ["harbor-sim", "ledger-api"] as const;

/** The JSON's keys of each row 4-5 object, in its order (src/api/daemonKeys.test.ts holds the types to the daemon's). */
const KEYS = {
  HealthCode: ["files", "rs", "ron", "skipped"],
  HealthRow: ["state", "causes", "counts", "rows"],
  AssumesCounts: ["markers", "rows", "pending", "as_assumed", "otherwise", "applied", "rejected", "unknown", "archived", "in_question"],
  AssumesRow: ["id", "state", "path", "line", "column", "relation", "proposal", "node"],
  AssumedProposal: ["kind", "status", "choice", "record_id", "record_path", "reason"],
  AssumedNode: ["id", "path", "line", "status", "successor", "proposals"],
  UnboundCounts: ["rules", "selected", "bound", "unbound"],
  UnboundRow: ["id", "kind", "title", "path", "line", "rules", "mentions", "markers"],
  UnboundMarker: ["path", "line", "column", "relation", "id_state", "binding", "reason"],
  HealthDrift: ["state", "causes", "counts", "rows", "tested", "notes"],
  DriftCounts: ["rows", "ok", "fix", "check", "cannot", "pre_existing", "skipped"],
  DriftRow: [
    "node",
    "relation",
    "file",
    "kind",
    "symbol",
    "item_kind",
    "state",
    "category",
    "detail",
    "class",
    "pre_existing",
    "markers",
    "node_rev",
    "lock",
    "compared",
    "levels",
    "anchor",
  ],
  DriftMarker: ["line", "rev"],
  DriftLock: ["rev", "commit", "verified_at"],
  DriftLevels: ["lock", "markers", "changed"],
  BindingAnchor: ["state", "from", "candidates"],
  BindingPlace: ["file", "kind", "symbol", "item_kind"],
  TestedRow: ["node", "state", "category", "rev", "node_rev", "commit", "verified_at"],
};

/** The relations in the lock's order (core `LockRelation`), verify's key order's second field. */
const RELATIONS = ["implements", "verifies", "configures", "assumes"];

/** verify's key order (core `RowKey`): node bytes, relation, file bytes, keyless first, kind, symbol, item kind. */
function byKey(a: DriftRow, b: DriftRow): number {
  return (
    byBytes(a.node, b.node) ||
    RELATIONS.indexOf(a.relation) - RELATIONS.indexOf(b.relation) ||
    byBytes(a.file, b.file) ||
    (a.kind === null ? 0 : 1) - (b.kind === null ? 0 : 1) ||
    byBytes(a.kind ?? "", b.kind ?? "") ||
    byBytes(a.symbol ?? "", b.symbol ?? "") ||
    byBytes(a.item_kind ?? "", b.item_kind ?? "")
  );
}

async function rejection(promise: Promise<unknown>): Promise<ClientError> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ClientError) {
      return error;
    }
    throw error;
  }
  throw new Error("the call resolved");
}

/** Byte order, as the core sorts paths. */
function byBytes(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

describe("the mock's health read", () => {
  it("normal: harbor-sim's two, the spec's decision with a dangling link and a draft spec amending RULE-BERTH-DRAFT, resolved", () => {
    const report = healthReportOf("harbor-sim", "normal");
    expect(Object.keys(report)).toEqual(["documents", "amendments", "notes", "code", "assumes", "unbound", "drift"]);
    expect(report.amendments.map((amendment) => Object.keys(amendment))).toEqual([
      ["path", "id", "title", "class", "status", "links"],
      ["path", "id", "title", "class", "status", "links"],
    ]);
    expect(report.amendments.map(({ path, id, class: kind, status }) => [path, id, kind, status])).toEqual([
      ["docs/decisions/DEC-0014.md", "DEC-0014", "decision", "accepted"],
      ["docs/features/deep-berth.md", null, "spec", "draft"],
    ]);
    expect(report.amendments.flatMap((amendment) => amendment.links.map(({ type, written, name, state }) => [type, written, name, state]))).toEqual([
      ["amends", "RULE-QUAY-LIGHTS", null, "dangling"],
      ["amends", "RULE-BERTH-DRAFT", "RULE-BERTH-DRAFT", "resolved"],
    ]);
    expect(report.notes).toEqual([]);
    expect(healthReportOf("ledger-api", "normal").amendments).toEqual([]);
  });

  it("normal: each of harbor-sim's two is a document of its corpus, listed as its node and `show --links` give it (AC-R2's rule, mocked)", () => {
    const { corpus } = harborSim(Date.parse("2026-10-09T12:00:00Z"));
    for (const amendment of healthReportOf("harbor-sim", "normal").amendments) {
      const view = nodeViewOf(corpus, amendment.path, { with: ["links"] });
      const node = view.nodes[0];
      expect([amendment.path, view.reason, view.nodes.length]).toEqual([amendment.path, null, 1]);
      expect([node?.id, node?.title, node?.status]).toEqual([amendment.id, amendment.title, amendment.status]);
      expect(node?.text).toContain(`class: ${amendment.class ?? "?"}`);
      expect(node?.links?.outgoing.filter((link) => link.type === "amends")).toEqual(amendment.links);
    }
  });

  it.each(SCENARIOS.filter((scenario) => scenario !== "cannot-check"))(
    "%s: every row 4-5 object holds its type's keys in the JSON's order, nothing else (UI-4 of health-code-ui)",
    (scenario) => {
      for (const project of PROJECTS) {
        const { code, assumes, unbound } = healthReportOf(project, scenario);
        const at = (what: string) => `${project} ${scenario} ${what}`;
        if (code !== null) {
          expect([at("code"), Object.keys(code)]).toEqual([at("code"), KEYS.HealthCode]);
        }
        for (const row of [assumes, unbound]) {
          expect([at("row"), Object.keys(row)]).toEqual([at("row"), KEYS.HealthRow]);
        }
        if (assumes.counts !== null) {
          expect([at("assumes counts"), Object.keys(assumes.counts)]).toEqual([at("assumes counts"), KEYS.AssumesCounts]);
        }
        for (const line of assumes.rows ?? []) {
          expect([at(line.id), Object.keys(line)]).toEqual([at(line.id), KEYS.AssumesRow]);
          if (line.proposal !== null) {
            expect([at(line.id), Object.keys(line.proposal)]).toEqual([at(line.id), KEYS.AssumedProposal]);
          }
          if (line.node !== null) {
            expect([at(line.id), Object.keys(line.node)]).toEqual([at(line.id), KEYS.AssumedNode]);
          }
        }
        if (unbound.counts !== null) {
          expect([at("unbound counts"), Object.keys(unbound.counts)]).toEqual([at("unbound counts"), KEYS.UnboundCounts]);
        }
        for (const node of unbound.rows ?? []) {
          expect([at(node.id), Object.keys(node)]).toEqual([at(node.id), KEYS.UnboundRow]);
          for (const marker of node.markers) {
            expect([at(node.id), Object.keys(marker)]).toEqual([at(node.id), KEYS.UnboundMarker]);
          }
        }
      }
    },
  );

  it.each(SCENARIOS.filter((scenario) => scenario !== "cannot-check"))(
    "%s: a row measured has counts, rows and no cause; any other none of them and a cause; the counts sum to the rows; the daemon's order (UI-4)",
    (scenario) => {
      for (const project of PROJECTS) {
        const { code, assumes, unbound } = healthReportOf(project, scenario);
        for (const row of [assumes, unbound]) {
          if (row.state === "measured") {
            expect([project, row.causes, row.counts === null, row.rows === null]).toEqual([project, [], false, false]);
            expect(code).not.toBeNull();
          } else {
            expect([project, row.counts, row.rows, row.causes.length > 0]).toEqual([project, null, null, true]);
          }
        }
        if (assumes.counts !== null && assumes.rows !== null) {
          const { markers, rows, ...states } = assumes.counts;
          expect([project, rows, markers >= rows]).toEqual([project, assumes.rows.length, true]);
          expect(Object.values(states).reduce((sum, count) => sum + count, 0)).toBe(rows);
          for (const state of ASSUMED_STATES) {
            expect([project, state, states[state as keyof typeof states]]).toEqual([project, state, assumes.rows.filter((line) => line.state === state).length]);
          }
          const keys = assumes.rows.map((line) => [line.id, line.path, line.line, line.column] as const);
          expect(keys).toEqual([...keys].sort((a, b) => byBytes(a[0], b[0]) || byBytes(a[1], b[1]) || a[2] - b[2] || a[3] - b[3]));
        }
        if (unbound.counts !== null && unbound.rows !== null) {
          const { rules, selected, bound } = unbound.counts;
          expect([project, unbound.counts.unbound, bound + unbound.counts.unbound]).toEqual([project, unbound.rows.length, selected]);
          expect(unbound.rows.every((node) => node.rules.length > 0 && node.rules.every((rule) => rule >= 1 && rule <= rules))).toBe(true);
          const keys = unbound.rows.map((node) => [node.path, node.line, node.id] as const);
          expect(keys).toEqual([...keys].sort((a, b) => byBytes(a[0], b[0]) || a[1] - b[1] || byBytes(a[2], b[2])));
        }
      }
    },
  );

  it.each(["normal", "error", "slow", "conflict", "large", "editorial"] as const)("%s: ledger-api has no [code] table, both rows off, the code layer's sentence their cause, code null", (scenario) => {
    const { code, assumes, unbound } = healthReportOf("ledger-api", scenario);
    expect(code).toBeNull();
    for (const row of [assumes, unbound]) {
      expect(row).toEqual({ state: "off", causes: [CODE_OFF_CAUSE], counts: null, rows: null });
    }
  });

  it.each(SCENARIOS.filter((scenario) => scenario !== "cannot-check"))(
    "%s: every row 6 object holds its type's keys in the JSON's order, nothing else (UI-5 of health-drift-ui)",
    (scenario) => {
      for (const project of PROJECTS) {
        const { drift } = healthReportOf(project, scenario);
        const at = (what: string) => `${project} ${scenario} ${what}`;
        expect([at("drift"), Object.keys(drift)]).toEqual([at("drift"), KEYS.HealthDrift]);
        if (drift.counts !== null) {
          expect([at("counts"), Object.keys(drift.counts)]).toEqual([at("counts"), KEYS.DriftCounts]);
        }
        for (const row of drift.rows ?? []) {
          expect([at(row.node), Object.keys(row)]).toEqual([at(row.node), KEYS.DriftRow]);
          for (const marker of row.markers) {
            expect([at(row.node), Object.keys(marker)]).toEqual([at(row.node), KEYS.DriftMarker]);
          }
          if (row.lock !== null) {
            expect([at(row.node), Object.keys(row.lock)]).toEqual([at(row.node), KEYS.DriftLock]);
          }
          if (row.levels !== null) {
            expect([at(row.node), Object.keys(row.levels)]).toEqual([at(row.node), KEYS.DriftLevels]);
          }
          if (row.anchor !== null) {
            expect([at(row.node), Object.keys(row.anchor)]).toEqual([at(row.node), KEYS.BindingAnchor]);
            for (const place of [row.anchor.from, ...(row.anchor.candidates ?? [])].filter((candidate) => candidate !== null)) {
              expect([at(row.node), Object.keys(place)]).toEqual([at(row.node), KEYS.BindingPlace]);
            }
          }
        }
        for (const entry of drift.tested ?? []) {
          expect([at(entry.node), Object.keys(entry)]).toEqual([at(entry.node), KEYS.TestedRow]);
        }
      }
    },
  );

  it.each(SCENARIOS.filter((scenario) => scenario !== "cannot-check"))(
    "%s: row 6 measured has counts, rows and test evidence, no cause; any other state none of them, a cause, never 0; verify's counts; the key order; no ok row, no verified entry (UI-5)",
    (scenario) => {
      for (const project of PROJECTS) {
        const { drift } = healthReportOf(project, scenario);
        expect([project, drift.notes]).toEqual([project, []]);
        if (drift.state !== "measured") {
          expect([project, drift.counts, drift.rows, drift.tested, drift.causes.length > 0]).toEqual([project, null, null, null, true]);
          continue;
        }
        expect([project, drift.causes, drift.counts === null, drift.rows === null, drift.tested === null]).toEqual([project, [], false, false, false]);
        const counts = drift.counts;
        const rows = drift.rows ?? [];
        if (counts === null) {
          throw new Error("measured without counts");
        }
        expect([project, counts.pre_existing, counts.fix + counts.check + counts.cannot, counts.rows]).toEqual([project, 0, rows.length, rows.length + counts.ok]);
        for (const value of DRIFT_CLASSES) {
          expect([project, value, counts[value as "fix" | "check" | "cannot"]]).toEqual([project, value, rows.filter((row) => row.class === value).length]);
        }
        expect(rows.every((row) => row.class !== null && row.state !== "ok" && !row.pre_existing)).toBe(true);
        expect(rows.map((row) => `${row.node} ${row.relation} ${row.file}`)).toEqual([...rows].sort(byKey).map((row) => `${row.node} ${row.relation} ${row.file}`));
        const tested = drift.tested ?? [];
        expect(tested.every((entry) => entry.state !== "verified")).toBe(true);
        expect(tested.map((entry) => entry.node)).toEqual(tested.map((entry) => entry.node).sort(byBytes));
      }
    },
  );

  it.each(["normal", "error", "slow", "conflict", "large", "editorial"] as const)("%s: ledger-api's row 6 is off, the code layer's sentence its cause, nothing counted", (scenario) => {
    expect(healthReportOf("ledger-api", scenario).drift).toEqual({ state: "off", causes: [CODE_OFF_CAUSE], counts: null, rows: null, tested: null, notes: [] });
  });

  it("normal: harbor-sim's row 6 holds every class, a moved and an ambiguous anchor, a look-alike ID, keyless rows; its evidence a stale, an unverifiable and a gone node's", () => {
    const { drift } = healthReportOf("harbor-sim", "normal");
    const rows = drift.rows ?? [];
    expect(rows.map((row) => [row.node, row.class, row.state, row.category])).toEqual([
      ["MEC-CARGO-CRANES", "check", "unlocked", null],
      ["MEC-FAIRWAY", "check", "unlocked", null],
      ["MEC-MOORING", "check", "code_ahead", "code_changed"],
      ["MEC-TIDES", "fix", "conflict", null],
      [LOOKALIKE_BASIN, "fix", "bad_marker", "homoglyph"],
      ["RULE-BERTH-DRAFT", "check", "spec_ahead", "marker_behind"],
      ["RULE-HARBOR-CLOCK", "cannot", "cannot_verify", "ambiguous_symbol"],
      ["RULE-HIGH-WATER", "fix", "broken_binding", "symbol_gone"],
      ["RULE-MOOR-CREW", "fix", "bad_marker", "unanchored"],
      ["RULE-MOOR-NIGHT", "cannot", "cannot_verify", "item_error"],
      ["RULE-SPRING-WINDOW", "check", "unlocked", null],
      ["RULE-TIDE-WINDOW", "fix", "predated", null],
    ]);
    expect(new Set(rows.map((row) => row.class))).toEqual(new Set(DRIFT_CLASSES));
    expect(drift.counts).toEqual({ rows: 21, ok: 9, fix: 5, check: 5, cannot: 2, pre_existing: 0, skipped: 6 });
    // In key order, so neither class, state nor file is: a sort by any of them in the UI would show.
    for (const field of ["class", "state", "file"] as const) {
      const words = rows.map((row) => row[field] ?? "");
      expect([field, words]).not.toEqual([field, [...words].sort(byBytes)]);
    }
    expect(rows.find((row) => row.anchor?.state === "moved")?.anchor?.from).toEqual({ file: "src/sim/berths.rs", kind: "item", symbol: "harbor_sim::berths::moor", item_kind: "function_item" });
    expect(rows.find((row) => row.anchor?.state === "ambiguous")?.anchor?.candidates).toHaveLength(2);
    expect(rows.filter((row) => row.kind === null).map((row) => row.node)).toEqual(["RULE-MOOR-CREW", "RULE-MOOR-NIGHT"]);
    expect(LOOKALIKE_BASIN).toContain(String.fromCodePoint(0x410));
    expect(drift.tested?.map((entry) => [entry.node, entry.state])).toEqual([
      ["MEC-TIDES", "stale"],
      ["RULE-QUAY-LIGHTS", "node_gone"],
      ["RULE-TIDE-WINDOW", "unverifiable"],
    ]);
    expect(new Set([...(drift.tested ?? []).map((entry) => entry.state), "verified"])).toEqual(new Set(TESTED_STATES));
  });

  it("normal: every node a row 6 line or entry names is a node of harbor-sim's corpus but the look-alike and the gone one", () => {
    const { corpus } = harborSim(Date.parse("2026-10-09T12:00:00Z"));
    const { drift } = healthReportOf("harbor-sim", "normal");
    const named = [...(drift.rows ?? []).map((row) => row.node), ...(drift.tested ?? []).map((entry) => entry.node)];
    for (const id of new Set(named)) {
      if (id === LOOKALIKE_BASIN) {
        // Refused with its Latin fix, as `spec show` refuses a look-alike.
        expect(() => nodeViewOf(corpus, id)).toThrow(/look-alike letters; IDs are Latin only: write `RULE-BASIN-NAME`/);
        continue;
      }
      const view = nodeViewOf(corpus, id, { with: ["links"], archive: true });
      expect([id, view.nodes.length]).toEqual([id, id === "RULE-QUAY-LIGHTS" ? 0 : 1]);
    }
  });

  it("empty: row 6 measured, none, over the bindings it judged, for either project", () => {
    for (const project of PROJECTS) {
      const { drift } = healthReportOf(project, "empty");
      expect([project, drift.state, drift.rows, drift.tested, drift.counts?.fix, drift.counts?.check, drift.counts?.cannot]).toEqual([project, "measured", [], [], 0, 0, 0]);
      expect(drift.counts?.rows).toBeGreaterThan(0);
    }
  });

  it("large: harbor-sim's row 6 lists 3 000 rows in key order, its twelve among them, each key once, the generated ones unlocked", () => {
    const { drift } = healthReportOf("harbor-sim", "large");
    const rows = drift.rows ?? [];
    expect([rows.length, drift.counts?.rows]).toEqual([LARGE_DRIFT, LARGE_DRIFT + 9]);
    expect(new Set(rows.map((row) => `${row.node} ${row.relation} ${row.file} ${row.symbol ?? ""}`)).size).toBe(LARGE_DRIFT);
    const normal = healthReportOf("harbor-sim", "normal").drift.rows ?? [];
    expect(normal.every((row) => rows.some((listed) => listed.node === row.node && listed.file === row.file))).toBe(true);
    expect(rows.filter((row) => row.node.includes("-GEN-")).every((row) => row.state === "unlocked" && row.class === "check")).toBe(true);
  });

  it("code-cannot: row 6 could not be measured for either project, its causes as plain verify words them, raw: harbor-sim's the code walk's gap, ledger-api's its lock", () => {
    for (const project of PROJECTS) {
      const { drift } = healthReportOf(project, "code-cannot");
      expect([project, drift]).toEqual([project, { state: "cannot", causes: DRIFT_CANNOT_CAUSES[project], counts: null, rows: null, tested: null, notes: [] }]);
    }
    expect(DRIFT_CANNOT_CAUSES["harbor-sim"]).toEqual([CODE_GAP_CAUSE]);
    expect(DRIFT_CANNOT_CAUSES["ledger-api"]).toEqual([LOCK_CAUSE]);
  });

  it("not-configured: row 6 as normal for harbor-sim, measured and none for ledger-api", () => {
    expect(healthReportOf("harbor-sim", "not-configured").drift).toEqual(healthReportOf("harbor-sim", "normal").drift);
    expect(healthReportOf("ledger-api", "not-configured").drift.rows).toEqual([]);
  });

  it("normal: harbor-sim's row 4 holds a line in each of the eight states, its row 5 a marker of each reason and a node no marker names", () => {
    const { code, assumes, unbound } = healthReportOf("harbor-sim", "normal");
    expect(code).toEqual({ files: 9, rs: 7, ron: 2, skipped: 0 });
    expect(assumes.rows?.map((line) => [line.id, line.state])).toEqual([
      ["MEC-OLD-QUAYS", "archived"],
      ["PR-0012", "applied"],
      ["PR-0019", "as_assumed"],
      ["PR-0023", "otherwise"],
      ["PR-0027", "rejected"],
      ["PR-0041", "pending"],
      ["PR-7", "unknown"],
      ["RULE-BERTH-DRAFT", "in_question"],
    ]);
    expect(new Set(assumes.rows?.map((line) => line.state))).toEqual(new Set(ASSUMED_STATES));
    // Ordered by ID, so a line's path order differs: a sort by place in the UI would show.
    const paths = assumes.rows?.map((line) => line.path) ?? [];
    expect(paths).not.toEqual([...paths].sort(byBytes));
    expect(unbound.rows?.map((node) => [node.id, node.markers.map((marker) => marker.reason)])).toEqual([
      ["RULE-BASIN-NAME", ["homoglyph"]],
      ["RULE-MOOR-CREW", ["unanchored"]],
      ["RULE-MOOR-NIGHT", ["cannot_verify"]],
      ["MEC-FAIRWAY", ["relation"]],
      ["RULE-PILOT-REQ", []],
    ]);
    expect(new Set(unbound.rows?.flatMap((node) => node.markers.map((marker) => marker.reason)))).toEqual(new Set(UNBOUND_REASONS));
    // Ordered by path, so the IDs are not: a sort by ID in the UI would show.
    const ids = unbound.rows?.map((node) => node.id) ?? [];
    expect(ids).not.toEqual([...ids].sort(byBytes));
    expect(unbound.counts).toEqual({ rules: 2, selected: 14, bound: 9, unbound: 5 });
  });

  it("normal: every node a row names is a node of harbor-sim's corpus, its place, kind and title as the corpus has them; every open item it names is in its queue", () => {
    const { corpus, proposals } = harborSim(Date.parse("2026-10-09T12:00:00Z"));
    const { assumes, unbound } = healthReportOf("harbor-sim", "normal");
    const nodeOf = (id: string) => {
      const view = nodeViewOf(corpus, id, { with: ["links"], archive: true });
      expect([id, view.reason, view.nodes.length]).toEqual([id, null, 1]);
      return view.nodes[0];
    };
    for (const node of unbound.rows ?? []) {
      const shown = nodeOf(node.id);
      expect([node.id, shown?.path, shown?.line, shown?.kind, shown?.title]).toEqual([node.id, node.path, node.line, node.kind, node.title]);
    }
    const queue = new Map(proposals.map(({ review }) => [review.id, review]));
    for (const line of assumes.rows ?? []) {
      if (line.node !== null) {
        const shown = nodeOf(line.node.id);
        expect([line.id, shown?.path, shown?.line]).toEqual([line.id, line.node.path, line.node.line]);
        for (const id of line.node.proposals) {
          expect([id, queue.get(id)?.status, queue.get(id)?.target_ids.includes(line.node.id)]).toEqual([id, "open", true]);
        }
      }
      if (line.proposal?.status === "open") {
        expect([line.id, queue.get(line.id)?.kind, queue.get(line.id)?.status]).toEqual([line.id, line.proposal.kind, "open"]);
      }
      if (line.proposal?.record_id != null) {
        expect(nodeOf(line.proposal.record_id)?.path).toBe(line.proposal.record_path);
      }
    }
  });

  it("normal: texts stored raw, as the JSON carries them: the rejection's note holds a line feed and a right-to-left override", () => {
    const rejected = healthReportOf("harbor-sim", "normal").assumes.rows?.find((line) => line.state === "rejected");
    expect(rejected?.proposal?.reason).toBe(REJECTED_NOTE);
    expect(REJECTED_NOTE).toContain(String.fromCharCode(10));
    expect(REJECTED_NOTE).toContain(String.fromCodePoint(0x202e));
  });

  it("empty: both rows measured, none, over what was read, for either project", () => {
    for (const project of PROJECTS) {
      const { code, assumes, unbound } = healthReportOf(project, "empty");
      expect([project, code === null, assumes.state, assumes.rows, unbound.state, unbound.rows]).toEqual([project, false, "measured", [], "measured", []]);
      expect(assumes.counts?.markers).toBeGreaterThan(0);
      expect(unbound.counts?.selected).toBeGreaterThan(0);
    }
  });

  it("large: harbor-sim's row 5 lists 3 000 unbound nodes by path, its five among them, each once", () => {
    const { unbound } = healthReportOf("harbor-sim", "large");
    const rows = unbound.rows ?? [];
    expect([rows.length, unbound.counts?.unbound]).toEqual([LARGE_UNBOUND, LARGE_UNBOUND]);
    expect(new Set(rows.map((node) => `${node.id} ${node.path}:${String(node.line)}`)).size).toBe(LARGE_UNBOUND);
    expect(rows.slice(0, 4).map((node) => node.id)).toEqual(["RULE-BASIN-NAME", "RULE-MOOR-CREW", "RULE-MOOR-NIGHT", "MEC-FAIRWAY"]);
    expect(rows.at(-1)?.id).toBe("RULE-PILOT-REQ");
  });

  it("code-cannot: rows 4-5 could not be measured, each cause as the CLI words it, raw; row 3 as normal", () => {
    for (const project of PROJECTS) {
      const report = healthReportOf(project, "code-cannot");
      const causes = CODE_CANNOT_CAUSES[project];
      expect([project, report.assumes.state, report.assumes.causes]).toEqual([project, "cannot", causes?.assumes]);
      expect([project, report.unbound.state, report.unbound.causes]).toEqual([project, "cannot", causes?.unbound]);
      expect(report.amendments).toEqual(healthReportOf(project, "normal").amendments);
    }
    // The code walk failed: no code count; the queue and the rule failing leave it.
    expect(healthReportOf("harbor-sim", "code-cannot").code).toBeNull();
    expect(healthReportOf("ledger-api", "code-cannot").code).not.toBeNull();
    expect(CODE_GAP_CAUSE).toContain(String.fromCharCode(0x1b));
  });

  it("not-configured: row 5 without a rule for either project, its sentence the cause; row 4 measured", () => {
    for (const project of PROJECTS) {
      const { assumes, unbound } = healthReportOf(project, "not-configured");
      expect([project, unbound]).toEqual([project, { state: "not_configured", causes: [NO_RULE_CAUSE], counts: null, rows: null }]);
      expect([project, assumes.state]).toEqual([project, "measured"]);
    }
  });

  it("empty: none for either project", () => {
    for (const project of PROJECTS) {
      expect(healthReportOf(project, "empty").amendments).toEqual([]);
    }
  });

  it("large: harbor-sim's 3 000 by path, each with one link, never empty", () => {
    const { amendments } = healthReportOf("harbor-sim", "large");
    expect(amendments).toHaveLength(LARGE_AMENDMENTS);
    const paths = amendments.map((amendment) => amendment.path);
    expect(paths).toEqual([...paths].sort(byBytes));
    expect(new Set(paths).size).toBe(LARGE_AMENDMENTS);
    expect(amendments.every((amendment) => amendment.links.length === 1)).toBe(true);
    expect(new Set(amendments.flatMap((amendment) => amendment.links.map((link) => link.state)))).toEqual(new Set(["resolved", "dangling"]));
  });

  it.each(SCENARIOS.filter((scenario) => scenario !== "cannot-check"))("%s: documents are the check's counts.documents for the same project", (scenario) => {
    for (const project of PROJECTS) {
      expect([project, healthReportOf(project, scenario).documents]).toEqual([project, checkReportOf(project, scenario).counts.documents]);
    }
  });

  it("returns copies the caller cannot change", async () => {
    const client = new MockClient("normal");
    const first: HealthReport = await client.getHealth("harbor-sim");
    first.amendments.pop();
    expect((await client.getHealth("harbor-sim")).amendments).toHaveLength(2);
  });

  it("cannot-check: a 503 naming the walk's first gap, as the CLI words it; the check's causes hold it", async () => {
    const client = new MockClient("cannot-check");
    for (const project of PROJECTS) {
      const error = await rejection(client.getHealth(project));
      expect([error.status, error.message]).toEqual([503, walkGapMessage(project)]);
      expect(error.message).toMatch(/^spec: the walk is incomplete: .+: unapplied amendments not measured$/);
      const first = checkReportOf(project, "cannot-check").cannot_check[0];
      expect(error.message).toContain(`\`${first?.path ?? "?"}\``);
    }
  });

  it("error: a 503 as every read; an unknown project its 404", async () => {
    const error = await rejection(new MockClient("error").getHealth("harbor-sim"));
    expect(error.status).toBe(503);
    expect((await rejection(new MockClient("normal").getHealth("zeta"))).status).toBe(404);
  });

  it("slow: answers after the scenario's delay", async () => {
    vi.useFakeTimers();
    try {
      const client = new MockClient("slow");
      let answered = false;
      void client.getHealth("harbor-sim").then(() => {
        answered = true;
      });
      await vi.advanceTimersByTimeAsync(1_000);
      expect(answered).toBe(false);
      await vi.advanceTimersByTimeAsync(600);
      expect(answered).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });
});

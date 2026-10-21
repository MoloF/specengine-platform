import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { HealthReport } from "../api/types";
import { checkReportOf } from "./check";
import { nodeViewOf } from "./corpus";
import { harborSim } from "./harbor-sim/fixtures";
import { ASSUMED_STATES, UNBOUND_REASONS } from "../health/labels";
import {
  CODE_CANNOT_CAUSES,
  CODE_GAP_CAUSE,
  CODE_OFF_CAUSE,
  healthReportOf,
  LARGE_AMENDMENTS,
  LARGE_UNBOUND,
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
// not-configured. Row 6 (docs/features/health-drift.md "Data", UI-1): `off` everywhere until 9b-2.

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
};

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
    "%s: row 6 is off for either project, its keys in the JSON's order, the code layer's sentence its cause, nothing counted, never 0 (UI-1 of health-drift)",
    (scenario) => {
      for (const project of PROJECTS) {
        const { drift } = healthReportOf(project, scenario);
        expect([project, Object.keys(drift)]).toEqual([project, KEYS.HealthDrift]);
        expect([project, drift]).toEqual([project, { state: "off", causes: [CODE_OFF_CAUSE], counts: null, rows: null, tested: null, notes: [] }]);
      }
    },
  );

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

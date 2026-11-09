import { describe, expect, it } from "vitest";
import type { FlowOptions } from "../api/client";
import type { FlowEdge, FlowView } from "../api/types";
import { GAP_REMEDIES as BUILDERS_REMEDIES } from "../test/builders";
import { GAP_REMEDIES } from "./flow";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The mock's `spec flow` (docs/features/flow-canvas.md "Mock"), as `docs/canon/spec-flow.md` words
// it: the keys, the scope, the walk restarted per read, the fold and the lifted edges.

function read(options: FlowOptions, project = "harbor-sim", scenario: Scenario = "normal"): Promise<FlowView> {
  return new MockClient(scenario, { delayMs: 0 }).getFlow(project, options);
}

const names = (view: FlowView) => view.nodes.map((node) => node.id ?? node.path);
const line = (edge: FlowEdge) => `${edge.from} -> ${edge.to} | ${edge.type}${edge.kind === null ? "" : ` | ${edge.kind}`} | x${String(edge.count)}`;

describe("harbor-sim's flow", () => {
  it("folds start and end into what shows them, at every depth, Top and a group's read alike, and moves no gap row (AC-02 of trace-views)", async () => {
    const top = await read({ depth: 0 });
    expect([top.nodes[0]?.start, top.nodes[0]?.end]).toEqual([true, true]);
    const group = await read({ root: "DOM-HARBOR", depth: 1 });
    const at = (name: string) => group.nodes.find((node) => node.id === name);
    expect([at("DOM-WATER")?.start, at("DOM-WATER")?.end]).toEqual([true, false]);
    expect([at("DOM-BERTHS")?.start, at("DOM-BERTHS")?.end]).toEqual([false, true]);
    expect([at("MEC-PILOTAGE")?.start, at("MEC-PILOTAGE")?.end]).toEqual([false, false]);
    expect(top.gaps).toEqual((await read({})).gaps);
  });

  it("serves a node's own stage, and no rollup or counts without `trace` (AC-03 of trace-views)", async () => {
    const plain = await read({ root: "DOM-HARBOR", depth: 1 });
    const at = plain.nodes.find((node) => node.id === "MEC-PILOTAGE");
    expect([at?.stage, at?.rollup, at?.counts]).toEqual(["landfall", null, null]);
    expect(plain.nodes.find((node) => node.id === "DOM-WATER")?.stage).toBeNull();
  });

  it("joins each box's rollup and counts with `trace`, each as the trace read says it (AC-03 of trace-views)", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const view = await client.getFlow("harbor-sim", { root: "DOM-HARBOR", depth: 1, trace: true });
    const rows = (await client.getTrace("harbor-sim")).nodes;
    for (const node of view.nodes) {
      const row = rows.find((candidate) => candidate.id === node.id);
      expect([node.id, node.rollup, node.counts]).toEqual([node.id, row?.rollup, row?.counts]);
    }
    expect(view.nodes.find((node) => node.id === "DOM-WATER")?.counts?.confirmed).toBe(2);
  });

  it("answers depth 0 with one top group: every document it holds folded, no edge left over", async () => {
    const top = await read({ depth: 0 });
    expect(Object.keys(top)).toEqual(["reason", "root", "depth", "notes", "left_out", "truncated", "omitted_nodes", "omitted_edges", "gaps", "nodes", "edges"]);
    expect(names(top)).toEqual(["DOM-HARBOR"]);
    expect(top.nodes[0]?.holds).toBe(14);
    expect(top.nodes[0]?.gaps).toBe(5);
    expect(top.edges).toEqual([]);
    expect(top.nodes[0]?.inner.map((inner) => inner.type)).toEqual(["follows", "supplies", "uses"]);
    expect(top.gaps.map((gap) => gap.code)).toEqual(["no-start", "two-starts", "unreachable", "no-way-to-end", "trap-cycle", "unjoined-parallel", "cycle-no-repeat", "definition-unused", "definition-once", "supply-unfed"]);
  });

  it("answers a group with itself and its children, each at the depth of its own walk", async () => {
    const group = await read({ root: "DOM-HARBOR", depth: 1 });
    expect(names(group).sort()).toEqual(["DOM-BERTHS", "DOM-HARBOR", "DOM-PILOTS", "DOM-WATER", "MEC-FAIRWAY", "MEC-PILOTAGE"]);
    const at = (name: string) => group.nodes.find((node) => node.id === name);
    expect(at("DOM-WATER")?.distance).toBe(0);
    expect(at("DOM-WATER")?.group).toBe("DOM-HARBOR");
    expect(at("DOM-WATER")?.holds).toBe(4);
    expect(at("DOM-HARBOR")?.distance).toBeNull();
    expect(at("DOM-HARBOR")?.holds).toBe(0);
  });

  it("holds the three flow link types and every kind, against the walk with a kind and without; no `depends_on` (writer-studio)", async () => {
    const group = await read({ root: "DOM-HARBOR", depth: 1 });
    expect(group.edges.map(line)).toEqual(
      expect.arrayContaining([
        "DOM-BERTHS -> MEC-PILOTAGE | follows | repeat | x1",
        "MEC-FAIRWAY -> MEC-PILOTAGE | follows | x1",
        "MEC-PILOTAGE -> MEC-FAIRWAY | follows | alternative | x1",
        "DOM-BERTHS -> DOM-WATER | uses | x1",
        "DOM-WATER -> DOM-BERTHS | supplies | x1",
        "MEC-FAIRWAY -> DOM-BERTHS | follows | alternative | x1",
        "MEC-FAIRWAY -> DOM-BERTHS | follows | parallel | x1",
        "DOM-WATER -> DOM-PILOTS | follows | x1",
        "DOM-BERTHS -> DOM-PILOTS | follows | x1",
      ]),
    );
    for (const project of ["harbor-sim", "ledger-api"]) {
      const whole = await read({}, project);
      expect(new Set(whole.edges.map((edge) => edge.type))).toEqual(new Set(project === "harbor-sim" ? ["follows", "uses", "supplies"] : ["follows"]));
    }
  });

  it("restarts the walk per read: a child entered from outside is its lane's start, no crossing edge in the read", async () => {
    const berths = await read({ root: "DOM-BERTHS", depth: 1 });
    const mooring = berths.nodes.find((node) => node.id === "MEC-MOORING");
    expect([mooring?.distance, mooring?.start, mooring?.end]).toEqual([0, true, true]);
    expect(berths.edges).toEqual([]);
  });

  it("holds a child with no flow link in a rooted read only, off the walk", async () => {
    const berths = await read({ root: "DOM-BERTHS", depth: 1 });
    const linkless = berths.nodes.find((node) => node.id === null);
    expect(linkless?.distance).toBeNull();
    expect(linkless?.group).toBe("DOM-BERTHS");
    const whole = await read({ depth: 0 });
    expect(whole.nodes[0]?.holds).toBeLessThan(15);
  });

  it("has a gap item two levels down: the closed group counts it by its own read's rows", async () => {
    const group = await read({ root: "DOM-HARBOR", depth: 1 });
    expect(group.nodes.find((node) => node.id === "DOM-WATER")?.gaps).toBe(2);
    const water = await read({ root: "DOM-WATER", depth: 1 });
    expect(water.nodes.find((node) => node.id === "MEC-TIDES")?.gaps).toBe(1);
    const tides = await read({ root: "MEC-TIDES", depth: 1 });
    expect(tides.nodes.find((node) => node.id === "MEC-TIDE-TABLES")?.gaps).toBe(1);
    expect(tides.nodes.find((node) => node.id === "MEC-TIDE-TABLES")?.distance).toBeNull();
    expect((await read({ depth: 0 })).gaps.find((gap) => gap.code === "definition-unused")?.items).toEqual([["MEC-TIDE-TABLES"]]);
  });

  it("keeps a self-edge on the box that shows it, in `inner` once folded", async () => {
    const tides = await read({ root: "MEC-TIDES", depth: 1 });
    expect(tides.edges.map(line)).toContain("RULE-SPRING-WINDOW -> RULE-SPRING-WINDOW | follows | repeat | x1");
    const water = await read({ root: "DOM-WATER", depth: 1 });
    expect(water.edges.map(line).join()).not.toContain("RULE-SPRING-WINDOW");
  });

  it("cuts a stage after the fold, across groups, under its ROOT: each kept node and edge its own, the rows the read's (writer-studio)", async () => {
    const scope = await read({ root: "DOM-HARBOR", trace: true });
    const landfall = await read({ root: "DOM-HARBOR", stage: "landfall", trace: true });
    expect(names(landfall).sort()).toEqual(["MEC-PILOT-CALL", "MEC-PILOTAGE", "RULE-HIGH-WATER", "RULE-SPRING-WINDOW"]);
    expect(new Set(landfall.nodes.map((node) => node.group))).toEqual(new Set(["DOM-WATER", "MEC-TIDES", "DOM-HARBOR", "DOM-PILOTS"]));
    for (const node of landfall.nodes) {
      expect(node).toEqual(scope.nodes.find((other) => other.id === node.id));
    }
    // The edges whose two ends are kept: MEC-PILOTAGE's to MEC-FAIRWAY, in passage, is not.
    expect(landfall.edges.map(line).sort()).toEqual([
      "RULE-HIGH-WATER -> RULE-SPRING-WINDOW | follows | x1",
      "RULE-SPRING-WINDOW -> MEC-PILOT-CALL | follows | x1",
      "RULE-SPRING-WINDOW -> MEC-PILOTAGE | follows | x1",
      "RULE-SPRING-WINDOW -> RULE-SPRING-WINDOW | follows | repeat | x1",
    ]);
    expect(landfall.gaps).toEqual(scope.gaps);
    expect([landfall.root, landfall.left_out, landfall.notes]).toEqual([scope.root, scope.left_out, scope.notes]);
    // `-`: the shown steps with no stage, never a group or a definition.
    expect(names(await read({ root: "DOM-HARBOR", stage: "-" }))).toEqual(["MEC-NIGHT-PASSAGE"]);
    expect((await read({ root: "DOM-BERTHS", stage: "quayside" })).nodes.map((node) => [node.id, node.group])).toEqual([["MEC-MOORING", "DOM-BERTHS"]]);
    expect((await read({ root: "DOM-HARBOR", stage: "nowhere" })).nodes).toEqual([]);
  });

  it("serves a ROOT Top's rows, keeping the items with a member in scope: the mock's reading", async () => {
    const top = await read({ depth: 0 });
    const fairway = await read({ root: "MEC-FAIRWAY", depth: 1 });
    const kept = (view: FlowView, code: string) => view.gaps.find((gap) => gap.code === code)?.items;
    expect(kept(fairway, "cycle-no-repeat")).toEqual(kept(top, "cycle-no-repeat"));
    expect(kept(fairway, "unjoined-parallel")).toEqual([["MEC-NIGHT-PASSAGE"]]);
    expect(kept(fairway, "definition-unused")).toEqual([]);
  });

  it("answers a ROOT naming nothing with the exit-1 document, as data; a bad depth is refused in the CLI's words", async () => {
    const nothing = await read({ root: "NOPE-1", depth: 1 });
    expect(nothing.reason).not.toBeNull();
    expect(nothing.nodes).toEqual([]);
    await expect(read({ depth: -1 })).rejects.toThrow(/--depth -1/);
  });
});

describe("ledger-api's flow", () => {
  it("draws with no [flow]: seven rows not configured, each with its cause, three measured at 0", async () => {
    const top = await read({ depth: 0 }, "ledger-api");
    expect(top.nodes.length).toBeGreaterThan(0);
    expect(top.gaps.map((gap) => [gap.state, gap.count])).toEqual([
      ["not_configured", null],
      ["not_configured", null],
      ["not_configured", null],
      ["not_configured", null],
      ["not_configured", null],
      ["measured", 0],
      ["measured", 0],
      ["not_configured", null],
      ["not_configured", null],
      ["measured", 0],
    ]);
    expect(top.gaps[0]?.cause).toMatch(/^no \[\[flow\.start\]\] rule: /);
  });
});

describe("a row's cause (AC-02 of flow-authoring)", () => {
  it("is a one-line remedy above 0 and null at 0, on every row, read whole and rooted", async () => {
    const whole = await read({});
    const rooted = await read({ root: "MEC-TIDES", depth: 1 });
    expect(whole.gaps.filter((gap) => (gap.count ?? 0) > 0).length).toBeGreaterThan(rooted.gaps.filter((gap) => (gap.count ?? 0) > 0).length);
    for (const gap of [...whole.gaps, ...rooted.gaps]) {
      expect([gap.code, gap.cause]).toEqual([gap.code, gap.count === 0 ? null : GAP_REMEDIES[gap.code]]);
    }
  });

  it("holds one table in the mock and the test builder, a remedy for each of the ten rows", async () => {
    expect(BUILDERS_REMEDIES).toEqual(GAP_REMEDIES);
    expect(Object.keys(GAP_REMEDIES).sort()).toEqual((await read({})).gaps.map((gap) => gap.code).sort());
  });
});

describe("the scenarios", () => {
  it("no-gaps: harbor-sim's flow with its ten rows at 0, no cause, no box counting a gap", async () => {
    const top = await read({ root: "DOM-HARBOR", depth: 1 }, "harbor-sim", "no-gaps");
    expect(top.nodes).toEqual((await read({ root: "DOM-HARBOR", depth: 1 })).nodes.map((node) => ({ ...node, gaps: 0 })));
    expect(top.gaps.map((gap) => [gap.state, gap.count, gap.cause])).toEqual(Array.from({ length: 10 }, () => ["measured", 0, null]));
  });

  it("empty: no node, the rows stay; `no-start` names the scope, `unreachable` cannot be measured", async () => {
    const top = await read({ depth: 0 }, "harbor-sim", "empty");
    expect(top.nodes).toEqual([]);
    expect(top.gaps).toHaveLength(10);
    expect(top.gaps[0]?.items).toEqual([["all"]]);
    expect(top.gaps[2]?.state).toBe("cannot");
  });

  it("large: 500 documents in 25 groups", async () => {
    const top = await read({ depth: 0 }, "harbor-sim", "large");
    expect(top.nodes).toHaveLength(25);
    expect(top.nodes.reduce((sum, node) => sum + 1 + node.holds, 0)).toBe(500);
    expect(top.edges.length).toBeGreaterThan(20);
    const group = await read({ root: "ZONE-GEN-02", depth: 1 }, "harbor-sim", "large");
    expect(group.nodes).toHaveLength(20);
  });

  it("error: 503 in the mock's words", async () => {
    await expect(read({ depth: 0 }, "harbor-sim", "error")).rejects.toThrow(/database is locked/);
  });
});

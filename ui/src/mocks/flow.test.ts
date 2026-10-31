import { describe, expect, it } from "vitest";
import type { FlowOptions } from "../api/client";
import type { FlowEdge, FlowView } from "../api/types";
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
    expect(top.nodes[0]?.holds).toBe(10);
    expect(top.nodes[0]?.gaps).toBe(5);
    expect(top.edges).toEqual([]);
    expect(top.nodes[0]?.inner.map((inner) => inner.type)).toEqual(["depends_on", "follows", "supplies", "uses"]);
    expect(top.gaps.map((gap) => gap.code)).toEqual(["no-start", "two-starts", "unreachable", "no-way-to-end", "trap-cycle", "unjoined-parallel", "cycle-no-repeat", "definition-unused", "definition-once", "supply-unfed"]);
  });

  it("answers a group with itself and its children, each at the depth of its own walk", async () => {
    const group = await read({ root: "DOM-HARBOR", depth: 1 });
    expect(names(group).sort()).toEqual(["DOM-BERTHS", "DOM-HARBOR", "DOM-WATER", "MEC-FAIRWAY", "MEC-PILOTAGE"]);
    const at = (name: string) => group.nodes.find((node) => node.id === name);
    expect(at("DOM-WATER")?.distance).toBe(0);
    expect(at("DOM-WATER")?.group).toBe("DOM-HARBOR");
    expect(at("DOM-WATER")?.holds).toBe(4);
    expect(at("DOM-HARBOR")?.distance).toBeNull();
    expect(at("DOM-HARBOR")?.holds).toBe(0);
  });

  it("holds every link type and kind, against the walk with a kind and without", async () => {
    const group = await read({ root: "DOM-HARBOR", depth: 1 });
    expect(group.edges.map(line)).toEqual(
      expect.arrayContaining([
        "DOM-BERTHS -> MEC-PILOTAGE | follows | repeat | x1",
        "MEC-FAIRWAY -> MEC-PILOTAGE | follows | x1",
        "MEC-PILOTAGE -> MEC-FAIRWAY | follows | alternative | x1",
        "DOM-BERTHS -> DOM-WATER | uses | x1",
        "MEC-FAIRWAY -> DOM-WATER | depends_on | x1",
        "DOM-WATER -> DOM-BERTHS | supplies | x1",
        "MEC-FAIRWAY -> DOM-BERTHS | follows | alternative | x1",
        "MEC-FAIRWAY -> DOM-BERTHS | follows | parallel | x1",
      ]),
    );
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
    expect(whole.nodes[0]?.holds).toBeLessThan(11);
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

describe("the scenarios", () => {
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

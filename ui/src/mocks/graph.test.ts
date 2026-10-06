import { describe, expect, it } from "vitest";
import { ClientError, type GraphOptions } from "../api/client";
import type { GraphView } from "../api/types";
import { MockClient } from "./MockClient";

// AC-13 of docs/features/ui-graph.md: the mock's `spec graph` as `crates/specengine-cli/src/graph.rs`
// `followed` and `crates/specengine-core/src/check/spec_graph.rs` `walk` give it: the followed
// types in all three cases, a visited node's edge listed, distance = depth not expanded, the
// archived link left out unless `archive`, nodes by (distance, path, line), edges by (type, path,
// line, written).

function mock(scenario: "normal" | "large" = "normal") {
  return new MockClient(scenario, { delayMs: 0 });
}

function graph(options: GraphOptions, scenario: "normal" | "large" = "normal"): Promise<GraphView> {
  return mock(scenario).getGraph("harbor-sim", options);
}

const names = (view: GraphView) => view.nodes.map((node) => `${String(node.distance)} ${node.id ?? node.path}`);
const lines = (view: GraphView) => view.edges.map((edge) => `${edge.src ?? "-"} --${edge.type}--> ${edge.dst ?? edge.written}`);

/** `specengine-model`'s `LINK_TYPES`, in its order. */
const SHARED = [
  "derived_from",
  "depends_on",
  "constrains",
  "supersedes",
  "revises",
  "amends",
  "answers",
  "working_answer",
  "uses_term",
  "canon",
  "verifies",
  "adopts",
];

describe("the followed types (AC-13)", () => {
  it("by default: the twelve shared types in the model's order, then the corpus's unknown ones by name, outgoing", async () => {
    const view = await graph({ ref: "MEC-TIDES" });
    expect(view.types).toEqual([...SHARED, "precedes"].map((type) => ({ type, direction: "out" })));
  });

  it("under impact: the impact table in its order and directions", async () => {
    const view = await graph({ ref: "MEC-TIDES", impact: true });
    expect(view.types).toEqual([
      { type: "depends_on", direction: "in" },
      { type: "derived_from", direction: "in" },
      { type: "verifies", direction: "in" },
      { type: "uses_term", direction: "in" },
      { type: "constrains", direction: "out" },
    ]);
  });

  it("given types: in the order given, a repeat once; outgoing, or under impact the table's direction, else incoming", async () => {
    const given = ["constrains", "mentions", "constrains", "precedes"];
    expect((await graph({ ref: "MEC-TIDES", types: given })).types).toEqual([
      { type: "constrains", direction: "out" },
      { type: "mentions", direction: "out" },
      { type: "precedes", direction: "out" },
    ]);
    expect((await graph({ ref: "MEC-TIDES", types: given, impact: true })).types).toEqual([
      { type: "constrains", direction: "out" },
      { type: "mentions", direction: "in" },
      { type: "precedes", direction: "in" },
    ]);
  });
});

describe("the walk (AC-13)", () => {
  it("includes a node's nested sections' links and lists an edge to a visited node", async () => {
    const view = await graph({ ref: "MEC-TIDES" });
    expect(names(view)).toEqual(["0 MEC-TIDES", "1 DOM-BERTHS", "1 RULE-BERTH-DRAFT", "1 RULE-HIGH-WATER"]);
    expect(lines(view)).toEqual([
      "RULE-TIDE-WINDOW --adopts--> RULE-HIGH-WATER",
      "MEC-TIDES --canon--> docs/canon/tides.md",
      "MEC-TIDES --constrains--> RULE-BERTH-DRAFT",
      "MEC-TIDES --depends_on--> DOM-BERTHS",
      "MEC-TIDES --depends_on--> harbor-ops:MEC-SHIFTS",
      "MEC-TIDES --uses_term--> RULE-HIGH-WATER",
      "MEC-TIDES --uses_term--> TERM-SLACK-WATER",
    ]);
    expect(view.edges.map((edge) => edge.state)).toEqual(["resolved", "unchecked", "resolved", "resolved", "skipped", "resolved", "dangling"]);
    expect(view.edges.find((edge) => edge.written === "TERM-SLACK-WATER")?.reason).toBe("`TERM-SLACK-WATER` resolves to no ID and no alias");
  });

  it("does not expand a node at distance = depth", async () => {
    expect(names(await graph({ ref: "MEC-PILOTAGE", depth: 1 }))).toEqual(["0 MEC-PILOTAGE", "1 MEC-MOORING", "1 MEC-TIDES"]);
    expect(names(await graph({ ref: "MEC-PILOTAGE", depth: 2 }))).toContain("2 DOM-BERTHS");
    expect(names(await graph({ ref: "MEC-PILOTAGE", depth: 0 }))).toEqual(["0 MEC-PILOTAGE"]);
  });

  it("Impact from MEC-TIDES: an edit's reach, a link landing on a nested section, the archived link left out", async () => {
    const view = await graph({ ref: "MEC-TIDES", impact: true });
    expect(names(view)).toEqual([
      "0 MEC-TIDES",
      "1 RULE-BERTH-DRAFT",
      "1 MEC-NIGHT-PASSAGE",
      "1 MEC-PILOTAGE",
      "1 MEC-TIDE-TABLES",
      "1 DOM-WATER",
    ]);
    expect(lines(view)).toEqual([
      "MEC-TIDES --constrains--> RULE-BERTH-DRAFT",
      "MEC-NIGHT-PASSAGE --depends_on--> RULE-TIDE-WINDOW",
      "MEC-PILOTAGE --depends_on--> MEC-TIDES",
      "MEC-TIDE-TABLES --depends_on--> MEC-TIDES",
      "DOM-WATER --depends_on--> MEC-TIDES",
    ]);
    expect(view.left_out).toEqual({ generated: 0, tier3: 1 });
    const archived = await graph({ ref: "MEC-TIDES", impact: true, archive: true });
    expect(archived.left_out).toEqual({ generated: 0, tier3: 0 });
    expect(archived.nodes.find((node) => node.id === "MEC-OLD-QUAYS")).toMatchObject({ distance: 1, archived: true });
    expect(lines(archived)).toContain("MEC-OLD-QUAYS --depends_on--> MEC-TIDES");
  });

  it("orders nodes by (distance, path, line) and edges by (type, path, line, written)", async () => {
    for (const options of [{ ref: "MEC-TIDES" }, { ref: "MEC-TIDES", impact: true, archive: true }, { ref: "MEC-TIDES", types: ["mentions"] }]) {
      const view = await graph(options);
      const nodeKeys = view.nodes.map((node) => [node.distance, node.path, node.line] as const);
      const sortedNodes = [...nodeKeys].sort((a, b) => a[0] - b[0] || (a[1] < b[1] ? -1 : a[1] > b[1] ? 1 : 0) || a[2] - b[2]);
      expect(nodeKeys).toEqual(sortedNodes);
      const edgeKeys = view.edges.map((edge) => [edge.type, edge.path, edge.line, edge.written] as const);
      const text = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
      const sortedEdges = [...edgeKeys].sort((a, b) => text(a[0], b[0]) || text(a[1], b[1]) || a[2] - b[2] || text(a[3], b[3]));
      expect(edgeKeys).toEqual(sortedEdges);
    }
  });

  it("answers an unknown REF with its exit-1 document, refuses a look-alike and a negative depth (503)", async () => {
    expect(await graph({ ref: "R-404", depth: 2 })).toEqual({
      ref: "R-404",
      reason: "`R-404` resolves to no ID and no alias",
      impact: false,
      types: [],
      depth: 2,
      archive: false,
      notes: [],
      left_out: { generated: 0, tier3: 0 },
      truncated: false,
      nodes: [],
      edges: [],
    });
    for (const options of [{ ref: "ME\u0421-TIDES" }, { ref: "MEC-TIDES", depth: -1 }]) {
      const error = await graph(options).catch((caught: unknown) => caught);
      expect(error instanceof ClientError ? error.status : null).toBe(503);
    }
  });

  it("large: Impact from DOM-GEN-01 holds 17, 102 and 396 nodes at distance 1 to 3 (516)", async () => {
    const view = await graph({ ref: "DOM-GEN-01", impact: true }, "large");
    const counts = [0, 1, 2, 3].map((distance) => view.nodes.filter((node) => node.distance === distance).length);
    expect(counts).toEqual([1, 17, 102, 396]);
    expect(view.nodes).toHaveLength(516);
    expect(view.edges).toHaveLength(515);
    expect(view.truncated).toBe(false);
  });
});

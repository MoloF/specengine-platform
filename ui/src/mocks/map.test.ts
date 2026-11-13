import { describe, expect, it } from "vitest";
import { harborSim } from "./harbor-sim/fixtures";
import { LARGE_EDGES, LARGE_NODES, mockMap } from "./map";
import { MockClient } from "./MockClient";

// The mock's map (docs/features/corpus-map.md "Acceptance criteria", CM8, CM12): harbor-sim's 30
// nodes over two directions' kinds, an unlisted kind and none, 4 sections, 2 orphans, 3 ghosts, every
// index in range and every list in the daemon's order; `large` 5 000 nodes and 15 000 links from a
// fixed generator, the same on every read; ledger-api its tree's documents, no link; `empty` none.

const corpus = harborSim(0).corpus;

describe("the mock's map", () => {
  it("serves harbor-sim's 30 nodes as the daemon orders them, its counts its own", () => {
    const answer = mockMap(corpus, "harbor-sim", "normal", false);
    expect(answer.counts).toEqual({ documents: 26, sections: 4, edges: answer.edges.length, parents: 18, ghosts: 3, orphans: 2 });
    const places = answer.nodes.map((node) => `${node.path}\u0000${String(node.line).padStart(6, "0")}`);
    expect(places).toEqual([...places].sort());
    expect(answer.edges.map((edge) => [edge.src, edge.dst, edge.type])).toEqual(
      [...answer.edges].sort((a, b) => a.src - b.src || a.dst - b.dst || (a.type < b.type ? -1 : a.type > b.type ? 1 : 0)).map((edge) => [edge.src, edge.dst, edge.type]),
    );
    expect(answer.edges.every((edge) => edge.src !== edge.dst && edge.weak === (edge.type === "mentions"))).toBe(true);
    expect(answer.ghosts.map((ghost) => ghost.written)).toEqual([...answer.ghosts.map((ghost) => ghost.written)].sort());
    expect(answer.nodes.filter((node) => node.of !== null).every((node) => node.orphan === null && answer.nodes[node.of ?? -1]?.of === null)).toBe(true);
    expect([...new Set(answer.nodes.map((node) => node.kind))].sort()).toEqual(["domain", "mechanic", "rule", null].sort());
    expect(answer.nodes.filter((node) => node.orphan === true).map((node) => node.name)).toEqual(["docs/spec/notes/harbour-cats.md", "DEC-0013"]);
  });

  it("names only nodes the corpus holds, so each opens: the lock keeper, the fog signal and the harbour cats' note included (M: a node the map alone holds)", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const answer = await client.getMap("harbor-sim", { archive: false });
    const opened = await Promise.all(
      answer.nodes.map(async (node) => {
        const view = await client.getNode("harbor-sim", node.name);
        return [node.name, view.reason, view.nodes[0]?.path ?? null];
      }),
    );
    expect(opened).toEqual(answer.nodes.map((node) => [node.name, null, node.path]));
  });

  it("serves `large`: 5 000 nodes and 15 000 links, the same on every read, the sizes spread", () => {
    const first = mockMap(corpus, "harbor-sim", "large", false);
    expect([first.nodes.length, first.edges.length]).toEqual([LARGE_NODES, LARGE_EDGES]);
    expect(mockMap(corpus, "harbor-sim", "large", false)).toEqual(first);
    expect(Math.max(...first.nodes.map((node) => node.used_by))).toBeGreaterThan(20);
  });

  it("serves ledger-api its tree's documents and no link, `empty` no node, and echoes the archive", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const ledger = await client.getMap("ledger-api", { archive: true });
    expect([ledger.nodes.length > 0, ledger.edges, ledger.ghosts, ledger.archive]).toEqual([true, [], [], true]);
    expect((await new MockClient("empty", { delayMs: 0 }).getMap("harbor-sim", { archive: false })).nodes).toEqual([]);
    await expect(new MockClient("error", { delayMs: 0 }).getMap("harbor-sim", { archive: false })).rejects.toMatchObject({ status: 503 });
  });
});

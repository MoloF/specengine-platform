import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import { specFile, type MockCorpus } from "./build";
import { nodeViewOf } from "./corpus";
import { BASIN_NAME_DECOMPOSED, DRAFT_FILE, TIDE_FILE } from "./harbor-sim/fixtures";
import { EMPTY_TREE_NOTE, MockClient } from "./MockClient";

// The mock's reads as docs/features/ui-tree-node.md "Data" (Mocks) and the daemon-read contract
// ask: uncut; an unknown REF as its exit-1 document; refusals (exit 2) as 503 in the CLI's words;
// search as the store; bundles with a minimum and a b3: hash; the empty and large scenarios.

const NOW = Date.parse("2026-10-06T12:00:00Z");

function mock(scenario: "normal" | "empty" | "large" = "normal") {
  return new MockClient(scenario, { now: () => NOW, delayMs: 0 });
}

async function refusal(promise: Promise<unknown>): Promise<ClientError> {
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

describe("getTree", () => {
  it("walks pre-order: roots by path, sections before child documents, depth from the roots", async () => {
    const view = await mock().getTree("harbor-sim");
    const names = view.nodes.map((node) => `${"  ".repeat(node.depth)}${node.id ?? node.path}`);
    expect(names.slice(0, 8)).toEqual([
      // health-left's two amendments, roots by path before docs/spec/ (docs/features/health-left.md "Data", Mocks).
      "DEC-0014",
      "docs/features/deep-berth.md",
      "MEC-CARGO-CRANES",
      "DOM-HARBOR",
      "  RULE-HARBOR-CLOCK",
      "  DOM-BERTHS",
      "    RULE-BASIN-NAME",
      `    ${DRAFT_FILE}`,
    ]);
    expect(Math.max(...view.nodes.map((node) => node.depth))).toBeGreaterThanOrEqual(3);
    expect(view.truncated).toBe(false);
    expect(view.left_out).toEqual({ generated: 1, tier3: 1 });
    expect(view.nodes.find((node) => node.id === "MEC-CARGO-CRANES")?.mark).toBe("dangling-parent");
    const lockA = view.nodes.findIndex((node) => node.id === "MEC-LOCK-A");
    expect(view.nodes[lockA]?.mark).toBe("parent-cycle");
    expect([view.nodes[lockA + 1]?.id, view.nodes[lockA + 1]?.depth, view.nodes[lockA + 1]?.parent]).toEqual(["MEC-LOCK-B", 1, "MEC-LOCK-A"]);
    expect(view.nodes.filter((node) => node.id === "RULE-FAIRWAY-SPEED")).toHaveLength(2);
    expect(view.nodes.some((node) => node.archived)).toBe(false);
  });

  it("admits the archive with archive, a ROOT's subtree from depth 0, and says why a ROOT names nothing", async () => {
    const client = mock();
    expect((await client.getTree("harbor-sim", { archive: true })).nodes.some((node) => node.id === "MEC-OLD-QUAYS")).toBe(true);
    const rooted = await client.getTree("harbor-sim", { root: "MEC-TIDES" });
    expect(rooted.nodes[0]).toMatchObject({ id: "MEC-TIDES", depth: 0, parent: null });
    expect(rooted.ref).toBe("MEC-TIDES");
    const nowhere = await client.getTree("harbor-sim", { root: "R-404" });
    expect([nowhere.reason, nowhere.nodes]).toEqual(["`R-404` resolves to no ID and no alias", []]);
  });
});

describe("getNode", () => {
  it.each([
    ["MEC-TIDES", ["MEC-TIDES"]],
    ["tide-cycle/MEC-TIDES", ["MEC-TIDES"]],
    ["MEC-TIDES#RULE-TIDE-WINDOW", ["RULE-TIDE-WINDOW"]],
    [TIDE_FILE, ["MEC-TIDES"]],
    [DRAFT_FILE, [null]],
    ["RULE-FAIRWAY-SPEED", ["RULE-FAIRWAY-SPEED", "RULE-FAIRWAY-SPEED"]],
  ])("resolves %s", async (ref, ids) => {
    const view = await mock().getNode("harbor-sim", ref);
    expect([view.reason, view.nodes.map((node) => node.id)]).toEqual([null, ids]);
  });

  it("gives a section its span, a document its whole file, a span hash, uncut", async () => {
    const client = mock();
    const section = (await client.getNode("harbor-sim", "RULE-TIDE-WINDOW")).nodes[0];
    const whole = (await client.getNode("harbor-sim", "MEC-TIDES")).nodes[0];
    expect(section?.text.startsWith("## RULE-TIDE-WINDOW: Entry only inside the tide window\n")).toBe(true);
    expect(whole?.text.startsWith("---\nid: MEC-TIDES\n")).toBe(true);
    expect(whole?.text).toContain(section?.text);
    expect(whole?.sections).toEqual(["RULE-TIDE-WINDOW"]);
    expect(whole?.span_hash).toMatch(/^b3:[0-9a-f]{64}$/);
    const long = (await client.getNode("harbor-sim", "MEC-TIDE-TABLES")).nodes[0];
    expect([long?.truncated, (long?.text.length ?? 0) > 40000]).toEqual([false, true]);
  });

  it("answers an unknown REF with its exit-1 document", async () => {
    expect(await mock().getNode("harbor-sim", "R-404")).toEqual({
      ref: "R-404",
      reason: "`R-404` resolves to no ID and no alias",
      notes: [],
      nodes: [],
    });
  });

  it.each([
    ["\u0410", "A"],
    ["\u0415", "E"],
    ["\u041e", "O"],
    ["\u0420", "P"],
    ["\u0421", "C"],
  ])("refuses a look-alike %s with 503, naming the Latin fix", async (letter, latin) => {
    const written = `R-${letter}X`;
    const error = await refusal(mock().getNode("harbor-sim", written));
    expect([error.status, error.message]).toEqual([
      503,
      `spec: \`${written}\` mixes scripts or uses look-alike letters; IDs are Latin only: write \`R-${latin}X\``,
    ]);
  });

  it("refuses archive without links, as show does", async () => {
    const error = await refusal(mock().getNode("harbor-sim", "MEC-TIDES", { archive: true }));
    expect([error.status, error.message]).toEqual([503, "spec: --archive applies to --links only: add --links, or drop --archive"]);
  });

  it("lists links in four states and mentions, both ways, with reasons; archived ones only with archive", async () => {
    const client = mock();
    const links = (await client.getNode("harbor-sim", "MEC-TIDES", { with: ["links"] })).nodes[0]?.links;
    const states = new Set([...(links?.outgoing ?? []), ...(links?.incoming ?? [])].map((link) => link.state));
    expect(states).toEqual(new Set(["resolved", "dangling", "skipped", "unchecked"]));
    expect(links?.outgoing.some((link) => link.type === "mentions")).toBe(true);
    expect(links?.incoming.some((link) => link.type === "mentions")).toBe(true);
    expect([...(links?.outgoing ?? []), ...(links?.incoming ?? [])].some((link) => link.state === "resolved" && link.reason !== null)).toBe(true);
    const weak = (links?.outgoing ?? []).findIndex((link) => link.type === "mentions");
    expect((links?.outgoing ?? []).slice(weak).every((link) => link.type === "mentions")).toBe(true);
    // The archived document's mention and its depends_on (docs/features/ui-graph.md "Data", Mock).
    expect(links?.left_out.tier3).toBe(2);
    const archived = (await client.getNode("harbor-sim", "MEC-TIDES", { with: ["links"], archive: true })).nodes[0]?.links;
    expect(archived?.left_out.tier3).toBe(0);
    expect(archived?.incoming.some((link) => link.path.includes("archive/"))).toBe(true);
  });
});

describe("search", () => {
  it("ANDs case-folded terms of three or more letters over title and text, by (path, line)", async () => {
    const results = await mock().search("harbor-sim", { query: "TIDE window" });
    const places = results.hits.map((hit) => `${hit.path}:${String(hit.line)}`);
    expect(places).toEqual([...places].sort((a, b) => a.localeCompare(b, "en", { numeric: true })));
    expect(results.hits.map((hit) => hit.id)).toContain("RULE-TIDE-WINDOW");
    expect(results.limit).toBe(20);
    expect(results.truncated).toBe(false);
  });

  it("marks hits in a structured snippet around the first one, its cut ends flagged; ** stays text", async () => {
    const results = await mock().search("harbor-sim", { query: "bold" });
    const snippet = results.hits[0]?.snippet;
    expect(snippet?.segments.filter((segment) => segment.hit).map((segment) => segment.text.toLowerCase())).toContain("bold");
    expect(snippet?.cut_start).toBe(true);
    expect(snippet?.segments.map((segment) => segment.text).join("")).toContain("**not ");
  });

  it("drops short terms with a note, refuses a query without a long one (503)", async () => {
    const client = mock();
    expect((await client.search("harbor-sim", { query: "of tide" })).notes).toEqual(["search terms shorter than 3 characters dropped: `of`"]);
    const error = await refusal(client.search("harbor-sim", { query: "of a" }));
    expect([error.status, error.message]).toEqual([
      503,
      "spec: no search term of 3 or more characters; to read a node by its ID, use `spec show <ID>`",
    ]);
  });

  it("counts archived matches left out, includes them with archive", async () => {
    const client = mock();
    const live = await client.search("harbor-sim", { query: "dredging" });
    expect(live.tier3_left_out).toBeGreaterThan(0);
    const all = await client.search("harbor-sim", { query: "dredging", archive: true });
    expect(all.tier3_left_out).toBe(0);
    expect(all.hits.some((hit) => hit.archived)).toBe(true);
  });

  it("does not normalise: the decomposed name matches only as decomposed", async () => {
    const client = mock();
    const word = BASIN_NAME_DECOMPOSED.split(" ")[1] ?? "";
    expect((await client.search("harbor-sim", { query: word })).hits.length).toBeGreaterThan(0);
    expect((await client.search("harbor-sim", { query: word.normalize("NFC") })).hits).toEqual([]);
  });

  it("stops at the limit", async () => {
    expect((await mock().search("harbor-sim", { query: "tide", limit: 2 })).hits).toHaveLength(2);
  });
});

describe("getBundle", () => {
  it("bundles a linked node's layers within the default budget, hashed", async () => {
    const bundle = await mock().getBundle("harbor-sim", { node_ids: ["MEC-TIDES"] });
    expect(bundle.reason).toBeNull();
    expect(bundle.budget).toBe(2000);
    expect(bundle.bundle_hash).toMatch(/^b3:[0-9a-f]{64}$/);
    expect(bundle.layers?.targets.map((item) => [item.name, item.form])).toEqual([["MEC-TIDES", "text"]]);
    expect(bundle.layers?.ancestors.map((item) => item.name)).toEqual(["DOM-WATER", "DOM-HARBOR"]);
    expect(bundle.layers?.open_questions.map((item) => item.name)).toEqual(["RULE-SPRING-WINDOW"]);
    expect(bundle.layers?.open_questions[0]?.working_answer?.name).toBe("RULE-TIDE-WINDOW");
    expect(bundle.layers?.neighbours.map((item) => item.name)).toEqual(["RULE-BERTH-DRAFT", "DOM-BERTHS"].sort());
    expect(bundle.layers?.terms.map((item) => item.name)).toEqual(["RULE-HIGH-WATER"]);
    expect(bundle.tokens).toBeLessThanOrEqual(2000);
    expect(bundle.body?.startsWith("# Bundle: MEC-TIDES\n\n## Targets\n")).toBe(true);
    expect(bundle.bytes).toBe(new TextEncoder().encode(bundle.body ?? "").length);
  });

  it("gives an unlinked node its targets only", async () => {
    const bundle = await mock().getBundle("harbor-sim", { node_ids: ["MEC-CARGO-CRANES"] });
    const layers = bundle.layers;
    if (layers === null) {
      throw new Error("no layers");
    }
    const filled = (Object.keys(layers) as (keyof typeof layers)[]).filter((key) => layers[key].length > 0);
    expect(filled).toEqual(["targets"]);
  });

  it("refuses a budget under the minimum with 503, naming it and the budget's source", async () => {
    const error = await refusal(mock().getBundle("harbor-sim", { node_ids: ["MEC-TIDES"], budget: 5 }));
    expect(error.status).toBe(503);
    expect(error.message).toMatch(/^spec: --budget 5 is below this bundle's minimum of \d+ tokens .*: raise the budget to \d+ or more$/);
  });

  it("leaves what does not fit to the tail, named", async () => {
    const bundle = await mock().getBundle("harbor-sim", { node_ids: ["MEC-TIDES"], budget: 120 });
    expect((bundle.tail?.length ?? 0) + (bundle.more ?? 0)).toBeGreaterThan(0);
    expect(bundle.layers?.targets[0]?.form).not.toBe("text");
  });

  it("answers an unknown REF with its exit-1 document", async () => {
    const bundle = await mock().getBundle("harbor-sim", { node_ids: ["R-404"] });
    expect(bundle).toMatchObject({ refs: ["R-404"], reason: "`R-404` resolves to no ID and no alias", budget: null, body: null, layers: null });
  });
});

describe("scenarios and decisions", () => {
  it("empty: the tree's note, no hits, every REF unknown", async () => {
    const client = mock("empty");
    const view = await client.getTree("harbor-sim");
    expect([view.nodes, view.notes]).toEqual([[], [EMPTY_TREE_NOTE]]);
    expect((await client.search("harbor-sim", { query: "tide" })).hits).toEqual([]);
    expect((await client.getNode("harbor-sim", "MEC-TIDES")).reason).toBe("`MEC-TIDES` resolves to no ID and no alias");
  });

  it("large: the same deterministic tree twice, 3 000 and more nodes, depth up to 5, uncut", async () => {
    const first = await mock("large").getTree("harbor-sim");
    const second = await mock("large").getTree("harbor-sim");
    expect(first).toEqual(second);
    expect(first.nodes.length).toBeGreaterThanOrEqual(3000);
    expect(Math.max(...first.nodes.map((node) => node.depth))).toBe(5);
    expect(first.truncated).toBe(false);
  });

  it("a staged accept changes no node text: it is confirmed, and applied, only on a terminal", async () => {
    const client = mock();
    const before = await client.getNode("harbor-sim", "RULE-BERTH-DRAFT");
    const read = (await client.getProposal("harbor-sim", "PR-0042")).updated_at ?? "";
    await client.stageDecision("harbor-sim", "PR-0042", { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, read);
    expect(await client.getNode("harbor-sim", "RULE-BERTH-DRAFT")).toEqual(before);
  });
});

// UI-1 of docs/features/norm-hash.md: the mock's shown nodes carry a norm hash, `b3:` and 64
// lowercase hex, deterministic, never the span hash; null where the spec gives none.

/** A one-file corpus: front matter, an H1, a section R-1 with a nested R-2. */
function oneFile(path: string, parts: { title?: string; cost?: string; edge?: string; tail?: string } = {}): MockCorpus {
  const lines = [
    "---",
    "id: DOC-1",
    "kind: widget",
    "---",
    "",
    "# The document",
    "",
    `Intro text.${parts.tail ?? ""}`,
    "",
    `## ${parts.title ?? "Cost"} {#R-1}`,
    "",
    parts.cost ?? "Costs 12 units.",
    "",
    "### Edge {#R-2}",
    "",
    parts.edge ?? "Empty tank.",
  ];
  const file = specFile({
    path,
    id: "DOC-1",
    kind: "widget",
    title: "The document",
    lines,
    sections: [
      { id: "R-1", kind: "rule", title: "Cost" },
      { id: "R-2", kind: "edge", title: "Edge" },
    ],
  });
  return { documents: [file], links: [], treeNotes: [] };
}

function hashes(corpus: MockCorpus): Record<string, [string | null, string]> {
  return Object.fromEntries(
    ["DOC-1", "R-1", "R-2"].map((id) => {
      const node = nodeViewOf(corpus, id).nodes[0];
      return [id, [node?.norm_hash ?? null, node?.span_hash ?? ""]];
    }),
  );
}

describe("norm_hash (UI-1 of norm-hash)", () => {
  it("gives every shown node b3: and 64 lowercase hex, the same on each read, never its span hash", async () => {
    for (const ref of ["MEC-TIDES", "RULE-TIDE-WINDOW", DRAFT_FILE, "RULE-FAIRWAY-SPEED"]) {
      const first = (await mock().getNode("harbor-sim", ref)).nodes;
      const again = (await mock().getNode("harbor-sim", ref)).nodes;
      for (const [index, node] of first.entries()) {
        expect([ref, node.norm_hash]).toEqual([ref, expect.stringMatching(/^b3:[0-9a-f]{64}$/)]);
        expect(node.norm_hash).not.toBe(node.span_hash);
        expect(node.norm_hash).toBe(again[index]?.norm_hash);
      }
    }
  });

  it("keeps it when the file moves, a heading or a nested section changes, or white space is trimmed", () => {
    const base = hashes(oneFile("docs/spec/a.md"));
    const moved = hashes(oneFile("docs/spec/elsewhere/b.md"));
    expect(Object.values(moved).map(([norm]) => norm)).toEqual(Object.values(base).map(([norm]) => norm));
    expect(moved["R-1"]?.[1]).not.toBe(base["R-1"]?.[1]);
    const retitled = hashes(oneFile("docs/spec/a.md", { title: "Price" }));
    expect(retitled["R-1"]?.[0]).toBe(base["R-1"]?.[0]);
    const edge = hashes(oneFile("docs/spec/a.md", { edge: "A full tank." }));
    expect([edge["DOC-1"]?.[0], edge["R-1"]?.[0]]).toEqual([base["DOC-1"]?.[0], base["R-1"]?.[0]]);
    expect(edge["R-2"]?.[0]).not.toBe(base["R-2"]?.[0]);
    expect(hashes(oneFile("docs/spec/a.md", { tail: " \t" }))["DOC-1"]?.[0]).toBe(base["DOC-1"]?.[0]);
  });

  it("changes it when a node's own text changes", () => {
    const base = hashes(oneFile("docs/spec/a.md"));
    const cost = hashes(oneFile("docs/spec/a.md", { cost: "Costs 13 units." }));
    expect([cost["DOC-1"]?.[0], cost["R-2"]?.[0]]).toEqual([base["DOC-1"]?.[0], base["R-2"]?.[0]]);
    expect(cost["R-1"]?.[0]).not.toBe(base["R-1"]?.[0]);
    expect(hashes(oneFile("docs/spec/a.md", { tail: " More." }))["DOC-1"]?.[0]).not.toBe(base["DOC-1"]?.[0]);
  });

  it("is null for a non-UTF-8 file's whole-file node", () => {
    const corpus = oneFile("docs/spec/a.md");
    const document = corpus.documents[0];
    if (document === undefined) {
      throw new Error("no document");
    }
    document.node.utf8 = false;
    const node = nodeViewOf(corpus, "DOC-1").nodes[0];
    expect([node?.utf8, node?.norm_hash]).toEqual([false, null]);
    expect(node?.span_hash).toMatch(/^b3:[0-9a-f]{64}$/);
  });
});

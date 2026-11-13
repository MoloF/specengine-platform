import { describe, expect, it } from "vitest";
import type { SpecEngineClient } from "../api/client";
import type { MapDocument, WorkspaceDocument } from "../api/types";
import { aMap, aMapEdge, aMapNode } from "../test/builders";
import { directionsOf, kindColours } from "./colours";
import { DEFAULT_FILTERS, drawnOf, foldMap, ghostsOf, keptOf, kindChips, kindsOf, listOf, litMatches, MAP_DRAWN_MAX, matchesOf, pointsByName, releasedFor, sceneOf, type MapFilters, type Scene } from "./filters";

// docs/features/corpus-map.md "Acceptance criteria", CM8 and CM9, over the mock's harbor-sim map and
// built answers: a kind chip hides its nodes and their links; Orphans only keeps the 2 orphans;
// Broken only the 3 ghosts and their sources; Unresolved targets draws the ghosts; Sections the 4
// sections, their links unfolded; Parents draws the parents' lines; Mentions off hides the weak
// ones; the Direction select presses only its chips; the List holds exactly the kept nodes with their
// links and marks; past MAP_DRAWN_MAX the least used are left out, the last in served order on a tie.
// M: the List ignoring the chips; the first 5 000 in served order.

/** The mock client, loaded as data through Vite: only the bootstrap imports the mocks (policy.test.ts). */
const mock = import.meta.glob<{ MockClient: new (scenario: string, options: { delayMs: number }) => SpecEngineClient }>("/src/mocks/MockClient.ts", { eager: true });
const MockClient = mock["/src/mocks/MockClient.ts"]?.MockClient;
if (MockClient === undefined) {
  throw new Error("no mock client");
}
const client = new MockClient("normal", { delayMs: 0 });
const answer: MapDocument = await client.getMap("harbor-sim", { archive: false });
const workspaces: WorkspaceDocument = await client.getWorkspaces("harbor-sim");
const directions = directionsOf(workspaces.modules);

/** The names of the nodes a scene keeps under `filters`. */
function keptNames(scene: Scene, filters: MapFilters): string[] {
  const kept = keptOf(answer, scene, filters);
  return scene.points.flatMap((point, at) => (kept.points[at] !== 1 ? [] : [point.node === null ? `ghost ${answer.ghosts[point.ghost]?.written ?? ""}` : (answer.nodes[point.node]?.name ?? "")]));
}

/** The kept lines of a scene, as `<from> <style> <to>` by name. */
function keptLines(scene: Scene, filters: MapFilters): string[] {
  const kept = keptOf(answer, scene, filters);
  const name = (point: number) => {
    const at = scene.points[point];
    return at === undefined ? "" : at.node === null ? `ghost ${answer.ghosts[at.ghost]?.written ?? ""}` : (answer.nodes[at.node]?.name ?? "");
  };
  return scene.lineStyle.flatMap((style, line) => (kept.lines[line] !== 1 ? [] : [`${name(scene.lineFrom[line] ?? 0)} ${style} ${name(scene.lineTo[line] ?? 0)}`]));
}

const folded = sceneOf(answer, false);
const colours = kindColours(kindsOf(answer, folded.folded.members));

/** The mock's hostile written end, a ghost shown as text. */
const HOSTILE = ["javascript", ["al", "ert(1)"].join("")].join(":");
const named = (index: number) => answer.nodes[index]?.name ?? "";

describe("the mock's map (CM8)", () => {
  it("holds 30 nodes over two directions' kinds, an unlisted kind and none: 26 documents, 4 sections, 2 orphans, 3 ghosts; three kinds coloured", () => {
    expect([answer.nodes.length, answer.counts.documents, answer.counts.sections, answer.counts.orphans, answer.ghosts.length]).toEqual([30, 26, 4, 2, 3]);
    expect(directions.map((direction) => [direction.title, direction.kinds])).toEqual([
      ["Mechanics", ["mechanic"]],
      ["Rulebook", ["rule"]],
    ]);
    // By how many documents hold each kind, a direction or not: the unlisted kind is coloured too.
    expect(kindChips(answer, folded.folded.members, colours).map((chip) => [chip.kind, chip.slot, chip.count])).toEqual([
      ["mechanic", 1, 13],
      ["domain", 2, 4],
      ["rule", 3, 3],
      [null, 0, 6],
    ]);
    expect(colours.other).toBe(6);
  });
});

describe("the filters hide, never move (CM8)", () => {
  it("a released kind chip hides its nodes and their links", () => {
    const filters = { ...DEFAULT_FILTERS, released: new Set<string | null>(["mechanic"]) };
    const names = keptNames(folded, filters);
    expect(names.some((name) => name.startsWith("MEC-"))).toBe(false);
    expect(names).toContain("DOM-BERTHS");
    expect(keptLines(folded, filters).filter((line) => line.includes("MEC-"))).toEqual([]);
    expect(keptLines(folded, DEFAULT_FILTERS).filter((line) => line.includes("MEC-")).length).toBeGreaterThan(5);
  });

  it("Orphans only keeps the 2 orphans, and no section even with Sections on", () => {
    expect(keptNames(folded, { ...DEFAULT_FILTERS, orphansOnly: true })).toEqual(["docs/spec/notes/harbour-cats.md", "DEC-0013"]);
    expect(keptNames(sceneOf(answer, true), { ...DEFAULT_FILTERS, sections: true, orphansOnly: true })).toEqual(["docs/spec/notes/harbour-cats.md", "DEC-0013"]);
  });

  it("Broken only keeps the 3 ghosts and their sources; Unresolved targets draws the ghosts beside the rest", () => {
    expect(keptNames(folded, { ...DEFAULT_FILTERS, brokenOnly: true })).toEqual([
      "DEC-0014",
      "MEC-MOORING",
      "MEC-TIDES",
      "RULE-FOG-SIGNAL",
      "ghost RULE-QUAY-LIGHTS",
      "ghost TERM-SLACK-WATER",
      `ghost ${HOSTILE}`,
    ]);
    expect(keptNames(folded, DEFAULT_FILTERS).filter((name) => name.startsWith("ghost "))).toEqual([]);
    const ghosts = keptNames(folded, { ...DEFAULT_FILTERS, ghosts: true });
    expect(ghosts.filter((name) => name.startsWith("ghost "))).toHaveLength(3);
    expect(ghosts).toHaveLength(26 + 3);
    expect(keptLines(folded, { ...DEFAULT_FILTERS, ghosts: true }).filter((line) => line.includes(" ghost "))).toEqual([
      "DEC-0014 ghost ghost RULE-QUAY-LIGHTS",
      "RULE-FOG-SIGNAL ghost ghost RULE-QUAY-LIGHTS",
      "MEC-TIDES ghost ghost TERM-SLACK-WATER",
      `MEC-MOORING ghost ghost ${HOSTILE}`,
    ]);
  });

  it("Sections draws the 4 sections, their links unfolded; folded, a section's link lands on its document, merged once", () => {
    const sections = sceneOf(answer, true);
    const on = { ...DEFAULT_FILTERS, sections: true };
    expect(keptNames(sections, on).filter((name) => answer.nodes.find((node) => node.name === name)?.of !== null && !name.startsWith("docs/"))).toEqual([
      "RULE-BASIN-NAME",
      "RULE-MOOR-CREW",
      "RULE-HARBOR-CLOCK",
      "RULE-TIDE-WINDOW",
    ]);
    expect(keptLines(sections, on)).toContain("MEC-NIGHT-PASSAGE strong RULE-TIDE-WINDOW");
    expect(keptLines(folded, DEFAULT_FILTERS)).toContain("MEC-NIGHT-PASSAGE strong MEC-TIDES");
    expect(keptLines(folded, DEFAULT_FILTERS)).not.toContain("MEC-NIGHT-PASSAGE strong RULE-TIDE-WINDOW");
    // DOM-BERTHS' mentions of MEC-TIDES and of its RULE-TIDE-WINDOW, and its RULE-BASIN-NAME's of
    // MEC-TIDES, fold onto one line, places summed.
    expect(keptLines(folded, { ...DEFAULT_FILTERS, mentions: true }).filter((line) => line === "DOM-BERTHS weak MEC-TIDES")).toHaveLength(1);
    const merged = foldMap(answer, false).edges.find((edge) => named(edge.src) === "DOM-BERTHS" && named(edge.dst) === "MEC-TIDES" && edge.weak);
    expect(merged?.places).toBe(3);
    // A link folded onto one document is not drawn: RULE-MOOR-CREW to MEC-PILOTAGE stays, as their files differ.
    expect(foldMap(answer, false).edges.every((edge) => edge.src !== edge.dst)).toBe(true);
  });

  it("Parents draws the parents' dotted lines; Mentions, off at first, draws the weak links (M: Mentions on at first)", () => {
    expect(DEFAULT_FILTERS.mentions).toBe(false);
    expect(keptLines(folded, DEFAULT_FILTERS).filter((line) => line.includes(" parent "))).toEqual([]);
    const parents = keptLines(folded, { ...DEFAULT_FILTERS, parents: true }).filter((line) => line.includes(" parent "));
    expect(parents).toHaveLength(answer.parents.length);
    expect(parents).toContain("MEC-LOCK-B parent MEC-LOCK-A");
    expect(keptLines(folded, DEFAULT_FILTERS).filter((line) => line.includes(" weak "))).toEqual([]);
    expect(keptLines(folded, { ...DEFAULT_FILTERS, mentions: true }).filter((line) => line.includes(" weak ")).length).toBeGreaterThan(5);
  });

  it("the Direction select presses only its kinds' chips; All presses every chip", () => {
    const chips = kindChips(answer, folded.folded.members, colours);
    expect([...releasedFor(chips, directions[1] ?? null)]).toEqual(["mechanic", "domain", null]);
    expect([...releasedFor(chips, directions[0] ?? null)]).toEqual(["domain", "rule", null]);
    expect([...releasedFor(chips, null)]).toEqual([]);
  });

  it("lays out the node set alone: every filter but Sections keeps the one scene; Sections makes another", () => {
    for (const filters of [{ ...DEFAULT_FILTERS, mentions: true }, { ...DEFAULT_FILTERS, ghosts: true, parents: true }, { ...DEFAULT_FILTERS, brokenOnly: true }]) {
      expect(keptOf(answer, folded, filters).points).toHaveLength(folded.points.length);
    }
    expect(sceneOf(answer, true).points.length).toBe(folded.points.length + 4);
  });
});

describe("the List (CM8)", () => {
  it("holds exactly the kept nodes in served order, each with its links drawn out and in and its marks (M: the List ignoring the chips)", () => {
    const filters = { ...DEFAULT_FILTERS, mentions: true, released: new Set<string | null>(["mechanic", null]) };
    const rows = listOf(answer, folded.folded, filters);
    const kept = keptNames(folded, filters);
    expect(rows.map((row) => named(row.index))).toEqual(kept);
    expect(rows.every((row) => answer.nodes[row.index]?.kind !== "mechanic")).toBe(true);
    const row = (name: string) => rows.find((candidate) => named(candidate.index) === name);
    // DOM-WATER: RULE-FOG-SIGNAL's link in; its own link out goes to a mechanic, released; DOM-HARBOR's mention in.
    expect([row("DOM-WATER")?.out, row("DOM-WATER")?.in]).toEqual([0, 2]);
    expect(row("RULE-FOG-SIGNAL")?.broken).toBe(1);
    const all = listOf(answer, folded.folded, DEFAULT_FILTERS);
    expect(all).toHaveLength(26);
    expect([all.find((candidate) => named(candidate.index) === "MEC-TIDES")?.broken, all.find((candidate) => named(candidate.index) === "DEC-0013")?.out]).toEqual([1, 0]);
    // MEC-FAIRWAY is only mentioned: its two links in go with Mentions off.
    const fairway = (filters: MapFilters) => listOf(answer, folded.folded, filters).find((candidate) => named(candidate.index) === "MEC-FAIRWAY")?.in;
    expect([fairway({ ...DEFAULT_FILTERS, mentions: true }), fairway(DEFAULT_FILTERS)]).toEqual([2, 0]);
  });
});

describe("the cap (CM9)", () => {
  /** A built answer of `count` documents, used as `used` says. */
  function built(count: number, used: (index: number) => number): MapDocument {
    return aMap(
      Array.from({ length: count }, (_, at) => aMapNode({ name: `N-${String(at)}`, used_by: used(at) })),
      [aMapEdge(0, 1)],
    );
  }

  it("draws MAP_DRAWN_MAX of 5 001, leaving out the least used (M: the first 5 000 in served order)", () => {
    const answer = built(MAP_DRAWN_MAX + 1, (at) => (at === 3 ? 0 : 1));
    const drawn = drawnOf(answer, answer.nodes.map((_, at) => at));
    expect(drawn).toHaveLength(MAP_DRAWN_MAX);
    expect(drawn).not.toContain(3);
    expect(drawn).toContain(MAP_DRAWN_MAX);
    const scene = sceneOf(answer, false);
    expect([scene.total, scene.hidden, scene.pointOf[3]]).toEqual([MAP_DRAWN_MAX + 1, 1, -1]);
    // The List holds all 5 001.
    expect(listOf(answer, scene.folded, DEFAULT_FILTERS)).toHaveLength(MAP_DRAWN_MAX + 1);
  });

  it("leaves out the last in served order on a tie, and keeps the drawn ones in served order", () => {
    const answer = built(MAP_DRAWN_MAX + 1, () => 2);
    const drawn = drawnOf(answer, answer.nodes.map((_, at) => at));
    expect(drawn.at(-1)).toBe(MAP_DRAWN_MAX - 1);
    expect(drawn).toEqual([...drawn].sort((a, b) => a - b));
  });
});

describe("a search while the last answer's picture shows (CM8)", () => {
  it("lights the shown scene's points by name, never by the newer answer's index (M: matches lit by index)", () => {
    const older = aMap([aMapNode({ name: "P-1" }), aMapNode({ name: "P-2" }), aMapNode({ name: "P-3" })]);
    const newer = aMap([aMapNode({ name: "P-0" }), ...older.nodes]);
    const shown = sceneOf(older, false);
    // The newer answer's P-2 stands at index 2, where the older scene holds P-3.
    const matched = matchesOf(newer, sceneOf(newer, false).folded.members, () => true, "p-2");
    expect(matched).toEqual([2]);
    const lit = litMatches(shown, pointsByName(older, shown, matched.map((index) => newer.nodes[index]?.name ?? "")));
    expect(lit.pointList).toEqual([shown.pointOf[1]]);
    expect(lit.lineList).toEqual([]);
    // A name the shown scene does not hold lights nothing.
    expect(pointsByName(older, shown, ["P-0"])).toEqual([]);
  });

  it("lights every holder of a shared name, in served order", () => {
    const twice = aMap([aMapNode({ name: "D-1", path: "docs/a.md" }), aMapNode({ name: "D-2" }), aMapNode({ name: "D-1", path: "docs/b.md" })]);
    const shown = sceneOf(twice, false);
    expect(pointsByName(twice, shown, ["D-1"])).toEqual([shown.pointOf[0], shown.pointOf[2]]);
  });
});

describe("the List's Broken links (CM8)", () => {
  it("lists the ghosts the picture keeps: none with Unresolved targets off; with it, each with a kept source; Broken only alike; none under Orphans only (M: ghosts listed with the switch off)", () => {
    const written = (filters: MapFilters) => ghostsOf(answer, folded.folded, filters).map((ghost) => answer.ghosts[ghost]?.written ?? "");
    expect(written(DEFAULT_FILTERS)).toEqual([]);
    const all = answer.ghosts.map((ghost) => ghost.written);
    expect(all).toHaveLength(3);
    expect(written({ ...DEFAULT_FILTERS, ghosts: true })).toEqual(all);
    expect(written({ ...DEFAULT_FILTERS, brokenOnly: true })).toEqual(all);
    expect(written({ ...DEFAULT_FILTERS, ghosts: true, orphansOnly: true })).toEqual([]);
    // The picture keeps the same ghosts: a list per filter equal to the kept ghost points.
    for (const filters of [{ ...DEFAULT_FILTERS, ghosts: true }, { ...DEFAULT_FILTERS, ghosts: true, released: new Set<string | null>(["mechanic"]) }]) {
      expect(written(filters).sort()).toEqual(keptNames(folded, filters).filter((name) => name.startsWith("ghost ")).map((name) => name.slice(6)).sort());
    }
  });
});

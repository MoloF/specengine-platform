import { describe, expect, it } from "vitest";
import { MockClient } from "./MockClient";

// The mock's `spec trace` (docs/canon/spec-trace.md "The read", docs/features/trace-views.md "Data"),
// and its card list's marks: what `pnpm dev` shows is what the daemon's rules say.

const client = () => new MockClient("normal", { delayMs: 0 });

describe("the mock's trace", () => {
  it("lists every live document, so `no card` steps count, and every section a card targets, by place", async () => {
    const { nodes, counts } = await client().getTrace("harbor-sim");
    expect(nodes.find((node) => node.id === "MEC-PILOTAGE")?.rollup).toBe("no card");
    expect(nodes.find((node) => node.id === "RULE-TIDE-WINDOW")?.rollup).toBe("departed");
    expect(nodes.some((node) => node.id === "RULE-HARBOR-CLOCK")).toBe(false);
    expect(nodes.map((node) => node.path)).toEqual([...nodes.map((node) => node.path)].sort());
    expect(counts.no_card + counts.in_work + counts.suspect + counts.departed + counts.confirmed).toBe(nodes.length);
  });

  it("gives a row its stage, its distance and its counts over what it holds; a section its document's", async () => {
    const { nodes } = await client().getTrace("harbor-sim");
    const at = (id: string) => nodes.find((node) => node.id === id);
    expect([at("RULE-HIGH-WATER")?.stage, at("RULE-HIGH-WATER")?.distance]).toEqual(["landfall", 0]);
    expect(at("DOM-WATER")?.rollup).toBe("in work");
    expect(at("DOM-WATER")?.counts).toEqual({ no_card: 0, in_work: 2, suspect: 0, departed: 1, confirmed: 2 });
    expect(at("RULE-TIDE-WINDOW")?.distance).toBe(at("MEC-TIDES")?.distance);
  });

  it("keeps one stage's rows across its groups with --stage, those with none with `-`, none with --summary; the lanes whole in every read", async () => {
    const mock = client();
    const whole = await mock.getTrace("harbor-sim");
    const print = await mock.getTrace("harbor-sim", { stage: "landfall" });
    expect(print.nodes.map((node) => node.id).sort()).toEqual(["MEC-PILOTAGE", "RULE-HIGH-WATER", "RULE-SPRING-WINDOW"]);
    expect(new Set(print.nodes.map((node) => node.path.split("/")[2])).size).toBeGreaterThan(1);
    const none = await mock.getTrace("harbor-sim", { stage: "-" });
    expect(none.nodes.map((node) => node.id)).toEqual(["MEC-NIGHT-PASSAGE"]);
    const summary = await mock.getTrace("harbor-sim", { summary: true });
    expect(summary.nodes).toEqual([]);
    for (const read of [print, none, summary]) {
      expect(read.stages).toEqual(whole.stages);
      expect(read.counts).toEqual(whole.counts);
    }
  });

  it("orders lanes by least distance, unreached after, the null stage last, each counting its steps", async () => {
    const { stages } = await client().getTrace("harbor-sim", { summary: true });
    expect(stages.map((lane) => [lane.stage, lane.distance])).toEqual([
      ["landfall", 0],
      ["passage", 3],
      ["quayside", 4],
      [null, 3],
    ]);
    expect(stages[0]?.counts).toEqual({ no_card: 1, in_work: 0, suspect: 0, departed: 0, confirmed: 2 });
  });

  it("keeps a document neither a step nor staged out of every lane: it is counted, and stands in no lane", async () => {
    const { nodes, stages, counts } = await client().getTrace("harbor-sim");
    expect(nodes.some((node) => node.id === "DEC-0012")).toBe(true);
    const inLanes = stages.reduce((sum, lane) => sum + lane.counts.no_card + lane.counts.in_work + lane.counts.suspect + lane.counts.departed + lane.counts.confirmed, 0);
    expect(inLanes).toBeLessThan(nodes.length);
    expect(counts.no_card + counts.in_work + counts.suspect + counts.departed + counts.confirmed).toBe(nodes.length);
    expect(stages.find((lane) => lane.stage === null)?.counts).toEqual({ no_card: 0, in_work: 1, suspect: 0, departed: 0, confirmed: 0 });
  });

  it("reads REF and what it holds, and lists who uses it, one level", async () => {
    const { nodes } = await client().getTrace("harbor-sim", { ref: "DOM-WATER" });
    expect(nodes.some((node) => node.id === "RULE-HIGH-WATER")).toBe(true);
    expect(nodes.find((node) => node.id === "RULE-HIGH-WATER")?.uses.map((use) => [use.id, use.rollup])).toEqual([["MEC-MOORING", "suspect"]]);
  });

  it("says a gone target in a note alone: no node holds it", async () => {
    const { nodes, notes } = await client().getTrace("harbor-sim");
    expect(notes).toEqual(["C-0008: `MEC-OLD-SLIPWAY` no longer resolves"]);
    expect(nodes.some((node) => node.id === "MEC-OLD-SLIPWAY")).toBe(false);
  });

  it("serves one lane where no document has a stage", async () => {
    const { stages } = await client().getTrace("ledger-api", { summary: true });
    expect(stages.map((lane) => lane.stage)).toEqual([null]);
  });
});

describe("the mock's card list", () => {
  it("serves no mark without `marks`, every key null", async () => {
    const { cards } = await client().getCards("harbor-sim");
    expect(cards.every((row) => row.mark === null && row.on === null && row.discrepancy === null && row.correction === null)).toBe(true);
  });

  it("serves each row's mark over all its targets, a gone target's too, the trace's marks alike (AC-09 of trace-views)", async () => {
    const mock = client();
    const { cards } = await mock.getCards("harbor-sim", { marks: true });
    const row = (id: string) => cards.find((candidate) => candidate.id === id);
    expect([row("C-0002")?.mark, row("C-0002")?.on]).toEqual(["suspect", "DOM-BERTHS"]);
    expect([row("C-0008")?.mark, row("C-0008")?.on]).toEqual(["suspect", "MEC-OLD-SLIPWAY"]);
    expect([row("C-0010")?.mark, row("C-0010")?.on, row("C-0010")?.discrepancy]).toEqual(["departed", "RULE-TIDE-WINDOW", "PR-0041"]);
    expect([row("C-0001")?.mark, row("C-0011")?.mark]).toEqual(["open", "accepted"]);
    // A row of the trace reads a card over that node's targets, the list over all of them: where the trace says something, it says the same.
    const traced = (await mock.getTrace("harbor-sim")).nodes.flatMap((node) => node.cards).filter((card) => card.on !== null);
    expect(traced.length).toBeGreaterThan(3);
    for (const card of traced) {
      expect([card.id, row(card.id)?.mark, row(card.id)?.on]).toEqual([card.id, card.mark, card.on]);
    }
  });

  it("gives a cancelled card no mark: a read ignores it", async () => {
    const { cards } = await client().getCards("harbor-sim", { with_cancelled: true, marks: true });
    expect(cards.find((row) => row.cancelled)?.mark).toBeNull();
  });
});

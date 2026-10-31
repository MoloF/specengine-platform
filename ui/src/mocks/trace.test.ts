import { describe, expect, it } from "vitest";
import type { Card } from "../api/types";
import { cardListOf, clearCardIn, mockBoard, type MockBoard } from "./cards";
import type { FlowDocument } from "./flow";
import { MockClient } from "./MockClient";
import { traceOf, type TraceWorld } from "./trace";

// The mock's `spec trace` (docs/canon/spec-trace.md "The read", docs/features/trace-views.md "Data",
// docs/features/trace-truth.md), and its card list's marks: what `pnpm dev` shows is what the daemon's
// rules say.

const client = () => new MockClient("normal", { delayMs: 0 });

const NONE = { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 0 };

/** A live document of a hand-made flow. */
function aDocument(name: string, parent: string | null, stage: string | null): FlowDocument {
  return { name, id: name, kind: "step", title: name, path: `docs/${name.toLowerCase()}.md`, line: 1, parent, stage };
}

/** What a read takes, over documents alone: harbor-sim and ledger-api have no lane of holders. */
function worldOf(documents: FlowDocument[], distance: readonly (readonly [string, number])[]): TraceWorld {
  return {
    flow: { documents, links: [], starts: [], ends: [], steps: [], rows: [], leftOut: { generated: 0, tier3: 0 } },
    corpus: { documents: [], links: [], treeNotes: [] },
    distance: new Map(distance),
    settled: () => false,
  };
}

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

describe("a node holding documents (docs/features/trace-truth.md)", () => {
  const ledger = async () => {
    const read = await client().getTrace("ledger-api");
    return { ...read, at: (id: string) => read.nodes.find((node) => node.id === id) };
  };

  it("counts a holder with no card of its own nowhere: its row says `no card`, the header leaves it out", async () => {
    const { at, counts } = await ledger();
    expect(at("SVC-LEDGER")?.rollup).toBe("no card");
    expect(at("SVC-LEDGER")?.cards).toEqual([]);
    expect(counts).toEqual({ ...NONE, no_card: 1, in_work: 5 });
  });

  it("gives a holder its parts' sum, so a book's counts equal its parts'", async () => {
    const { at } = await ledger();
    expect(at("POL-WRITES")?.counts).toEqual({ ...NONE, in_work: 1 });
    expect(at("SVC-REFUNDS")?.counts).toEqual({ ...NONE, no_card: 1, in_work: 2 });
    expect(at("SVC-LEDGER")?.counts).toEqual({ ...NONE, no_card: 1, in_work: 3 });
  });

  it("still counts a holder with a card, and a document holding nothing live: an archived child holds nothing", async () => {
    const { at, nodes } = await ledger();
    expect([at("SVC-REFUNDS")?.rollup, at("SVC-REFUNDS")?.cards.length]).toEqual(["in work", 1]);
    expect(nodes.some((node) => node.id === "EP-REFUND-V0")).toBe(false);
    expect([at("EP-REFUNDS")?.rollup, at("EP-REFUNDS")?.counts]).toEqual(["no card", { ...NONE, no_card: 1 }]);
  });

  it("agrees with the rows: every listed node is in the header but the holders with no card", async () => {
    const { nodes, counts } = await ledger();
    const sum = counts.no_card + counts.in_work + counts.suspect + counts.departed + counts.confirmed;
    expect(sum).toBe(nodes.length - 1);
  });

  it("keeps a lane whose only row is a holder, at zero counts and its distance", () => {
    const world = worldOf(
      [aDocument("BOOK", null, null), aDocument("PART", "BOOK", "print"), aDocument("STEP-A", "PART", "proof"), aDocument("STEP-B", "PART", "proof")],
      [
        ["PART", 2],
        ["STEP-A", 3],
        ["STEP-B", 4],
      ],
    );
    const { stages, counts, nodes } = traceOf(mockBoard("ledger-api", "empty", 0), {}, world);
    expect(stages).toEqual([
      { stage: "print", distance: 2, counts: NONE },
      { stage: "proof", distance: 3, counts: { ...NONE, no_card: 2 } },
    ]);
    expect(counts).toEqual({ ...NONE, no_card: 2 });
    expect(nodes.find((node) => node.id === "PART")?.rollup).toBe("no card");
    expect(nodes.find((node) => node.id === "PART")?.counts).toEqual({ ...NONE, no_card: 2 });
    expect(nodes.find((node) => node.id === "BOOK")?.counts).toEqual({ ...NONE, no_card: 2 });
  });

  it("serves the holder's row under its own --stage: a lane's count can differ from its rows", () => {
    const world = worldOf([aDocument("PART", null, "print"), aDocument("STEP-A", "PART", "proof")], [["PART", 2]]);
    const { nodes, stages } = traceOf(mockBoard("ledger-api", "empty", 0), { stage: "print" }, world);
    expect(nodes.map((node) => [node.id, node.rollup])).toEqual([["PART", "no card"]]);
    expect(stages.find((lane) => lane.stage === "print")?.counts).toEqual(NONE);
  });

  it("joins the same counts on the flow's boxes", async () => {
    const mock = client();
    const view = await mock.getFlow("ledger-api", { depth: 0, trace: true });
    const box = view.nodes.find((node) => node.id === "SVC-LEDGER");
    expect([box?.rollup, box?.counts]).toEqual(["no card", { ...NONE, no_card: 1, in_work: 3 }]);
  });
});

describe("a card's frozen hashes (docs/features/trace-truth.md)", () => {
  const now = "b3:dom-berths-now";

  /** harbor-sim's board, with C-0002's frozen entry on DOM-BERTHS as `entry` says. */
  function boardWith(entry: { norm_hash: string | null; within_hash: string | null }): MockBoard {
    const board = mockBoard("harbor-sim", "normal", 0);
    board.changed.set("DOM-BERTHS", now);
    const card = board.cards.find((candidate) => candidate.id === "C-0002");
    card?.snapshot?.forEach((frozen) => {
      if (frozen.id === "DOM-BERTHS") {
        Object.assign(frozen, entry);
      }
    });
    return board;
  }

  const markOf = (board: MockBoard) => cardListOf(board, { marks: true }, () => false).cards.find((row) => row.id === "C-0002")?.mark;

  async function shown(id: string): Promise<Card> {
    const read = await client().getCard("harbor-sim", id);
    if (!("flow" in read)) {
      throw new Error(`no card ${id}`);
    }
    return read;
  }

  it("freezes within_hash beside norm_hash: one member, the same string", async () => {
    const card = await shown("C-0001");
    expect(card.snapshot?.length).toBeGreaterThan(0);
    for (const entry of card.snapshot ?? []) {
      expect(Object.keys(entry)).toEqual(["id", "rev", "norm_hash", "within_hash", "also"]);
      expect(entry.within_hash).toBe(entry.norm_hash);
    }
  });

  it("compares within_hash where the entry holds one: its own hash holding marks no less", () => {
    expect(markOf(boardWith({ norm_hash: now, within_hash: "b3:before" }))).toBe("suspect");
    expect(markOf(boardWith({ norm_hash: "b3:before", within_hash: now }))).toBe("open");
  });

  it("compares norm_hash on an entry stored with no within_hash: no false mark, and a change still marks", () => {
    expect(markOf(boardWith({ norm_hash: now, within_hash: null }))).toBe("open");
    expect(markOf(boardWith({ norm_hash: "b3:before", within_hash: null }))).toBe("suspect");
  });

  it("marks nothing where neither hash was frozen", () => {
    expect(markOf(boardWith({ norm_hash: null, within_hash: null }))).toBe("open");
  });

  it("clears by the hash an entry compares: from and to differ though its own hash held, and both freeze", () => {
    const board = boardWith({ norm_hash: now, within_hash: "b3:before" });
    clearCardIn(board, "C-0002", { outcome: "no_effect", reason: "the berths read the same", revision: "1", as: "Mira Holt" }, "2026-10-31T09:00:00Z");
    expect(board.events.at(-1)?.payload).toMatchObject({ nodes: [{ id: "DOM-BERTHS", from: "b3:before", to: now }] });
    expect(board.cards.find((card) => card.id === "C-0002")?.snapshot?.find((entry) => entry.id === "DOM-BERTHS")).toMatchObject({ norm_hash: now, within_hash: now });
    expect(markOf(board)).toBe("open");
  });
});

describe("the card document", () => {
  it("holds the list's four keys null, as `GET cards/:id` serves them, on a card the list marks", async () => {
    const mock = client();
    const { cards } = await mock.getCards("harbor-sim", { marks: true });
    expect(cards.find((row) => row.id === "C-0002")?.mark).toBe("suspect");
    const read = await mock.getCard("harbor-sim", "C-0002");
    if (!("flow" in read)) {
      throw new Error("no card C-0002");
    }
    expect(read).toMatchObject({ mark: null, on: null, discrepancy: null, correction: null });
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

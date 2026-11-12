import { describe, expect, it } from "vitest";
import { apiErrorOf } from "../api/client";
import { categoryOf } from "../board/category";
import { progressOf } from "../board/rollup";
import { departmentFlows, tallyOf } from "../overview/department";
import { pinnedCountsJson, pinnedHubCountsJson } from "../test/countsStub";
import { countsOf, pinnedCounts, type CountedTask } from "./cardCounts";
import { MockClient } from "./MockClient";

// The mock's task counts (src/mocks/cardCounts.ts;
// docs/features/department-overview.md "Acceptance criteria", DO8, DO9, DO10, DO12): its rules over
// the spec's scenario give the spec's JSON key for key; over harbor-sim's cards they agree with the
// Board's list, which reads the same cards, and its ready parents with the rollup's `ready`;
// `local-only` and `hub` serve the hub's shape; `empty` serves no flow; `error` refuses as every read.

const AT = Date.parse("2026-11-12T12:00:00Z");

function client(scenario: ConstructorParameters<typeof MockClient>[0] = "normal") {
  return new MockClient(scenario, { now: () => AT });
}

describe("the spec's scenario (DO9, DO10)", () => {
  it("counts to the spec's JSON, key for key and in order", () => {
    expect(JSON.stringify(pinnedCounts())).toBe(JSON.stringify(pinnedCountsJson()));
  });

  it("serves the hub's shape with no agent task read: local_only, every list null (DO4)", () => {
    expect(JSON.stringify(pinnedCounts(false))).toBe(JSON.stringify(pinnedHubCountsJson()));
  });
});

describe("harbor-sim's counts agree with the Board (DO12)", () => {
  it("counts every task once, in its flow and column: open, accepted and cancelled as the Board's list reads them", async () => {
    const mock = client();
    const counts = await mock.getCardCounts("harbor-sim");
    const list = await mock.getCards("harbor-sim", { with_cancelled: true });
    expect(counts.flows).toEqual(list.flows);
    for (const row of counts.columns) {
      const here = list.cards.filter((card) => card.flow === row.flow && card.column === row.column);
      expect([row.flow, row.column, row.open, row.accepted, row.cancelled, row.open_subtasks]).toEqual([
        row.flow,
        row.column,
        here.filter((card) => !card.accepted && !card.cancelled).length,
        here.filter((card) => card.accepted && !card.cancelled).length,
        here.filter((card) => card.cancelled).length,
        here.filter((card) => !card.accepted && !card.cancelled && card.parent !== null).length,
      ]);
    }
    const total = counts.columns.reduce((sum, row) => sum + row.open + row.accepted + row.cancelled, 0);
    expect(total).toBe(list.cards.length);
  });

  it("lists each configured flow's columns in config order, zeros too, and every flow once in by_flow", async () => {
    const counts = await client().getCardCounts("harbor-sim");
    expect(counts.columns.map((row) => `${row.flow}/${row.column}`)).toEqual([
      "copy/draft",
      "copy/edit",
      "copy/proof",
      "copy/filed",
      "plates/queued",
      "plates/blocked",
      "plates/checked",
      "plates/struck",
    ]);
    expect(counts.by_flow.map((row) => row.flow)).toEqual(["copy", "plates"]);
    expect(counts.agent_tasks).toBe("counted");
  });

  it("counts open items on their task's flow, the oldest by its stored time; T-0107, in plan review, on copy", async () => {
    const counts = await client().getCardCounts("harbor-sim");
    const copy = counts.by_flow.find((row) => row.flow === "copy");
    const plates = counts.by_flow.find((row) => row.flow === "plates");
    expect([copy?.open_items, plates?.open_items]).toEqual([1, 2]);
    expect(copy?.review_agent_tasks).toEqual(["T-0107"]);
    expect(plates?.review_agent_tasks).toEqual([]);
    expect(plates?.oldest_open_item).not.toBeNull();
  });

  it("puts ledger-api's task left behind in a column its flow no longer lists after the flow's own, which the Board calls not in the flow", async () => {
    const counts = await client().getCardCounts("ledger-api");
    const stranded = counts.columns.filter((row) => categoryOf(counts.flows.find((flow) => flow.name === row.flow), row.column) === "stranded");
    expect(stranded.map((row) => [row.flow, row.column, row.open])).toEqual([["plain", "parked", 1]]);
    expect(counts.columns.at(-1)?.column).toBe("parked");
  });
});

describe("the store's rules on their edges (DO8, DO9)", () => {
  const flows = pinnedCountsJson().flows;
  const task = (fields: Partial<CountedTask> & Pick<CountedTask, "id">): CountedTask => ({ flow: "copy", column: "todo", parent: null, accepted: false, cancelled: false, tasks: [], ...fields });

  it("counts a parent ready on its children alone: a grandchild never counts, a parent with no child never does, an accepted child alone is not enough", () => {
    const counts = countsOf({
      flows,
      tasks: [
        // P: its one child accepted, the grandchild under that child still open: ready.
        task({ id: "C-0001" }),
        task({ id: "C-0002", parent: "C-0001", accepted: true, column: "done" }),
        task({ id: "C-0003", parent: "C-0002" }),
        // Q: no child: not ready.
        task({ id: "C-0004" }),
        // R: one child accepted, one open: not ready.
        task({ id: "C-0005" }),
        task({ id: "C-0006", parent: "C-0005", accepted: true, column: "done" }),
        task({ id: "C-0007", parent: "C-0005" }),
        // S: closed, its child cancelled: not ready.
        task({ id: "C-0008", accepted: true, column: "done" }),
        task({ id: "C-0009", parent: "C-0008", cancelled: true }),
      ],
      items: [],
      agentTask: () => null,
      tasksHere: true,
    });
    expect(counts.by_flow.find((row) => row.flow === "copy")?.ready_parents).toBe(1);
  });
});

describe("the scenarios (DO4, DO12)", () => {
  it.each(["local-only", "hub"] as const)("%s: the hub's shape, local_only and every list null", async (scenario) => {
    const counts = await client(scenario).getCardCounts("harbor-sim");
    expect(counts.agent_tasks).toBe("local_only");
    expect(counts.by_flow.map((row) => row.review_agent_tasks)).toEqual([null, null]);
  });

  it("empty: no flow, no column, no row", async () => {
    const counts = await client("empty").getCardCounts("harbor-sim");
    expect([counts.flows, counts.columns, counts.by_flow]).toEqual([[], [], []]);
  });

  it("error: refused in the daemon's words, as every read", async () => {
    const failure = await client("error")
      .getCardCounts("harbor-sim")
      .then(
        () => null,
        (error: unknown) => error,
      );
    expect(apiErrorOf(failure).status).toBe(503);
  });

  it("an unknown project: 404", async () => {
    const failure = await client()
      .getCardCounts("nowhere")
      .then(
        () => null,
        (error: unknown) => error,
      );
    expect(apiErrorOf(failure)).toEqual({ status: 404, message: "no project nowhere" });
  });
});

describe("Ready to accept is the rollup's ready (DO8)", () => {
  it("over the mock, each flow's ready count is the open parents there whose progressOf(...).ready holds", async () => {
    const mock = client();
    for (const project of ["harbor-sim", "ledger-api"]) {
      const counts = await mock.getCardCounts(project);
      const list = await mock.getCards(project, { with_cancelled: true });
      const parents = list.cards.filter((row) => list.cards.some((child) => child.parent === row.id));
      for (const { flow } of counts.by_flow) {
        const expected = parents.filter((row) => row.flow === flow && progressOf(list.cards, list.flows, { id: row.id, open: !row.accepted && !row.cancelled }).ready).length;
        expect([project, flow, tallyOf(counts, [flow]).ready]).toEqual([project, flow, expected]);
      }
      expect(tallyOf(counts, departmentFlows(counts, null)).ready).toBe(parents.filter((row) => progressOf(list.cards, list.flows, { id: row.id, open: !row.accepted && !row.cancelled }).ready).length);
    }
    // harbor-sim's C-0004, in proof, its one subtask accepted: one parent ready.
    expect(tallyOf(await mock.getCardCounts("harbor-sim"), ["copy"]).ready).toBe(1);
  });
});

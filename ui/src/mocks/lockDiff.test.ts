import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { DriftRow, LockDiffRow } from "../api/types";
import { bindingsNote } from "./bindings";
import { DRAFT_FILE, TIDE_FILE } from "./harbor-sim/fixtures";
import { CODE_GAP_CAUSE, CODE_OFF_CAUSE, harborDrift, LOCK_CAUSE } from "./health";
import { harborLockDiffRows, rowKeyOf, sideNotesOf } from "./lockDiff";
import { MockClient } from "./MockClient";

// UI-3 of docs/features/lock-diff.md: the mock's lock diff. harbor-sim's rows are Health's row 6 by
// key, those a lock entry holds whose state raises drift, and their sides follow "Sides"; a REF's
// holders are the node read's, each with the rows of its set; a run cause leaves `rows` null with
// its one note; a REF naming nothing answers the exit-1 document.

const NOW = Date.parse("2026-10-05T12:00:00Z");
const clock = () => NOW;

/** The states core `drift::raises(state, detail, false)` raises (docs/features/lock-diff.md "Description and interactions", step 4). */
const RAISES = ["code_ahead", "spec_ahead", "conflict", "broken_binding"];

/** Whether "Sides" gives a side: always, never, or only when (R, H) moved (the mock cannot see H). */
type Given = "yes" | "no" | "when moved";

/** docs/features/lock-diff.md "Sides", row by row. */
function sidesOf(row: DriftRow): { spec: Given; code: Given } {
  const levels = row.levels;
  const levelsMoved = levels !== null && ((levels.changed ?? []).length > 0 || levels.lock.join() !== levels.markers.join());
  switch (`${row.state} ${row.category ?? ""}`.trim()) {
    case "conflict":
      return { spec: "yes", code: "yes" };
    case "code_ahead code_changed":
    case "code_ahead levels_changed":
      return { spec: "no", code: "yes" };
    case "code_ahead raised":
      return { spec: "yes", code: levelsMoved ? "yes" : "no" };
    case "spec_ahead spec_changed":
    case "spec_ahead same_revision":
      return { spec: "yes", code: "no" };
    case "spec_ahead marker_behind":
      return { spec: "when moved", code: "no" };
    case "broken_binding node_gone":
      return { spec: "yes", code: "no" };
    case "broken_binding file_gone":
    case "broken_binding symbol_gone":
    case "broken_binding marker_gone":
      return { spec: "when moved", code: "yes" };
    default:
      throw new Error(`no "Sides" row for ${row.state} ${row.category ?? ""}`);
  }
}

function given(side: unknown, rule: Given): boolean {
  return rule === "when moved" || (side !== null) === (rule === "yes");
}

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

/** The one kept row of `node`. */
function keptOf(node: string): LockDiffRow {
  const found = harborLockDiffRows().find(({ row }) => row.node === node);
  if (found === undefined) {
    throw new Error(`no kept row of ${node}`);
  }
  return found;
}

/** git's hunks: from the first `@@`, LF-terminated. */
function isHunks(diff: string): boolean {
  return diff.startsWith("@@ -") && diff.endsWith("\n");
}

describe("harbor-sim's kept rows (UI-3 of lock-diff)", () => {
  const kept = harborLockDiffRows();

  it("are Health's rows a lock entry holds whose state raises drift, each equal to Health's row of the same key, in verify's order", () => {
    const health = harborDrift();
    const expected = health.filter((row) => row.lock !== null && RAISES.includes(row.state));
    expect(kept.map(({ row }) => row)).toEqual(expected);
    for (const { row } of kept) {
      expect(health.filter((other) => rowKeyOf(other) === rowKeyOf(row))).toEqual([row]);
      expect(row.pre_existing).toBe(false);
    }
    expect(kept.map(({ row }) => `${row.node} ${row.state} ${row.category ?? ""}`.trim())).toEqual([
      "MEC-MOORING code_ahead code_changed",
      "MEC-TIDES conflict",
      "RULE-BERTH-DRAFT spec_ahead marker_behind",
      "RULE-HIGH-WATER broken_binding symbol_gone",
    ]);
  });

  it.each(harborLockDiffRows().map((entry): [string, LockDiffRow] => [rowKeyOf(entry.row), entry]))("%s has the sides \"Sides\" gives its state", (_key, { row, spec, code }) => {
    const rule = sidesOf(row);
    expect([spec !== null, given(spec, rule.spec)]).toEqual([spec !== null, true]);
    expect([code !== null, given(code, rule.code)]).toEqual([code !== null, true]);
    if (spec !== null) {
      expect(spec.commit).toBe(row.lock?.commit);
      expect(spec.diff === null || isHunks(spec.diff)).toBe(true);
    }
    if (code !== null) {
      // The entry's file: the row's, a moved row's old one.
      expect(code.file).toBe(row.anchor?.from?.file ?? row.file);
      expect(code.blob).toMatch(/^[0-9a-f]{40}$/);
      expect(code.diff === null || isHunks(code.diff)).toBe(true);
    }
    if (row.category === "file_gone" || row.category === "symbol_gone") {
      // New text "": only `-` lines, against nothing.
      const lines = (code?.diff ?? "").split("\n").slice(1, -1);
      expect(code?.diff?.split("\n")[0]).toMatch(/^@@ -1,\d+ \+0,0 @@$/);
      expect(lines.length).toBeGreaterThan(0);
      expect(lines.every((line) => line.startsWith("-"))).toBe(true);
    }
  });

  it("gives one note per side not shown, naming the row's key and the side, never one for a side shown or equal", () => {
    for (const { row, spec, code } of kept) {
      const notShown = [spec, code].filter((side) => side !== null && side.diff === null).length;
      const notes = sideNotesOf(row);
      expect([rowKeyOf(row), notes.length]).toEqual([rowKeyOf(row), notShown]);
      for (const note of notes) {
        expect(note.startsWith(`lock diff: ${rowKeyOf(row)}: no `)).toBe(true);
      }
    }
  });

  it("holds the slice's four cases: a conflict with both sides, a code_ahead with its code side, a side the daemon could not read, a gone symbol", () => {
    const tides = keptOf("MEC-TIDES");
    expect([tides.spec?.path, tides.spec?.diff === null, tides.code?.diff === null]).toEqual([TIDE_FILE, false, false]);
    const mooring = keptOf("MEC-MOORING");
    expect([mooring.spec, mooring.row.anchor?.state, mooring.code?.file]).toEqual([null, "moved", "src/sim/berths.rs"]);
    const draft = keptOf("RULE-BERTH-DRAFT");
    expect([draft.spec?.path, draft.spec?.diff, draft.code]).toEqual([DRAFT_FILE, null, null]);
    expect(sideNotesOf(draft.row)).toEqual([
      `lock diff: RULE-BERTH-DRAFT | assumes | data/berths.ron | ron berths[4].draft_m: no spec diff: ${DRAFT_FILE} cannot be read at ${draft.row.lock?.commit ?? ""}`,
    ]);
    const highWater = keptOf("RULE-HIGH-WATER");
    expect([highWater.spec, highWater.code?.file]).toEqual([null, "src/sim/tide_table.rs"]);
  });

  it("names each spec side's file as the node read finds the row's node now", async () => {
    const client = new MockClient("normal", { now: clock });
    for (const { row, spec } of kept) {
      if (spec !== null) {
        const shown = await client.getNode("harbor-sim", row.node);
        expect([row.node, spec.path]).toEqual([row.node, shown.nodes[0]?.path]);
      }
    }
  });
});

describe("MockClient.getLockDiff (UI-3 of lock-diff)", () => {
  it("answers a REF's holders as the node read finds them, each with the kept rows of its set: MEC-TIDES's conflict, both sides", async () => {
    const client = new MockClient("normal", { now: clock });
    const shown = await client.getNode("harbor-sim", "MEC-TIDES");
    const answer = await client.getLockDiff("harbor-sim", "MEC-TIDES");
    expect(answer).toEqual({ ref: "MEC-TIDES", reason: null, notes: [], nodes: [{ id: "MEC-TIDES", path: TIDE_FILE, line: shown.nodes[0]?.line, rows: [keptOf("MEC-TIDES")] }] });
    expect(answer.nodes[0]?.rows?.[0]?.row.state).toBe("conflict");
  });

  it("answers the ID-less document by its path: id null, its section's row with a spec side not shown and that side's one note", async () => {
    const answer = await new MockClient("normal", { now: clock }).getLockDiff("harbor-sim", DRAFT_FILE);
    const draft = keptOf("RULE-BERTH-DRAFT");
    expect(answer.nodes.map((node) => [node.id, node.path, node.rows])).toEqual([[null, DRAFT_FILE, [draft]]]);
    expect(answer.notes).toEqual([...sideNotesOf(draft.row)]);
    expect(answer.notes).toHaveLength(1);
  });

  it("answers [] for a holder with no drifted row a lock entry holds: RULE-TIDE-WINDOW's row is predated, never kept", async () => {
    const answer = await new MockClient("normal", { now: clock }).getLockDiff("harbor-sim", "MEC-TIDES#RULE-TIDE-WINDOW");
    expect(answer.nodes.map((node) => [node.id, node.rows])).toEqual([["RULE-TIDE-WINDOW", []]]);
    expect(answer.notes).toEqual([]);
  });

  it("answers a REF naming nothing with the exit-1 document, as data", async () => {
    const answer = await new MockClient("normal", { now: clock }).getLockDiff("harbor-sim", "NOPE-1");
    expect(answer).toEqual({ ref: "NOPE-1", reason: "`NOPE-1` resolves to no ID and no alias", notes: [], nodes: [] });
  });

  it("refuses a look-alike ID 503 in the CLI's words, naming its Latin form", async () => {
    const error = await rejection(new MockClient("normal", { now: clock }).getLockDiff("harbor-sim", "MEC-TID\u0415S"));
    expect([error.status, error.message]).toEqual([
      503,
      "spec: `MEC-TID\u0415S` mixes scripts or uses look-alike letters; IDs are Latin only: write `MEC-TIDES`",
    ]);
  });

  it("leaves ledger-api's rows null with the no-[code] note: not judged, never []", async () => {
    const client = new MockClient("normal", { now: clock });
    const answer = await client.getLockDiff("ledger-api", "SVC-REFUNDS");
    expect(answer.notes).toEqual([bindingsNote(CODE_OFF_CAUSE)]);
    expect(answer.nodes.map((node) => [node.id, node.rows])).toEqual([["SVC-REFUNDS", null]]);
  });

  it.each([
    ["harbor-sim", "MEC-TIDES", CODE_GAP_CAUSE],
    ["ledger-api", "SVC-REFUNDS", LOCK_CAUSE],
  ])("code-cannot: %s's rows null with its one bindings note", async (project, ref, cause) => {
    const answer = await new MockClient("code-cannot", { now: clock }).getLockDiff(project, ref);
    expect(answer.notes).toEqual([bindingsNote(cause)]);
    expect(answer.nodes.map((node) => node.rows)).toEqual([null]);
  });

  it("not-configured: ledger-api has [code] and no drifted row, so its rows are [] and no note", async () => {
    const answer = await new MockClient("not-configured", { now: clock }).getLockDiff("ledger-api", "SVC-REFUNDS");
    expect([answer.notes, answer.nodes.map((node) => node.rows)]).toEqual([[], [[]]]);
  });

  it("answers the error scenario's 503, and an unknown project's 404, in the daemon's words", async () => {
    const down = await rejection(new MockClient("error", { now: clock }).getLockDiff("harbor-sim", "MEC-TIDES"));
    expect(down.status).toBe(503);
    const unknown = await rejection(new MockClient("normal", { now: clock }).getLockDiff("zeta", "MEC-TIDES"));
    expect([unknown.status, unknown.message]).toEqual([404, "no project zeta"]);
  });

  it("hands out a copy: a change to one answer reaches no later one", async () => {
    const client = new MockClient("normal", { now: clock });
    const first = await client.getLockDiff("harbor-sim", "MEC-TIDES");
    const side = first.nodes[0]?.rows?.[0]?.code;
    if (side !== undefined && side !== null) {
      side.diff = "changed";
    }
    const second = await client.getLockDiff("harbor-sim", "MEC-TIDES");
    expect(second.nodes[0]?.rows?.[0]?.code?.diff).not.toBe("changed");
  });
});

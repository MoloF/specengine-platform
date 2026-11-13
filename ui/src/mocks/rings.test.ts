import { describe, expect, it } from "vitest";
import { apiErrorOf } from "../api/client";
import { ROLLUP_WORDS } from "../flow/labels";
import { MockClient } from "./MockClient";
import { LEDGER_RING_NOTE } from "./rings";
import type { Scenario } from "./scenario";

// The mock's rings (src/mocks/rings.ts; docs/features/scope-rings.md "Acceptance criteria", SR7,
// SR9): harbor-sim's four, innermost first, of 12, 5, 1 and 0 sectors with every state and a long
// title, each node in one ring, each ring in the tree's order; ledger-api's bad table, a note and no
// ring; `empty`, neither; `error`, the daemon's refusal.

function client(scenario: Scenario = "normal") {
  return new MockClient(scenario, { now: () => Date.parse("2026-11-13T12:00:00Z") });
}

describe("the mock's rings (SR7, SR9)", () => {
  it("gives harbor-sim four rings of 13, 5, 1 and 0 sectors, their config's words kept", async () => {
    const { notes, rings } = await client().getRings("harbor-sim");
    expect(notes).toEqual([]);
    expect(rings.map((ring) => [ring.title, ring.root, ring.kinds, ring.sectors.length])).toEqual([
      // Twelve, and the lock keeper (corpus-map's).
      ["Every mechanic", null, ["mechanic"], 13],
      ["Berths", "DOM-BERTHS", ["domain", "rule"], 5],
      ["Pilot station", "DOM-PILOTS", null, 1],
      ["Locks", "MEC-LOCK-A", ["rule"], 0],
    ]);
  });

  it("holds every rollup and one word this build does not know, and a long title", async () => {
    const { rings } = await client().getRings("harbor-sim");
    const states = new Set(rings.flatMap((ring) => ring.sectors.map((sector) => sector.state)));
    for (const word of ROLLUP_WORDS) {
      expect([word, states.has(word)]).toEqual([word, true]);
    }
    expect([...states].filter((state) => !ROLLUP_WORDS.includes(state))).toEqual(["parked"]);
    expect(Math.max(...rings.flatMap((ring) => ring.sectors.map((sector) => (sector.title ?? "").length)))).toBeGreaterThan(100);
  });

  it("puts each node in one ring, the inner one, each ring in the tree's order", async () => {
    const mock = client();
    const { rings } = await mock.getRings("harbor-sim");
    const places = rings.flatMap((ring) => ring.sectors.map((sector) => `${sector.path}:${String(sector.line)}`));
    expect(new Set(places).size).toBe(places.length);
    const tree = (await mock.getTree("harbor-sim", { kinds: ["mechanic"] })).nodes.map((node) => node.id);
    expect(rings[0]?.sectors.map((sector) => sector.id)).toEqual(tree);
    // The pilot station's mechanics are the first ring's: its own document stands alone.
    expect(rings[2]?.sectors.map((sector) => sector.name)).toEqual(["DOM-PILOTS"]);
    expect(rings[1]?.sectors.map((sector) => sector.name)).toEqual(["DOM-BERTHS", "RULE-BASIN-NAME", "RULE-BERTH-DRAFT", "RULE-MOOR-CREW", "RULE-MOOR-NIGHT"]);
  });

  it("gives ledger-api its bad table's note and no ring; `empty` neither; `error` the daemon's refusal", async () => {
    expect(await client().getRings("ledger-api")).toEqual({ notes: [LEDGER_RING_NOTE], rings: [] });
    expect(LEDGER_RING_NOTE.endsWith("; no ring is served")).toBe(true);
    for (const project of ["harbor-sim", "ledger-api"]) {
      expect(await client("empty").getRings(project)).toEqual({ notes: [], rings: [] });
    }
    const refused = await client("error")
      .getRings("harbor-sim")
      .then(
        () => null,
        (error: unknown) => error,
      );
    expect(apiErrorOf(refused).status).toBe(503);
  });

  it("answers a copy each time, so a caller's change never reaches the next read", async () => {
    const mock = client();
    const first = await mock.getRings("harbor-sim");
    first.rings.pop();
    expect((await mock.getRings("harbor-sim")).rings).toHaveLength(4);
  });
});

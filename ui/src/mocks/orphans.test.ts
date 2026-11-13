import { describe, expect, it } from "vitest";
import type { HealthOrphan } from "../api/types";
import { MockClient } from "./MockClient";
import { judgedDocuments, nodeViewOf, usageAt } from "./corpus";
import { harborSim } from "./harbor-sim/fixtures";
import { LARGE_ORPHANS, ORPHANS_QUEUE_NOTE } from "./orphans";

// UO4, UO5, UO11 of docs/features/usage-orphans.md "Acceptance criteria", the mock: row 8 judged from
// the corpus as the core judges it, the catalog's and `show --links`' counts the same numbers;
// harbor-sim a used document, unused ones, a used section and five orphans, two with an open task;
// `empty` none; `large` 3 000; `orphan-notes` the queue not read.

const KEYS = ["name", "id", "kind", "title", "path", "line", "status", "tasks", "tasks_open"];

function client(scenario: ConstructorParameters<typeof MockClient>[0]) {
  return new MockClient(scenario, { delayMs: 0, now: () => Date.parse("2026-11-13T12:00:00Z") });
}

function brief(row: HealthOrphan): string {
  return `${row.name} ${row.kind ?? "-"} ${String(row.tasks)}/${String(row.tasks_open)}`;
}

describe("the mock's orphan documents (UO4)", () => {
  it("normal: harbor-sim's five over its 23 live documents, by kind then path, no kind last; two mechanics with an open task each, three decisions with none; each row its 9 keys", async () => {
    const { orphans } = await client("normal").getHealth("harbor-sim");
    expect([orphans.live, orphans.notes]).toEqual([23, []]);
    expect(orphans.rows.map(brief)).toEqual(["MEC-CARGO-CRANES mechanic 1/1", "MEC-LOCK-B mechanic 1/1", "DEC-0014 - 0/0", "DEC-0012 - 0/0", "DEC-0013 - 0/0"]);
    expect(orphans.rows.map((row) => Object.keys(row))).toEqual(orphans.rows.map(() => KEYS));
    expect(orphans.rows.map((row) => row.path)).toEqual(["docs/spec/cranes.md", "docs/spec/locks/lock-b.md", "docs/decisions/DEC-0014.md", "docs/spec/records/DEC-0012-simulated-weather.md", "docs/spec/records/DEC-0013-crane-reach.md"]);
  });

  it("normal: ledger-api none over its five; empty: none over none", async () => {
    expect((await client("normal").getHealth("ledger-api")).orphans).toEqual({ live: 5, rows: [], notes: [] });
    for (const project of ["harbor-sim", "ledger-api"]) {
      expect([project, (await client("empty").getHealth(project)).orphans]).toEqual([project, { live: 0, rows: [], notes: [] }]);
    }
  });

  it("large: 3 000, harbor-sim's five among them, by kind then path, no kind last; ledger-api as normal", async () => {
    const { orphans } = await client("large").getHealth("harbor-sim");
    expect(orphans.rows).toHaveLength(LARGE_ORPHANS);
    expect(orphans.live).toBeGreaterThanOrEqual(LARGE_ORPHANS);
    for (const name of ["MEC-CARGO-CRANES", "MEC-LOCK-B", "DEC-0012", "DEC-0013", "DEC-0014"]) {
      expect([name, orphans.rows.filter((row) => row.name === name).length]).toEqual([name, 1]);
    }
    const order = (row: HealthOrphan) => `${row.kind === null ? "\u{10ffff}" : row.kind}\u0000${row.path}`;
    const keys = orphans.rows.map(order);
    expect(keys).toEqual([...keys].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0)));
    expect(orphans.rows.some((row) => row.tasks !== null && row.tasks > 0)).toBe(true);
    expect((await client("large").getHealth("ledger-api")).orphans.rows).toEqual([]);
  });

  it("orphan-notes: the rows as normal, each one's tasks null, the note why", async () => {
    const { orphans } = await client("orphan-notes").getHealth("harbor-sim");
    expect(orphans.rows.map((row) => row.name)).toEqual(["MEC-CARGO-CRANES", "MEC-LOCK-B", "DEC-0014", "DEC-0012", "DEC-0013"]);
    expect(orphans.rows.every((row) => row.tasks === null && row.tasks_open === null)).toBe(true);
    expect(orphans.notes).toEqual([ORPHANS_QUEUE_NOTE]);
  });
});

describe("one count everywhere (UO1, UO5)", () => {
  const { corpus } = harborSim(Date.parse("2026-11-13T12:00:00Z"));

  it("every live document's `show --links` usage equals its judged counts; a used one, unused ones, a used section", () => {
    for (const { node, usage } of judgedDocuments(corpus)) {
      const shown = nodeViewOf(corpus, node.id ?? node.path, { with: ["links"] }).nodes.find((each) => each.path === node.path);
      expect([node.path, shown?.links?.usage]).toEqual([node.path, { used_by: usage.used_by, mentioned_by: usage.mentioned_by }]);
    }
    expect(nodeViewOf(corpus, "MEC-TIDES", { with: ["links"] }).nodes[0]?.links?.usage).toEqual({ used_by: 5, mentioned_by: 2 });
    expect(nodeViewOf(corpus, "DOM-HARBOR", { with: ["links"] }).nodes[0]?.links?.usage).toEqual({ used_by: 0, mentioned_by: 0 });
    const section = corpus.documents.flatMap((file) => file.sections).find((node) => node.id === "RULE-TIDE-WINDOW");
    expect(section === undefined ? null : usageAt(corpus, section.path, section.line)).toEqual({ used_by: 2, mentioned_by: 1, orphan: null });
  });

  it("`--archive` widens In and never the counts", () => {
    for (const { node } of judgedDocuments(corpus)) {
      const plain = nodeViewOf(corpus, node.id ?? node.path, { with: ["links"] }).nodes[0]?.links?.usage;
      const archived = nodeViewOf(corpus, node.id ?? node.path, { with: ["links"], archive: true }).nodes[0]?.links?.usage;
      expect([node.path, archived]).toEqual([node.path, plain]);
    }
  });

  it("the catalog's corpus rows carry the same counts and flag; a section's flag null", async () => {
    const catalog = await client("normal").getCatalog("harbor-sim", { kinds: ["mechanic"], archive: false });
    const own = catalog.rows.filter((row) => !row.path.startsWith("docs/spec/catalog/"));
    expect(own.length).toBeGreaterThan(0);
    for (const row of own) {
      const usage = usageAt(corpus, row.path, row.line);
      expect([row.path, row.line, row.used_by, row.mentioned_by, row.orphan]).toEqual([row.path, row.line, usage?.used_by, usage?.mentioned_by, usage?.orphan]);
    }
    expect(catalog.rows.find((row) => row.id === "MEC-CARGO-CRANES")?.orphan).toBe(true);
    const invented = catalog.rows.filter((row) => row.path.startsWith("docs/spec/catalog/"));
    expect(invented.some((row) => row.orphan === true)).toBe(true);
    expect(invented.some((row) => row.used_by === 0 && row.mentioned_by > 0)).toBe(true);
    expect(invented.filter((row) => row.line !== 1).every((row) => row.orphan === null)).toBe(true);
    expect(catalog.rows.map((row) => Object.keys(row).slice(11, 16))).toEqual(catalog.rows.map(() => ["tasks_open", "used_by", "mentioned_by", "orphan", "last_change"]));
  });
});

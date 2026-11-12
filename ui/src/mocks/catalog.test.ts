import { describe, expect, it } from "vitest";
import { apiErrorOf } from "../api/client";
import type { CatalogDocument } from "../api/types";
import { pinnedCatalog as pinnedJson } from "../test/catalogStub";
import { HARBOR_CATALOG_ROWS, pinnedCatalog } from "./catalog";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The mock's catalog (src/mocks/catalog.ts;
// docs/features/direction-catalog.md "Acceptance criteria", DC-6, DC-9, DC-10): the spec's scenario
// as its JSON pins it; harbor-sim's direction, 130 rows without the archive, the tree's own first,
// every shape the table draws; `empty`, an empty direction; ledger-api, the tree's rows alone; no
// kind refused 400.

/** A document with each OID read as `<oid>` once it has the shape the spec gives it. */
function oidsByShape(document: CatalogDocument): string {
  return JSON.stringify(document, (key, value: unknown) => (key === "commit" && typeof value === "string" && /^([0-9a-f]{40}|[0-9a-f]{64})$/.test(value) ? "<oid>" : value));
}

async function catalog(scenario: Scenario, project = "harbor-sim", kinds = ["mechanic"], archive = false): Promise<CatalogDocument> {
  return new MockClient(scenario, { now: () => Date.parse("2026-11-12T12:00:00Z") }).getCatalog(project, { kinds, archive });
}

describe("the spec's scenario (DC-9)", () => {
  it("is the spec's JSON key for key and in order, the OIDs 40 hex digits", () => {
    expect(oidsByShape(pinnedCatalog())).toBe(oidsByShape(pinnedJson()));
    expect(oidsByShape(pinnedJson())).toContain('"commit":"<oid>"');
  });
});

describe("harbor-sim's direction (DC-6, DC-10)", () => {
  it("serves 130 rows without the archive, the tree's mechanics first in its order, then invented ones", async () => {
    const client = new MockClient("normal");
    const document = await client.getCatalog("harbor-sim", { kinds: ["mechanic"], archive: false });
    const tree = await client.getTree("harbor-sim", { kinds: ["mechanic"] });
    expect(document.rows).toHaveLength(HARBOR_CATALOG_ROWS);
    expect(HARBOR_CATALOG_ROWS).toBe(130);
    expect(document.rows.slice(0, tree.nodes.length).map((row) => [row.id, row.title, row.path, row.line, row.rev])).toEqual(tree.nodes.map((node) => [node.id, node.title, node.path, node.line, node.rev]));
    expect(document.rows.slice(tree.nodes.length).every((row) => row.id?.startsWith("MEC-CAT-") === true && row.kind === "mechanic")).toBe(true);
    expect(new Set(document.rows.map((row) => `${row.path}:${String(row.line)}`)).size).toBe(HARBOR_CATALOG_ROWS);
    expect(document).toMatchObject({ kinds: ["mechanic"], archive: false, notes: [], truncated: false, omitted: null });
  });

  it("holds every shape: each state and one unknown, no version, a section's change of its file, none read, open items and tasks above 0, any script", async () => {
    const { rows } = await catalog("normal");
    expect(new Set(rows.map((row) => row.state))).toEqual(new Set(["in work", "no card", "confirmed", "suspect", "departed", "parked"]));
    expect(rows.some((row) => row.rev === null)).toBe(true);
    expect(rows.some((row) => row.line > 1 && row.last_change?.of_file === true)).toBe(true);
    expect(rows.every((row) => row.last_change === null || row.last_change.of_file === row.line > 1)).toBe(true);
    expect(rows.some((row) => row.last_change === null)).toBe(true);
    expect(rows.some((row) => row.open_items > 0)).toBe(true);
    expect(rows.some((row) => row.tasks > 0 && row.tasks_open > 0)).toBe(true);
    expect(rows.some((row) => row.title === null)).toBe(true);
    expect(rows.some((row) => row.title !== null && /[\u0400-\u04ff]/.test(row.title))).toBe(true);
    expect(rows.some((row) => row.last_change !== null && /[\u3000-\u9fff]/.test(row.last_change.by_name))).toBe(true);
    expect(rows.every((row) => row.last_change === null || row.last_change.by.startsWith(`${row.last_change.by_name} <`))).toBe(true);
    expect(rows.some((row) => row.last_change?.proposal === null) && rows.some((row) => row.last_change?.proposal !== null && row.last_change !== null)).toBe(true);
  });

  it("with the archive, the archived mechanic too, the invented rows unchanged", async () => {
    const plain = await catalog("normal");
    const archived = await catalog("normal", "harbor-sim", ["mechanic"], true);
    expect(archived.archive).toBe(true);
    expect(archived.rows.filter((row) => row.archived).map((row) => row.id)).toEqual(["MEC-OLD-QUAYS"]);
    expect(archived.rows).toHaveLength(HARBOR_CATALOG_ROWS + 1);
    expect(archived.rows.filter((row) => row.id?.startsWith("MEC-CAT-") === true).map((row) => row.id)).toEqual(plain.rows.filter((row) => row.id?.startsWith("MEC-CAT-") === true).map((row) => row.id));
  });

  it("empty: the direction holds no row", async () => {
    expect((await catalog("empty")).rows).toEqual([]);
  });

  it("large: the corpus's own mechanics alone, more than 130", async () => {
    const { rows } = await catalog("large");
    expect(rows.length).toBeGreaterThan(HARBOR_CATALOG_ROWS);
    expect(rows.some((row) => row.id?.startsWith("MEC-CAT-") === true)).toBe(false);
  });

  it("ledger-api: the tree's rows of the kinds asked, none invented", async () => {
    const client = new MockClient("normal");
    const tree = await client.getTree("ledger-api", { kinds: ["mechanic"] });
    expect((await client.getCatalog("ledger-api", { kinds: ["mechanic"], archive: false })).rows).toHaveLength(tree.nodes.length);
  });

  it("no kind: refused 400 in the daemon's words; error: the scenario's 503", async () => {
    const refused = await new MockClient("normal").getCatalog("harbor-sim", { kinds: [], archive: false }).then(
      () => null,
      (error: unknown) => apiErrorOf(error),
    );
    expect(refused?.status).toBe(400);
    expect(refused?.message).toContain("kinds");
    const failed = await catalog("error").then(
      () => null,
      (error: unknown) => apiErrorOf(error),
    );
    expect(failed?.status).toBe(503);
  });
});

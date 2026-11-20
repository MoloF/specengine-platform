import { describe, expect, it } from "vitest";
import { harborSim } from "./harbor-sim/fixtures";
import { ledgerApi } from "./ledger-api/fixtures";
import { LARGE_LIBRARY_ROWS, libraryOf } from "./library";
import { MockClient } from "./MockClient";

// The mock's library (docs/canon/spec-library.md "Rows"): harbor-sim's documents in path
// order with the route's 23 keys (`disciplines` after `tags` since docs/features/department-practices.md
// "Acceptance criteria", DP14), tags on some rows, three practices, an accepted and a superseded decision on one
// canon page, one row behind its decision, the corpus's orphans, the modules the served `[[module]]`s
// hold; Tier 3 under archive; ledger-api's with git not read and its note; `empty` none; `large`
// 5 000 rows, the same on every read.

const ROW_KEYS = "name, id, path, title, class, kind, status, archived, tier, owner, reviewed, scope, tags, disciplines, rev, parent, modules, used_by, mentioned_by, orphan, last_change, decisions, behind";
const DECISION_KEYS = "name, id, title, path, status, group, date, section, last_change, newer";

const harbor = harborSim(0).corpus;

describe("the mock's library", () => {
  it("serves harbor-sim's live documents in path order, each with the route's 23 keys, the generated one among them", () => {
    const answer = libraryOf(harbor, "harbor-sim", "normal", false);
    expect(Object.keys(answer).join(", ")).toBe("archive, notes, counts, rows, truncated, omitted");
    const paths = answer.rows.map((row) => row.path);
    expect(paths).toEqual([...paths].sort());
    expect(answer.rows.every((row) => Object.keys(row).join(", ") === ROW_KEYS)).toBe(true);
    expect(answer.rows.flatMap((row) => row.decisions).every((decision) => Object.keys(decision).join(", ") === DECISION_KEYS)).toBe(true);
    expect(answer.rows).toHaveLength(harbor.documents.filter((file) => !file.node.archived).length);
    expect(answer.rows.some((row) => row.class === "generated")).toBe(true);
    expect(answer.rows.some((row) => row.archived)).toBe(false);
    expect([answer.notes, answer.truncated, answer.omitted]).toEqual([[], false, null]);
  });

  it("names disciplines on three live canon pages and a spec page, the harbour's two, and on the Tier 3 old quays under archive alone (docs/features/department-practices.md \"The library\")", () => {
    const live = libraryOf(harbor, "harbor-sim", "normal", false);
    expect(live.rows.filter((row) => row.disciplines.length > 0).map((row) => [row.name, row.disciplines])).toEqual([
      ["MEC-MOORING", ["copy"]],
      ["MEC-CARGO-CRANES", ["plates"]],
      ["DOM-HARBOR", ["copy", "plates"]],
      ["MEC-TIDES", ["copy"]],
    ]);
    const archived = libraryOf(harbor, "harbor-sim", "normal", true).rows.filter((row) => row.archived && row.disciplines.length > 0);
    expect(archived.map((row) => [row.name, row.tier, row.disciplines])).toEqual([["MEC-OLD-QUAYS", 3, ["copy"]]]);
  });

  it("holds tags on some rows, in any script, and six canon pages", () => {
    const answer = libraryOf(harbor, "harbor-sim", "normal", false);
    const tagged = answer.rows.filter((row) => row.tags.length > 0);
    expect(tagged.length).toBeGreaterThan(5);
    expect(tagged.length).toBeLessThan(answer.rows.length);
    const tags = tagged.flatMap((row) => row.tags);
    expect(tags).toContain("\u043f\u0440\u0438\u0447\u0430\u043b");
    expect(tags).toContain("\u6f6e");
    expect(answer.rows.filter((row) => row.class === "canon").map((row) => row.name).sort()).toEqual(["DOM-BERTHS", "DOM-HARBOR", "DOM-PILOTS", "DOM-WATER", "MEC-CARGO-CRANES", "MEC-TIDES"]);
  });

  it("puts an accepted and a superseded decision on the harbour's canon page, newer than it: the one row behind its decision", () => {
    const answer = libraryOf(harbor, "harbor-sim", "normal", false);
    const page = answer.rows.find((row) => row.name === "DOM-HARBOR");
    expect(page?.decisions.map((decision) => [decision.name, decision.group, decision.newer])).toEqual([
      ["DEC-0012", "accepted", true],
      ["DEC-0009", "superseded", false],
      ["DEC-0010", "rejected", false],
    ]);
    expect(page?.decisions[0]?.section).toBe("RULE-HARBOR-CLOCK");
    expect(page?.decisions[0]?.date).toBe("2026-09-28");
    expect(page?.behind).toBe(true);
    expect(answer.rows.filter((row) => row.behind === true).map((row) => row.name)).toEqual(["DOM-HARBOR"]);
    // The waters' page committed after its accepted decision; the cranes' page never committed.
    expect(answer.rows.find((row) => row.name === "DOM-WATER")?.behind).toBe(false);
    expect(answer.rows.find((row) => row.name === "MEC-CARGO-CRANES")?.behind).toBeNull();
    expect(answer.counts).toEqual({ documents: answer.rows.length, archived: 0, decisions: 6, behind: 1, orphans: answer.rows.filter((row) => row.orphan === true).length });
    expect(answer.counts.orphans).toBeGreaterThan(0);
  });

  it("names the corpus's decisions, so each opens; a decision's last change is its own row's", () => {
    const answer = libraryOf(harbor, "harbor-sim", "normal", false);
    const byName = new Map(answer.rows.map((row) => [row.name, row]));
    for (const name of ["DEC-0012", "DEC-0013", "DEC-0014"]) {
      const listed = answer.rows.flatMap((row) => row.decisions).find((decision) => decision.name === name);
      expect([name, listed?.last_change]).toEqual([name, byName.get(name)?.last_change]);
      expect(byName.get(name)?.class).toBe("decision");
    }
  });

  it("holds the modules the served [[module]]s give: the waters' subtree, the mechanics by kind", () => {
    const answer = libraryOf(harbor, "harbor-sim", "normal", false);
    const holding = (module: string) => answer.rows.filter((row) => row.modules.includes(module)).map((row) => row.name);
    expect(holding("waters")).toContain("DOM-WATER");
    expect(holding("waters")).toContain("MEC-TIDES");
    expect(holding("waters")).not.toContain("DOM-HARBOR");
    expect(answer.rows.filter((row) => row.kind === "mechanic").every((row) => row.modules.includes("mechanics"))).toBe(true);
  });

  it("adds Tier 3 under archive, its decisions listed either way", () => {
    const live = libraryOf(harbor, "harbor-sim", "normal", false);
    const all = libraryOf(harbor, "harbor-sim", "normal", true);
    expect(all.archive).toBe(true);
    const archived = all.rows.filter((row) => row.archived);
    expect(archived.map((row) => [row.name, row.tier])).toEqual([["MEC-OLD-QUAYS", 3]]);
    expect(all.counts.archived).toBe(1);
    expect(all.rows).toHaveLength(live.rows.length + 1);
    expect(all.counts.decisions).toBe(live.counts.decisions);
  });

  it("serves ledger-api with git not read: every last change null, its note once", () => {
    const answer = libraryOf(ledgerApi(0).corpus, "ledger-api", "normal", false);
    expect(answer.rows.length).toBeGreaterThan(0);
    expect(answer.rows.every((row) => row.last_change === null && row.decisions.length === 0 && row.behind === false)).toBe(true);
    expect(answer.notes).toEqual(["library: last change not read: not a repository"]);
  });

  it("serves `large`'s 5 000 rows, the same on every read; `empty` none", async () => {
    const large = new MockClient("large", { delayMs: 0 });
    const first = await large.getLibrary("harbor-sim", { archive: false });
    expect(first.rows).toHaveLength(LARGE_LIBRARY_ROWS);
    expect(await large.getLibrary("harbor-sim", { archive: false })).toEqual(first);
    expect(first.counts.behind).toBeGreaterThan(0);
    expect(new Set(first.rows.map((row) => row.path)).size).toBe(LARGE_LIBRARY_ROWS);
    const empty = new MockClient("empty", { delayMs: 0 });
    expect((await empty.getLibrary("harbor-sim", { archive: false })).rows).toEqual([]);
  });
});

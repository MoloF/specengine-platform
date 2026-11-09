import { describe, expect, it } from "vitest";
import { IMPLICIT_SECTIONS } from "../test/builders";
import { ALL_SECTIONS, homeHash, navOf, parseHash, sectionHash } from "./routes";
import { ru } from "../i18n/ru";
import { enT, ruT } from "../test/text";
import { aboutOf, isSectionId, sectionLabel, SECTIONS } from "./sections";

describe("hash routes", () => {
  it.each([
    ["", { type: "home" }],
    ["#", { type: "home" }],
    ["#/", { type: "home" }],
    ["#/alpha/inbox", { type: "section", project: "alpha", section: "inbox", id: null }],
    ["#/alpha/inbox/", { type: "section", project: "alpha", section: "inbox", id: null }],
    ["#/alpha/inbox/PR-0042", { type: "section", project: "alpha", section: "inbox", id: "PR-0042" }],
    ["#/alpha/tree/R-12", { type: "section", project: "alpha", section: "tree", id: "R-12" }],
    ["#/alpha/roadmap", { type: "section", project: "alpha", section: "roadmap", id: null }],
    ["#/alpha/roadmap/RM-0004", { type: "section", project: "alpha", section: "roadmap", id: "RM-0004" }],
    ["#/alpha/board", { type: "section", project: "alpha", section: "board", id: null }],
    ["#/alpha/board/C-0001", { type: "section", project: "alpha", section: "board", id: "C-0001" }],
    ["#/alpha/flow", { type: "section", project: "alpha", section: "flow", id: null }],
    ["#/a/flow/MEC-TIDES", { type: "section", project: "a", section: "flow", id: "MEC-TIDES" }],
    ["#/alpha", { type: "project", project: "alpha" }],
    ["#/alpha/", { type: "project", project: "alpha" }],
    ["#/a%20b", { type: "project", project: "a b" }],
    ["#/alpha//", { type: "not_found" }],
    ["#/alpha//x", { type: "not_found" }],
    ["#/%E0%A4%A", { type: "not_found" }],
    ["#/alpha/nowhere", { type: "not_found" }],
    ["#/alpha/inbox/PR-1/extra", { type: "not_found" }],
    ["#main", { type: "not_found" }],
    ["#/alpha/inbox/%E0%A4%A", { type: "not_found" }],
  ])("reads %j", (hash, route) => {
    expect(parseHash(hash)).toEqual(route);
  });

  it("lists the nav over every section: Overview to the home, then the nine, Board third, Roadmap fourth, Flow fifth (the palette's list, and the nav when no workspace was read)", () => {
    expect(navOf("a b", ALL_SECTIONS, enT).map(({ label, hash }) => [label, hash])).toEqual([
      ["Overview", "#/a%20b"],
      ["Inbox", "#/a%20b/inbox"],
      ["Tasks", "#/a%20b/tasks"],
      ["Board", "#/a%20b/board"],
      ["Roadmap", "#/a%20b/roadmap"],
      ["Flow", "#/a%20b/flow"],
      ["Spec tree", "#/a%20b/tree"],
      ["Graph", "#/a%20b/graph"],
      ["Health", "#/a%20b/health"],
      ["Questions", "#/a%20b/questions"],
    ]);
    expect(ALL_SECTIONS).toEqual(SECTIONS.map((section) => section.id));
  });

  // WS-1 of docs/features/workspaces.md: the implicit workspace's nav, Tasks off it (M2: the nav drawn from SECTIONS).
  it("lists Overview, then a workspace's sections in its order: the implicit eight leave Tasks out", () => {
    expect(navOf("ledger-api", IMPLICIT_SECTIONS, enT).map(({ label }) => label)).toEqual(["Overview", "Inbox", "Board", "Roadmap", "Flow", "Spec tree", "Graph", "Health", "Questions"]);
    expect(navOf("p", ["health", "board", "tree"], enT).map(({ key, hash }) => [key, hash])).toEqual([
      ["overview", "#/p"],
      ["health", "#/p/health"],
      ["board", "#/p/board"],
      ["tree", "#/p/tree"],
    ]);
  });

  it("draws a served ID only if it is one of the nine, and once; a list of none is Overview alone (A4 and the edge cases of workspaces)", () => {
    expect(navOf("p", ["board", "stage", "board", "overview", "Tree", "tasks"], enT).map(({ key }) => key)).toEqual(["overview", "board", "tasks"]);
    expect(navOf("p", [], enT).map(({ key }) => key)).toEqual(["overview"]);
    expect(navOf("p", ["stage"], enT).map(({ key }) => key)).toEqual(["overview"]);
  });

  // WS-2: Tasks leaves the nav by data, never the routes (M3: tasks dropped from isSectionId).
  it("keeps Tasks a section and a route", () => {
    expect(isSectionId("tasks")).toBe(true);
    expect(parseHash("#/ledger-api/tasks/T-0107")).toEqual({ type: "section", project: "ledger-api", section: "tasks", id: "T-0107" });
    expect(ALL_SECTIONS).toContain("tasks");
  });

  it("writes what it reads", () => {
    expect(homeHash("alpha")).toBe("#/alpha");
    expect(parseHash(homeHash("a/b c"))).toEqual({ type: "project", project: "a/b c" });
    expect(sectionHash("alpha", "inbox")).toBe("#/alpha/inbox");
    expect(sectionHash("alpha", "inbox", "PR-1")).toBe("#/alpha/inbox/PR-1");
    expect(sectionHash("alpha", "roadmap")).toBe("#/alpha/roadmap");
    expect(sectionHash("alpha", "roadmap", "RM-0004")).toBe("#/alpha/roadmap/RM-0004");
    expect(sectionHash("alpha", "flow", "MEC-TIDES")).toBe("#/alpha/flow/MEC-TIDES");
    expect(parseHash(sectionHash("alpha", "roadmap", "RM-0004"))).toEqual({ type: "section", project: "alpha", section: "roadmap", id: "RM-0004" });
    expect(parseHash(sectionHash("a b", "graph", "X/Y"))).toEqual({
      type: "section",
      project: "a b",
      section: "graph",
      id: "X/Y",
    });
  });
});

// AC-05 of docs/features/ui-tree-node.md: a REF is one segment, encoded by sectionHash, decoded
// once by parseHash: an ID, `slug/ID`, `ID#SECTION` (%23) or a root-relative path (%2F).
describe("the tree's REF in the hash", () => {
  it.each([
    ["MEC-TIDES", "#/harbor-sim/tree/MEC-TIDES"],
    ["MEC-TIDES#RULE-TIDE-WINDOW", "#/harbor-sim/tree/MEC-TIDES%23RULE-TIDE-WINDOW"],
    ["tide-cycle/MEC-TIDES", "#/harbor-sim/tree/tide-cycle%2FMEC-TIDES"],
    ["docs/spec/tides/tide-cycle.md", "#/harbor-sim/tree/docs%2Fspec%2Ftides%2Ftide-cycle.md"],
  ])("writes %s as one segment and reads it back", (ref, hash) => {
    expect(sectionHash("harbor-sim", "tree", ref)).toBe(hash);
    expect(parseHash(hash)).toEqual({ type: "section", project: "harbor-sim", section: "tree", id: ref });
  });

  it("decodes once: %2523 stays %23", () => {
    expect(parseHash("#/p/tree/A%2523B")).toEqual({ type: "section", project: "p", section: "tree", id: "A%23B" });
  });

  it("refuses an unencoded slash as an extra segment", () => {
    expect(parseHash("#/p/tree/docs/spec/a.md")).toEqual({ type: "not_found" });
  });
});

// LG-3 of docs/features/ui-languages.md: each of the nine sections' label and about is ru.ts's under `ruT`.
describe("the sections in Russian (LG-3)", () => {
  it("label and describe each section from ru.ts", () => {
    expect(SECTIONS).toHaveLength(9);
    for (const { id } of SECTIONS) {
      expect([id, sectionLabel(id, ruT), aboutOf(id, ruT)]).toEqual([id, ru.app.sections[id].label, ru.app.sections[id].about]);
    }
    expect(navOf("p", ["inbox"], ruT).map(({ label }) => label)).toEqual([ru.app.overview, ru.app.sections.inbox.label]);
  });
});

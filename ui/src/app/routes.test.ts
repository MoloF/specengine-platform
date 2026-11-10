import { describe, expect, it } from "vitest";
import { IMPLICIT_SECTIONS } from "../test/builders";
import { ALL_SECTIONS, cardPageHash, docOf, docPageHash, flowHash, homeHash, isBareFlow, navOf, parseFlowPlace, parseHash, sectionHash, withDoc, withoutDoc, type FlowPlace } from "./routes";
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
      ["Agent tasks", "#/a%20b/tasks"],
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

// ST-1 of docs/features/writer-studio.md: Flow's place in the hash, `#/<p>/flow[/<name>][?module=&scope=&stage=&edge=]`.
describe("Flow's place (ST-1 of writer-studio)", () => {
  const read = (hash: string): FlowPlace => {
    const route = parseHash(hash);
    if (route.type !== "section" || route.section !== "flow") {
      throw new Error(`${hash} is no Flow route`);
    }
    return parseFlowPlace(route);
  };

  const MODULES = [null, "", "berths"];
  const SCOPES = [null, "DOM-BERTHS", "docs/a b/c#d?e&f=g,h+i%j.md"];
  const STAGES = [null, "-", "quayside", "a b+c&d=e/f?#,%", "\u043f\u0435\u0447\u0430\u0442\u044c"];
  const SELECTS: FlowPlace["select"][] = [
    null,
    { node: "MEC-MOORING" },
    { node: "docs/x y.md" },
    { edge: { from: "A", to: "B", type: "follows", kind: null } },
    { edge: { from: "A/B?c", to: "C D&e=f", type: "uses", kind: "a,b" } },
  ];

  it("round-trips every place: modules, scopes, stages and selections crossed, 225 of them (M3: edge parts unencoded)", () => {
    let count = 0;
    for (const module of MODULES) {
      for (const scope of SCOPES) {
        for (const stage of STAGES) {
          for (const select of SELECTS) {
            const place: FlowPlace = { module, scope, stage, select };
            expect(read(flowHash("harbor-sim", place))).toEqual(place);
            count += 1;
          }
        }
      }
    }
    expect(count).toBeGreaterThanOrEqual(200);
  });

  it("writes the keys in order, each only when set, the name as the path segment", () => {
    expect(flowHash("harbor-sim", { module: "", scope: "DOM-BERTHS", stage: "quayside", select: { node: "MEC-MOORING" } })).toBe("#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS&stage=quayside");
    expect(flowHash("harbor-sim", { module: "", scope: null, stage: null, select: { edge: { from: "MEC-FAIRWAY", to: "DOM-BERTHS", type: "follows", kind: "alternative" } } })).toBe("#/harbor-sim/flow?module=&edge=MEC-FAIRWAY,DOM-BERTHS,follows,alternative");
    expect(flowHash("harbor-sim", { module: null, scope: null, stage: null, select: null })).toBe("#/harbor-sim/flow");
    expect(flowHash("a b", { module: null, scope: "x,y", stage: "-", select: null })).toBe("#/a%20b/flow?scope=x%2Cy&stage=-");
  });

  it("reads the old form as a selection alone (M2)", () => {
    expect(read("#/harbor-sim/flow/MEC-MOORING")).toEqual({ module: null, scope: null, stage: null, select: { node: "MEC-MOORING" } });
    expect(isBareFlow({ id: null })).toBe(true);
    expect(isBareFlow({ id: null, query: "module=" })).toBe(false);
    expect(isBareFlow({ id: "MEC-MOORING" })).toBe(false);
  });

  it("decodes with decodeURIComponent alone: a `+` is no space (M1)", () => {
    expect(read("#/p/flow?scope=a+b&stage=c%20d").scope).toBe("a+b");
    expect(read("#/p/flow?scope=a+b&stage=c%20d").stage).toBe("c d");
  });

  it("lets a name win over an edge, the first of a repeated key, and drops what does not read", () => {
    expect(read("#/p/flow/N?edge=A,B,t").select).toEqual({ node: "N" });
    expect(read("#/p/flow?scope=A&scope=B").scope).toBe("A");
    expect(read("#/p/flow?scope=&stage=").scope).toBeNull();
    expect(read("#/p/flow?scope=&stage=").stage).toBeNull();
    expect(read("#/p/flow?module=").module).toBe("");
    expect(read("#/p/flow?scope=%E0%A4%A&stage=s").scope).toBeNull();
    expect(read("#/p/flow?scope=%E0%A4%A&stage=s").stage).toBe("s");
    expect(read("#/p/flow?other=1&scope=S&module").scope).toBe("S");
    expect(read("#/p/flow?module").module).toBeNull();
    for (const edge of ["A,B", "A,,t", "A,B,t,k,x", "A,B,%E0%A4%A"]) {
      expect([edge, read(`#/p/flow?edge=${edge}`).select]).toEqual([edge, null]);
    }
    expect(read("#/p/flow?edge=A,B,t,").select).toBeNull();
    expect(read("#/p/flow?edge=A,B,t&edge=C,D,u").select).toEqual({ edge: { from: "A", to: "B", type: "t", kind: null } });
  });

  it("splits the first raw `?` on every route: Flow takes what follows as its query, any other section ignores it (DM-5 of doc-modal)", () => {
    expect(parseHash("#/p/tree/A?B")).toEqual({ type: "section", project: "p", section: "tree", id: "A" });
    expect(parseHash("#/p/flow/A?scope=B")).toEqual({ type: "section", project: "p", section: "flow", id: "A", query: "scope=B" });
    expect(parseHash("#/p/flow?scope=B?C")).toEqual({ type: "section", project: "p", section: "flow", id: null, query: "scope=B?C" });
    expect(parseHash("#/p/inbox/PR-1")).toEqual({ type: "section", project: "p", section: "inbox", id: "PR-1" });
  });
});

// BH-6 of board-handoff: a card's page is a fourth segment after the card's ID, never a tenth section.
describe("a card's page", () => {
  it("parses exactly `board`, `<C>`, `page` as the board with the card's page; any other fourth segment is not found (M8)", () => {
    expect(parseHash("#/alpha/board/C-0001/page")).toEqual({ type: "section", project: "alpha", section: "board", id: "C-0001", view: "page" });
    expect(parseHash("#/alpha/board/C-0001")).toEqual({ type: "section", project: "alpha", section: "board", id: "C-0001" });
    for (const hash of ["#/alpha/board/C-0001/pages", "#/alpha/board/C-0001/page/x", "#/alpha/board/C-0001/", "#/alpha/board//page", "#/alpha/tasks/T-1/page"]) {
      expect([hash, parseHash(hash).type]).toEqual([hash, "not_found"]);
    }
  });

  it("round-trips both forms for IDs and projects holding %, / and a space", () => {
    for (const project of ["alpha", "a b", "50%", "x/y"]) {
      for (const id of ["C-0001", "C 1", "C%2", "C/3"]) {
        expect(parseHash(cardPageHash(project, id))).toEqual({ type: "section", project, section: "board", id, view: "page" });
        expect(parseHash(sectionHash(project, "board", id))).toEqual({ type: "section", project, section: "board", id });
      }
    }
    expect(cardPageHash("a b", "C/3")).toBe("#/a%20b/board/C%2F3/page");
  });

  it("is no section: the nine stay pinned", () => {
    expect(ALL_SECTIONS).toHaveLength(9);
    expect(isSectionId("page")).toBe(false);
  });
});

// DM-5 of docs/features/doc-modal.md: the document modal is the `doc` pair of any project route.
describe("the document modal's pair (DM-5 of doc-modal)", () => {
  it("reads `doc` on the project's home, a card, a page and Flow; Flow's query keeps the rest in order", () => {
    expect(parseHash("#/p?doc=A")).toEqual({ type: "project", project: "p", doc: "A" });
    expect(parseHash("#/p/board/C-1?doc=A")).toEqual({ type: "section", project: "p", section: "board", id: "C-1", doc: "A" });
    expect(parseHash("#/p/tree/R/page?doc=A")).toEqual({ type: "section", project: "p", section: "tree", id: "R", view: "page", doc: "A" });
    expect(parseHash("#/p/flow?scope=S&doc=A&stage=x")).toEqual({ type: "section", project: "p", section: "flow", id: null, query: "scope=S&stage=x", doc: "A" });
    expect(parseHash("#/p/inbox?doc=DOM-HARBOR%23RULE-HARBOR-CLOCK")).toEqual({ type: "section", project: "p", section: "inbox", id: null, doc: "DOM-HARBOR#RULE-HARBOR-CLOCK" });
  });

  it("leaves Flow bare under the modal alone, drops an empty or undecodable `doc`, and lets the first decodable one win", () => {
    expect(parseHash("#/p/flow?doc=A")).toEqual({ type: "section", project: "p", section: "flow", id: null, doc: "A" });
    expect(isBareFlow({ id: null, query: (parseHash("#/p/flow?doc=A") as { query?: string }).query })).toBe(true);
    expect(parseHash("#/p/inbox?doc=")).toEqual({ type: "section", project: "p", section: "inbox", id: null });
    expect(parseHash("#/p/inbox?doc=%E0%A4%A")).toEqual({ type: "section", project: "p", section: "inbox", id: null });
    expect(parseHash("#/p?doc")).toEqual({ type: "project", project: "p" });
    expect(docOf("#/p/inbox?doc=&doc=%E0%A4%A&doc=B&doc=C")).toBe("B");
    expect(parseHash("#/p/flow?doc=&scope=S")).toEqual({ type: "section", project: "p", section: "flow", id: null, query: "scope=S" });
    // `#/` and a route that names nothing take no modal.
    expect(docOf("#/?doc=A")).toBeNull();
    expect(docOf("#/p/nowhere?doc=A")).toBeNull();
  });

  it("ends a Tree id at a raw `?`; `tree/<REF>/page` is a document's page, any other fourth segment nothing (DM-11)", () => {
    expect(parseHash("#/p/tree/A?B")).toEqual({ type: "section", project: "p", section: "tree", id: "A" });
    expect(parseHash("#/p/tree/R/page")).toEqual({ type: "section", project: "p", section: "tree", id: "R", view: "page" });
    for (const hash of ["#/p/tree/R/pages", "#/p/tree/R/page/x", "#/p/tree//page", "#/p/graph/R/page", "#/p/flow/R/page", "#/p/inbox/PR-1/page"]) {
      expect([hash, parseHash(hash).type]).toEqual([hash, "not_found"]);
    }
    expect(parseHash("#/p/board/C-1/page")).toEqual({ type: "section", project: "p", section: "board", id: "C-1", view: "page" });
  });

  it("writes a document's page with its REF encoded: `#` and `/` held in one segment", () => {
    expect(docPageHash("p", "ID#SEC")).toBe("#/p/tree/ID%23SEC/page");
    expect(docPageHash("a b", "docs/x y.md")).toBe("#/a%20b/tree/docs%2Fx%20y.md/page");
    for (const ref of ["ID#SEC", "docs/x y.md", "slug/ID", "50%"]) {
      expect(parseHash(docPageHash("p", ref))).toEqual({ type: "section", project: "p", section: "tree", id: ref, view: "page" });
    }
  });

  it("puts one `doc` pair last and takes every one away, round-tripping Flow's pairs with an edge's raw commas (M: a second `doc` kept)", () => {
    const flow = "#/p/flow?module=&scope=DOM-BERTHS&edge=MEC-FAIRWAY,DOM-BERTHS,follows,alternative";
    expect(withDoc(flow, "MEC-MOORING")).toBe(`${flow}&doc=MEC-MOORING`);
    expect(withoutDoc(withDoc(flow, "MEC-MOORING"))).toBe(flow);
    expect(withDoc(`${flow}&doc=A`, "B")).toBe(`${flow}&doc=B`);
    expect(withDoc("#/p/flow?doc=A&scope=S&doc=B", "C")).toBe("#/p/flow?scope=S&doc=C");
    expect(withDoc("#/p/board/C-1", "ID#SEC")).toBe("#/p/board/C-1?doc=ID%23SEC");
    expect(withoutDoc("#/p/board/C-1?doc=A")).toBe("#/p/board/C-1");
    expect(withoutDoc("#/p/inbox")).toBe("#/p/inbox");
    expect(parseFlowPlace(parseHash(withDoc(flow, "X")) as { id: string | null; query?: string })).toEqual(parseFlowPlace(parseHash(flow) as { id: string | null; query?: string }));
    expect(docOf(withDoc("#/p", "docs/a b.md"))).toBe("docs/a b.md");
  });
});

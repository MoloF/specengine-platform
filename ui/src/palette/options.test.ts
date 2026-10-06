import { describe, expect, it } from "vitest";
import type { InboxEntry, Project, SearchHit, TaskListEntry } from "../api/types";
import { anEntry, aSearchHit, aTaskEntry } from "../test/builders";
import { SOME_TASKS } from "../test/taskStub";
import { groupLabel, paletteGroups, type Held, type PaletteInput, type PaletteOption } from "./options";

// docs/features/ui-home.md "Palette": groups in order, at most ten options each, NFC-folded
// substring matching on the trimmed text, an exact ID's group first (AC-13).

const PROJECTS: Project[] = [
  { slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" },
  { slug: "beta", name: "Beta harbour", root: "/work/beta", branch: null },
];

function ready<T>(value: T): Held<T> {
  return { state: "ready", value };
}

function input(text: string, fields: Partial<PaletteInput> = {}): PaletteInput {
  return {
    project: "alpha",
    text,
    tasks: ready<readonly TaskListEntry[]>(SOME_TASKS),
    inbox: ready<readonly InboxEntry[]>([anEntry({ id: "PR-1", summary: "Tide window" }), anEntry({ id: "PR-2", summary: "Berth T-0002" })]),
    projects: ready<readonly Project[]>(PROJECTS),
    search: null,
    ...fields,
  };
}

function shape(text: string, fields: Partial<PaletteInput> = {}): [string, string[]][] {
  return paletteGroups(input(text, fields)).map((group) => [groupLabel(group), group.options.map((option) => option.key)]);
}

function hashOf(option: PaletteOption | undefined): string | null {
  return option === undefined || option.type === "search" ? null : option.hash;
}

describe("the palette's groups", () => {
  it("lists Sections (Overview first) and Projects for an empty or blank text", () => {
    const expected = [
      [
        "Sections",
        ["section:overview", "section:inbox", "section:tasks", "section:tree", "section:graph", "section:health", "section:questions"],
      ],
      ["Projects", ["project:alpha", "project:beta"]],
    ];
    expect(shape("")).toEqual(expected);
    expect(shape("   ")).toEqual(expected);
    const sections = paletteGroups(input(""))[0]?.options ?? [];
    expect(sections.map(hashOf)).toEqual([
      "#/alpha",
      "#/alpha/inbox",
      "#/alpha/tasks",
      "#/alpha/tree",
      "#/alpha/graph",
      "#/alpha/health",
      "#/alpha/questions",
    ]);
  });

  it("matches sections on the label, tasks on ID and title, proposals on ID and summary, projects on slug and name", () => {
    expect(shape("t")).toEqual([
      ["Sections", ["section:tasks", "section:tree", "section:health", "section:questions"]],
      [
        "Tasks",
        ["task:T-0002", "task:T-0003", "task:T-0005", "task:T-0007", "task:T-0004", "task:T-0006", "task:T-0001", "task:T-0008", "task:T-0009"],
      ],
      ["Inbox", ["proposal:PR-1", "proposal:PR-2"]],
      ["Projects", ["project:beta"]],
      ["Spec tree", ["search", "ref"]],
    ]);
    expect(shape("harbour")).toEqual([
      ["Projects", ["project:beta"]],
      ["Spec tree", ["search", "ref"]],
    ]);
  });

  it("puts a task whose ID folds equal to the trimmed text first, and its group first", () => {
    expect(shape("  t-0004 ")).toEqual([
      ["Tasks", ["task:T-0004"]],
      ["Spec tree", ["search", "ref"]],
    ]);
    expect(shape("t-000")[0]?.[0]).toBe("Tasks");
    const exact = shape("T-0002");
    expect(exact[0]).toEqual(["Tasks", ["task:T-0002"]]);
    expect(exact[1]).toEqual(["Inbox", ["proposal:PR-2"]]);
    expect(shape("pr-2")[0]).toEqual(["Inbox", ["proposal:PR-2"]]);
  });

  it("puts the exact ID first inside its group too, before the earlier matches", () => {
    const entries = [aTaskEntry({ id: "T-10", status: "review" }), aTaskEntry({ id: "T-1", status: "draft" })];
    expect(shape("t-1", { tasks: ready(entries) })[0]).toEqual(["Tasks", ["task:T-1", "task:T-10"]]);
  });

  it("matches a decomposed title with a precomposed query and back (NFC)", () => {
    const composed = "Caf\u00e9 berth";
    const decomposed = "Cafe\u0301 berth";
    const tasks = ready([aTaskEntry({ id: "T-1", title: decomposed }), aTaskEntry({ id: "T-2", title: composed })]);
    expect(shape("caf\u00e9", { tasks })[0]).toEqual(["Tasks", ["task:T-1", "task:T-2"]]);
    expect(shape("CAFE\u0301", { tasks })[0]).toEqual(["Tasks", ["task:T-1", "task:T-2"]]);
  });

  it("shows ten options of a larger group and says how many matched", () => {
    const many = Array.from({ length: 23 }, (_, index) => aTaskEntry({ id: `T-${String(100 + index)}`, status: "draft" }));
    const tasks = paletteGroups(input("T-1", { tasks: ready(many) })).find((group) => group.name === "Tasks");
    expect(tasks?.options).toHaveLength(10);
    expect(tasks === undefined ? "" : groupLabel(tasks)).toBe("Tasks, 10 of 23");
  });

  it("labels a group whose read is on its way and leaves out a failed one", () => {
    const loading = shape("z", { tasks: { state: "loading" }, inbox: { state: "failed" } });
    expect(loading).toEqual([
      ["Tasks, loading", []],
      ["Spec tree", ["search", "ref"]],
    ]);
  });

  it("offers a search with the text as typed and the REF trimmed, unchecked", () => {
    const groups = paletteGroups(input("  MEC-TIDES#RULE-TIDE-WINDOW "));
    const tree = groups.find((group) => group.name === "Spec tree");
    const [search, ref] = tree?.options ?? [];
    expect(search).toEqual({ type: "search", key: "search", query: "  MEC-TIDES#RULE-TIDE-WINDOW " });
    expect(hashOf(ref)).toBe("#/alpha/tree/MEC-TIDES%23RULE-TIDE-WINDOW");
  });

  it("lists the search's hits after the two, each to its ID, else its path", () => {
    const hits: SearchHit[] = [aSearchHit({ id: "R-1" }), aSearchHit({ id: null, path: "docs/spec/a b.md" })];
    const tree = paletteGroups(input("text", { search: ready(hits) })).find((group) => group.name === "Spec tree");
    expect(tree?.options.map(hashOf)).toEqual([null, "#/alpha/tree/text", "#/alpha/tree/R-1", "#/alpha/tree/docs%2Fspec%2Fa%20b.md"]);
    const busy = paletteGroups(input("text", { search: { state: "loading" } })).find((group) => group.name === "Spec tree");
    expect(busy === undefined ? "" : groupLabel(busy)).toBe("Spec tree, loading");
  });

  it("keys a hit by its place in the spec, not its rank: the same hit keeps its key in another answer", () => {
    const first = aSearchHit({ id: "R-1", path: "docs/spec/a.md", line: 3 });
    const second = aSearchHit({ id: "R-2", path: "docs/spec/a.md", line: 9 });
    const keys = (hits: SearchHit[]) =>
      paletteGroups(input("text", { search: ready(hits) }))
        .find((group) => group.name === "Spec tree")
        ?.options.filter((option) => option.type === "hit")
        .map((option) => option.key);
    const before = keys([first, second]) ?? [];
    const after = keys([second, first]) ?? [];
    expect(new Set(before).size).toBe(2);
    expect(after).toEqual([before[1], before[0]]);
  });

  it("jumps tasks, proposals and projects to their screens", () => {
    const groups = paletteGroups(input("T-0003"));
    expect(hashOf(groups[0]?.options[0])).toBe("#/alpha/tasks/T-0003");
    expect(hashOf(paletteGroups(input("PR-1"))[0]?.options[0])).toBe("#/alpha/inbox/PR-1");
    expect(hashOf(paletteGroups(input("beta")).find((group) => group.name === "Projects")?.options[0])).toBe("#/beta");
  });

  it("has only Projects with no project, and nothing to list before the projects are read", () => {
    expect(shape("", { project: null, projects: ready([]) })).toEqual([]);
    expect(shape("a", { project: null, projects: { state: "loading" } })).toEqual([["Projects, loading", []]]);
  });
});

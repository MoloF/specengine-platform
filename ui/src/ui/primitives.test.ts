import { describe, expect, it } from "vitest";

// DS9 of docs/features/design-sweep.md "Acceptance criteria": Inbox, Questions, the Spec tree, the
// node's cards and the shared states build their buttons and empty states from the primitives, never
// from the classes by hand. The patterns are assembled so this file does not match itself.

const SOURCES = import.meta.glob<string>(["/src/{inbox,round,tree,trace}/**/*.tsx", "!/src/**/*.test.tsx", "/src/ui/states.tsx"], {
  query: "?raw",
  import: "default",
  eager: true,
});

const RAW_BUTTON = new RegExp(["className=\"button", "\"button button-"].map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|"));
const OLD_EMPTY = new RegExp(["className=\"empty-state\"", "\"empty-filter\""].join("|"));

function offending(pattern: RegExp): string[] {
  return Object.entries(SOURCES).flatMap(([path, text]) =>
    text
      .split("\n")
      .map((line, index) => ({ line, index }))
      .filter(({ line }) => pattern.test(line))
      .map(({ line, index }) => `${path}:${String(index + 1)}: ${line.trim()}`),
  );
}

describe("the four directories on the primitives (DS9 of design-sweep)", () => {
  it("reads the views of the four directories and the shared states", () => {
    expect(Object.keys(SOURCES)).toEqual(
      expect.arrayContaining(["/src/inbox/ProposalCard.tsx", "/src/inbox/StagedChoice.tsx", "/src/round/RoundScreen.tsx", "/src/tree/NodePane.tsx", "/src/trace/CardsBlock.tsx", "/src/ui/states.tsx"]),
    );
    expect(Object.keys(SOURCES).filter((path) => path.endsWith(".test.tsx"))).toEqual([]);
  });

  it("writes no button class by hand: a Button, or buttonClasses on a link", () => {
    expect(offending(RAW_BUTTON)).toEqual([]);
  });

  it("draws no old empty state: an EmptyState", () => {
    expect(offending(OLD_EMPTY)).toEqual([]);
  });

  it("catches each form it refuses (the detector)", () => {
    const button = ["<button type=\"button\" className=", "\"button\">"].join("");
    const look = ["className={a ? ", "\"button button-danger\" : x}"].join("");
    const empty = ["<div className=", "\"empty-state\">"].join("");
    const filter = ["<div className=", "\"empty-filter\">"].join("");
    expect([button, look, "<Button appearance=\"primary\">", "className=\"link-button\""].map((line) => RAW_BUTTON.test(line))).toEqual([true, true, false, false]);
    expect([empty, filter, "<EmptyState level={2} />", "className=\"empty-state-box\""].map((line) => OLD_EMPTY.test(line))).toEqual([true, true, false, false]);
  });
});

// DB6 of docs/features/design-sweep-b.md "Acceptance criteria": Health, Graph, Roadmap, Agent tasks,
// the palette and the sync line build their buttons and empty states from the primitives; the
// Board's three old empty states are EmptyStates; app.css keeps no rule of the old two. The Board
// gives up only its empty states (D9), so its other raw buttons stand and BoardView.tsx is read for
// the old empty states alone.
const SWEPT = import.meta.glob<string>(
  ["/src/{health,graph,roadmap,tasks,palette}/**/*.tsx", "!/src/**/*.test.tsx", "/src/app/SyncLine.tsx", "/src/board/BoardView.tsx", "/src/board/CardPanel.tsx"],
  { query: "?raw", import: "default", eager: true },
);
const STYLES = import.meta.glob<string>("/src/styles/app.css", { query: "?raw", import: "default", eager: true });
/** "empty-state" or "empty-filter" as a class of its own, never `empty-state-box`. */
const OLD_EMPTY_CLASS = new RegExp(["([\"'`\\s])empty-(?:state|filter)(?=[\"'`\\s])"].join(""));
/** A selector of the old two in a style sheet. */
const OLD_EMPTY_RULE = new RegExp(["\\.empty-(?:state|filter)(?![\\w-])"].join(""));
/** Read for the old empty states only: its other raw buttons are not this slice's (D9). */
const EMPTY_STATES_ONLY = ["/src/board/BoardView.tsx"];

function sweptOffending(pattern: RegExp, skip: readonly string[] = []): string[] {
  return Object.entries(SWEPT)
    .filter(([path]) => !skip.includes(path))
    .flatMap(([path, text]) =>
      text
        .split("\n")
        .map((line, index) => ({ line, index }))
        .filter(({ line }) => pattern.test(line))
        .map(({ line, index }) => `${path}:${String(index + 1)}: ${line.trim()}`),
    );
}

describe("the swept directories on the primitives (DB6 of design-sweep-b)", () => {
  it("reads the views of the five directories, the sync line and the Board's two", () => {
    expect(Object.keys(SWEPT)).toEqual(
      expect.arrayContaining([
        "/src/health/CheckRegion.tsx",
        "/src/health/FindingsRegion.tsx",
        "/src/graph/GraphDetails.tsx",
        "/src/graph/GraphView.tsx",
        "/src/roadmap/RoadmapView.tsx",
        "/src/roadmap/RoadmapLanes.tsx",
        "/src/tasks/TaskDetail.tsx",
        "/src/tasks/TaskList.tsx",
        "/src/palette/Palette.tsx",
        "/src/app/SyncLine.tsx",
        "/src/board/BoardView.tsx",
        "/src/board/CardPanel.tsx",
      ]),
    );
    expect(Object.keys(SWEPT).filter((path) => path.endsWith(".test.tsx"))).toEqual([]);
  });

  it("writes no button class by hand: a Button, or buttonClasses on a link", () => {
    expect(sweptOffending(RAW_BUTTON, EMPTY_STATES_ONLY)).toEqual([]);
  });

  it("draws no old empty state, the Board's three included: an EmptyState", () => {
    expect(sweptOffending(OLD_EMPTY_CLASS)).toEqual([]);
  });

  it("keeps no .empty-state or .empty-filter rule in app.css", () => {
    const css = (STYLES["/src/styles/app.css"] ?? "").replace(/\/\*[\s\S]*?\*\//g, "");
    expect(css.length).toBeGreaterThan(1000);
    expect(css.split("\n").filter((line) => OLD_EMPTY_RULE.test(line))).toEqual([]);
    expect(css).toContain(".empty-state-box");
  });

  it("catches each form it refuses (the detector)", () => {
    const old = ["<div className=", "\"empty-state board-notice\">"].join("");
    const filter = ["<div className=", "\"empty-filter\">"].join("");
    expect([old, filter, "<EmptyState level={2} />", "className=\"empty-state-box\"", "querySelector(\".empty-state-title\")"].map((line) => OLD_EMPTY_CLASS.test(line))).toEqual([true, true, false, false, false]);
    const rule = [".empty", "-filter {"].join("");
    expect([rule, ".empty-state,", ".empty-state-box {", ".empty-state-title {"].map((line) => OLD_EMPTY_RULE.test(line))).toEqual([true, true, false, false]);
  });
});

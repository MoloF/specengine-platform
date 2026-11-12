import { describe, expect, it } from "vitest";
import roadmapSource from "../roadmap/RoadmapView.tsx?raw";
import treeSource from "../tree/TreeView.tsx?raw";
import shellSource from "./Shell.tsx?raw";

// docs/features/handoff-progress.md "Acceptance criteria", S-1: the Roadmap and the Graph leave
// first load. The Shell reaches each view by a dynamic import alone; its types stay `import type`.

/** The Shell's import declarations, one per entry, whitespace folded. */
function importLines(text: string): string[] {
  return Array.from(text.matchAll(/^import\s[^;]*;/gm), (match) => match[0].replace(/\s+/g, " "));
}

describe("the lazy views", () => {
  it.each([
    ["../roadmap/RoadmapView", "RoadmapView"],
    ["../graph/GraphView", "GraphView"],
  ])("reaches %s by lazy(() => import(...)) only, never by a value import", (path, name) => {
    const fromIt = importLines(shellSource).filter((line) => line.endsWith(`"${path}";`));
    // Any static import of the module is a type import alone.
    for (const line of fromIt) {
      expect(line.startsWith("import type ")).toBe(true);
    }
    // The component itself comes through React's lazy and a dynamic import.
    expect(shellSource).toContain(`const ${name} = lazy(() => import("${path}").then((module) => ({ default: module.${name} })));`);
  });

  it("stands each lazy view in a Suspense whose fallback is its skeleton", () => {
    expect(shellSource).toMatch(/<Suspense key=\{route\.project\} fallback=\{<Skeleton label=\{t\("app\.lazy\.roadmap"\)\}/);
    expect(shellSource).toMatch(/<Suspense key=\{route\.project\} fallback=\{<Skeleton label=\{t\("app\.lazy\.graph"\)\}/);
  });
});

// docs/features/delivery-analytics.md "Acceptance criteria", DA-7: the Delivery tab's tables are a
// chunk of their own inside the Roadmap's, reached by a dynamic import alone, as the Chain's canvas is.
describe("the Roadmap's lazy tabs (DA-7 of delivery-analytics)", () => {
  it.each([
    ["./RoadmapChain", "RoadmapChain"],
    ["./RoadmapDelivery", "RoadmapDelivery"],
  ])("RoadmapView reaches %s by lazy(() => import(...)) only, never by a value import", (path, name) => {
    const fromIt = importLines(roadmapSource).filter((line) => line.endsWith(`"${path}";`));
    for (const line of fromIt) {
      expect(line.startsWith("import type ")).toBe(true);
    }
    expect(roadmapSource).toContain(`const ${name} = lazy(() => import("${path}").then((module) => ({ default: module.${name} })));`);
  });

  it("stands the Delivery tab in a Suspense whose fallback is its skeleton", () => {
    expect(roadmapSource).toMatch(/<Suspense fallback=\{<Skeleton label=\{t\("roadmap\.delivery\.loading", \{ project \}\)\} lines=\{4\} \/>\}>\s*<RoadmapDelivery project=\{project\} \/>/);
  });
});

// docs/features/direction-catalog.md "Acceptance criteria", DC-10: a direction's table is a chunk of
// its own, reached from the Spec tree by a dynamic import alone, standing in a Suspense whose fallback
// is its skeleton, inside the node column's pane so the view's heading never waits for it.
describe("the Spec tree's lazy table (DC-10 of direction-catalog)", () => {
  it("TreeView reaches ./DirectionCatalog by lazy(() => import(...)) only, never by a value import", () => {
    const fromIt = importLines(treeSource).filter((line) => line.endsWith('"./DirectionCatalog";'));
    for (const line of fromIt) {
      expect(line.startsWith("import type ")).toBe(true);
    }
    expect(treeSource).toContain('const DirectionCatalog = lazy(() => import("./DirectionCatalog").then((module) => ({ default: module.DirectionCatalog })));');
    // M: `import { DirectionCatalog } from "./DirectionCatalog";` beside the lazy one.
    expect(treeSource).not.toMatch(/^import \{[^}]*\bDirectionCatalog\b[^}]*\} from "\.\/DirectionCatalog";$/m);
  });

  it("stands the table in a Suspense whose fallback is its skeleton, inside NoNodePane", () => {
    expect(treeSource).toMatch(
      /<NoNodePane>\s*<Suspense fallback=\{<Skeleton label=\{t\("tree\.catalog\.loading", \{ direction \}\)\} lines=\{6\} \/>\}>\s*<DirectionCatalog key=\{JSON\.stringify\(listing\)\} project=\{project\} kinds=\{listing\}/,
    );
  });
});

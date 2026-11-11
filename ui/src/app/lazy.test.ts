import { describe, expect, it } from "vitest";
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

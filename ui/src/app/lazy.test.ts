import { describe, expect, it } from "vitest";
import cardItemsSource from "../board/CardItems.tsx?raw";
import graphHomeSource from "../graph/GraphHome.tsx?raw";
import graphViewSource from "../graph/GraphView.tsx?raw";
import pairSlotSource from "../inbox/PairSlot.tsx?raw";
import proposalCardSource from "../inbox/ProposalCard.tsx?raw";
import homeSource from "../overview/HomeView.tsx?raw";
import roundItemSource from "../round/RoundItemCard.tsx?raw";
import deliverySource from "../roadmap/RoadmapDelivery.tsx?raw";
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

// docs/features/delivery-timeline.md "Acceptance criteria", DT10: the Delivery tab's timeline is a
// chunk of its own inside the tab's, reached by a dynamic import alone, standing in a Suspense whose
// fallback is its skeleton, mounted only when Timeline is chosen.
describe("the Delivery tab's lazy timeline (DT10 of delivery-timeline)", () => {
  it("RoadmapDelivery reaches ./RoadmapTimeline by lazy(() => import(...)) only, never by a value import (M: a static import of RoadmapTimeline)", () => {
    const fromIt = importLines(deliverySource).filter((line) => line.endsWith('"./RoadmapTimeline";'));
    for (const line of fromIt) {
      expect(line.startsWith("import type ")).toBe(true);
    }
    expect(deliverySource).toContain('const RoadmapTimeline = lazy(() => import("./RoadmapTimeline").then((module) => ({ default: module.RoadmapTimeline })));');
    expect(deliverySource).not.toMatch(/^import \{[^}]*\bRoadmapTimeline\b[^}]*\} from "\.\/RoadmapTimeline";$/m);
    // Nor any static path to its day arithmetic: it rides in the timeline's chunk.
    expect(importLines(deliverySource).filter((line) => line.endsWith('"./timeline";'))).toEqual([]);
  });

  it("stands the timeline in a Suspense whose fallback is its skeleton, mounted when Timeline is chosen", () => {
    expect(deliverySource).toMatch(
      /view === "tables" \? \(\s*<DeliveryTables project=\{project\} asOf=\{asOf\} \/>\s*\) : \(\s*<Suspense fallback=\{<Skeleton label=\{t\("roadmap\.delivery\.loading", \{ project \}\)\} lines=\{6\} \/>\}>\s*<RoadmapTimeline project=\{project\} asOf=\{asOf\} \/>/,
    );
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

// docs/features/department-overview.md "Acceptance criteria", DO7: the Overview's blocks are a chunk
// of their own, reached from HomeView by a dynamic import alone, standing in a Suspense whose fallback
// is their skeleton under the view's heading, so first load never holds them.
describe("the Overview's lazy blocks (DO7 of department-overview)", () => {
  it("HomeView reaches ./DepartmentOverview by lazy(() => import(...)) only, never by a value import", () => {
    const fromIt = importLines(homeSource).filter((line) => line.endsWith('"./DepartmentOverview";'));
    for (const line of fromIt) {
      expect(line.startsWith("import type ")).toBe(true);
    }
    expect(homeSource).toContain('const DepartmentOverview = lazy(() => import("./DepartmentOverview").then((module) => ({ default: module.DepartmentOverview })));');
    // M: `import { DepartmentOverview } from "./DepartmentOverview";` beside the lazy one.
    expect(homeSource).not.toMatch(/^import \{[^}]*\bDepartmentOverview\b[^}]*\} from "\.\/DepartmentOverview";$/m);
    // The Shell reaches the blocks only through HomeView.
    expect(importLines(shellSource).filter((line) => line.includes("DepartmentOverview"))).toEqual([]);
  });

  it("stands the blocks in a Suspense whose fallback is their skeleton, after the view's heading", () => {
    expect(homeSource).toMatch(
      /<Suspense fallback=\{<Skeleton label=\{t\("home\.department\.loading", \{ project \}\)\} lines=\{6\} \/>\}>\s*<DepartmentOverview project=\{project\} workspace=\{workspace\} module=\{module\} workspaces=\{workspaces\} \/>/,
    );
    expect(homeSource.indexOf("<Suspense")).toBeGreaterThan(homeSource.indexOf('t("home.title")'));
  });
});

// docs/features/collision-pairs.md "Acceptance criteria", PP10: a collision pair's view is a chunk of
// its own, reached by a dynamic import alone in src/inbox/PairSlot.tsx, which stands it in a Suspense
// whose fallback is its skeleton; the Inbox's card, a Questions item and a task's Answer… reach it
// only through that slot, never by a value import of the view.
describe("the collision pair's lazy view (PP10 of collision-pairs)", () => {
  it("PairSlot reaches ./PairView by lazy(() => import(...)) only, never by a value import", () => {
    const fromIt = importLines(pairSlotSource).filter((line) => line.endsWith('"./PairView";'));
    for (const line of fromIt) {
      expect(line.startsWith("import type ")).toBe(true);
    }
    expect(pairSlotSource).toContain('const PairView = lazy(() => import("./PairView"));');
    expect(pairSlotSource).toMatch(/<Suspense fallback=\{<Skeleton label=\{t\("inbox\.pair\.label"\)\} lines=\{4\} \/>\}>\s*<PairView pair=\{pair\} evidence=\{evidence\} onOpen=\{openDocFrom\} \/>/);
  });

  it.each([
    ["ProposalCard.tsx", proposalCardSource, '"./PairSlot";'],
    ["RoundItemCard.tsx", roundItemSource, '"../inbox/PairSlot";'],
    ["CardItems.tsx", cardItemsSource, '"../inbox/PairSlot";'],
  ])("%s imports no PairView and stands the pair through PairSlot alone", (_name, text, slot) => {
    // M: `import PairView from "./PairView";` in ProposalCard.tsx.
    expect(importLines(text).filter((line) => line.includes("PairView"))).toEqual([]);
    expect(text).not.toContain('import("./PairView")');
    expect(text).not.toContain('import("../inbox/PairView")');
    expect(importLines(text).filter((line) => line.endsWith(slot))).toEqual([`import { PairSlot } from ${slot}`]);
    expect(text).toMatch(/<PairSlot pair=\{\w+\.pair\} evidence=\{\w+\.evidence\} \/>/);
  });
});

// docs/features/scope-rings.md "Acceptance criteria", SR8: the Rings view is a chunk of its own,
// reached from the Graph's home by a dynamic import alone, standing in a Suspense whose fallback is
// its skeleton; the home is the Graph view's, itself out of first load, and nothing else reaches it.
describe("the Graph home's lazy Rings view (SR8 of scope-rings)", () => {
  it("GraphHome reaches ../rings/RingsView by lazy(() => import(...)) only, never by a value import", () => {
    const fromIt = importLines(graphHomeSource).filter((line) => line.endsWith('"../rings/RingsView";'));
    for (const line of fromIt) {
      expect(line.startsWith("import type ")).toBe(true);
    }
    expect(graphHomeSource).toContain('const RingsView = lazy(() => import("../rings/RingsView").then((module) => ({ default: module.RingsView })));');
    // M: `import { RingsView } from "../rings/RingsView";` beside the lazy one.
    expect(graphHomeSource).not.toMatch(/^import \{[^}]*\bRingsView\b[^}]*\} from "\.\.\/rings\/RingsView";$/m);
    // The Graph view mounts the home; neither it nor the Shell reaches the rings by any import.
    expect(importLines(graphViewSource).filter((line) => line.endsWith('"./GraphHome";'))).toEqual(['import { GraphHome } from "./GraphHome";']);
    for (const text of [graphViewSource, shellSource]) {
      expect(importLines(text).filter((line) => line.includes("rings/"))).toEqual([]);
      expect(text).not.toContain('import("../rings/');
    }
  });

  it("stands the view in a Suspense whose fallback is its skeleton", () => {
    expect(graphHomeSource).toMatch(/<Suspense fallback=\{<Skeleton label=\{t\("rings\.loading"\)\} lines=\{6\} \/>\}>\s*<RingsView project=\{project\} \/>/);
  });
});

// docs/features/corpus-map.md "Acceptance criteria", CM11: the Map view and its layout engine are a
// chunk of their own, reached from the Graph's home by a dynamic import alone, standing in a Suspense
// whose fallback is its skeleton; neither the Graph view nor the Shell reaches src/map/ by any import.
// M: a static import of MapView.
describe("the Graph home's lazy Map view (CM11 of corpus-map)", () => {
  it("GraphHome reaches ../map/MapView by lazy(() => import(...)) only, never by a value import", () => {
    const fromIt = importLines(graphHomeSource).filter((line) => line.includes('"../map/'));
    for (const line of fromIt) {
      expect(line.startsWith("import type ")).toBe(true);
    }
    expect(graphHomeSource).toContain('const MapView = lazy(() => import("../map/MapView").then((module) => ({ default: module.MapView })));');
    expect(graphHomeSource).not.toMatch(/^import \{[^}]*\bMapView\b[^}]*\} from "\.\.\/map\/MapView";$/m);
    for (const text of [graphViewSource, shellSource]) {
      expect(importLines(text).filter((line) => line.includes("/map/"))).toEqual([]);
      expect(text).not.toContain('import("../map/');
    }
  });

  it("stands the view in a Suspense whose fallback is its skeleton, the Map first", () => {
    expect(graphHomeSource).toMatch(/<Suspense fallback=\{<Skeleton label=\{t\("map\.loading"\)\} lines=\{6\} \/>\}>\s*<MapView project=\{project\} \/>/);
    expect(graphHomeSource).toContain('const VIEWS: readonly GraphHomeView[] = ["map", "rings"];');
  });
});

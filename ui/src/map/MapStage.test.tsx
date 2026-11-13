import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { createElement, type ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MapDocument } from "../api/types";
import appCss from "../styles/app.css?raw";
import { aDirection, aMap, aMapEdge, aMapNode, aWorkspace, aWorkspaces } from "../test/builders";
import { pathsOf, pensPerCanvas, visibleAndBase, type RecordingPen } from "../test/pen";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { sceneOf, type Scene } from "./filters";
import { layoutAll } from "./layout";
import { BASE_SETTLE_MS } from "./MapCanvas";
import { GHOST_CARD_SOURCES } from "./MapStage";

// docs/features/corpus-map.md "Acceptance criteria", CM12 (a hover's cost) on the screen, over the
// stub client: a hover is the stage's own state, so it renders neither the List, the legend nor the
// controls; the List is mounted on its own tab alone, never while the Picture shows; the hover card
// is an overlay in the canvas's box, so the legend's place never changes and nothing in the page's
// flow moves. Each of the three is counted through a wrapper of the real component.
// M: hover state back in MapView; list rows rendered while hidden.

const renders = vi.hoisted(() => ({ list: 0, legend: 0, controls: 0 }));

vi.mock("./MapList", async (importOriginal) => {
  const real = await importOriginal<typeof import("./MapList")>();
  return {
    MapList: (props: ComponentProps<typeof real.MapList>) => {
      renders.list += 1;
      return createElement(real.MapList, props);
    },
  };
});

vi.mock("./MapLegend", async (importOriginal) => {
  const real = await importOriginal<typeof import("./MapLegend")>();
  return {
    MapLegend: (props: ComponentProps<typeof real.MapLegend>) => {
      renders.legend += 1;
      return createElement(real.MapLegend, props);
    },
  };
});

vi.mock("./MapControls", async (importOriginal) => {
  const real = await importOriginal<typeof import("./MapControls")>();
  return {
    MapControls: (props: ComponentProps<typeof real.MapControls>) => {
      renders.controls += 1;
      return createElement(real.MapControls, props);
    },
  };
});

const ANSWER: MapDocument = aMap(
  [
    aMapNode({ name: "A-1", kind: "scene", title: "Arrival", path: "docs/a-1.md", used_by: 2 }),
    aMapNode({ name: "A-2", kind: "scene", title: "Arrival at night", path: "docs/a-2.md", used_by: 1 }),
    aMapNode({ name: "C-1", kind: "cue", title: "Bell cue", path: "docs/c-1.md" }),
    aMapNode({ name: "X-1", kind: "prop", title: "Lantern", path: "docs/x-1.md", orphan: true }),
  ],
  [aMapEdge(1, 0), aMapEdge(2, 0)],
);

let pens: Map<HTMLCanvasElement, RecordingPen>;

beforeEach(() => {
  renders.list = 0;
  renders.legend = 0;
  renders.controls = 0;
  const made = pensPerCanvas();
  pens = made.pens;
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(made.getContext);
});

/** The world transform of the base's latest drawing. */
function transform(): { zoom: number; e: number; f: number } {
  const calls = visibleAndBase(pens).base?.calls ?? [];
  const start = calls.map((call) => call.op).lastIndexOf("fillRect");
  const set = calls.slice(Math.max(0, start)).filter((call) => call.op === "setTransform")[0];
  return { zoom: Number(set?.args[0] ?? 0), e: Number(set?.args[4] ?? 0), f: Number(set?.args[5] ?? 0) };
}

/** A point's place on the screen: the layout run again (one answer, one picture) through the drawn transform. */
function screenAt(served: MapDocument, pick: (scene: Scene) => number): { clientX: number; clientY: number } {
  const scene = sceneOf(served, false);
  const positions = layoutAll(scene.points.length, Array.from(scene.lineFrom, (from, at) => ({ source: from, target: scene.lineTo[at] ?? from })));
  const point = pick(scene);
  const { zoom, e, f } = transform();
  return { clientX: (positions[point * 2] ?? 0) * zoom + e, clientY: (positions[point * 2 + 1] ?? 0) * zoom + f };
}

/** A node's place on the screen. */
function screenOf(name: string, served: MapDocument = ANSWER): { clientX: number; clientY: number } {
  return screenAt(served, (scene) => scene.pointOf[served.nodes.findIndex((node) => node.name === name)] ?? -1);
}

function mapClient(served: MapDocument = ANSWER) {
  const stub = stubClient();
  stub.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "all" })], [aDirection({ name: "scenes", title: "Scenes", kinds: ["scene"] })]));
  stub.getMap.mockResolvedValue(served);
  return stub;
}

async function onMap(stub = mapClient()): Promise<HTMLElement> {
  renderApp(stub, "#/alpha/graph");
  const region = await screen.findByRole("region", { name: "Map picture" });
  await waitFor(() => {
    expect(pathsOf(visibleAndBase(pens).base?.calls ?? []).some((path) => path.op === "fill")).toBe(true);
  });
  return region;
}

/** The hover card, when one shows. */
function hoverCard(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".map-overlay.is-passive");
}

describe("a hover renders the stage alone (CM12)", () => {
  it("renders neither the List, the legend nor the controls, entering, moving over and leaving a node (M: hover state back in MapView)", async () => {
    const region = await onMap();
    const before = { ...renders };
    expect(before.legend).toBeGreaterThan(0);
    expect(before.controls).toBeGreaterThan(0);
    const at = screenOf("A-1");
    fireEvent.pointerMove(region, { ...at, pointerId: 1 });
    await waitFor(() => {
      expect(hoverCard()?.querySelector(".map-card-title")?.textContent).toBe("Arrival");
    });
    fireEvent.pointerMove(region, { clientX: at.clientX + 1, clientY: at.clientY, pointerId: 1 });
    fireEvent.pointerMove(region, { clientX: at.clientX + 50, clientY: at.clientY + 50, pointerId: 1 });
    await waitFor(() => {
      expect(hoverCard()).toBeNull();
    });
    expect(renders).toEqual(before);
  });

  it("mounts no List row while the Picture shows, and the List on its tab (M: list rows rendered while hidden)", async () => {
    const region = await onMap();
    expect(document.querySelectorAll(".map-table tbody tr")).toHaveLength(0);
    fireEvent.pointerMove(region, { ...screenOf("A-2"), pointerId: 1 });
    await waitFor(() => {
      expect(hoverCard()).not.toBeNull();
    });
    expect(renders.list).toBe(0);
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(document.querySelectorAll(".map-table tbody tr")).toHaveLength(4);
    expect(renders.list).toBeGreaterThan(0);
  });

  it("shows the hover card over the canvas, out of the page's flow: the legend's column and place never change", async () => {
    const region = await onMap();
    const side = document.querySelector<HTMLElement>(".map-side");
    const column = () => Array.from(side?.children ?? [], (child) => child.className);
    const legend = within(side as HTMLElement).getByRole("heading", { level: 3, name: "Legend" }).closest("section");
    expect(column()).toEqual(["map-legend"]);
    fireEvent.pointerMove(region, { ...screenOf("C-1"), pointerId: 1 });
    await waitFor(() => {
      expect(hoverCard()).not.toBeNull();
    });
    // The card stands in the canvas's box, next to the canvas, never in the side column.
    expect(hoverCard()?.parentElement?.className).toBe("map-canvas-wrap");
    expect(hoverCard()?.parentElement).toBe(region.parentElement);
    expect(column()).toEqual(["map-legend"]);
    expect(side?.firstElementChild).toBe(legend);
    // Its rule takes it out of the flow, inside a box the canvas sizes, its own size capped.
    const rule = (selector: string) => new RegExp(`(^|\\n)${selector.replaceAll(".", "\\.")}\\s*\\{([^}]*)\\}`).exec(appCss)?.[2] ?? "";
    expect(rule(".map-canvas-wrap")).toMatch(/position:\s*relative/);
    expect(rule(".map-overlay")).toMatch(/position:\s*absolute/);
    expect(rule(".map-overlay")).toMatch(/max-height:\s*calc\(100% - /);
    expect(rule(".map-overlay")).toMatch(/overflow:\s*auto/);
    expect(rule(".map-overlay.is-passive")).toMatch(/pointer-events:\s*none/);
  });
});

/** The lit node's pane: the overlay that takes the pointer. */
function pane(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".map-overlay:not(.is-passive)");
}

/** Lights a node by the search, its words typed and Enter, then waits for the base drawn for the view centred on it. */
async function light(text: string, title: string) {
  const search = screen.getByRole("searchbox", { name: "Find on the map" });
  fireEvent.change(search, { target: { value: text } });
  fireEvent.keyDown(search, { key: "Enter" });
  await waitFor(() => {
    expect(pane()?.querySelector(".map-card-title")?.textContent).toBe(title);
  });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, BASE_SETTLE_MS + 50));
  });
}

describe("the lit node's pane and the hover card", () => {
  it("keeps the pane, and focus on its act, while a hover card shows another node beside it; none for the lit node itself (M: the hover card in the pane's place)", async () => {
    const region = await onMap();
    await light("night", "Arrival at night");
    const open = within(pane() as HTMLElement).getByRole("button", { name: "Open document" });
    open.focus();
    fireEvent.pointerMove(region, { ...screenOf("C-1"), pointerId: 1 });
    await waitFor(() => {
      expect(hoverCard()?.querySelector(".map-card-title")?.textContent).toBe("Bell cue");
    });
    expect(pane()?.querySelector(".map-card-title")?.textContent).toBe("Arrival at night");
    expect(document.activeElement).toBe(open);
    // Below the pane, never over it.
    expect(hoverCard()?.classList.contains("is-below")).toBe(true);
    fireEvent.pointerMove(region, { ...screenOf("A-2"), pointerId: 1 });
    await waitFor(() => {
      expect(hoverCard()).toBeNull();
    });
    expect(document.activeElement).toBe(open);
  });

  it("sends focus back to the canvas's region when a new answer takes the lit node and its pane with focus in it (M: focus left to the page)", async () => {
    const without = aMap(
      ANSWER.nodes.map((node) => (node.name === "A-2" ? { ...node, name: "A-3", title: "Arrival by day" } : node)),
      ANSWER.edges,
    );
    const stub = mapClient();
    stub.getMap.mockImplementation((_project, options) => Promise.resolve(options.archive ? without : ANSWER));
    const region = await onMap(stub);
    await light("night", "Arrival at night");
    within(pane() as HTMLElement).getByRole("button", { name: "Show in Spec tree" }).focus();
    // A test's click moves no focus: it stays in the pane while the new answer comes.
    fireEvent.click(within(document.querySelector(".map-controls") as HTMLElement).getByRole("checkbox", { name: "Include archive" }));
    await waitFor(() => {
      expect(pane()).toBeNull();
    });
    expect(document.activeElement).toBe(region);
  });

  it("leaves focus where it is when the pane goes with focus elsewhere", async () => {
    const region = await onMap();
    await light("night", "Arrival at night");
    region.focus();
    fireEvent.keyDown(region, { key: "Escape" });
    await waitFor(() => {
      expect(pane()).toBeNull();
    });
    expect(document.activeElement).toBe(region);
    const search = screen.getByRole("searchbox", { name: "Find on the map" });
    await light("night", "Arrival at night");
    search.focus();
    fireEvent.keyDown(region, { key: "Escape" });
    await waitFor(() => {
      expect(pane()).toBeNull();
    });
    expect(document.activeElement).toBe(search);
  });
});

describe("a ghost's hover card", () => {
  const REASON = "`GONE-1` resolves to no ID and no alias";
  const GHOSTED = aMap(ANSWER.nodes, ANSWER.edges, { ghosts: [{ written: "GONE-1", reason: REASON, sources: [1, 2], places: 2 }] });

  it("tells what is written as text, the daemon's reason in English, its places and its sources (M: no card for a ghost)", async () => {
    const region = await onMap(mapClient(GHOSTED));
    fireEvent.click(screen.getByRole("checkbox", { name: "Unresolved targets" }));
    fireEvent.pointerMove(region, { ...screenAt(GHOSTED, (scene) => scene.points.findIndex((point) => point.ghost === 0)), pointerId: 1 });
    await waitFor(() => {
      expect(hoverCard()).not.toBeNull();
    });
    const card = hoverCard() as HTMLElement;
    expect(card.querySelector(".map-card-title")?.textContent).toBe("GONE-1");
    expect(card.querySelector(".map-card-title")?.querySelector("a")).toBeNull();
    expect(card.querySelector("[lang='en']")?.textContent).toBe(REASON);
    expect(within(card).getByText("Unresolved target")).toBeTruthy();
    expect(within(card).getByText("2 places")).toBeTruthy();
    expect(within(card).getByText("Linked from")).toBeTruthy();
    expect(Array.from(card.querySelectorAll(".map-card-sources li"), (item) => item.textContent)).toEqual(["Arrival at night", "Bell cue"]);
  });

  it("names its first sources and counts the rest", async () => {
    const nodes = Array.from({ length: 12 }, (_, at) => aMapNode({ name: `S-${String(at)}`, title: `Source ${String(at)}` }));
    const served = aMap(nodes, [], { ghosts: [{ written: "GONE-2", reason: null, sources: nodes.map((_, at) => at), places: 12 }] });
    const region = await onMap(mapClient(served));
    fireEvent.click(screen.getByRole("checkbox", { name: "Unresolved targets" }));
    fireEvent.pointerMove(region, { ...screenAt(served, (scene) => scene.points.findIndex((point) => point.ghost === 0)), pointerId: 1 });
    await waitFor(() => {
      expect(hoverCard()).not.toBeNull();
    });
    const items = Array.from(hoverCard()?.querySelectorAll(".map-card-sources li") ?? [], (item) => item.textContent);
    expect(items).toHaveLength(GHOST_CARD_SOURCES + 1);
    expect(items.at(-1)).toBe(`and ${String(12 - GHOST_CARD_SOURCES)} more`);
    expect(hoverCard()?.querySelector("[lang='en']")).toBeNull();
  });
});

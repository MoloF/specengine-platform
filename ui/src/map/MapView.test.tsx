import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { MapDocument } from "../api/types";
import { docOf } from "../app/routes";
import { en } from "../i18n/en";
import { aDirection, aMap, aMapEdge, aMapNode, aWorkspace, aWorkspaces } from "../test/builders";
import { pathsOf, pensPerCanvas, visibleAndBase, type RecordingPen } from "../test/pen";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { ruT } from "../test/text";
import { DEFAULT_FILTERS, keptOf, sceneOf } from "./filters";
import { layoutAll } from "./layout";
import { ZOOM_STEP } from "./MapCanvas";

// docs/features/corpus-map.md "Acceptance criteria", CM7 and CM8 on the screen, over the stub client
// at `#/alpha/graph`, the canvas's 2D context a recording pen: a press of 4 px opens the node's
// document on the route, one of 6 px opens nothing; the zoom keys act on the canvas, never in the
// search field; a chip hides and never lays out again, Sections does; the Direction select presses
// its chips; the List holds the kept nodes with counts and marks; the search's ArrowDown and Enter
// light the second match and say it, Enter again opens it; Broken only with no ghost says so; a new
// answer that drops the lit node clears it, says so, and keeps the zoom; the count line, the cap's
// line and the states. M: a filter that lays out again; the List ignoring the chips.

/** Four documents and a section of the first: two directions' kinds, an unlisted one, an orphan. */
function answer(fields: Partial<MapDocument> = {}): MapDocument {
  return aMap(
    [
      aMapNode({ name: "A-1", kind: "scene", title: "Arrival", path: "docs/a-1.md", used_by: 2 }),
      aMapNode({ name: "A-2", kind: "scene", title: "Arrival at night", path: "docs/a-2.md", used_by: 1 }),
      aMapNode({ name: "C-1", kind: "cue", title: "Bell cue", path: "docs/c-1.md", mentioned_by: 0 }),
      aMapNode({ name: "X-1", kind: "prop", title: "Lantern", path: "docs/x-1.md", orphan: true }),
      aMapNode({ name: "SEC-1", kind: "scene", title: "Arrival detail", path: "docs/a-1.md", line: 9, of: 0, orphan: null }),
    ],
    [aMapEdge(1, 0), aMapEdge(2, 0), aMapEdge(2, 1, { type: "mentions", weak: true }), aMapEdge(4, 2)],
    fields,
  );
}

const DIRECTIONS = aWorkspaces([aWorkspace({ name: "all" })], [aDirection({ name: "scenes", title: "Scenes", kinds: ["scene"] }), aDirection({ name: "cues", title: "Cues", kinds: ["cue"] })]);

let pens: Map<HTMLCanvasElement, RecordingPen>;

beforeEach(() => {
  const made = pensPerCanvas();
  pens = made.pens;
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(made.getContext);
});

function client(served: MapDocument = answer()) {
  const stub = stubClient();
  stub.getWorkspaces.mockResolvedValue(DIRECTIONS);
  stub.getMap.mockResolvedValue(served);
  return stub;
}

/** The calls of the offscreen base's latest drawing, every kept node and line: from its surface fill on. */
function lastDrawing() {
  const calls = visibleAndBase(pens).base?.calls ?? [];
  const start = calls.map((call) => call.op).lastIndexOf("fillRect");
  return start < 0 ? [] : calls.slice(start);
}

/** The latest drawing's world transform: the zoom and the offset. */
function transform(): { zoom: number; e: number; f: number } {
  // The surface is filled under the device's transform; the world's follows it.
  const set = lastDrawing().filter((call) => call.op === "setTransform")[0];
  return { zoom: Number(set?.args[0] ?? 0), e: Number(set?.args[4] ?? 0), f: Number(set?.args[5] ?? 0) };
}

/** How many discs the latest drawing filled. */
function filledDiscs(): number {
  return pathsOf(lastDrawing())
    .filter((path) => path.op === "fill")
    .reduce((sum, path) => sum + path.arcs.length, 0);
}

/** The map open at `#/alpha/graph`, drawn once. */
async function onMap(stub = client()) {
  renderApp(stub, "#/alpha/graph");
  const region = await screen.findByRole("region", { name: "Map picture" });
  await waitFor(() => {
    expect(filledDiscs()).toBeGreaterThan(0);
  });
  return { stub, region };
}

/** A node's place on the screen: the layout run again (one answer, one picture) through the drawn transform. */
function screenOf(served: MapDocument, name: string): { clientX: number; clientY: number } {
  const scene = sceneOf(served, false);
  const positions = layoutAll(scene.points.length, Array.from(scene.lineFrom, (from, at) => ({ source: from, target: scene.lineTo[at] ?? from })));
  const point = scene.pointOf[served.nodes.findIndex((node) => node.name === name)] ?? -1;
  const { zoom, e, f } = transform();
  return { clientX: (positions[point * 2] ?? 0) * zoom + e, clientY: (positions[point * 2 + 1] ?? 0) * zoom + f };
}

function polite(): string {
  return document.querySelector('[aria-live="polite"]')?.textContent ?? "";
}

function chip(name: RegExp): HTMLElement {
  return within(screen.getByRole("group", { name: "Kinds" })).getByRole("button", { name });
}

describe("the Map on the screen (CM7, CM8)", () => {
  it("is the home's first view: the View group's Map pressed, the corpus map under it, the count line its canvas's description", async () => {
    const { region } = await onMap();
    const views = screen.getByRole("group", { name: "View" });
    expect(within(views).getAllByRole("button").map((button) => [button.textContent, button.getAttribute("aria-pressed")])).toEqual([
      ["Map", "true"],
      ["Rings", "false"],
    ]);
    expect(screen.getByRole("heading", { level: 2, name: "Corpus map" })).toBeTruthy();
    const count = document.getElementById(region.getAttribute("aria-describedby") ?? "");
    expect(count?.textContent).toBe("4 documents, 4 links, 1 orphan");
    expect(screen.queryByText(/most used of/)).toBeNull();
  });

  it("opens a node's document on a press of 4 px, and nothing on a drag of 6 px", async () => {
    const served = answer();
    const { region } = await onMap(client(served));
    const at = screenOf(served, "A-2");
    fireEvent.pointerDown(region, { ...at, button: 0, pointerId: 1 });
    fireEvent.pointerMove(region, { clientX: at.clientX + 6, clientY: at.clientY, pointerId: 1 });
    fireEvent.pointerUp(region, { clientX: at.clientX + 6, clientY: at.clientY, pointerId: 1 });
    expect(docOf(window.location.hash)).toBeNull();
    // The drag panned: the next drawing is 6 px over.
    await waitFor(() => {
      expect(screenOf(served, "A-2").clientX).toBeCloseTo(at.clientX + 6, 6);
    });
    const moved = screenOf(served, "A-2");
    fireEvent.pointerDown(region, { ...moved, button: 0, pointerId: 1 });
    fireEvent.pointerMove(region, { clientX: moved.clientX + 4, clientY: moved.clientY, pointerId: 1 });
    fireEvent.pointerUp(region, { clientX: moved.clientX + 4, clientY: moved.clientY, pointerId: 1 });
    expect(docOf(window.location.hash)).toBe("A-2");
  });

  it("zooms by key on the focused canvas, never from the search field", async () => {
    const { region } = await onMap();
    const before = transform().zoom;
    expect(before).toBeGreaterThan(0);
    const search = screen.getByRole("searchbox", { name: "Find on the map" });
    search.focus();
    fireEvent.keyDown(search, { key: "-" });
    fireEvent.keyDown(search, { key: "ArrowLeft" });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 40));
    });
    expect(transform().zoom).toBe(before);
    region.focus();
    fireEvent.keyDown(region, { key: "-" });
    await waitFor(() => {
      expect(transform().zoom).toBeCloseTo(before / ZOOM_STEP, 9);
    });
  });

  it("hides a kind's nodes by its chip and never lays out again; Sections does (M: a filter that lays out again)", async () => {
    await onMap();
    expect(filledDiscs()).toBe(4);
    const lantern = screenOf(answer(), "X-1");
    fireEvent.click(chip(/^scene/));
    // No layout starts: no progress line, the stage never busy.
    expect(screen.queryByText(/^Laying out/)).toBeNull();
    expect(document.querySelector(".map-stage")?.getAttribute("aria-busy")).toBe("false");
    expect(chip(/^scene/).getAttribute("aria-pressed")).toBe("false");
    await waitFor(() => {
      expect(filledDiscs()).toBe(2);
    });
    expect(screenOf(answer(), "X-1")).toEqual(lantern);
    fireEvent.click(chip(/^scene/));
    fireEvent.click(screen.getByRole("checkbox", { name: "Sections" }));
    expect(screen.getByText("Laying out 5 nodes")).toBeTruthy();
    await waitFor(() => {
      expect(filledDiscs()).toBe(5);
    });
    expect(screen.queryByText(/^Laying out/)).toBeNull();
  });

  it("presses only a direction's chips from the Direction select, and every chip from All", async () => {
    await onMap();
    const chips = () => within(screen.getByRole("group", { name: "Kinds" })).getAllByRole("button").map((button) => [button.textContent.replace(/\s+/g, " ").trim(), button.getAttribute("aria-pressed")]);
    expect(chips()).toEqual([
      ["scene 2", "true"],
      ["cue 1", "true"],
      ["prop 1", "true"],
    ]);
    fireEvent.change(screen.getByRole("combobox", { name: "Direction" }), { target: { value: "cues" } });
    expect(chips().map(([, pressed]) => pressed)).toEqual(["false", "true", "false"]);
    fireEvent.change(screen.getByRole("combobox", { name: "Direction" }), { target: { value: "" } });
    expect(chips().map(([, pressed]) => pressed)).toEqual(["true", "true", "true"]);
  });

  it("lists exactly the kept nodes with their counts, links and marks (M: the List ignoring the chips)", async () => {
    await onMap();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const rows = () =>
      Array.from(document.querySelectorAll(".map-table tbody tr"), (row) => Array.from(row.querySelectorAll("td"), (cell) => cell.textContent.replace(/\s+/g, " ").trim()));
    expect(rows()).toEqual([
      // SEC-1's link to Bell cue folds onto its document: A-1 out 1. Mentions are off at first.
      ["ArrivalA-1", "Scenesscene", "2", "0", "1 \u00b7 2", ""],
      ["Arrival at nightA-2", "Scenesscene", "1", "0", "1 \u00b7 0", ""],
      ["Bell cueC-1", "Cuescue", "0", "0", "1 \u00b7 1", ""],
      ["LanternX-1", "prop", "0", "0", "0 \u00b7 0", "Orphan"],
    ]);
    // Mentions on: Bell cue's mention of Arrival at night counts.
    fireEvent.click(screen.getByRole("checkbox", { name: "Mentions" }));
    expect(rows().map((row) => row[4])).toEqual(["1 \u00b7 2", "1 \u00b7 1", "2 \u00b7 1", "0 \u00b7 0"]);
    fireEvent.click(chip(/^scene/));
    expect(rows().map((row) => row[0])).toEqual(["Bell cueC-1", "LanternX-1"]);
    // Bell cue's links to the released scenes are not counted.
    expect(rows()[0]?.[4]).toBe("0 \u00b7 0");
    const link = within(document.querySelector(".map-table") as HTMLElement).getByRole("link", { name: "Lantern" });
    expect(link.getAttribute("href")).toBe("#/alpha/tree/X-1/page");
  });

  it("steps to the second match by ArrowDown, lights and says it by Enter, opens it by Enter again", async () => {
    await onMap();
    const search = screen.getByRole("searchbox", { name: "Find on the map" });
    fireEvent.change(search, { target: { value: "ARRIVAL" } });
    await waitFor(() => {
      expect(polite()).toBe("2 matches");
    });
    fireEvent.keyDown(search, { key: "ArrowDown" });
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() => {
      expect(polite()).toBe("Arrival at night, scene, Used by 1 \u00b7 Mentioned by 0");
    });
    const card = document.querySelector(".map-card");
    expect(card?.querySelector(".map-card-title")?.textContent).toBe("Arrival at night");
    expect(within(card as HTMLElement).getAllByRole("button").map((button) => button.textContent)).toEqual(["Open document", "Show in graph", "Show in Spec tree"]);
    expect(docOf(window.location.hash)).toBeNull();
    fireEvent.keyDown(search, { key: "Enter" });
    expect(docOf(window.location.hash)).toBe("A-2");
  });

  it("says there is no unresolved link under Broken only with no ghost", async () => {
    await onMap();
    fireEvent.click(screen.getByRole("checkbox", { name: "Broken only" }));
    expect(screen.getByRole("heading", { level: 3, name: "No unresolved link" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Map picture" })).toBeNull();
  });

  it("lays a new answer out, keeping the zoom, and clears and announces a lit node it no longer holds", async () => {
    const without = answer({ nodes: answer().nodes.map((node) => (node.name === "A-2" ? { ...node, name: "A-3", title: "Arrival by day" } : node)) });
    const stub = client();
    stub.getMap.mockImplementation((_project, options) => Promise.resolve(options.archive ? without : answer()));
    const { region } = await onMap(stub);
    const fitted = transform().zoom;
    region.focus();
    fireEvent.keyDown(region, { key: "-" });
    await waitFor(() => {
      expect(transform().zoom).toBeCloseTo(fitted / ZOOM_STEP, 9);
    });
    const search = screen.getByRole("searchbox", { name: "Find on the map" });
    fireEvent.change(search, { target: { value: "night" } });
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() => {
      expect(document.querySelector(".map-card-title")?.textContent).toBe("Arrival at night");
    });
    // The search cleared, the node stays lit.
    fireEvent.change(search, { target: { value: "" } });
    const zoom = transform().zoom;
    fireEvent.click(within(document.querySelector(".map-controls") as HTMLElement).getByRole("checkbox", { name: "Include archive" }));
    await waitFor(() => {
      expect(polite()).toBe("The lit node left the map");
    });
    expect(stub.getMap.mock.calls.at(-1)?.[1]).toEqual({ archive: true });
    expect(document.querySelector(".map-card")).toBeNull();
    await waitFor(() => {
      expect(screen.queryByText(/^Laying out/)).toBeNull();
    });
    expect(transform().zoom).toBe(zoom);
  });

  it("names how many it leaves out past 5 000 drawn, and the List pages through all (CM9)", async () => {
    const many = aMap(Array.from({ length: 5001 }, (_, at) => aMapNode({ name: `N-${String(at)}`, used_by: at === 7 ? 0 : 1 })));
    renderApp(client(many), "#/alpha/graph");
    expect(await screen.findByText("Showing the 5000 most used of 5001 nodes; the List holds all.")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(document.querySelectorAll(".map-table tbody tr")).toHaveLength(200);
    fireEvent.click(screen.getByRole("button", { name: "Show 200 more" }));
    expect(document.querySelectorAll(".map-table tbody tr")).toHaveLength(400);
    // What the picture keeps is the scene's; the List's 5 001 rows are its own.
    const scene = sceneOf(many, false);
    expect(keptOf(many, scene, DEFAULT_FILTERS).points.reduce((sum, one) => sum + one, 0)).toBe(5000);
  });
});

describe("the Map's states (CM8)", () => {
  it("shows a skeleton while reading, then the daemon's refusal with Retry, then the map", async () => {
    const stub = client();
    let fail: (reason: unknown) => void = () => undefined;
    stub.getMap.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          fail = reject;
        }),
    );
    renderApp(stub, "#/alpha/graph");
    expect(await screen.findByRole("status", { name: "Reading the map" })).toBeTruthy();
    const message = "spec map: the queue's schema 14 is newer than this binary's 13";
    act(() => {
      fail(new ClientError({ status: 503, message }));
    });
    expect(await screen.findByText("The map could not be read")).toBeTruthy();
    expect(screen.getAllByText(message, { exact: false }).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: /Retry/ }));
    expect(await screen.findByRole("region", { name: "Map picture" })).toBeTruthy();
  });

  it("says there is no document to draw when the answer holds none", async () => {
    renderApp(client(aMap([])), "#/alpha/graph");
    expect(await screen.findByRole("heading", { level: 3, name: "No document to draw" })).toBeTruthy();
  });

  it("keeps the home's view per project for the page's life", async () => {
    await onMap();
    fireEvent.click(within(screen.getByRole("group", { name: "View" })).getByRole("button", { name: "Rings" }));
    expect(await screen.findByRole("heading", { level: 2, name: "Scope rings" })).toBeTruthy();
    act(() => {
      window.location.hash = "#/alpha/tree";
    });
    await screen.findByRole("heading", { level: 1, name: /Spec tree/ });
    act(() => {
      window.location.hash = "#/alpha/graph";
    });
    const views = await screen.findByRole("group", { name: "View" });
    expect(within(views).getByRole("button", { name: "Rings" }).getAttribute("aria-pressed")).toBe("true");
  });
});

describe("the List's pages (CM9)", () => {
  it("moves focus to the first row each Show more adds, the last page's too, so it never falls to the page (M: focus left on the button)", async () => {
    const many = aMap(Array.from({ length: 450 }, (_, at) => aMapNode({ name: `N-${String(at)}`, title: `Node ${String(at)}` })));
    renderApp(client(many), "#/alpha/graph");
    await screen.findByRole("region", { name: "Map picture" });
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const first = screen.getByRole("button", { name: "Show 200 more" });
    first.focus();
    fireEvent.click(first);
    await waitFor(() => {
      expect(document.activeElement?.textContent).toBe("Node 200");
    });
    expect(document.activeElement?.tagName).toBe("A");
    // The last page: its button goes with the click, focus to its first row all the same.
    const last = screen.getByRole("button", { name: "Show 50 more" });
    last.focus();
    fireEvent.click(last);
    await waitFor(() => {
      expect(document.activeElement?.textContent).toBe("Node 400");
    });
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
    expect(document.activeElement).not.toBe(document.body);
    expect(document.querySelectorAll(".map-table tbody tr")).toHaveLength(450);
  });
});

describe("the List's paging and the filters (CM8)", () => {
  it("starts the List again at its first page on any filter, a switch as much as a chip (M: paging kept across a switch)", async () => {
    const many = aMap(Array.from({ length: 450 }, (_, at) => aMapNode({ name: `N-${String(at)}`, kind: at % 2 === 0 ? "scene" : "cue" })));
    renderApp(client(many), "#/alpha/graph");
    await screen.findByRole("region", { name: "Map picture" });
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const rows = () => document.querySelectorAll(".map-table tbody tr").length;
    fireEvent.click(screen.getByRole("button", { name: "Show 200 more" }));
    expect(rows()).toBe(400);
    fireEvent.click(screen.getByRole("checkbox", { name: "Mentions" }));
    expect(rows()).toBe(200);
    fireEvent.click(screen.getByRole("button", { name: "Show 200 more" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Parents" }));
    expect(rows()).toBe(200);
    fireEvent.click(screen.getByRole("button", { name: "Show 200 more" }));
    fireEvent.click(chip(/^cue/));
    expect(rows()).toBe(200);
  });
});

describe("the List's Broken links (CM8)", () => {
  /** The mock's hostile written end, a ghost shown as text. */
  const HOSTILE = ["javascript", ["al", "ert(1)"].join("")].join(":");
  const REASON = "`GONE-1` resolves to no ID and no alias";

  function withGhosts(): MapDocument {
    return answer({
      ghosts: [
        { written: "GONE-1", reason: REASON, sources: [0, 2], places: 2 },
        { written: HOSTILE, reason: null, sources: [3], places: 1 },
      ],
    });
  }

  /** The Broken links section's rows, each cell's text, a list's items joined by ", ". */
  function ghostRows(heading = "Broken links"): string[][] {
    const section = screen.queryByRole("region", { name: heading });
    const text = (cell: Element) => {
      const items = Array.from(cell.querySelectorAll("li"), (item) => item.textContent);
      return items.length > 0 ? items.join(", ") : cell.textContent.replace(/\s+/g, " ").trim();
    };
    return Array.from(section?.querySelectorAll("tbody tr") ?? [], (row) => Array.from(row.querySelectorAll("td"), text));
  }

  it("lists the ghosts the picture keeps: what is written as text, the daemon's reason in English, the places, each source a document link (M: ghosts listed with Unresolved targets off)", async () => {
    await onMap(client(withGhosts()));
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    // Unresolved targets off: the picture draws no ghost, so the List lists none.
    expect(screen.queryByRole("heading", { level: 3, name: "Broken links" })).toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "Unresolved targets" }));
    const section = screen.getByRole("region", { name: "Broken links" });
    expect(within(section).getAllByRole("columnheader").map((header) => header.textContent)).toEqual(["Target as written", "Reason", "Places", "Linked from"]);
    expect(ghostRows()).toEqual([
      ["GONE-1", REASON, "2 places", "Arrival, Bell cue"],
      [HOSTILE, "", "1 place", "Lantern"],
    ]);
    const [gone, hostile] = Array.from(section.querySelectorAll("tbody tr"));
    expect(gone?.querySelector("[lang='en']")?.textContent).toBe(REASON);
    // What is written is text, never a link: the hostile one too.
    expect(gone?.querySelector("td")?.querySelector("a")).toBeNull();
    expect(hostile?.querySelector("td")?.querySelector("a")).toBeNull();
    expect(within(gone as HTMLElement).getAllByRole("link").map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
      ["Arrival", "#/alpha/tree/A-1/page"],
      ["Bell cue", "#/alpha/tree/C-1/page"],
    ]);
    fireEvent.click(within(gone as HTMLElement).getByRole("link", { name: "Bell cue" }));
    expect(docOf(window.location.hash)).toBe("C-1");
  });

  it("follows the node filters as the picture does: a released source's ghost goes, Orphans only lists none", async () => {
    await onMap(client(withGhosts()));
    fireEvent.click(screen.getByRole("checkbox", { name: "Unresolved targets" }));
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    fireEvent.click(chip(/^prop/));
    expect(ghostRows().map((row) => row[0])).toEqual(["GONE-1"]);
    fireEvent.click(chip(/^prop/));
    fireEvent.click(screen.getByRole("checkbox", { name: "Orphans only" }));
    expect(screen.queryByRole("region", { name: "Broken links" })).toBeNull();
  });

  it("says it in Russian from ru.ts, the daemon's reason still in English", async () => {
    await onMap(client(withGhosts()));
    fireEvent.click(screen.getByRole("checkbox", { name: "Unresolved targets" }));
    fireEvent.change(screen.getByRole<HTMLSelectElement>("combobox", { name: en.language.label }), { target: { value: "ru" } });
    fireEvent.click(await screen.findByRole("tab", { name: ruT("graph.tabList") }));
    const section = screen.getByRole("region", { name: ruT("map.brokenLinks") });
    expect(within(section).getAllByRole("columnheader").map((header) => header.textContent)).toEqual([ruT("map.ghost.written"), ruT("map.ghost.reason"), ruT("map.ghost.placesColumn"), ruT("map.ghost.sources")]);
    expect(ghostRows(ruT("map.brokenLinks")).map((row) => row[2])).toEqual([ruT("map.ghost.places", { count: 2 }), ruT("map.ghost.places", { count: 1 })]);
    expect(section.querySelector("[lang='en']")?.textContent).toBe(REASON);
  });
});

/** A role's colour as the canvas reads it: its hidden swatch's, else the role's name. */
function resolved(role: string): string {
  const swatch = document.querySelector<HTMLElement>(`.map-swatches [data-role="${role}"]`);
  return swatch === null ? "" : window.getComputedStyle(swatch).color || role;
}

describe("a colour per kind (CM7)", () => {
  /** Seven documents of three kinds and none, in an order no count sorts in. */
  const KINDED = aMap([
    aMapNode({ name: "P-1", kind: "prop", title: "Lamp" }),
    aMapNode({ name: "C-1", kind: "cue", title: "Bell" }),
    aMapNode({ name: "S-1", kind: "scene", title: "Dock" }),
    aMapNode({ name: "N-1", kind: null, title: "Loose note" }),
    aMapNode({ name: "S-2", kind: "scene", title: "Quay" }),
    aMapNode({ name: "C-2", kind: "cue", title: "Horn" }),
    aMapNode({ name: "S-3", kind: "scene", title: "Pier" }),
  ]);

  /** The fill the base drew a node's disc in. */
  function fillOf(name: string): string | undefined {
    const scene = sceneOf(KINDED, false);
    const positions = layoutAll(scene.points.length, []);
    const point = scene.pointOf[KINDED.nodes.findIndex((node) => node.name === name)] ?? -1;
    const x = positions[point * 2];
    return pathsOf(lastDrawing()).find((path) => path.op === "fill" && path.arcs.some((arc) => arc.x === x))?.fillStyle;
  }

  it("colours a corpus with no direction by its kinds, most held first, and names each in the legend with its count (M: colour keyed on direction)", async () => {
    const stub = stubClient();
    stub.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "all" })], []));
    stub.getMap.mockResolvedValue(KINDED);
    await onMap(stub);
    const fills = Object.fromEntries(["S-1", "S-2", "S-3", "C-1", "C-2", "P-1", "N-1"].map((name) => [name, fillOf(name)]));
    expect(fills).toEqual({
      "S-1": resolved("--map-kind-1"),
      "S-2": resolved("--map-kind-1"),
      "S-3": resolved("--map-kind-1"),
      "C-1": resolved("--map-kind-2"),
      "C-2": resolved("--map-kind-2"),
      "P-1": resolved("--map-kind-3"),
      "N-1": resolved("--map-kind-other"),
    });
    expect(new Set(Object.values(fills)).size).toBe(4);
    const legend = screen.getByRole("region", { name: "Legend" });
    const kinds = within(legend).getByRole("list", { name: "Kinds" });
    expect(within(kinds).getAllByRole("listitem").map((item) => [item.textContent.replace(/\s+/g, " ").trim(), item.querySelector(".map-swatch")?.className])).toEqual([
      ["scene3 nodes", "map-swatch map-slot-1"],
      ["cue2 nodes", "map-swatch map-slot-2"],
      ["prop1 node", "map-swatch map-slot-3"],
      ["Other kinds1 node", "map-swatch map-slot-0"],
    ]);
    // The chips carry the same swatches, the coloured kinds first.
    expect(Array.from(screen.getByRole("group", { name: "Kinds" }).querySelectorAll(".map-swatch"), (swatch) => swatch.className)).toEqual([
      "map-swatch map-slot-1",
      "map-swatch map-slot-2",
      "map-swatch map-slot-3",
      "map-swatch map-slot-0",
    ]);
  });

  it("keeps the Direction select a filter: it presses its kinds' chips and colours nothing", async () => {
    const stub = stubClient();
    stub.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "all" })], [aDirection({ name: "cues", title: "Cues", kinds: ["cue"] })]));
    stub.getMap.mockResolvedValue(KINDED);
    await onMap(stub);
    expect([fillOf("S-1"), fillOf("C-1")]).toEqual([resolved("--map-kind-1"), resolved("--map-kind-2")]);
    fireEvent.change(screen.getByRole("combobox", { name: "Direction" }), { target: { value: "cues" } });
    expect(chip(/^cue/).getAttribute("aria-pressed")).toBe("true");
    expect(chip(/^scene/).getAttribute("aria-pressed")).toBe("false");
    await waitFor(() => {
      expect(filledDiscs()).toBe(2);
    });
    expect(fillOf("C-1")).toBe(resolved("--map-kind-2"));
  });
});

describe("Mentions at first (CM8)", () => {
  it("starts with Mentions off, the base drawing no mention; one click draws them (M: Mentions on at first)", async () => {
    await onMap();
    const mentions = screen.getByRole<HTMLInputElement>("checkbox", { name: "Mentions" });
    expect(mentions.checked).toBe(false);
    const weak = () => pathsOf(lastDrawing()).filter((path) => path.op === "stroke" && path.segments > 0 && path.strokeStyle === resolved("--map-edge-weak")).reduce((sum, path) => sum + path.segments, 0);
    expect(weak()).toBe(0);
    fireEvent.click(mentions);
    expect(mentions.checked).toBe(true);
    await waitFor(() => {
      expect(weak()).toBe(1);
    });
  });
});

describe("the notes (CM8)", () => {
  const NOTES = ["docs/a.md:1: first", "docs/b.md:2: second", "docs/c.md:3: third", "docs/d.md:4: fourth", "docs/e.md:5: fifth"];

  /** The notes as shown: each item's text, those a collapsed disclosure hides left out. */
  function shownNotes(): string[] {
    return Array.from(document.querySelectorAll<HTMLElement>(".map-notes li"))
      .filter((item) => item.closest("[hidden]") === null)
      .map((item) => item.textContent);
  }

  it("shows the first three in English, the rest behind Show all N; opened, all (M: every note shown at once)", async () => {
    await onMap(client(answer({ notes: NOTES })));
    expect(shownNotes()).toEqual(NOTES.slice(0, 3));
    const more = screen.getByRole("button", { name: "Show all 5 notes" });
    expect(more.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(more);
    expect(more.getAttribute("aria-expanded")).toBe("true");
    expect(shownNotes()).toEqual(NOTES);
    expect(Array.from(document.querySelectorAll(".map-notes li"), (item) => item.getAttribute("lang"))).toEqual(NOTES.map(() => "en"));
  });

  it("shows three or fewer whole, with no disclosure", async () => {
    await onMap(client(answer({ notes: NOTES.slice(0, 3) })));
    expect(shownNotes()).toEqual(NOTES.slice(0, 3));
    expect(screen.queryByRole("button", { name: /^Show all/ })).toBeNull();
  });

  it("names the disclosure from ru.ts in Russian, the notes still in English", async () => {
    await onMap(client(answer({ notes: NOTES })));
    fireEvent.change(screen.getByRole<HTMLSelectElement>("combobox", { name: en.language.label }), { target: { value: "ru" } });
    expect(await screen.findByRole("button", { name: ruT("map.notesAll", { count: 5 }) })).toBeTruthy();
    expect(shownNotes()).toEqual(NOTES.slice(0, 3));
  });
});

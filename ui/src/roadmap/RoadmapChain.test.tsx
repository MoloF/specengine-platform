import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RoadmapItem } from "../api/types";
import { CANVAS_LIMIT } from "../graph/layout";
import { aRoadmap, aRoadmapGroup, aRoadmapItem } from "../test/builders";
import { renderApp } from "../test/render";
import { aPlan, roadmapClient } from "../test/roadmapStub";
import { chainOf } from "./chain";

// UI-4 of docs/features/roadmap-screen.md: the Chain tab's canvas is read only, as the Graph's is
// (no draggable box, no connectable handle, no key listener but the shell's, no word of deleting,
// moving, dragging or connecting), and it is the Lanes tab's twin: the same items, the same
// `waits_on` pairs between two items (across lanes too), the same cycle marks, the same task
// cells. A box's click selects through the route. Past CANVAS_LIMIT boxes, whole lanes only, the
// count of those not drawn named.

function boxes(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>('.react-flow__node[data-id^="i:"]'));
}

function box(id: string): HTMLElement {
  const found = boxes().find((element) => element.dataset.id === `i:${id}`);
  if (found === undefined) {
    throw new Error(`no box ${id}`);
  }
  return found;
}

async function openChain(client = roadmapClient(), hash = "#/alpha/roadmap") {
  renderApp(client, hash);
  fireEvent.click(await screen.findByRole("tab", { name: "Chain" }));
  await screen.findByRole("application", { name: "Roadmap canvas" });
  return client;
}

async function drawn() {
  await waitFor(() => {
    expect(boxes().length).toBeGreaterThan(0);
  });
}

/** What a tab shows of the plan, as text: items, waits between two items, cycle marks, task cells. */
interface Twin {
  items: string[];
  waits: string[];
  cycles: string[];
  tasks: string[];
}

function canvasTwin(): Twin {
  const ids = boxes().map((element) => (element.dataset.id ?? "").slice(2));
  return {
    items: ids.sort(),
    waits: Array.from(document.querySelectorAll(".react-flow__edge"), (edge) => edge.getAttribute("aria-label") ?? "").sort(),
    cycles: boxes()
      .filter((element) => element.querySelector("[data-cycle]") !== null)
      .map((element) => (element.dataset.id ?? "").slice(2))
      .sort(),
    tasks: boxes()
      .flatMap((element) =>
        Array.from(element.querySelectorAll<HTMLElement>("[data-task]"), (cell) => `${(element.dataset.id ?? "").slice(2)} ${cell.dataset.task ?? ""} ${cell.textContent.replace(cell.dataset.task ?? "", "").trim()}`),
      )
      .sort(),
  };
}

function lanesTwin(): Twin {
  const cards = Array.from(document.querySelectorAll<HTMLElement>("[data-item]"));
  return {
    items: cards.map((card) => card.dataset.item ?? "").sort(),
    waits: cards
      .flatMap((card) =>
        Array.from(card.querySelectorAll<HTMLElement>("[data-waits-on]"))
          .map((link) => link.dataset.waitsOn ?? "")
          .filter((from) => from !== card.dataset.item)
          .map((from) => `${card.dataset.item ?? ""} waits on ${from}`),
      )
      .sort(),
    cycles: cards
      .filter((card) => card.querySelector("[data-cycle]") !== null)
      .map((card) => card.dataset.item ?? "")
      .sort(),
    tasks: cards
      .flatMap((card) =>
        Array.from(card.querySelectorAll<HTMLElement>("[data-task]"), (cell) => `${card.dataset.item ?? ""} ${cell.dataset.task ?? ""} ${cell.querySelector(".badge-label")?.textContent ?? ""}`),
      )
      .sort(),
  };
}

describe("the canvas is read only (UI-4)", () => {
  it("drags, connects and selects nothing: no draggable or selectable box, no connectable handle, no focusable box", async () => {
    await openChain();
    await drawn();
    expect(document.querySelectorAll(".react-flow__node.draggable")).toHaveLength(0);
    expect(document.querySelectorAll(".react-flow__node.selectable")).toHaveLength(0);
    expect(document.querySelectorAll(".react-flow__handle.connectable, .react-flow__handle.connectablestart, .react-flow__handle.connectableend")).toHaveLength(0);
    expect(document.querySelectorAll(".react-flow__handle").length).toBeGreaterThan(0);
    expect(boxes().filter((element) => element.hasAttribute("tabindex"))).toEqual([]);
    expect(document.querySelectorAll(".react-flow__edge[tabindex]")).toHaveLength(0);
  });

  it("adds no key listener to the document or the window but the shell's chord listener", async () => {
    const onDocument = vi.spyOn(document, "addEventListener");
    const onWindow = vi.spyOn(window, "addEventListener");
    await openChain();
    await drawn();
    const keyListeners = [
      ...onDocument.mock.calls.map(([type, , options]) => ["document", type, options]),
      ...onWindow.mock.calls.map(([type, , options]) => ["window", type, options]),
    ].filter(([, type]) => typeof type === "string" && /^key/.test(type));
    expect(keyListeners).toEqual([["document", "keydown", true]]);
  });

  it("never speaks of deleting, moving, dragging or connecting on the canvas", async () => {
    await openChain();
    await drawn();
    const canvas = screen.getByRole("application", { name: "Roadmap canvas" });
    const words = [canvas.textContent];
    for (const element of [canvas, ...Array.from(canvas.querySelectorAll("*"))]) {
      for (const attribute of ["aria-label", "title", "aria-description"]) {
        words.push(element.getAttribute(attribute) ?? "");
      }
    }
    expect(words.filter((word) => /delete|move|drag|connect/i.test(word))).toEqual([]);
    // The attribution hidden on the canvas, credited as text beside it, never a link (ui/README.md "Screen rules", Canvas).
    expect(document.querySelector(".react-flow__attribution")).toBeNull();
    const credit = screen.getByText("Drawn with React Flow (MIT)");
    expect(credit.closest("a")).toBeNull();
    expect(canvas.contains(credit)).toBe(false);
    expect(document.getElementById(canvas.getAttribute("aria-describedby") ?? "")?.textContent).toBe("The Lanes tab holds every item, wait and mark as text.");
  });
});

describe("the canvas is the Lanes tab's twin (UI-4)", () => {
  it("carries the same items, waits between two items (across lanes too), cycle marks and task cells", async () => {
    await openChain();
    await drawn();
    const canvas = canvasTwin();
    fireEvent.click(screen.getByRole("tab", { name: "Lanes" }));
    await screen.findByRole("heading", { level: 2, name: "beta" });
    const lanes = lanesTwin();
    expect(canvas).toEqual(lanes);
    // The pairs that cross a lane: launch to beta, the pool to launch.
    expect(canvas.waits).toEqual(expect.arrayContaining(["RM-0022 waits on RM-0011", "RM-0040 waits on RM-0022"]));
    expect(canvas.items).toHaveLength(13);
    expect(canvas.tasks).toHaveLength(11);
  });

  it("draws no edge for an item waiting on itself; its In a cycle mark carries it", async () => {
    await openChain();
    await drawn();
    expect(canvasTwin().waits.filter((wait) => wait.startsWith("RM-0060"))).toEqual([]);
    expect(within(box("RM-0060")).getByText("In a cycle").querySelector("svg.icon")).not.toBeNull();
  });

  it("draws every group as a column with its name, the empty release too, and the cut's rule as text", async () => {
    await openChain();
    await drawn();
    expect(Array.from(document.querySelectorAll(".chain-lane-name"), (name) => name.textContent)).toEqual(["alpha", "beta", "launch", "Pool", "Unplaced"]);
    const marks = Array.from(document.querySelectorAll(".chain-lane-marks"), (line) => line.textContent);
    expect(marks[1]).toBe("CurrentMVP cut4 open, 1 done");
    expect(document.querySelector(".chain-cut-label")?.textContent).toBe("MVP cut: 10-12 days, 1 unestimated");
    // Box columns follow the groups: alpha's column empty, beta's boxes in the second, the pool's in the fourth.
    const layout = chainOf(aPlan());
    expect([0, 1, 2, 3, 4].map((column) => layout.boxes.filter((placed) => placed.column === column).length)).toEqual([0, 4, 5, 2, 2]);
    expect(layout.boxes.find((placed) => placed.item.id === "RM-0011")).toMatchObject({ column: 1, row: 2, x: 336 });
  });

  it("selects an item through the route when its box is clicked: one history entry, the box marked Selected", async () => {
    await openChain();
    await drawn();
    const entries = window.history.length;
    fireEvent.click(box("RM-0011"));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/roadmap/RM-0011");
    });
    expect(window.history.length).toBe(entries + 1);
    await waitFor(() => {
      expect(within(box("RM-0011")).getByText("Selected").querySelector("svg.icon")).not.toBeNull();
    });
    expect(box("RM-0011").querySelector(".chain-box")?.className).toContain("is-selected");
    expect(box("RM-0010").querySelector(".is-selected")).toBeNull();
    expect(screen.getByRole("tab", { name: "Chain" }).getAttribute("aria-selected")).toBe("true");
  });

  it("names each box by its ID, title, cycle, waits and tasks", async () => {
    await openChain();
    await drawn();
    expect(box("RM-0022").getAttribute("aria-label")).toBe("RM-0022, Ferry launch event, waits on RM-0030, RM-0020, RM-0011");
    expect(box("RM-0021").getAttribute("aria-label")).toBe("RM-0021, Ferry ticket booth, in a cycle, waits on RM-0020");
    expect(box("RM-0041").getAttribute("aria-label")).toBe("RM-0041, no title");
    expect(box("RM-0017").getAttribute("aria-label")).toBe(
      "RM-0017, Save and load, task T-0001 Draft, task T-0002 Analysis, task T-0003 Plan review, task T-0004 Changes requested, task T-0005 Ready",
    );
  });
});

function items(prefix: string, count: number, waitsOnPrevious = false): RoadmapItem[] {
  return Array.from({ length: count }, (_, at) =>
    aRoadmapItem({ id: `${prefix}-${String(at).padStart(4, "0")}`, waits_on: waitsOnPrevious && at > 0 ? [`${prefix}-${String(at - 1).padStart(4, "0")}`] : [] }),
  );
}

describe("the cap (UI-4)", () => {
  it("draws whole lanes left to right up to 200 boxes, then names the items not drawn; Lanes lists them all", async () => {
    const plan = aRoadmap([
      aRoadmapGroup("release", "beta", items("AA", 150, true), { current: true }),
      aRoadmapGroup("release", "launch", items("BB", 100)),
      aRoadmapGroup("pool", null, items("CC", 30)),
    ]);
    await openChain(roadmapClient(plan));
    await drawn();
    expect(boxes()).toHaveLength(150);
    expect(boxes().every((element) => (element.dataset.id ?? "").startsWith("i:AA-"))).toBe(true);
    expect(screen.getByRole("note").textContent).toBe("130 of 280 items not drawn");
    expect(Array.from(document.querySelectorAll(".chain-lane-name"), (name) => name.textContent)).toEqual(["beta"]);
    expect(document.querySelectorAll(".react-flow__edge")).toHaveLength(149);
    fireEvent.click(screen.getByRole("tab", { name: "Lanes" }));
    await screen.findByRole("heading", { level: 2, name: "beta" });
    expect(document.querySelectorAll("[data-item]")).toHaveLength(280);
  });

  it("draws no box when the first lane alone passes the cap, naming all n", async () => {
    const plan = aRoadmap([aRoadmapGroup("release", "beta", items("AA", CANVAS_LIMIT + 1), { current: true }), aRoadmapGroup("pool", null, items("CC", 2))]);
    await openChain(roadmapClient(plan));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(boxes()).toHaveLength(0);
    expect(screen.getByRole("note").textContent).toBe("203 of 203 items not drawn");
  });

  it("names nothing below the cap", async () => {
    await openChain();
    await drawn();
    expect(screen.queryByRole("note")).toBeNull();
  });
});

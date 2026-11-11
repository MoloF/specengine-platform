import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { FlowEdge, FlowNode, FlowView as FlowAnswer, QueueEvent, TraceCounts, TraceStage } from "../api/types";
import { aCardWrite, aFlowEdge, aFlowGap, aFlowNode, aFlowView, aTreeNode, tenGapRows } from "../test/builders";
import { ASIDE, asideClient, asideFiring, asideLens, OPEN_DAY } from "../test/asideStub";
import { studioClient, TYPE_A, TYPE_B, type StudioReads } from "../test/flowStub";
import { mouse, pointer, pressAndMove, stubCanvas, stubFocusVisible, viewOf } from "../test/press";
import { renderApp } from "../test/render";
import { enT } from "../test/text";
import { focusables } from "../ui/Dialog";
import { FLOW_DASHES, dashOf, edgeWords, shortWhen, typeColour, typesOf, WHEN_SHOWN } from "./labels";

// docs/features/writer-studio.md on the stub client: ST-3, ST-5, ST-7 to ST-11 and the rules R2 to
// R8; the criteria over harbor-sim (ST-2, ST-6, ST-12, ST-13) run on the mock, in
// src/mocks/flow.smoke.test.tsx, the one place a test may import it. The stub's project `alpha`
// serves no module, so no hash names one.

afterEach(() => {
  Reflect.deleteProperty(window, "matchMedia");
});

const NONE: TraceCounts = { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 0 };

const node = (fields: Partial<FlowNode> & Pick<FlowNode, "id" | "distance">) => aFlowNode({ group: "G", ...fields });

/** G's children: a walk A, B, C (a group), E; D off the walk; stages across them. */
const CHILDREN: FlowNode[] = [
  node({ id: "A", distance: 0, start: true, stage: "gather", title: "Ask", rollup: "confirmed" }),
  node({ id: "B", distance: 1, stage: "gather", title: "Bind", rollup: "no card" }),
  node({ id: "C", distance: 2, holds: 2, title: "Craft", counts: { ...NONE, in_work: 2 }, gaps: 1 }),
  node({ id: "E", distance: 3, end: true, stage: "print", title: "Engrave", rollup: "in work" }),
  node({ id: "D", distance: null, title: "Dye" }),
];

const EDGES: FlowEdge[] = [
  aFlowEdge({ from: "A", to: "B", type: TYPE_A }),
  aFlowEdge({ from: "B", to: "C", type: TYPE_A, kind: "alternative", when: "the ink is dry" }),
  aFlowEdge({ from: "C", to: "E", type: TYPE_A, count: 3, path: "docs/spec/e.md", line: 14 }),
  aFlowEdge({ from: "E", to: "B", type: TYPE_A, kind: "repeat" }),
  aFlowEdge({ from: "D", to: "A", type: TYPE_B }),
  aFlowEdge({ from: "G", to: "D", type: TYPE_B }),
];

// Each member titled as the daemon serves it beside its items (docs/features/doc-modal.md "Acceptance criteria", DM-15).
const ROWS = tenGapRows([
  aFlowGap("no-start", [["G"]], { titles: [["The guild"]] }),
  aFlowGap("cycle-no-repeat", [["B", "E"]], { titles: [["Bind", "Engrave"]] }),
  aFlowGap("unreachable", [["X"]], { titles: [["Title of X"]] }),
]);

function scopeOfG(fields: Partial<FlowAnswer> = {}): FlowAnswer {
  return aFlowView([aFlowNode({ id: "G", distance: null, kind: "domain", title: "The guild", counts: { ...NONE, confirmed: 2, no_card: 3 } }), ...CHILDREN], EDGES, { root: "G", depth: 1, gaps: ROWS, ...fields });
}

const SCOPE_C = aFlowView([aFlowNode({ id: "C", distance: null, group: null, title: "Craft" }), aFlowNode({ id: "X", distance: 0, group: "C", start: true }), aFlowNode({ id: "Y", distance: 1, group: "C" })], [aFlowEdge({ from: "X", to: "Y" })], { root: "C", depth: 1 });

const lane = (stage: string | null, distance: number | null, counts: Partial<TraceCounts>): TraceStage => ({ stage, distance, counts: { ...NONE, ...counts } });

const LANES = [lane("gather", 0, { confirmed: 1, no_card: 1 }), lane("print", 3, { in_work: 1 }), lane(null, 2, { no_card: 1 })];

const TREE = [aTreeNode({ id: "G", depth: 0 }), ...["A", "B", "C", "D", "E"].map((id) => aTreeNode({ id, depth: 1, parent: "G" })), ...["X", "Y"].map((id) => aTreeNode({ id, depth: 2, parent: "C" }))];

function reads(fields: Partial<StudioReads> = {}): StudioReads {
  return {
    top: aFlowView([aFlowNode({ id: "G", distance: 0, holds: 7, start: true, end: true, title: "The guild" })], []),
    scopes: { G: scopeOfG(), C: SCOPE_C },
    lanes: { G: LANES, C: [lane(null, 0, { no_card: 2 })] },
    tree: TREE,
    ...fields,
  };
}

function boxes(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".react-flow__node[data-id^='n:']"));
}

function box(name: string): HTMLElement {
  const found = boxes().find((element) => element.dataset.id === `n:${name}`);
  if (found === undefined) {
    throw new Error(`no box ${name}`);
  }
  return found;
}

async function drawn(name: string): Promise<HTMLElement> {
  await waitFor(() => {
    expect(boxes().some((element) => element.dataset.id === `n:${name}`)).toBe(true);
  });
  return box(name);
}

function drawnNames(): string[] {
  return boxes().map((element) => (element.dataset.id ?? "").slice(2));
}

function panel(): HTMLElement {
  return screen.getByRole("complementary", { name: "Details" });
}

function places(): HTMLElement {
  return screen.getByRole("navigation", { name: "Flow places" });
}

/** The bar's Gaps toggle (flow-scenario D10). */
function gapsToggle(): HTMLElement {
  return screen.getByRole("button", { name: /^Gaps/ });
}

/** The Gaps control, open over the canvas. */
function gapsControl(): HTMLElement {
  return screen.getByRole("region", { name: "Gaps" });
}

/** Opens the Gaps control and gives it. */
function openGaps(): HTMLElement {
  if (gapsToggle().getAttribute("aria-expanded") !== "true") {
    fireEvent.click(gapsToggle());
  }
  return gapsControl();
}

function edgeNamed(label: string): HTMLElement {
  const found = Array.from(document.querySelectorAll<HTMLElement>(".react-flow__edge")).find((element) => element.getAttribute("aria-label") === label);
  if (found === undefined) {
    throw new Error(`no edge ${label}`);
  }
  return found;
}

function hitOf(edge: HTMLElement): SVGPathElement {
  const hit = edge.querySelector<SVGPathElement>(".react-flow__edge-interaction");
  if (hit === null) {
    throw new Error("no interaction path");
  }
  return hit;
}

function fact(name: string): string {
  const term = within(panel()).getAllByText(name, { selector: "dt" })[0];
  return term?.nextElementSibling?.textContent ?? "";
}

function stubWidth(stacked: boolean, narrow = false) {
  Object.defineProperty(window, "matchMedia", {
    value: (media: string) => ({
      media,
      matches: (stacked && media === "(max-width: 63.99rem)") || (narrow && media === "(max-width: 48rem)"),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }),
    configurable: true,
    writable: true,
  });
}

/** Every read the stub served, by method. */
function readsOf(client: ReturnType<typeof studioClient>) {
  return { flow: client.getFlow.mock.calls.length, trace: client.getTrace.mock.calls.length, node: client.getNode.mock.calls.length, tree: client.getTree.mock.calls.length };
}

describe("the lens root and the reads (D2, D3)", () => {
  it("draws Top's one group's children, the group itself in the panel, from one read of each form", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow");
    await drawn("E");
    expect(drawnNames().sort()).toEqual(["A", "B", "C", "D", "E"]);
    expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("The guild");
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([
      { depth: 0, in_flow: true, trace: true },
      { root: "G", depth: 1, in_flow: true, trace: true },
    ]);
    expect(client.getTrace.mock.calls.map((call) => call[1])).toEqual([{ ref: "G", summary: true }]);
    expect(client.getNode.mock.calls.map((call) => [call[1], call[2]])).toEqual([["G", { with: ["links"] }]]);
    expect(client.getTree).not.toHaveBeenCalled();
  });

  it("stands unrooted at a top of two nodes: its boxes, no stage read, no lanes; a stage named is dropped by a replace (R2)", async () => {
    const top = aFlowView([aFlowNode({ id: "P", distance: 0, holds: 2 }), aFlowNode({ id: "Q", distance: 1 })], [aFlowEdge({ from: "P", to: "Q" })]);
    const client = studioClient(reads({ top }));
    const before = window.history.length;
    renderApp(client, "#/alpha/flow?stage=gather");
    await drawn("Q");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow");
    });
    expect(window.history.length).toBe(before);
    expect(within(places()).getByText("Stages show inside a group: open one.")).toBeTruthy();
    expect(within(places()).getAllByRole("link").some((link) => link.getAttribute("href") === "#/alpha/flow?scope=P")).toBe(true);
    expect(client.getTrace).not.toHaveBeenCalled();
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([{ depth: 0, in_flow: true, trace: true }]);
    expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("The whole project");
  });

  it("drops a module no `[[module]]` serves (R2), and the scope's own node as a selection (R3), each by a replace", async () => {
    const before = window.history.length;
    renderApp(studioClient(reads()), "#/alpha/flow/G?module=nope");
    await drawn("A");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow");
    });
    expect(window.history.length).toBe(before);
  });
});

describe("the place in the hash (D4, D5)", () => {
  it("replaces on a selection, pushes on a scope; Back returns", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    fireEvent.click(await drawn("A"));
    expect(window.location.hash).toBe("#/alpha/flow/A");
    const before = window.history.length;
    fireEvent.click(box("C").querySelector("[data-flow-toggle]") ?? box("C"));
    expect(window.location.hash).toBe("#/alpha/flow?scope=C");
    expect(window.history.length).toBe(before + 1);
    await drawn("X");
    act(() => {
      window.history.back();
    });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/A");
    });
    await drawn("A");
  });

  it("seeks a name no read holds once: the tree's parent, a replace, the panel; a name the tree lacks, a note and a Tree link (R3)", async () => {
    const client = studioClient(reads());
    const before = window.history.length;
    renderApp(client, "#/alpha/flow/X");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/X?scope=C");
    });
    await drawn("X");
    expect(window.history.length).toBe(before);
    expect(client.getTree).toHaveBeenCalledTimes(1);
    expect(client.getTree.mock.calls[0]?.[1]).toBeUndefined();
    expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Title of X");
    act(() => {
      window.location.hash = "#/alpha/flow/NOPE";
    });
    expect(await screen.findByText("No box here is named NOPE.")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Open NOPE in the Spec tree" }).getAttribute("href")).toBe("#/alpha/tree/NOPE");
    expect(client.getTree).toHaveBeenCalledTimes(1);
  });

  it("drops an arrow this read does not hold, by a replace, and says so (R4)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow?edge=A,E,alpha_link");
    await drawn("A");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow");
    });
    await waitFor(() => {
      expect(document.querySelector("[aria-live='polite']")?.textContent).toBe("That arrow is not in this view.");
    });
  });
});

describe("an edge click (ST-3)", () => {
  it("selects the arrow: its served words in the panel, `edge=` by a replace, no name, no read, the view still", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow");
    await drawn("E");
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__edge").length).toBeGreaterThan(3);
    });
    const hit = hitOf(edgeNamed("From Craft to Engrave, alpha_link"));
    expect(Number(hit.getAttribute("stroke-width"))).toBeGreaterThanOrEqual(12);
    const transform = document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform;
    const before = window.history.length;
    const served = readsOf(client);
    fireEvent.click(hit);
    expect(window.location.hash).toBe("#/alpha/flow?edge=C,E,alpha_link");
    expect(window.history.length).toBe(before);
    expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Arrow");
    expect([fact("From"), fact("To"), fact("Type"), fact("Kind"), fact("When"), fact("Links"), fact("Where")]).toEqual(["C Open C", "E", TYPE_A, "none", "none", "3", "docs/spec/e.md:14"]);
    // D9: the count stands in the panel, never on the canvas.
    expect(Array.from(document.querySelectorAll(".flow-edge-label"), (label) => label.textContent).some((text) => /x\d/.test(text))).toBe(false);
    expect(document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform).toBe(transform);
    expect(readsOf(client)).toEqual(served);
    // M9: no box took the click.
    expect(boxes().some((element) => (element.getAttribute("aria-label") ?? "").endsWith("selected"))).toBe(false);
  });

  it("says a kind and a when raw, and selects an end by a link, the hash replaced", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("C");
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__edge").length).toBeGreaterThan(3);
    });
    fireEvent.click(hitOf(edgeNamed(`From Bind to Craft, ${TYPE_A}, alternative, the ink is dry`)));
    expect(window.location.hash).toBe("#/alpha/flow?edge=B,C,alpha_link,alternative");
    expect([fact("Kind"), fact("When")]).toEqual(["alternative", "the ink is dry"]);
    const before = window.history.length;
    fireEvent.click(within(panel()).getByRole("link", { name: "B" }));
    expect(window.location.hash).toBe("#/alpha/flow/B");
    expect(window.history.length).toBe(before);
    expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Bind");
  });

  it("gives no edge a tab stop or a focus (M22): the keyboard reaches arrows through the List", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__edge").length).toBeGreaterThan(3);
    });
    for (const edge of document.querySelectorAll(".react-flow__edge")) {
      expect(edge.getAttribute("tabindex")).toBeNull();
    }
  });
});

describe("the panel (ST-5)", () => {
  it("is mounted with nothing selected: the scope's title, counts and ways out; name, kind and place under Technical details, collapsed; no gap row (D10, D12)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    const shown = within(panel());
    expect(shown.getByRole("heading", { level: 2 }).textContent).toBe("The guild");
    const details = shown.getByRole("button", { name: "Technical details" });
    expect(details.getAttribute("aria-expanded")).toBe("false");
    const region = document.getElementById(details.getAttribute("aria-controls") ?? "");
    expect(region?.hidden).toBe(true);
    expect([fact("Name"), fact("Kind"), fact("Where")]).toEqual(["G", "domain", "docs/spec/g.md:1"]);
    expect(region?.contains(within(panel()).getAllByText("Name", { selector: "dt" })[0] ?? null)).toBe(true);
    expect(shown.getByText("2 confirmed, 3 no task")).toBeTruthy();
    // The scope's acts (doc-modal DM-14): Open document, the one primary, and New card; the tree is the modal's to open.
    expect(shown.getByRole("button", { name: "Open document" }).className).toContain("button-primary");
    expect(shown.getByRole("button", { name: "New task" })).toBeTruthy();
    expect(shown.queryByRole("link", { name: "Open in Spec tree" })).toBeNull();
    expect(panel().querySelectorAll("li.flow-gap")).toHaveLength(0);
  });

  it("swaps a box's content in and back on Esc, in the same element", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    const aside = panel();
    fireEvent.click(await drawn("A"));
    await waitFor(() => {
      expect(within(aside).getByRole("heading", { level: 2 }).textContent).toBe("Ask");
    });
    expect(fact("Where")).toBe("docs/spec/a.md:1");
    expect(within(aside).getByRole("button", { name: "Back to The guild" })).toBeTruthy();
    fireEvent.keyDown(box("A"), { key: "Escape" });
    expect(window.location.hash).toBe("#/alpha/flow");
    expect(panel()).toBe(aside);
    expect(within(aside).getByRole("heading", { level: 2 }).textContent).toBe("The guild");
  });

  it("borrows nothing of the Graph's panel: no `graph-details` class in Flow's DOM, a box or an arrow selected (ST-4)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    fireEvent.click(await drawn("A"));
    await waitFor(() => {
      expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Ask");
    });
    expect(document.querySelector("main [class*='graph-details']")).toBeNull();
    act(() => {
      window.location.hash = "#/alpha/flow?edge=C,E,alpha_link";
    });
    await within(panel()).findByRole("heading", { level: 2, name: "Arrow" });
    expect(document.querySelector("main [class*='graph-details']")).toBeNull();
  });

  it("has no Hide gaps: a stored `specengine.flow.gaps-hidden=on` hides nothing, and the key is neither read nor written (M12)", async () => {
    window.localStorage.setItem("specengine.flow.gaps-hidden", "on");
    const getItem = vi.spyOn(Storage.prototype, "getItem");
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    fireEvent.click(gapsToggle());
    expect(gapsControl().querySelectorAll("li.flow-gap")).toHaveLength(3);
    expect(screen.queryByRole("button", { name: /gaps$/ })).toBeNull();
    expect([...getItem.mock.calls, ...setItem.mock.calls].some((call) => call[0] === "specengine.flow.gaps-hidden")).toBe(false);
  });

  it("shows a group's own Open, entering it; a holder that is the scope, none", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow/C");
    await drawn("C");
    expect((await within(panel()).findByRole("link", { name: "Open Craft" })).getAttribute("href")).toBe("#/alpha/flow?scope=C");
    expect(within(panel()).getByText(/^2 documents/)).toBeTruthy();
    expect(within(panel()).queryByText("Group", { selector: "dt" })).toBeNull();
  });

  it("shows a box's holder with Open when it is not the scope: a stage across groups", async () => {
    const deeper = node({ id: "Z", distance: 4, group: "C", stage: "print", title: "Zinc" });
    const scopes = { G: aFlowView([...scopeOfG().nodes, deeper], EDGES, { root: "G", depth: 1, gaps: ROWS }), C: SCOPE_C };
    renderApp(studioClient(reads({ scopes })), "#/alpha/flow/Z?stage=print");
    await drawn("Z");
    // No read in hand titles C on this fresh mount (the stage read holds Z and E alone): its name stands, a place link.
    expect(within(panel()).getByText(/^Group:/).closest("p")?.textContent).toBe("Group: C");
    expect(within(panel()).getByRole("link", { name: "C" }).getAttribute("href")).toBe("#/alpha/flow?scope=C");
  });
});

describe("the header (ST-7)", () => {
  it("is one line: the h1, the scope's title, two tabs and one disclosure holding the about text and each note once", async () => {
    const client = studioClient(reads({ scopes: { G: scopeOfG({ notes: ["a note from the daemon"] }), C: SCOPE_C } }));
    client.getTrace.mockImplementation((_project, options) => Promise.resolve({ ref: options?.ref ?? null, reason: null, notes: ["a note from the daemon", "a lane note"], counts: NONE, stages: LANES, nodes: [], truncated: false, omitted: null }));
    renderApp(client, "#/alpha/flow?stage=gather");
    await drawn("A");
    const head = document.querySelector<HTMLElement>(".flow-head");
    expect(head).not.toBeNull();
    expect(screen.getAllByRole("heading", { level: 1 }).map((heading) => heading.textContent)).toEqual(["Flow"]);
    expect(head?.querySelector(".flow-head-title")?.textContent).toBe("The guild · gather");
    expect(within(head as HTMLElement).getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Canvas", "List"]);
    const about = within(head as HTMLElement).getByRole("button", { name: "About and notes (2)" });
    expect(about.getAttribute("aria-expanded")).toBe("false");
    const body = document.getElementById(about.getAttribute("aria-controls") ?? "");
    expect(body?.hidden).toBe(true);
    expect(screen.getAllByText("a note from the daemon")).toHaveLength(1);
    expect(body?.textContent).toContain("The spec is read only here.");
    // M15: the about text inline: it stands once, in the disclosure alone.
    expect(document.querySelectorAll(".view-about")).toHaveLength(0);
    const abouts = screen.getAllByText((_text, element) => element?.tagName === "P" && element.textContent.includes("The spec is read only here."), { ignore: false });
    expect(abouts).toHaveLength(1);
    expect(body?.contains(abouts[0] ?? null)).toBe(true);
    fireEvent.click(about);
    expect(about.getAttribute("aria-expanded")).toBe("true");
    expect(body?.hidden).toBe(false);
  });
});

describe("the legend (ST-8)", () => {
  it("opens over the canvas; the first Esc closes it, focus to its toggle; the second clears the selection (M16)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    fireEvent.click(await drawn("A"));
    const toggle = screen.getByRole("button", { name: "Legend" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    const legend = screen.getByRole("region", { name: "Legend" });
    for (const word of [TYPE_A, TYPE_B, "alternative", "parallel", "repeat"]) {
      expect(within(legend).getByText(word)).toBeTruthy();
    }
    expect(within(legend).getByText("Drawn with React Flow (MIT)")).toBeTruthy();
    // F-6 of handoff-progress: the walk left to right, the arcs under the boxes.
    expect(within(legend).getByText(enT("flow.legend.about")).textContent).toMatch(/^Left to right/);
    fireEvent.keyDown(toggle, { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Legend" })).toBeNull();
    expect(document.activeElement).toBe(toggle);
    expect(window.location.hash).toBe("#/alpha/flow/A");
    fireEvent.keyDown(toggle, { key: "Escape" });
    expect(window.location.hash).toBe("#/alpha/flow");
  });

  // docs/features/room-and-scroll.md "Acceptance criteria", RS-7: after the arc's row, a selection's lines, in words.
  it("lists the selection's lines after the arc's row: a sample in the accent at its width, and words that say it (RS-7)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    fireEvent.click(screen.getByRole("button", { name: "Legend" }));
    const legend = screen.getByRole("region", { name: "Legend" });
    const arc = within(legend).getByText(enT("flow.legend.arc")).closest("li");
    const row = arc?.nextElementSibling;
    expect(row?.textContent).toBe(enT("flow.legend.selected"));
    const line = row?.querySelector<SVGLineElement>("line");
    // M: the sample at `var(--graph-edge)`.
    expect([line?.style.stroke, line?.style.strokeWidth]).toEqual(["var(--graph-edge-emphasis)", "var(--graph-edge-width-emphasis)"]);
  });

  it("closes on a focus in the canvas: nothing focused lies under it (R10)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    fireEvent.click(screen.getByRole("button", { name: "Legend" }));
    expect(screen.getByRole("region", { name: "Legend" })).toBeTruthy();
    act(() => {
      box("A").focus();
    });
    expect(screen.queryByRole("region", { name: "Legend" })).toBeNull();
  });
});

describe("the List (ST-9, cut 2c)", () => {
  async function list(hash: string, studio: StudioReads = reads()) {
    renderApp(studioClient(studio), hash);
    await screen.findByRole("tab", { name: "List" });
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    return screen.findByRole("region", { name: "Every box and arrow of the view" });
  }

  it("holds every node and arrow of the read, each row a link selecting it, the scope's own arrows first; no row says path:line", async () => {
    const outline = await list("#/alpha/flow");
    await waitFor(() => {
      expect(outline.querySelectorAll("[data-flow-row]")).toHaveLength(5);
    });
    expect(Array.from(outline.querySelectorAll("[data-flow-edge]"))).toHaveLength(EDGES.length);
    const first = outline.querySelector("[data-flow-edge] a");
    expect(first?.textContent).toBe(`to Dye, ${TYPE_B}`);
    const row = outline.querySelector<HTMLElement>("[data-flow-row='C']");
    expect(row?.querySelector("a")?.getAttribute("href")).toBe("#/alpha/flow/C");
    expect(within(row as HTMLElement).getByRole("link", { name: "Open Craft" }).getAttribute("href")).toBe("#/alpha/flow?scope=C");
    expect(within(row as HTMLElement).getByText("2 documents")).toBeTruthy();
    expect(within(row as HTMLElement).getByRole("link", { name: `to Engrave, ${TYPE_A}, \u00d73` }).getAttribute("href")).toBe("#/alpha/flow?edge=C,E,alpha_link");
    expect(outline.textContent).not.toMatch(/\.md:\d/);
    const before = window.history.length;
    fireEvent.click(within(row as HTMLElement).getByRole("link", { name: `to Engrave, ${TYPE_A}, \u00d73` }));
    expect(window.location.hash).toBe("#/alpha/flow?edge=C,E,alpha_link");
    expect(window.history.length).toBe(before);
    expect(fact("Where")).toBe("docs/spec/e.md:14");
  });

  it("in a stage view, stands each box under its group", async () => {
    const outline = await list("#/alpha/flow?stage=gather");
    await waitFor(() => {
      expect(Array.from(outline.querySelectorAll("[data-flow-row]"), (row) => row.getAttribute("data-flow-row"))).toEqual(["A", "B"]);
    });
    expect(within(outline).getByRole("heading", { level: 2, name: "The guild" })).toBeTruthy();
    expect(within(outline).getByRole("link", { name: `to Bind, ${TYPE_A}` })).toBeTruthy();
  });

  it("holds every box past the cap; the canvas draws 200 and the bar counts the rest (R6)", async () => {
    const many = Array.from({ length: 205 }, (_, index) => node({ id: `S-${String(index).padStart(3, "0")}`, distance: index % 12 }));
    const studio = reads({ scopes: { G: aFlowView([aFlowNode({ id: "G", distance: null }), ...many], [], { root: "G", depth: 1 }) } });
    renderApp(studioClient(studio), "#/alpha/flow");
    await drawn("S-000");
    await waitFor(() => {
      expect(boxes()).toHaveLength(200);
    });
    expect(screen.getByText("5 of 205 boxes not drawn: choose a stage, enter a group, or read the List")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const outline = await screen.findByRole("region", { name: "Every box and arrow of the view" });
    expect(outline.querySelectorAll("[data-flow-row]")).toHaveLength(205);
  });

  it("takes the focus a group's Open took away, on its region, never the body (R9)", async () => {
    const outline = await list("#/alpha/flow");
    const open = await within(outline).findByRole("link", { name: "Open Craft" });
    act(() => {
      open.focus();
    });
    fireEvent.click(open);
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow?scope=C");
    });
    await waitFor(() => {
      expect(outline.querySelector("[data-flow-row='X']")).not.toBeNull();
    });
    expect(document.activeElement).toBe(screen.getByRole("region", { name: "Every box and arrow of the view" }));
  });

  it("comes first under 48rem unless another tab is remembered", async () => {
    stubWidth(true, true);
    renderApp(studioClient(reads()), "#/alpha/flow");
    expect((await screen.findByRole("tab", { name: "List" })).getAttribute("aria-selected")).toBe("true");
  });
});

describe("landmarks and keys (ST-10)", () => {
  it("names the places a nav apart from the app's, its items links to places; the canvas a region, the panel complementary, one h1", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    expect(screen.getAllByRole("navigation").map((nav) => nav.getAttribute("aria-label"))).toEqual(["Sections", "Flow places"]);
    const items = within(places()).getAllByRole("link");
    expect(items.length).toBeGreaterThan(4);
    expect(items.every((item) => (item.getAttribute("href") ?? "").startsWith("#/alpha/flow"))).toBe(true);
    // M21: no place a button with a handler; the one button there opens Records for the AI.
    expect(within(places()).queryAllByRole("button").map((button) => button.textContent)).toEqual([]);
    expect(screen.getByRole("region", { name: "Flow canvas" })).toBeTruthy();
    expect(screen.getAllByRole("complementary", { name: "Details" })).toHaveLength(1);
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
  });

  it("holds one tab stop on the canvas; arrows move, Enter selects with focus to the panel, Esc clears with focus back", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    const region = screen.getByRole("region", { name: "Flow canvas" });
    expect(Array.from(region.querySelectorAll("[tabindex='0']"))).toHaveLength(1);
    expect(box("A").getAttribute("tabindex")).toBe("0");
    act(() => {
      box("A").focus();
    });
    // F-5 of handoff-progress: Up stays within A's column, which holds A alone; Right goes to the next column's nearest box.
    fireEvent.keyDown(box("A"), { key: "ArrowUp" });
    expect(document.activeElement).toBe(box("A"));
    fireEvent.keyDown(box("A"), { key: "ArrowRight" });
    expect(document.activeElement).toBe(box("B"));
    await waitFor(() => {
      expect(box("B").getAttribute("tabindex")).toBe("0");
    });
    expect(box("A").getAttribute("tabindex")).toBe("-1");
    expect(Array.from(region.querySelectorAll("[tabindex='0']"))).toHaveLength(1);
    fireEvent.keyDown(box("B"), { key: "Enter" });
    expect(window.location.hash).toBe("#/alpha/flow/B");
    await waitFor(() => {
      expect(document.activeElement?.textContent).toBe("Bind");
    });
    expect(document.activeElement?.tagName).toBe("H2");
    fireEvent.keyDown(document.activeElement as HTMLElement, { key: "Escape" });
    expect(window.location.hash).toBe("#/alpha/flow");
    expect(document.activeElement).toBe(box("B"));
  });

  it("enters a group with Space, a push, focus to the new picture's first box", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("C");
    const before = window.history.length;
    act(() => {
      box("C").focus();
    });
    fireEvent.keyDown(box("C"), { key: " " });
    expect(window.location.hash).toBe("#/alpha/flow?scope=C");
    expect(window.history.length).toBe(before + 1);
    await drawn("X");
    await waitFor(() => {
      expect(document.activeElement).toBe(box("X"));
    });
  });

  it("centres with `c` under a Russian layout, no route, no focus moved; ? lists Space", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    act(() => {
      box("A").focus();
    });
    fireEvent.keyDown(box("A"), { key: "\u0441", code: "KeyC" });
    expect(window.location.hash).toBe("#/alpha/flow");
    expect(document.activeElement).toBe(box("A"));
    fireEvent.keyDown(box("A"), { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText(enT("app.shortcuts.flow.space"))).toBeTruthy();
  });

  it("double-clicks a group to enter it; a click on a step selects it", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    fireEvent.click(await drawn("B"));
    expect(window.location.hash).toBe("#/alpha/flow/B");
    fireEvent.doubleClick(box("C"));
    expect(window.location.hash).toBe("#/alpha/flow?scope=C");
  });
});

// docs/features/handoff-progress.md "Acceptance criteria", C-1, C-3, C-4: no click lost on Flow.
describe("clicks on Flow (C-1, C-3, C-4)", () => {
  it("takes a press that moves 3 px for a click, box after box; one that moves 8 px pans and selects nothing (C-1)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    // M: `paneClickDistance` removed; at React Flow's 1 px the 3 px press selects nothing.
    pressAndMove(box("B"), 3, 1);
    expect(window.location.hash).toBe("#/alpha/flow/B");
    await waitFor(() => {
      expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Bind");
    });
    const before = window.history.length;
    pressAndMove(box("E"), 3, 1);
    expect(window.location.hash).toBe("#/alpha/flow/E");
    expect(window.history.length).toBe(before);
    await waitFor(() => {
      expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Engrave");
    });
    expect(box("B").getAttribute("aria-label")).not.toMatch(/selected$/);
    expect(box("E").getAttribute("aria-label")).toMatch(/selected$/);
    const still = viewOf();
    pressAndMove(box("A"), 8, 0);
    expect(window.location.hash).toBe("#/alpha/flow/E");
    expect(viewOf()).not.toBe(still);
  });

  it("moves nothing on a focus that is not :focus-visible, a mouse's, a touch's or a pen's; a keyboard focus on a box out of view centres it (C-3)", async () => {
    const restore = stubCanvas(400, 500);
    // jsdom never matches :focus-visible; a browser does for a focus that came from the keyboard.
    const focus = stubFocusVisible();
    try {
      renderApp(studioClient(reads()), "#/alpha/flow");
      await drawn("E");
      await waitFor(() => {
        expect(viewOf()).toMatch(/scale\(0\.92\)/);
      });
      const view = viewOf();
      // B, the second column, stands across the canvas's right edge at the fit.
      const target = box("B");
      // A mouse press: pointerdown, the focus it brings, pointerup, click. M: centring on every focus.
      pointer(target, "pointerdown");
      act(() => {
        target.focus();
      });
      pointer(target, "pointerup");
      mouse(target, "click", 100, 100);
      expect(window.location.hash).toBe("#/alpha/flow/B");
      expect(viewOf()).toBe(view);
      // A touch tap focuses after its pointerup: still not :focus-visible, still nothing moves.
      pointer(box("E"), "pointerup", "touch");
      act(() => {
        box("E").focus();
      });
      expect(viewOf()).toBe(view);
      // Tab to A, then Right to B: a keyboard focus, out of view, so centred as before.
      focus.keyboard = true;
      act(() => {
        box("A").focus();
      });
      fireEvent.keyDown(box("A"), { key: "ArrowRight" });
      expect(document.activeElement).toBe(box("B"));
      await waitFor(() => {
        expect(viewOf()).not.toBe(view);
      });
    } finally {
      focus.restore();
      restore();
    }
  });

  it("lets no divider and no band caption take a pointer; every box takes it (C-4)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    const words = Array.from(document.querySelectorAll<HTMLElement>(".react-flow__node-divider, .react-flow__node-band"));
    expect(document.querySelectorAll(".react-flow__node-divider").length).toBeGreaterThan(0);
    expect(document.querySelectorAll(".react-flow__node-band")).toHaveLength(1);
    // M: WORDS_ONLY without its style.
    expect(words.map((element) => element.style.pointerEvents)).toEqual(words.map(() => "none"));
    const shown = boxes();
    expect(shown.map((element) => element.style.pointerEvents)).toEqual(shown.map(() => "all"));
  });
});

describe("narrow widths (ST-11)", () => {
  it("under 64rem the wheel scrolls the page; above it the wheel pans (R11)", async () => {
    stubWidth(true);
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    expect(document.querySelector(".flow-canvas")?.getAttribute("data-wheel")).toBe("scrolls");
  });

  it("pans with the wheel where the panes stand side by side", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    expect(document.querySelector(".flow-canvas")?.getAttribute("data-wheel")).toBe("pans");
  });

  it("folds the places into a disclosure, closed, aria-expanded following", async () => {
    stubWidth(true);
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    const toggle = screen.getByRole("button", { name: "Places" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(document.getElementById(toggle.getAttribute("aria-controls") ?? "")).toBe(places());
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
  });
});

describe("the places pane", () => {
  it("lists All stages, then the lanes as served with their scores, No stage last; under the path's current entry, the scope's groups (D11)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    const pane = within(places());
    await pane.findByRole("link", { name: /^gather/ });
    const stages = Array.from(places().querySelectorAll(".flow-places-stages > li > a"), (link) => [link.textContent, link.getAttribute("href"), link.getAttribute("aria-current")]);
    expect(stages).toEqual([
      ["All stages", "#/alpha/flow", "true"],
      ["gather1 of 2 confirmed", "#/alpha/flow?stage=gather", null],
      ["print0 of 1 confirmed", "#/alpha/flow?stage=print", null],
      ["No stage0 of 1 confirmed", "#/alpha/flow?stage=-", null],
    ]);
    const groups = pane.getByRole("list", { name: "Groups here" });
    expect(groups.textContent).toBe("Craft");
    const path = pane.getByRole("list", { name: "Path" });
    expect(path.compareDocumentPosition(groups) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(pane.getByRole("link", { name: "The guild" }).getAttribute("aria-current")).toBe("location");
    expect(pane.queryByText(/^Up:/)).toBeNull();
  });

  it("replaces when a place link differs from the place shown by its selection alone (D5); a stage's link pushes", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow/A");
    await drawn("A");
    const replace = vi.spyOn(window.history, "replaceState");
    fireEvent.click(within(places()).getByRole("link", { name: "The guild" }));
    expect(window.location.hash).toBe("#/alpha/flow");
    expect(replace.mock.calls.map((call) => call[2])).toEqual(["#/alpha/flow"]);
    replace.mockClear();
    fireEvent.click(await within(places()).findByRole("link", { name: /^gather/ }));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow?stage=gather");
    });
    expect(replace.mock.calls.map((call) => call[2])).not.toContain("#/alpha/flow?stage=gather");
  });

  it("writes no lens root as a scope: a seek whose lens root was not read reads the top to know it (R3)", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow/B?scope=C");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/B");
    });
    await drawn("B");
    expect(client.getTree).toHaveBeenCalledTimes(1);
    expect(client.getFlow.mock.calls.map((call) => call[1])).toContainEqual({ depth: 0, in_flow: true, trace: true });
  });

  it("says a failed summary in the daemon's words with Retry; the canvas stands (R8)", async () => {
    const client = studioClient(reads());
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "spec: the queue is locked" }));
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    expect(await within(places()).findByText("Stages could not be read.", { exact: false })).toBeTruthy();
    expect(within(places()).getByText("spec: the queue is locked")).toBeTruthy();
    expect(within(places()).getByRole("button", { name: "Retry" })).toBeTruthy();
  });
});

describe("states (R5, R7, R8)", () => {
  it("a failed drawing read: the daemon's words verbatim and Retry in the canvas; the rows wait", async () => {
    const client = studioClient(reads());
    client.getFlow.mockImplementation((_project, options) =>
      options.root === undefined ? Promise.resolve(reads().top) : Promise.reject(new ClientError({ status: 503, message: "spec index unavailable: the database is locked" })),
    );
    renderApp(client, "#/alpha/flow");
    const region = await screen.findByRole("region", { name: "Flow canvas" });
    expect(await within(region).findByText("spec index unavailable: the database is locked")).toBeTruthy();
    expect(within(region).getByRole("button", { name: "Retry" })).toBeTruthy();
  });

  it("a failed re-read keeps the picture, says no state it cannot vouch for, rows Not read; Retry brings them back (R8)", async () => {
    const client = studioClient(reads());
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    expect(box("A").getAttribute("aria-label")).toContain("Confirmed");
    const served = client.getFlow.getMockImplementation();
    client.getFlow.mockImplementation((project, options, signal) => (options.root === "G" ? Promise.reject(new ClientError({ status: 503, message: "spec: index locked" })) : (served?.(project, options, signal) ?? Promise.reject(new Error("no stub")))));
    act(() => {
      emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1", commit: "abc" } });
    });
    expect(await screen.findByText(/could not be read again: spec: index locked/)).toBeTruthy();
    expect(screen.getByText(/The boxes show no state until it reads\./)).toBeTruthy();
    expect(box("A").getAttribute("aria-label")).not.toContain("Confirmed");
    expect(within(gapsToggle()).getByText("Not read")).toBeTruthy();
    expect(within(openGaps()).getAllByText("Not read")).toHaveLength(10);
    client.getFlow.mockImplementation(served ?? (() => Promise.reject(new Error("no stub"))));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(screen.queryByText(/could not be read again/)).toBeNull();
    });
    expect(box("A").getAttribute("aria-label")).toContain("Confirmed");
  });

  it("a scope naming nothing: the read's reason in the canvas", async () => {
    const studio = reads({ scopes: { G: scopeOfG(), NOPE: aFlowView([], [], { root: "NOPE", reason: "no node `NOPE`", gaps: [] }) } });
    renderApp(studioClient(studio), "#/alpha/flow?scope=NOPE");
    expect(await screen.findByText("no node `NOPE`")).toBeTruthy();
  });

  it("a scope holding nothing, a stage of no node, `-` with every step staged: each in words, the rows standing", async () => {
    const studio = reads({ scopes: { G: scopeOfG(), D: aFlowView([aFlowNode({ id: "D", distance: null })], [], { root: "D", depth: 1 }) } });
    renderApp(studioClient(studio), "#/alpha/flow?scope=D");
    expect(await screen.findByRole("heading", { level: 2, name: "Nothing is inside Title of D." })).toBeTruthy();
    expect(gapsToggle()).toBeTruthy();
    act(() => {
      window.location.hash = "#/alpha/flow?stage=nowhere";
    });
    expect(await screen.findByRole("heading", { level: 2, name: "No step here carries nowhere." })).toBeTruthy();
    act(() => {
      window.location.hash = "#/alpha/flow?stage=-";
    });
    expect(await screen.findByRole("heading", { level: 2, name: "Every step here has a stage." })).toBeTruthy();
  });

  it("no [flow]: rows Not configured with their cause verbatim, the canvas drawn", async () => {
    const rows = tenGapRows([aFlowGap("no-start", [], { state: "not_configured", count: null, cause: "no [[flow.start]] rule: add one naming the first step by paths" })]);
    renderApp(studioClient(reads({ scopes: { G: scopeOfG({ gaps: rows }) } })), "#/alpha/flow");
    await drawn("A");
    const control = within(openGaps());
    expect(control.getByText("Not configured")).toBeTruthy();
    expect(control.getByText("no [[flow.start]] rule: add one naming the first step by paths").getAttribute("lang")).toBe("en");
    expect(control.getByText("1 row needs [flow] rules in specengine.toml")).toBeTruthy();
  });
});

describe("gap items and the hand-off (D11, cut 2c), from the Gaps control (flow-scenario D10)", () => {
  it("activating a drawn item closes the control, selects its first member by a replace, marks every drawn member, focus to the panel heading", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    const before = window.history.length;
    fireEvent.click(within(openGaps()).getByRole("button", { name: "Bind + Engrave" }));
    expect(window.location.hash).toBe("#/alpha/flow/B");
    expect(window.history.length).toBe(before);
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
    await waitFor(() => {
      expect(document.activeElement?.textContent).toBe("Bind");
    });
    expect(box("B").getAttribute("aria-label")).toContain("marked");
    expect(box("E").getAttribute("aria-label")).toContain("marked");
  });

  it("activating a member not drawn seeks it, the fix a push", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    const before = window.history.length;
    fireEvent.click(await within(openGaps()).findByRole("button", { name: "Title of X" }));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/X?scope=C");
    });
    expect(window.history.length).toBe(before + 1);
    expect(client.getTree).toHaveBeenCalledTimes(1);
  });

  it("hands a measured item to the AI as a card: its members the targets in order, the cause the goal; one createCard, no task (ADR-0012)", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    const item = within(openGaps()).getByRole("button", { name: "Bind + Engrave" }).closest("li");
    fireEvent.click(within(item as HTMLElement).getByRole("button", { name: /^Hand to your AI/ }));
    const dialog = await screen.findByRole("dialog", { name: "New task" });
    expect(within(dialog).getByLabelText<HTMLInputElement>("Title").value).toBe("Gap cycle-no-repeat: B + E");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Goal").value).toBe("a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source", { exact: false }).value).toBe("B\nE");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ title: "Gap cycle-no-repeat: B + E", target: ["B", "E"], goal: "a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}" });
    expect(await within(panel()).findByText("C-0001 created.")).toBeTruthy();
    expect(client.getTasks).not.toHaveBeenCalled();
  });

  it("moves focus to the first item Show all brings, never the body", async () => {
    const many = Array.from({ length: 10 }, (_, index) => [`U-${String(index)}`]);
    renderApp(studioClient(reads({ scopes: { G: scopeOfG({ gaps: tenGapRows([aFlowGap("unreachable", many)]) }), C: SCOPE_C } })), "#/alpha/flow");
    await drawn("A");
    const control = openGaps();
    fireEvent.click(within(control).getByRole("button", { name: /^Show all 10/ }));
    await waitFor(() => {
      expect(document.activeElement?.textContent).toBe("U-8");
    });
  });

  it("hands `no-start`'s scope under a root; at the top its item hands nothing", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    const row = within(openGaps()).getByText("no-start").closest("li");
    fireEvent.click(within(row as HTMLElement).getByRole("button", { name: /^Hand to your AI/ }));
    const dialog = await screen.findByRole("dialog", { name: "New task" });
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source", { exact: false }).value).toBe("G");
  });
});

describe("a box on the canvas", () => {
  it("names its words, marks and gaps: Entry in a scope, End, a group's holdings and counts, its rollup", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    expect(box("A").getAttribute("aria-label")).toBe("Ask, gather, Entry, Confirmed");
    expect(box("E").getAttribute("aria-label")).toBe("Engrave, print, End, In work, named by a gap row");
    expect(box("C").getAttribute("aria-label")).toBe("Craft, group, 2 documents, 2 in work, 1 with gaps");
    expect(box("C").textContent).toContain("2 documents");
    expect(box("B").querySelector(".flow-box")?.classList.contains("is-hollow")).toBe(true);
    expect(screen.getByText("Not on the walk")).toBeTruthy();
  });
});

describe("forms (flow-canvas AC-06, kept)", () => {
  const edge = (fields: Partial<FlowEdge>) => aFlowEdge({ from: "A", to: "B", ...fields });

  it("keeps seven distinct dashes, one dotted, and that one for a repeat whatever types a project holds", () => {
    expect(new Set(FLOW_DASHES).size).toBe(7);
    const dotted = (dash: string) => dash !== "none" && Number(dash.split(" ")[0]) < 3;
    expect(FLOW_DASHES.filter(dotted)).toHaveLength(1);
    for (const types of [[], [TYPE_A], [TYPE_A, TYPE_B], ["a", "b", "c", "d"]]) {
      for (const type of [TYPE_A, ...types]) {
        expect(dotted(dashOf(edge({ type, kind: "repeat" }), types))).toBe(true);
      }
    }
  });

  it("takes the served types in their order, whatever the read's edges hold; a type the order lacks takes the first colour (D3)", () => {
    const folded = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 3, inner: [{ type: "zebra", count: 2 }] })], [aFlowEdge({ from: "G", to: "G", type: "yak" })], { types: ["zebra", "yak"] });
    expect(typesOf(folded)).toEqual(["zebra", "yak"]);
    expect(typeColour("yak", typesOf(folded))).toBe("var(--flow-edge-1)");
    expect(typeColour("nobody", typesOf(folded))).toBe("var(--flow-edge-0)");
  });

  it("says an edge's kind and its when clipped, never its count; a plain edge says nothing (D9)", () => {
    expect(edgeWords(edge({ kind: "alternative", when: "after dark" }))).toEqual(["alternative", "after dark"]);
    expect(edgeWords(edge({ count: 3 }))).toEqual([]);
    expect(edgeWords(edge({}))).toEqual([]);
    const long = String.fromCodePoint(0x437).repeat(WHEN_SHOWN + 5);
    expect(Array.from(shortWhen(long))).toHaveLength(WHEN_SHOWN + 1);
  });

  it("dots a repeat on the canvas, heavier, in its type's colour", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__edge-path").length).toBeGreaterThan(3);
    });
    const paths = Array.from(document.querySelectorAll<SVGPathElement>(".react-flow__edge-path"));
    expect(paths.filter((path) => path.style.strokeDasharray === "1 7")).toHaveLength(1);
    expect(paths.filter((path) => path.style.strokeWidth === "var(--flow-repeat-width)")).toHaveLength(1);
  });
});

// docs/features/room-and-scroll.md "Acceptance criteria", RS-4 and RS-5, on G's children: A->B, B->C
// (alternative), C->E, E->B (a repeat), and D->A off the walk. A selection lights its lines inline, so no
// stylesheet rule can lose to a type's inline colour; the rest fade while any is lit; boxes never change
// but for the selected one's own mark; the lit lines paint last.
describe("the selection's lines (RS-4, RS-5)", () => {
  const AB = `From Ask to Bind, ${TYPE_A}`;
  const BC = `From Bind to Craft, ${TYPE_A}, alternative, the ink is dry`;
  const CE = `From Craft to Engrave, ${TYPE_A}`;
  const EB = `From Engrave to Bind, ${TYPE_A}, repeat`;
  const DA = `From Dye to Ask, ${TYPE_B}`;
  const EMPHASIS = "var(--graph-edge-emphasis)";
  const EMPHASIS_WIDTH = "var(--graph-edge-width-emphasis)";
  const types = typesOf(scopeOfG());

  function pathOf(label: string): SVGPathElement {
    const path = edgeNamed(label).querySelector<SVGPathElement>(".react-flow__edge-path");
    if (path === null) {
      throw new Error(`no path for ${label}`);
    }
    return path;
  }

  /** The arrowhead the path's `marker-end` names. */
  function headOf(path: SVGPathElement): SVGPolylineElement {
    const id = /^url\('#(.+)'\)$/.exec(path.getAttribute("marker-end") ?? "")?.[1];
    const head = id === undefined ? null : (document.getElementById(id)?.querySelector<SVGPolylineElement>("polyline") ?? null);
    if (head === null) {
      throw new Error(`no arrowhead for ${path.getAttribute("marker-end") ?? "a path without one"}`);
    }
    return head;
  }

  /** A line's stroke, width, and its arrowhead's stroke and fill, inline. */
  function look(label: string): string[] {
    const path = pathOf(label);
    const head = headOf(path);
    return [label, path.style.stroke, path.style.strokeWidth, head.style.stroke, head.style.fill];
  }

  function labelOf(words: string): HTMLElement {
    const found = Array.from(document.querySelectorAll<HTMLElement>(".flow-edge-label")).find((element) => element.textContent === words);
    if (found === undefined) {
      throw new Error(`no label ${words}`);
    }
    return found;
  }

  function drawnEdges(): string[] {
    return Array.from(document.querySelectorAll(".react-flow__edge"), (edge) => edge.getAttribute("aria-label") ?? "");
  }

  function withClass(name: string): string[] {
    return Array.from(document.querySelectorAll(`.react-flow__edge.${name}`), (edge) => edge.getAttribute("aria-label") ?? "").sort();
  }

  /** Each node of the canvas, a box, the band's caption or a divider: its class and inline style, and its content's. */
  function nodeLooks(): string[][] {
    return Array.from(document.querySelectorAll<HTMLElement>(".react-flow__node"), (node) => {
      const content = node.firstElementChild;
      return [node.dataset.id ?? "", node.className, node.getAttribute("style") ?? "", content?.className ?? "", content?.getAttribute("style") ?? ""];
    });
  }

  async function canvasDrawn() {
    await drawn("E");
    await waitFor(() => {
      expect(drawnEdges()).toHaveLength(4);
    });
  }

  it("lights a box's lines inline, B clicked: the accent on stroke, width and arrowhead, a repeat's width too, dashes kept; the rest keep their type's look (RS-4)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await canvasDrawn();
    fireEvent.click(box("B"));
    expect(window.location.hash).toBe("#/alpha/flow/B");
    await waitFor(() => {
      expect(withClass("is-emphasis")).toEqual([AB, BC, EB].sort());
    });
    // M: edgeOf leaving the type's colour inline (the CSS-only fix); the marker keeping it; a repeat keeping its width.
    for (const label of [AB, BC, EB]) {
      expect(look(label)).toEqual([label, EMPHASIS, EMPHASIS_WIDTH, EMPHASIS, EMPHASIS]);
    }
    expect(pathOf(EB).style.strokeDasharray).toBe("1 7");
    expect(pathOf(BC).style.strokeDasharray).toBe(dashOf(aFlowEdge({ from: "B", to: "C", type: TYPE_A, kind: "alternative" }), types));
    expect(pathOf(BC).style.strokeDasharray).not.toBe("none");
    const colour = typeColour(TYPE_A, types);
    expect(look(CE)).toEqual([CE, colour, "", colour, colour]);
    // D->A has neither end selected: off the walk, it stays undrawn.
    expect(drawnEdges()).not.toContain(DA);
    // The lit lines paint last: every one after every other line in DOM order.
    const order = drawnEdges();
    expect(order.slice(order.length - 3).sort()).toEqual([AB, BC, EB].sort());
    expect(order.slice(0, order.length - 3)).toEqual([CE]);
  });

  it("lights an off-walk line with its selected end, A selected; and one arrow alone, C->E from the List (RS-4)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await canvasDrawn();
    fireEvent.click(box("A"));
    await waitFor(() => {
      expect(withClass("is-emphasis")).toEqual([AB, DA].sort());
    });
    expect(look(DA)).toEqual([DA, EMPHASIS, EMPHASIS_WIDTH, EMPHASIS, EMPHASIS]);
    expect(drawnEdges().slice(-2).sort()).toEqual([AB, DA].sort());

    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const outline = await screen.findByRole("region", { name: "Every box and arrow of the view" });
    const row = outline.querySelector<HTMLElement>("[data-flow-row='C']");
    fireEvent.click(within(row as HTMLElement).getByRole("link", { name: `to Engrave, ${TYPE_A}, \u00d73` }));
    expect(window.location.hash).toBe("#/alpha/flow?edge=C,E,alpha_link");
    fireEvent.click(screen.getByRole("tab", { name: "Canvas" }));
    await drawn("E");
    await waitFor(() => {
      expect(withClass("is-emphasis")).toEqual([CE]);
    });
    expect(look(CE)).toEqual([CE, EMPHASIS, EMPHASIS_WIDTH, EMPHASIS, EMPHASIS]);
    expect(drawnEdges().at(-1)).toBe(CE);
    expect(withClass("is-faded")).toEqual([AB, BC, EB].sort());
  });

  it("fades every other drawn line and its words while any is lit, A selected; B's alternative lit, its words too (RS-5)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await canvasDrawn();
    fireEvent.click(box("A"));
    await waitFor(() => {
      expect(withClass("is-faded")).toEqual([BC, CE, EB].sort());
    });
    expect(withClass("is-emphasis")).toEqual([AB, DA].sort());
    for (const words of ["alternativethe ink is dry", "repeat"]) {
      expect([words, labelOf(words).classList.contains("is-faded"), labelOf(words).classList.contains("is-emphasis")]).toEqual([words, true, false]);
    }
    // A faded line keeps its type's colour and arrowhead; only its group's opacity, from the stylesheet, fades it.
    expect(look(CE)).toEqual([CE, typeColour(TYPE_A, types), "", typeColour(TYPE_A, types), typeColour(TYPE_A, types)]);
    fireEvent.click(box("B"));
    await waitFor(() => {
      expect(labelOf("alternativethe ink is dry").classList.contains("is-emphasis")).toBe(true);
    });
    expect(labelOf("alternativethe ink is dry").classList.contains("is-faded")).toBe(false);
  });

  it("changes no box, band caption or divider but the selected box's own mark; after Esc nothing is faded or lit (RS-5)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await canvasDrawn();
    expect(document.querySelectorAll(".react-flow__node-divider").length).toBeGreaterThan(0);
    expect(document.querySelectorAll(".react-flow__node-band")).toHaveLength(1);
    const before = nodeLooks();
    expect(before.length).toBeGreaterThan(6);
    fireEvent.click(box("A"));
    await waitFor(() => {
      expect(withClass("is-faded").length).toBeGreaterThan(0);
    });
    const expected = before.map(([id, node, style, content, contentStyle]) => [id, node, style, id === "n:A" ? `${content ?? ""} is-selected` : content, contentStyle]);
    expect(nodeLooks()).toEqual(expected);
    expect(within(box("A")).getByText("Selected")).toBeTruthy();
    expect(document.querySelectorAll(".react-flow__node.is-faded, .react-flow__node .is-faded")).toHaveLength(0);
    fireEvent.keyDown(box("A"), { key: "Escape" });
    expect(window.location.hash).toBe("#/alpha/flow");
    await waitFor(() => {
      expect(document.querySelectorAll(".is-faded")).toHaveLength(0);
    });
    expect(Array.from(document.querySelectorAll<SVGPathElement>(".react-flow__edge-path")).filter((path) => path.style.stroke === EMPHASIS || path.style.strokeWidth === EMPHASIS_WIDTH)).toEqual([]);
    expect(withClass("is-emphasis")).toEqual([]);
    expect(nodeLooks()).toEqual(before);
  });

  it("fades nothing when the selected box has no drawn line (RS-5)", async () => {
    const alone = aFlowView(
      [aFlowNode({ id: "G", distance: null, kind: "domain", title: "The guild" }), node({ id: "A", distance: 0, start: true, title: "Ask" }), node({ id: "B", distance: 1, title: "Bind" }), node({ id: "Z", distance: 0, path: "docs/spec/z.md", title: "Zest" })],
      [aFlowEdge({ from: "A", to: "B", type: TYPE_A })],
      { root: "G", depth: 1 },
    );
    renderApp(studioClient(reads({ scopes: { G: alone } })), "#/alpha/flow");
    await drawn("Z");
    await waitFor(() => {
      expect(drawnEdges()).toEqual([AB]);
    });
    fireEvent.click(box("Z"));
    expect(window.location.hash).toBe("#/alpha/flow/Z");
    await waitFor(() => {
      expect(within(box("Z")).getByText("Selected")).toBeTruthy();
    });
    // M: an edge faded whenever it is not lit.
    expect(document.querySelectorAll(".is-faded")).toHaveLength(0);
    expect(withClass("is-emphasis")).toEqual([]);
    expect(pathOf(AB).style.stroke).toBe(typeColour(TYPE_A, typesOf(alone)));
  });
});

// flow-scenario FS-13, FS-16 to FS-21 (RM-0094) over the stub's `flow-aside`: the reads `spec serve`
// gives on the fixture, written down in src/test/asideStub.ts, the flags the daemon's.
describe("the scenario on flow-aside (RM-0094)", () => {
  const LENS_ROOT = `#/${ASIDE}/flow`;

  /** Flow's own reads of the stub, by method. */
  function flowCalls(client: ReturnType<typeof asideClient>) {
    return {
      flow: client.getFlow.mock.calls.map((call) => call[1]),
      node: client.getNode.mock.calls.map((call) => [call[1], call[2]]),
      trace: client.getTrace.mock.calls.map((call) => call[1]),
      tree: client.getTree.mock.calls.length,
    };
  }

  function drawnEdges(): number {
    return document.querySelectorAll(".react-flow__edge").length;
  }

  function pathEntries(): [string, string | null, string | null][] {
    return Array.from(within(within(places()).getByRole("list", { name: "Path" })).getAllByRole("link"), (link) => [link.textContent, link.getAttribute("href"), link.getAttribute("aria-current")]);
  }

  function recordsButton(): HTMLElement {
    return within(places()).getByRole("button", { name: /^Records for the AI/ });
  }

  describe("the reads (FS-13)", () => {
    it("on a cold deep mount reads the scope's cut form, its node and its lanes alone: no top, no tree, no ancestor", async () => {
      const client = asideClient();
      renderApp(client, `#/${ASIDE}/flow?scope=CONE-GLOST`);
      await waitFor(() => {
        expect(pathEntries().map(([text]) => text)).toEqual(["The pottery", "Kiln bay", "Glost cone"]);
      });
      await screen.findByRole("heading", { level: 2, name: "Nothing is inside Glost cone." });
      expect(flowCalls(client)).toEqual({
        flow: [{ root: "CONE-GLOST", depth: 1, in_flow: true, trace: true }],
        node: [["CONE-GLOST", { with: ["links"] }]],
        trace: [{ ref: "CONE-GLOST", summary: true }],
        tree: 0,
      });
    });

    it("cuts each drawing form: the top and the lens root, then a stage under its root", async () => {
      const client = asideClient();
      renderApp(client, LENS_ROOT);
      await drawn("KRUN-FIRING");
      act(() => {
        window.location.hash = `#/${ASIDE}/flow?scope=KRUN-FIRING&stage=glazing`;
      });
      await drawn("POT-FIRE");
      expect(flowCalls(client).flow).toEqual([
        { depth: 0, in_flow: true, trace: true },
        { root: "ATL-POTTERY", depth: 1, in_flow: true, trace: true },
        { root: "KRUN-FIRING", stage: "glazing", in_flow: true, trace: true },
      ]);
    });

    it("on aside40 draws the kiln bay's three boxes, its 5 679 records left out", async () => {
      renderApp(asideClient({ times: 40 }), `#/${ASIDE}/flow?scope=WBAY-KILN`);
      await drawn("CONE-GLOST");
      expect(drawnNames().sort()).toEqual(["CONE-BISQUE", "CONE-GLOST", "HUNCH-0007"]);
      expect(recordsButton().textContent).toBe("Records for the AI (5679)");
    });
  });

  describe("the canvas (FS-16)", () => {
    it("draws no edge off the walk unselected: the run's nine to its parts show with it selected, the kiln's one with the kiln, none after Esc", async () => {
      renderApp(asideClient(), LENS_ROOT);
      await drawn("WBAY-KILN");
      expect(drawnEdges()).toBe(0);
      fireEvent.click(box("KRUN-FIRING"));
      await waitFor(() => {
        expect(drawnEdges()).toBe(9);
      });
      fireEvent.click(box("WBAY-KILN"));
      await waitFor(() => {
        expect(drawnEdges()).toBe(1);
      });
      fireEvent.keyDown(box("WBAY-KILN"), { key: "Escape" });
      await waitFor(() => {
        expect(drawnEdges()).toBe(0);
      });
    });

    it("draws the firing run's eleven edges unselected, none labelled with a count; the List keeps the kiln's ×5", async () => {
      renderApp(asideClient(), `#/${ASIDE}/flow?scope=KRUN-FIRING`);
      await drawn("POT-PACK");
      await waitFor(() => {
        expect(drawnEdges()).toBe(11);
      });
      const labels = Array.from(document.querySelectorAll(".flow-edge-label"), (label) => label.textContent);
      expect(labels.length).toBeGreaterThan(0);
      expect(labels.some((text) => /x\d/.test(text))).toBe(false);
      act(() => {
        window.location.hash = LENS_ROOT;
      });
      await drawn("WBAY-KILN");
      fireEvent.click(screen.getByRole("tab", { name: "List" }));
      const outline = await screen.findByRole("region", { name: "Every box and arrow of the view" });
      const row = await waitFor(() => {
        const found = outline.querySelector<HTMLElement>("[data-flow-row='KRUN-FIRING']");
        if (found === null) {
          throw new Error("no row");
        }
        return found;
      });
      expect(within(row).getByRole("link", { name: "to Kiln bay, uses, \u00d75" })).toBeTruthy();
    });
  });

  describe("the Gaps control (FS-17)", () => {
    it("stands after Legend in the bar, collapsed, a warning lozenge counting the rows found; the scope view lists no row", async () => {
      renderApp(asideClient(), LENS_ROOT);
      await drawn("WBAY-KILN");
      const bar = document.querySelector<HTMLElement>(".flow-bar");
      expect(within(bar as HTMLElement).getAllByRole("button").map((button) => button.textContent)).toEqual(["Legend", "Gaps 1 found"]);
      expect(gapsToggle().getAttribute("aria-expanded")).toBe("false");
      expect(gapsToggle().querySelector(".lozenge")?.getAttribute("data-appearance")).toBe("warning");
      expect(panel().querySelectorAll("li.flow-gap")).toHaveLength(0);
    });

    it("opens the rows found first in plain words, then those not measured, then one line for the rest; each code and cause folded, the cause in English", async () => {
      renderApp(asideClient(), LENS_ROOT);
      await drawn("WBAY-KILN");
      const control = openGaps();
      const rows = Array.from(control.querySelectorAll<HTMLElement>("li.flow-gap"));
      expect(rows.map((row) => row.querySelector(".flow-gap-meaning")?.textContent)).toEqual(["The flow starts in more than one place.", "Definitions nothing uses.", "Definitions used only once."]);
      expect(rows.slice(1).map((row) => within(row).getByText("Not configured").textContent)).toEqual(["Not configured", "Not configured"]);
      expect(within(control).getByText("7 rows find nothing")).toBeTruthy();
      for (const row of rows) {
        const details = within(row).getByRole("button", { name: "Technical details" });
        expect(details.getAttribute("aria-expanded")).toBe("false");
        const region = document.getElementById(details.getAttribute("aria-controls") ?? "");
        expect(region?.hidden).toBe(true);
        expect(region?.querySelector(".flow-gap-cause")?.getAttribute("lang")).toBe("en");
        // M: the code is the folded fact, never the row's first words.
        expect(row.querySelector(".flow-gap-meaning")?.textContent).not.toMatch(/^[a-z]+-[a-z-]+$/);
      }
      expect(within(rows[0] as HTMLElement).getByText("more than one start: narrow [[flow.start]] to one").closest("[hidden]")).not.toBeNull();
    });

    // DM-15 of docs/features/doc-modal.md (RM-0101): the members' titles are the read's own `titles`.
    it("names an item's members by the read's titles, reading no tree while open; activating it closes the control and seeks, focus to the panel heading", async () => {
      const client = asideClient();
      renderApp(client, LENS_ROOT);
      await drawn("WBAY-KILN");
      const item = await within(openGaps()).findByRole("button", { name: "Wedging + Open day" });
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
      });
      expect(client.getTree).not.toHaveBeenCalled();
      const before = window.history.length;
      fireEvent.click(item);
      expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
      await waitFor(() => {
        expect(window.location.hash).toBe(`#/${ASIDE}/flow/POT-WEDGE?scope=KRUN-FIRING`);
      });
      expect(window.history.length).toBe(before + 1);
      await waitFor(() => {
        expect(document.activeElement?.textContent).toBe("Wedging");
      });
      expect(document.activeElement?.tagName).toBe("H2");
    });

    it("names a member the read leaves untitled by its name, still reading no tree (DM-15)", async () => {
      const client = asideClient();
      const served = client.getFlow.getMockImplementation();
      client.getFlow.mockImplementation(async (project, options, signal) => {
        const view = await (served?.(project, options, signal) ?? Promise.resolve(asideLens()));
        return { ...view, gaps: view.gaps.map((row) => (row.code === "two-starts" ? { ...row, titles: [["Wedging", null]] } : row)) };
      });
      renderApp(client, LENS_ROOT);
      await drawn("WBAY-KILN");
      expect(await within(openGaps()).findByRole("button", { name: `Wedging + ${OPEN_DAY}` })).toBeTruthy();
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
      });
      expect(client.getTree).not.toHaveBeenCalled();
    });

    it("hands an item to the AI as one card, no task", async () => {
      const client = asideClient();
      renderApp(client, LENS_ROOT);
      await drawn("WBAY-KILN");
      const item = (await within(openGaps()).findByRole("button", { name: "Wedging + Open day" })).closest("li");
      fireEvent.click(within(item as HTMLElement).getByRole("button", { name: /^Hand to your AI/ }));
      const dialog = await screen.findByRole("dialog", { name: "New task" });
      expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source", { exact: false }).value).toBe(`POT-WEDGE\n${OPEN_DAY}`);
      fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
      await waitFor(() => {
        expect(client.createCard).toHaveBeenCalledTimes(1);
      });
      expect(client.getTasks).not.toHaveBeenCalled();
    });

    it("stands after its toggle in the tab order: Tab from the toggle goes into the open control", async () => {
      renderApp(asideClient(), LENS_ROOT);
      await drawn("WBAY-KILN");
      const control = openGaps();
      await within(control).findByRole("button", { name: "Wedging + Open day" });
      const studio = document.querySelector<HTMLElement>(".flow-studio");
      const stops = focusables(studio as HTMLElement);
      const next = stops[stops.indexOf(gapsToggle()) + 1];
      expect(next !== undefined && control.contains(next)).toBe(true);
      expect(next?.textContent).toBe("Wedging + Open day");
    });

    it("never stands open beside the legend; Esc closes it, focus to its toggle", async () => {
      renderApp(asideClient(), LENS_ROOT);
      await drawn("WBAY-KILN");
      fireEvent.click(screen.getByRole("button", { name: "Legend" }));
      openGaps();
      expect(screen.queryByRole("region", { name: "Legend" })).toBeNull();
      fireEvent.click(screen.getByRole("button", { name: "Legend" }));
      expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
      openGaps();
      fireEvent.keyDown(gapsToggle(), { key: "Escape" });
      expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
      expect(document.activeElement).toBe(gapsToggle());
    });
  });

  describe("Records for the AI (FS-18)", () => {
    it("at the kiln bay reads nothing until asked; then one uncut read, a dialog labelled by its h2, the records by kind, each a document link by title; Esc gives focus back", async () => {
      const client = asideClient();
      renderApp(client, `#/${ASIDE}/flow?scope=WBAY-KILN`);
      await drawn("CONE-GLOST");
      const button = recordsButton();
      expect(button.textContent).toBe("Records for the AI (141)");
      expect(flowCalls(client).flow).toEqual([{ root: "WBAY-KILN", depth: 1, in_flow: true, trace: true }]);
      fireEvent.click(button);
      const dialog = await screen.findByRole("dialog", { name: "Records for the AI (141)" });
      expect(dialog.getAttribute("aria-modal")).toBe("true");
      expect(within(dialog).getByRole("heading", { level: 2 }).textContent).toBe("Records for the AI (141)");
      await within(dialog).findByRole("group", { name: "hunch" });
      expect(flowCalls(client).flow.slice(1)).toEqual([{ root: "WBAY-KILN", depth: 1 }]);
      expect(["edict", "hunch", "puzzle"].map((kind) => within(dialog).getByRole("group", { name: kind }).querySelectorAll("li").length)).toEqual([32, 59, 50]);
      const first = within(within(dialog).getByRole("group", { name: "edict" })).getAllByRole("link")[0];
      // Each a document link by title (doc-modal DM-10): its page the href, a plain click the modal over the dialog.
      expect([first?.textContent, first?.getAttribute("href")]).toEqual(["Kiln edict no. 1", `#/${ASIDE}/tree/EDICT-0001/page`]);
      expect(within(dialog).queryByText("Glaze crawls on damp bisque")).toBeNull();
      fireEvent.keyDown(dialog, { key: "Escape" });
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(document.activeElement).toBe(recordsButton());
    });

    it("keeps Flow's keys out of the open dialog: a ? inside it opens no list of shortcuts over it; Esc still closes it", async () => {
      renderApp(asideClient(), `#/${ASIDE}/flow?scope=WBAY-KILN`);
      await drawn("CONE-GLOST");
      fireEvent.click(recordsButton());
      const dialog = await screen.findByRole("dialog", { name: "Records for the AI (141)" });
      const first = (await within(dialog).findAllByRole("link"))[0] as HTMLElement;
      act(() => {
        first.focus();
      });
      fireEvent.keyDown(first, { key: "?" });
      expect(screen.queryByRole("dialog", { name: "Keyboard shortcuts" })).toBeNull();
      expect(screen.getAllByRole("dialog")).toHaveLength(1);
      fireEvent.keyDown(first, { key: "Escape" });
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("moves focus to the first entry Show all brings, never the body", async () => {
      renderApp(asideClient({ times: 40 }), `#/${ASIDE}/flow?scope=WBAY-KILN`);
      await drawn("CONE-GLOST");
      fireEvent.click(recordsButton());
      const dialog = await screen.findByRole("dialog", { name: "Records for the AI (5679)" });
      await within(dialog).findByRole("group", { name: "edict" });
      fireEvent.click(within(dialog).getByRole("button", { name: "Show all 5679" }));
      await waitFor(() => {
        expect(document.activeElement).toBe(dialog.querySelectorAll(".flow-records-group li a")[200]);
      });
    });

    it("says a read's reason in the daemon's words, in place of the none line", async () => {
      const client = asideClient();
      const served = client.getFlow.getMockImplementation();
      client.getFlow.mockImplementation((project, options, signal) =>
        options.in_flow !== true && options.root === "WBAY-KILN"
          ? Promise.resolve(aFlowView([], [], { root: "WBAY-KILN", reason: "`WBAY-KILN` is no indexed document", gaps: [] }))
          : (served?.(project, options, signal) ?? Promise.reject(new Error("no stub"))),
      );
      renderApp(client, `#/${ASIDE}/flow?scope=WBAY-KILN`);
      await drawn("CONE-GLOST");
      fireEvent.click(recordsButton());
      const dialog = await screen.findByRole("dialog", { name: "Records for the AI (141)" });
      expect((await within(dialog).findByText("`WBAY-KILN` is no indexed document")).getAttribute("lang")).toBe("en");
      expect(within(dialog).queryByText("Every document here is part of the flow.")).toBeNull();
    });

    it("at the lens root lists the four parts outside the flow, the office with what it holds, the eight notes as one Spec tree line", async () => {
      renderApp(asideClient(), LENS_ROOT);
      await drawn("WBAY-KILN");
      fireEvent.click(recordsButton());
      const dialog = await screen.findByRole("dialog", { name: "Records for the AI (14)" });
      const parts = await within(dialog).findByRole("group", { name: "workbay" });
      expect(Array.from(parts.querySelectorAll("li"), (row) => row.textContent)).toEqual(["Staff canteen", "Fuel shed", "Front office2 documents", "Water butts"]);
      expect(within(dialog).getByRole("link", { name: "8 documents without an ID are in the Spec tree" }).getAttribute("href")).toBe(`#/${ASIDE}/tree/ATL-POTTERY`);
      expect(within(dialog).queryByText("The pottery")).toBeNull();
    });

    it("never lists the scope's own node, though the daemon says it holds nothing of the flow (a ROOT stays in the cut)", async () => {
      renderApp(asideClient(), `#/${ASIDE}/flow?scope=WBAY-OFFICE`);
      fireEvent.click(await within(places()).findByRole("button", { name: "Records for the AI (2)" }));
      const dialog = await screen.findByRole("dialog", { name: "Records for the AI (2)" });
      const desk = await within(dialog).findByRole("group", { name: "deskpaper" });
      expect(Array.from(desk.querySelectorAll("li"), (row) => row.textContent)).toEqual(["Order ledger", "Weekly rota"]);
      expect(within(dialog).queryByText("Front office")).toBeNull();
      expect(within(dialog).queryByRole("group", { name: "workbay" })).toBeNull();
    });

    it("on aside40 shows 200 entries, then Show all 5679", async () => {
      renderApp(asideClient({ times: 40 }), `#/${ASIDE}/flow?scope=WBAY-KILN`);
      await drawn("CONE-GLOST");
      fireEvent.click(recordsButton());
      const dialog = await screen.findByRole("dialog", { name: "Records for the AI (5679)" });
      await within(dialog).findByRole("group", { name: "edict" });
      expect(dialog.querySelectorAll(".flow-records-group li")).toHaveLength(200);
      fireEvent.click(within(dialog).getByRole("button", { name: "Show all 5679" }));
      expect(dialog.querySelectorAll(".flow-records-group li")).toHaveLength(5679);
    });

    it("stands nowhere at N = 0 nor at an unrooted top; in a stage view with no scope read held, without a count", async () => {
      renderApp(asideClient(), `#/${ASIDE}/flow?scope=KRUN-FIRING&stage=glazing`);
      await drawn("POT-FIRE");
      expect(recordsButton().textContent).toBe("Records for the AI");
      act(() => {
        window.location.hash = `#/${ASIDE}/flow?scope=KRUN-FIRING`;
      });
      await drawn("POT-PACK");
      expect(within(places()).queryByRole("button", { name: /^Records for the AI/ })).toBeNull();
      act(() => {
        window.location.hash = `#/${ASIDE}/flow?scope=KRUN-FIRING&stage=glazing`;
      });
      await drawn("POT-FIRE");
      expect(within(places()).queryByRole("button", { name: /^Records for the AI/ })).toBeNull();
    });

    it("stands nowhere at an unrooted top", async () => {
      const top = aFlowView([aFlowNode({ id: "P", distance: 0, holds: 2 }), aFlowNode({ id: "Q", distance: 1 })], [aFlowEdge({ from: "P", to: "Q" })], { not_in_flow: 0 });
      renderApp(studioClient(reads({ top })), "#/alpha/flow");
      await drawn("Q");
      expect(within(places()).queryByRole("button", { name: /^Records for the AI/ })).toBeNull();
    });
  });

  describe("the path pane (FS-19)", () => {
    it("at the glost cone holds a Path list by titles, top first, each a flowHash link, the last where you are; a lens root not known yet named as a scope", async () => {
      renderApp(asideClient(), `#/${ASIDE}/flow?scope=CONE-GLOST`);
      await waitFor(() => {
        expect(pathEntries()).toEqual([
          ["The pottery", `#/${ASIDE}/flow?scope=ATL-POTTERY`, null],
          ["Kiln bay", `#/${ASIDE}/flow?scope=WBAY-KILN`, null],
          ["Glost cone", `#/${ASIDE}/flow?scope=CONE-GLOST`, "location"],
        ]);
      });
      expect(within(places()).getByRole("list", { name: "Path" }).tagName).toBe("OL");
    });

    it("at the lens root is one entry, the groups under it in served order; once the lens root is known, its entry writes no scope", async () => {
      renderApp(asideClient(), LENS_ROOT);
      await drawn("WBAY-KILN");
      await waitFor(() => {
        expect(pathEntries()).toEqual([["The pottery", LENS_ROOT, "location"]]);
      });
      expect(within(within(places()).getByRole("list", { name: "Groups here" })).getAllByRole("link").map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
        ["Gallery bay", `#/${ASIDE}/flow?scope=WBAY-GALLERY`],
        ["Firing run", `#/${ASIDE}/flow?scope=KRUN-FIRING`],
        ["Kiln bay", `#/${ASIDE}/flow?scope=WBAY-KILN`],
      ]);
      act(() => {
        window.location.hash = `#/${ASIDE}/flow?scope=CONE-GLOST`;
      });
      await waitFor(() => {
        expect(pathEntries().map(([text, href]) => [text, href])).toEqual([
          ["The pottery", LENS_ROOT],
          ["Kiln bay", `#/${ASIDE}/flow?scope=WBAY-KILN`],
          ["Glost cone", `#/${ASIDE}/flow?scope=CONE-GLOST`],
        ]);
      });
    });

    it("with the stub's module rooted at the kiln bay leaves out what stands above it", async () => {
      renderApp(asideClient({ module: "WBAY-KILN" }), `#/${ASIDE}/flow?module=kiln&scope=CONE-GLOST`);
      await waitFor(() => {
        expect(pathEntries()).toEqual([
          ["Kiln bay", `#/${ASIDE}/flow?module=kiln`, null],
          ["Glost cone", `#/${ASIDE}/flow?module=kiln&scope=CONE-GLOST`, "location"],
        ]);
      });
    });

    it("on a failed node read shows the current entry, the words and Retry; the canvas stands", async () => {
      const client = asideClient();
      client.getNode.mockRejectedValue(new ClientError({ status: 503, message: "spec: the index is locked" }));
      renderApp(client, `#/${ASIDE}/flow?scope=WBAY-KILN`);
      await drawn("CONE-GLOST");
      expect(await within(places()).findByText("The path could not be read.", { exact: false })).toBeTruthy();
      expect(pathEntries()).toEqual([["Kiln bay", `#/${ASIDE}/flow?scope=WBAY-KILN`, "location"]]);
      expect(within(places()).getByRole("button", { name: "Retry" })).toBeTruthy();
      expect(drawnNames().sort()).toEqual(["CONE-BISQUE", "CONE-GLOST", "HUNCH-0007"]);
    });

    it("renders no Up link; no source under src/ keeps Up's words or its held-read hooks", async () => {
      renderApp(asideClient(), `#/${ASIDE}/flow?scope=CONE-GLOST`);
      await waitFor(() => {
        expect(pathEntries()).toHaveLength(3);
      });
      expect(within(places()).queryByRole("link", { name: /^Up/ })).toBeNull();
      const sources = import.meta.glob<string>(["/src/**/*.{ts,tsx}", "!/src/flow/FlowView.test.tsx"], { query: "?raw", import: "default", eager: true });
      const gone = ["flow" + ".up\"", "flow" + ".upStart", "up" + "Start:", "useHeld" + "Holder", "useHeld" + "Title"];
      const kept = Object.entries(sources).flatMap(([path, text]) => gone.filter((word) => text.includes(word)).map((word) => `${path}: ${word}`));
      expect(kept).toEqual([]);
    });
  });

  describe("titles, technical details folded (FS-20)", () => {
    /** Every ID of a titled node the reads hold. */
    const TITLED = [asideLens(), asideFiring()].flatMap((view) => view.nodes).filter((node) => node.title !== null && node.id !== null).map((node) => node.id ?? "");

    it("names no titled node by its ID on the boxes, the List, the path and its groups, the gap items or the panel heading", async () => {
      renderApp(asideClient(), LENS_ROOT);
      await drawn("WBAY-KILN");
      const shown = [...boxes().map((element) => element.textContent), within(places()).getByRole("list", { name: "Path" }).textContent, within(places()).getByRole("list", { name: "Groups here" }).textContent];
      const items = await within(openGaps()).findByRole("button", { name: "Wedging + Open day" });
      shown.push(items.textContent, within(panel()).getByRole("heading", { level: 2 }).textContent);
      fireEvent.click(screen.getByRole("tab", { name: "List" }));
      const outline = await screen.findByRole("region", { name: "Every box and arrow of the view" });
      await waitFor(() => {
        expect(outline.querySelectorAll("[data-flow-row]").length).toBeGreaterThan(5);
      });
      shown.push(outline.textContent);
      for (const text of shown) {
        expect([text, TITLED.filter((id) => text.includes(id))]).toEqual([text, []]);
      }
    });

    it("shows a title-less node by its ID, an ID-less one by its path", async () => {
      const top = aFlowView([aFlowNode({ id: "NAMELESS", title: null, distance: 0 }), aFlowNode({ id: null, title: null, path: "docs/spec/loose.md", distance: 1 })], []);
      renderApp(studioClient(reads({ top })), "#/alpha/flow");
      await drawn("NAMELESS");
      expect(box("NAMELESS").querySelector(".flow-box-title")?.textContent).toBe("NAMELESS");
      expect(box("docs/spec/loose.md").querySelector(".flow-box-title")?.textContent).toBe("docs/spec/loose.md");
    });

    it("folds a box's name, kind, place, distance, marks and inner links under Technical details, collapsed", async () => {
      renderApp(asideClient(), `#/${ASIDE}/flow/KRUN-FIRING`);
      await drawn("KRUN-FIRING");
      await waitFor(() => {
        expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Firing run");
      });
      const details = within(panel()).getByRole("button", { name: "Technical details" });
      expect(details.getAttribute("aria-expanded")).toBe("false");
      const region = document.getElementById(details.getAttribute("aria-controls") ?? "");
      expect(region?.hidden).toBe(true);
      expect(Array.from(region?.querySelectorAll("dt") ?? [], (term) => term.textContent)).toEqual(["Name", "Kind", "Distance", "Where", "Group"]);
      expect(region?.textContent).toContain("KRUN-FIRING");
      expect(region?.textContent).toContain("docs/workshop/run/firing.md:1");
      expect(region?.textContent).toContain("Entry");
      fireEvent.click(details);
      expect(details.getAttribute("aria-expanded")).toBe("true");
      expect(region?.hidden).toBe(false);
    });
  });

  describe("empty states (FS-21)", () => {
    it("at the front office: nothing here is part of the flow, two documents for your AI, Records for the AI the one act", async () => {
      renderApp(asideClient(), `#/${ASIDE}/flow?scope=WBAY-OFFICE`);
      const region = await screen.findByRole("region", { name: "Flow canvas" });
      const heading = await within(region).findByRole("heading", { level: 2, name: "Nothing here is part of the flow." });
      const empty = heading.closest(".empty-state-box");
      expect(empty?.textContent).toContain("2 documents here are for your AI.");
      const acts = within(empty as HTMLElement).getAllByRole("button");
      expect(acts.map((act) => act.textContent)).toEqual(["Records for the AI (2)"]);
      fireEvent.click(acts[0] as HTMLElement);
      expect(await screen.findByRole("dialog", { name: "Records for the AI (2)" })).toBeTruthy();
      expect(boxes()).toHaveLength(0);
    });

    it("at a scope in the flow whose own documents all are not: the scope's empty words, Records for the AI beside them, not the out-of-flow state", async () => {
      const studio = reads({ scopes: { G: scopeOfG(), D: aFlowView([aFlowNode({ id: "D", distance: null, title: "Dye", in_flow: true })], [], { root: "D", depth: 1, not_in_flow: 3 }) } });
      renderApp(studioClient(studio), "#/alpha/flow?scope=D");
      const heading = await screen.findByRole("heading", { level: 2, name: "Nothing is inside Dye." });
      const empty = heading.closest(".empty-state-box");
      expect(within(empty as HTMLElement).getAllByRole("button").map((act) => act.textContent)).toEqual(["Records for the AI (3)"]);
      expect(screen.queryByText("Nothing here is part of the flow.")).toBeNull();
    });

    it("words the project, scope, stage and no-stage empties through EmptyState", async () => {
      const studio = reads({ top: aFlowView([], []), scopes: { G: scopeOfG(), D: aFlowView([aFlowNode({ id: "D", distance: null, title: "Dye" })], [], { root: "D", depth: 1 }) } });
      renderApp(studioClient(studio), "#/alpha/flow");
      const project = await screen.findByRole("heading", { level: 2, name: "No flow in this project yet. Next: have your AI add flow links." });
      expect(project.closest(".empty-state-box")).not.toBeNull();
      act(() => {
        window.location.hash = "#/alpha/flow?scope=D";
      });
      expect((await screen.findByRole("heading", { level: 2, name: "Nothing is inside Dye." })).closest(".empty-state-box")).not.toBeNull();
      act(() => {
        window.location.hash = "#/alpha/flow?scope=G&stage=nowhere";
      });
      expect((await screen.findByRole("heading", { level: 2, name: "No step here carries nowhere." })).closest(".empty-state-box")).not.toBeNull();
      act(() => {
        window.location.hash = "#/alpha/flow?scope=G&stage=-";
      });
      expect((await screen.findByRole("heading", { level: 2, name: "Every step here has a stage." })).closest(".empty-state-box")).not.toBeNull();
    });
  });
});

// DM-6 of docs/features/doc-modal.md: Flow's own replaces keep an open document modal.
describe("the document modal over Flow (DM-6, DM-10 of doc-modal)", () => {
  it("keeps `doc` through the bare-hash replacement: the remembered place, the pair after it (the review's R3)", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow/A");
    await drawn("A");
    await waitFor(() => {
      expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Ask");
    });
    act(() => {
      window.location.hash = "#/alpha/inbox";
    });
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    const before = window.history.length;
    act(() => {
      window.location.hash = "#/alpha/flow?doc=C";
    });
    // The nav's bare hash stands for Flow's last place, replaced in place: the modal's pair kept after it.
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/A?doc=C");
    });
    expect(window.history.length).toBe(before + 1);
    expect(await screen.findByRole("dialog", { name: "Craft" })).toBeTruthy();
    await drawn("A");
  });

  it("keeps `doc` through a seek's fix", async () => {
    const before = window.history.length;
    renderApp(studioClient(reads()), "#/alpha/flow/X?doc=A");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/X?scope=C&doc=A");
    });
    expect(window.history.length).toBe(before);
    expect(await screen.findByRole("dialog", { name: "Ask" })).toBeTruthy();
  });

  it("keeps `doc` through a module's drop", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow/G?module=nope&doc=A");
    await drawn("A");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow?doc=A");
    });
    expect(screen.getByRole("dialog", { name: "Ask" })).toBeTruthy();
  });

  it("opens a Records entry as the document over the dialog; Esc closes the document, then the dialog (DM-10)", async () => {
    renderApp(asideClient(), `#/${ASIDE}/flow?scope=WBAY-KILN`);
    await drawn("CONE-GLOST");
    fireEvent.click(within(places()).getByRole("button", { name: /^Records for the AI/ }));
    const records = await screen.findByRole("dialog", { name: "Records for the AI (141)" });
    const entry = within(await within(records).findByRole("group", { name: "edict" })).getAllByRole("link")[0] as HTMLElement;
    fireEvent.click(entry);
    expect(window.location.hash).toBe(`#/${ASIDE}/flow?scope=WBAY-KILN&doc=EDICT-0001`);
    const doc = await screen.findByRole("dialog", { name: "Kiln edict no. 1" });
    await waitFor(() => {
      expect(doc.contains(document.activeElement)).toBe(true);
    });
    expect((records.closest("[data-dialog-host]") as HTMLElement).inert).toBe(true);
    fireEvent.keyDown(document.activeElement ?? doc, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Kiln edict no. 1" })).toBeNull();
    });
    expect(screen.getByRole("dialog", { name: "Records for the AI (141)" })).toBe(records);
    await waitFor(() => {
      expect(document.activeElement).toBe(entry);
    });
    fireEvent.keyDown(entry, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.location.hash).toBe(`#/${ASIDE}/flow?scope=WBAY-KILN`);
  });
});

// DM-14 of docs/features/doc-modal.md: the panel is minimal; the whole document is the modal's.
describe("the minimal panel (DM-14 of doc-modal)", () => {
  const FIRING = `#/${ASIDE}/flow?scope=KRUN-FIRING`;

  async function bisque(client = asideClient()) {
    renderApp(client, FIRING);
    fireEvent.click(await drawn("POT-BISQUE"));
    await waitFor(() => {
      expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Bisque firing");
    });
    await within(panel()).findByText("Bone-dry pots are fired once, slowly, to a low cone.");
    return client;
  }

  it("shows a box's title, stage, work state and first paragraph alone (M: the whole text in the panel)", async () => {
    await bisque();
    const line = panel().querySelector(".flow-panel-line");
    expect(line?.textContent).toContain("bisque");
    const state = line?.querySelector(".lozenge");
    expect([state?.textContent, state?.querySelector("svg") !== null]).toEqual(["In work", true]);
    const lead = panel().querySelector(".flow-panel-lead");
    expect(Array.from(lead?.querySelectorAll("p") ?? [], (paragraph) => paragraph.textContent)).toEqual(["Bone-dry pots are fired once, slowly, to a low cone."]);
    expect(panel().textContent).not.toContain("The kiln is opened only when cold.");
    expect(lead?.querySelector("h1, h2, h3, h4, .md-front-matter")).toBeNull();
  });

  it("acts with Open document, the one primary, and New card alone; a group adds its Open; no links, cards, tree or graph in the panel", async () => {
    await bisque();
    const acts = Array.from(panel().querySelectorAll(".flow-panel-actions > *"), (act) => act.textContent);
    // Split to departments… after New task (U1 of department-split).
    expect(acts).toEqual(["Open document", "New task", "Split to departments…"]);
    expect(within(panel()).getByRole("button", { name: "Open document" }).className).toContain("button-primary");
    expect(panel().querySelectorAll(".button-primary")).toHaveLength(1);
    expect(panel().querySelector(".links-panel, .cards-block")).toBeNull();
    expect(within(panel()).queryByRole("region", { name: "Tasks" })).toBeNull();
    for (const name of ["Open in Spec tree", "Open in Graph"]) {
      expect([name, within(panel()).queryByRole("link", { name })]).toEqual([name, null]);
    }
    act(() => {
      window.location.hash = `#/${ASIDE}/flow/KRUN-FIRING`;
    });
    await waitFor(() => {
      expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Firing run");
    });
    expect(Array.from(panel().querySelectorAll(".flow-panel-actions > *"), (act) => act.textContent)).toEqual(["Open document", "New task", "Split to departments…", "Open Firing run"]);
  });

  it("folds name, kind, place and distance under Technical details, collapsed", async () => {
    await bisque();
    const details = within(panel()).getByRole("button", { name: "Technical details" });
    expect(details.getAttribute("aria-expanded")).toBe("false");
    expect(document.getElementById(details.getAttribute("aria-controls") ?? "")?.hidden).toBe(true);
    expect([fact("Name"), fact("Kind"), fact("Where"), fact("Distance")]).toEqual(["POT-BISQUE", "potstep", "docs/workshop/run/bisque.md:1", "4"]);
  });

  it("reads no trace for a selection; Open document pushes `doc` and the modal draws from the held read, no new call (M: getTrace on a selection)", async () => {
    const client = await bisque();
    expect(client.getTrace.mock.calls.map((call) => call[1])).not.toContainEqual({ ref: "POT-BISQUE" });
    const nodeCalls = client.getNode.mock.calls.length;
    const before = window.history.length;
    fireEvent.click(within(panel()).getByRole("button", { name: "Open document" }));
    expect(window.location.hash).toBe(`#/${ASIDE}/flow/POT-BISQUE?scope=KRUN-FIRING&doc=POT-BISQUE`);
    expect(window.history.length).toBe(before + 1);
    const modal = await screen.findByRole("dialog", { name: "Bisque firing" });
    await within(modal).findByText("The kiln is opened only when cold.");
    expect(client.getNode.mock.calls.length).toBe(nodeCalls);
  });

  it("gives focus back to Open document, the button pressed, when the modal closes, though a click focused it not (the review's R2)", async () => {
    await bisque();
    const open = within(panel()).getByRole("button", { name: "Open document" });
    expect(document.activeElement).not.toBe(open);
    fireEvent.click(open);
    const modal = await screen.findByRole("dialog", { name: "Bisque firing" });
    fireEvent.keyDown(modal, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(open);
    });
  });

  it("says a card made from a box in that box's panel alone: another selection takes the note away (the review's nit)", async () => {
    const client = await bisque();
    client.createCard.mockResolvedValue(aCardWrite("C-0042"));
    fireEvent.click(within(panel()).getByRole("button", { name: "New task" }));
    const dialog = await screen.findByRole("dialog", { name: "New task" });
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source", { exact: false }).value).toBe("POT-BISQUE");
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Fire it" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
    expect(await within(panel()).findByText("C-0042 created.")).toBeTruthy();
    fireEvent.keyDown(box("POT-BISQUE"), { key: "Escape" });
    await waitFor(() => {
      expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Firing run");
    });
    expect(within(panel()).queryByText("C-0042 created.")).toBeNull();
  });

  it("shows the scope's lead, Open document and New card; an unrooted top none", async () => {
    renderApp(asideClient(), FIRING);
    await drawn("POT-BISQUE");
    await waitFor(() => {
      expect(Array.from(panel().querySelectorAll(".flow-panel-actions > *"), (act) => act.textContent)).toEqual(["Open document", "New task", "Split to departments…"]);
    });
    renderApp(studioClient(reads({ top: aFlowView([aFlowNode({ id: "P", distance: 0 }), aFlowNode({ id: "Q", distance: 1 })], []) })), "#/alpha/flow");
    await drawn("Q");
    const panels = screen.getAllByRole("complementary", { name: "Details" });
    const top = panels[panels.length - 1] as HTMLElement;
    expect(within(top).queryByRole("button", { name: "Open document" })).toBeNull();
    expect(within(top).queryByRole("button", { name: "New task" })).toBeNull();
    expect(within(top).queryByRole("button", { name: "Split to departments…" })).toBeNull();
  });
});

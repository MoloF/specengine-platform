import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { FlowEdge, FlowNode, FlowView as FlowAnswer, QueueEvent, TraceCounts, TraceStage } from "../api/types";
import { aFlowEdge, aFlowGap, aFlowNode, aFlowView, aTreeNode, tenGapRows } from "../test/builders";
import { studioClient, TYPE_A, TYPE_B, type StudioReads } from "../test/flowStub";
import { renderApp } from "../test/render";
import { enT } from "../test/text";
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

const ROWS = tenGapRows([aFlowGap("no-start", [["G"]]), aFlowGap("cycle-no-repeat", [["B", "E"]]), aFlowGap("unreachable", [["X"]])]);

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
      { depth: 0, trace: true },
      { root: "G", depth: 1, trace: true },
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
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([{ depth: 0, trace: true }]);
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
    expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("X");
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
    const hit = hitOf(edgeNamed("C alpha_link E, x3"));
    expect(Number(hit.getAttribute("stroke-width"))).toBeGreaterThanOrEqual(12);
    const transform = document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform;
    const before = window.history.length;
    const served = readsOf(client);
    fireEvent.click(hit);
    expect(window.location.hash).toBe("#/alpha/flow?edge=C,E,alpha_link");
    expect(window.history.length).toBe(before);
    expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("Arrow");
    expect([fact("From"), fact("To"), fact("Type"), fact("Kind"), fact("When"), fact("Links"), fact("Where")]).toEqual(["C Open C", "E", TYPE_A, "none", "none", "3", "docs/spec/e.md:14"]);
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
    fireEvent.click(hitOf(edgeNamed(`B ${TYPE_A} C, alternative, the ink is dry`)));
    expect(window.location.hash).toBe("#/alpha/flow?edge=B,C,alpha_link,alternative");
    expect([fact("Kind"), fact("When")]).toEqual(["alternative", "the ink is dry"]);
    const before = window.history.length;
    fireEvent.click(within(panel()).getByRole("link", { name: "B" }));
    expect(window.location.hash).toBe("#/alpha/flow/B");
    expect(window.history.length).toBe(before);
    expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("B");
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
  it("is mounted with nothing selected: the scope's title, name, kind, counts, its ways out and ten rows", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    const shown = within(panel());
    expect(shown.getByRole("heading", { level: 2 }).textContent).toBe("The guild");
    expect([fact("Name"), fact("Kind")]).toEqual(["G", "domain"]);
    expect(shown.getByText("2 confirmed, 3 no card")).toBeTruthy();
    expect(shown.getByRole("link", { name: "Open in Spec tree" }).getAttribute("href")).toBe("#/alpha/tree/G");
    expect(shown.getByRole("button", { name: "New card" })).toBeTruthy();
    expect(panel().querySelectorAll("li.flow-gap")).toHaveLength(10);
  });

  it("swaps a box's content in and back on Esc, in the same element", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    const aside = panel();
    fireEvent.click(await drawn("A"));
    await waitFor(() => {
      expect(within(aside).getByRole("heading", { level: 2 }).textContent).toBe("A");
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
      expect(within(panel()).getByRole("heading", { level: 2 }).textContent).toBe("A");
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
    expect(panel().querySelectorAll("li.flow-gap")).toHaveLength(10);
    expect(screen.queryByRole("button", { name: /gaps$/ })).toBeNull();
    expect([...getItem.mock.calls, ...setItem.mock.calls].some((call) => call[0] === "specengine.flow.gaps-hidden")).toBe(false);
  });

  it("shows a group's own Open, entering it; a holder that is the scope, none", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow/C");
    await drawn("C");
    expect((await within(panel()).findByRole("link", { name: "Open C" })).getAttribute("href")).toBe("#/alpha/flow?scope=C");
    expect(within(panel()).getByText(/^2 documents/)).toBeTruthy();
    expect(within(panel()).queryByText("Group", { selector: "dt" })).toBeNull();
  });

  it("shows a box's holder with Open when it is not the scope: a stage across groups", async () => {
    const deeper = node({ id: "Z", distance: 4, group: "C", stage: "print", title: "Zinc" });
    const scopes = { G: aFlowView([...scopeOfG().nodes, deeper], EDGES, { root: "G", depth: 1, gaps: ROWS }), C: SCOPE_C };
    renderApp(studioClient(reads({ scopes })), "#/alpha/flow/Z?stage=print");
    await drawn("Z");
    expect(fact("Group")).toBe("C Open C");
    expect(within(panel()).getByRole("link", { name: "Open C" }).getAttribute("href")).toBe("#/alpha/flow?scope=C");
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
    fireEvent.keyDown(toggle, { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Legend" })).toBeNull();
    expect(document.activeElement).toBe(toggle);
    expect(window.location.hash).toBe("#/alpha/flow/A");
    fireEvent.keyDown(toggle, { key: "Escape" });
    expect(window.location.hash).toBe("#/alpha/flow");
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
    expect(first?.textContent).toBe(`${TYPE_B} D, Dye`);
    const row = outline.querySelector<HTMLElement>("[data-flow-row='C']");
    expect(row?.querySelector("a")?.getAttribute("href")).toBe("#/alpha/flow/C");
    expect(within(row as HTMLElement).getByRole("link", { name: "Open C" }).getAttribute("href")).toBe("#/alpha/flow?scope=C");
    expect(within(row as HTMLElement).getByText("2 documents")).toBeTruthy();
    expect(within(row as HTMLElement).getByRole("link", { name: `${TYPE_A} E, Engrave, ×3` }).getAttribute("href")).toBe("#/alpha/flow?edge=C,E,alpha_link");
    expect(outline.textContent).not.toMatch(/\.md:\d/);
    const before = window.history.length;
    fireEvent.click(within(row as HTMLElement).getByRole("link", { name: `${TYPE_A} E, Engrave, ×3` }));
    expect(window.location.hash).toBe("#/alpha/flow?edge=C,E,alpha_link");
    expect(window.history.length).toBe(before);
    expect(fact("Where")).toBe("docs/spec/e.md:14");
  });

  it("in a stage view, stands each box under its group", async () => {
    const outline = await list("#/alpha/flow?stage=gather");
    await waitFor(() => {
      expect(Array.from(outline.querySelectorAll("[data-flow-row]"), (row) => row.getAttribute("data-flow-row"))).toEqual(["A", "B"]);
    });
    expect(within(outline).getByRole("heading", { level: 2, name: "G" })).toBeTruthy();
    expect(within(outline).getByRole("link", { name: `${TYPE_A} B, Bind` })).toBeTruthy();
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
    const open = await within(outline).findByRole("link", { name: "Open C" });
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
    // M21: no item a button with a handler.
    expect(within(places()).queryAllByRole("button")).toEqual([]);
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
    fireEvent.keyDown(box("A"), { key: "ArrowRight" });
    expect(document.activeElement).toBe(box("B"));
    fireEvent.keyDown(box("B"), { key: "Enter" });
    expect(window.location.hash).toBe("#/alpha/flow/B");
    await waitFor(() => {
      expect(document.activeElement?.textContent).toBe("B");
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
  it("lists All stages, then the lanes as served with their scores, No stage last; under the current, the scope's groups", async () => {
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
    expect(pane.getByRole("list", { name: "Groups here" }).textContent).toContain("Craft");
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

  it("names Up the holder a held read gives, else the lens root; a lens root not read yet is not called the whole project", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow?scope=C");
    await drawn("X");
    // The hash named a scope, so the top was not read: Up goes to the lens root without naming it.
    expect(within(places()).getByRole("link", { name: "Up to the start" }).getAttribute("href")).toBe("#/alpha/flow");
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([{ root: "C", depth: 1, trace: true }]);
    act(() => {
      window.location.hash = "#/alpha/flow";
    });
    await drawn("A");
    fireEvent.click(within(places()).getByRole("link", { name: /^Craft/ }));
    await drawn("X");
    // Named by its title, which a held read gives (LS-8 of ui-languages-sweep).
    expect(within(places()).getByRole("link", { name: "Up: The guild" }).getAttribute("href")).toBe("#/alpha/flow");
    // In a stage view too: C's read, held, names X's holder.
    act(() => {
      window.location.hash = "#/alpha/flow?scope=X&stage=gather";
    });
    expect((await within(places()).findByRole("link", { name: "Up: Craft" })).getAttribute("href")).toBe("#/alpha/flow?scope=C");
  });

  it("names Up by the holder's title from a held read, else by its ID, reading nothing more (LS-8 of ui-languages-sweep)", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    fireEvent.click(within(places()).getByRole("link", { name: /^Craft/ }));
    await drawn("X");
    const calls = client.getFlow.mock.calls.length;
    // C's holder G, titled "The guild" by the top read and by G's own: the words Up says.
    expect(within(places()).getByRole("link", { name: enT("flow.up", { name: "The guild" }) }).getAttribute("href")).toBe("#/alpha/flow");
    expect(within(places()).queryByRole("link", { name: enT("flow.up", { name: "G" }) })).toBeNull();
    expect(client.getFlow.mock.calls.length).toBe(calls);

    // A holder no held read titles: its ID.
    const untitled = studioClient(
      reads({
        top: aFlowView([aFlowNode({ id: "G", distance: 0, holds: 7, start: true, end: true, title: null })], []),
        scopes: { G: scopeOfG(), C: SCOPE_C },
      }),
    );
    untitled.getFlow.mockImplementation((_project, options) =>
      Promise.resolve(structuredClone(options.root === "G" ? aFlowView([aFlowNode({ id: "G", distance: null, kind: "domain", title: null }), ...CHILDREN], EDGES, { root: "G", depth: 1 }) : options.root === "C" ? SCOPE_C : aFlowView([aFlowNode({ id: "G", distance: 0, holds: 7, start: true, end: true, title: null })], []))),
    );
    cleanup();
    renderApp(untitled, "#/alpha/flow");
    await drawn("A");
    fireEvent.click(within(places()).getByRole("link", { name: /^Craft/ }));
    await drawn("X");
    expect(within(places()).getByRole("link", { name: enT("flow.up", { name: "G" }) }).getAttribute("href")).toBe("#/alpha/flow");
  });

  it("writes no lens root as a scope: a seek whose lens root was not read reads the top to know it (R3)", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow/B?scope=C");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/B");
    });
    await drawn("B");
    expect(client.getTree).toHaveBeenCalledTimes(1);
    expect(client.getFlow.mock.calls.map((call) => call[1])).toContainEqual({ depth: 0, trace: true });
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
    expect(within(panel()).getAllByText("Not read")).toHaveLength(10);
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
    expect(await screen.findByText("Nothing is inside D.")).toBeTruthy();
    expect(panel().querySelectorAll("li.flow-gap")).toHaveLength(10);
    act(() => {
      window.location.hash = "#/alpha/flow?stage=nowhere";
    });
    expect(await screen.findByText("No step here carries nowhere.")).toBeTruthy();
    act(() => {
      window.location.hash = "#/alpha/flow?stage=-";
    });
    expect(await screen.findByText("Every step here has a stage.")).toBeTruthy();
  });

  it("no [flow]: rows Not configured with their cause verbatim, the canvas drawn", async () => {
    const rows = tenGapRows([aFlowGap("no-start", [], { state: "not_configured", count: null, cause: "no [[flow.start]] rule: add one naming the first step by paths" })]);
    renderApp(studioClient(reads({ scopes: { G: scopeOfG({ gaps: rows }) } })), "#/alpha/flow");
    await drawn("A");
    expect(within(panel()).getByText("Not configured")).toBeTruthy();
    expect(within(panel()).getByText("no [[flow.start]] rule: add one naming the first step by paths").getAttribute("lang")).toBe("en");
    expect(within(panel()).getByText("1 row needs [flow] rules in specengine.toml")).toBeTruthy();
  });
});

describe("gap items and the hand-off (D11, cut 2c)", () => {
  it("activating a drawn member selects it by a replace, marks every drawn member, focus to the panel heading", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    const before = window.history.length;
    fireEvent.click(within(panel()).getByRole("button", { name: "E" }));
    expect(window.location.hash).toBe("#/alpha/flow/B");
    expect(window.history.length).toBe(before);
    await waitFor(() => {
      expect(document.activeElement?.textContent).toBe("B");
    });
    expect(box("B").getAttribute("aria-label")).toContain("marked");
    expect(box("E").getAttribute("aria-label")).toContain("marked");
  });

  it("activating a member not drawn seeks it, the fix a push", async () => {
    const client = studioClient(reads());
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    const before = window.history.length;
    fireEvent.click(within(panel()).getByRole("button", { name: "X" }));
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
    const item = within(panel()).getByRole("button", { name: "E" }).closest("li");
    fireEvent.click(within(item as HTMLElement).getByRole("button", { name: /^Hand to your AI/ }));
    const dialog = await screen.findByRole("dialog", { name: "New card" });
    expect(within(dialog).getByLabelText<HTMLInputElement>("Title").value).toBe("Gap cycle-no-repeat: B + E");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Goal").value).toBe("a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source", { exact: false }).value).toBe("B\nE");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create card" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ title: "Gap cycle-no-repeat: B + E", target: ["B", "E"], goal: "a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}" });
    expect(await within(panel()).findByText("C-0001 created.")).toBeTruthy();
    expect(client.getTasks).not.toHaveBeenCalled();
  });

  it("hands `no-start`'s scope under a root; at the top its item hands nothing", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("A");
    const row = within(panel()).getByText("no-start").closest("li");
    fireEvent.click(within(row as HTMLElement).getByRole("button", { name: /^Hand to your AI/ }));
    const dialog = await screen.findByRole("dialog", { name: "New card" });
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source", { exact: false }).value).toBe("G");
  });
});

describe("a box on the canvas", () => {
  it("names its words, marks and gaps: Entry in a scope, End, a group's holdings and counts, its rollup", async () => {
    renderApp(studioClient(reads()), "#/alpha/flow");
    await drawn("E");
    expect(box("A").getAttribute("aria-label")).toBe("A, Ask, widget, Entry, Confirmed");
    expect(box("E").getAttribute("aria-label")).toBe("E, Engrave, widget, End, In work, named by a gap row");
    expect(box("C").getAttribute("aria-label")).toBe("C, Craft, widget, group, 2 documents, 2 in work, 1 with gaps");
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

  it("orders types by name over the read's edges and each node's inner; a type the order lacks takes the first colour", () => {
    const folded = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 3, inner: [{ type: "zebra", count: 2 }] })], [aFlowEdge({ from: "G", to: "G", type: "yak" })]);
    expect(typesOf(folded)).toEqual(["yak", "zebra"]);
    expect(typeColour("zebra", ["yak", "zebra"])).toBe("var(--flow-edge-1)");
    expect(typeColour("nobody", ["yak", "zebra"])).toBe("var(--flow-edge-0)");
  });

  it("says an edge's kind, its when clipped, its count; a plain edge says nothing", () => {
    expect(edgeWords(edge({ kind: "alternative", when: "after dark" }))).toEqual(["alternative", "after dark"]);
    expect(edgeWords(edge({ count: 3 }))).toEqual(["x3"]);
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

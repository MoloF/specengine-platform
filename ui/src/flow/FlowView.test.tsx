import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import { ApiProvider } from "../api/provider";
import type { FlowEdge, FlowGap, FlowNode, FlowView as FlowAnswer, QueueEvent, TraceCounts, TraceNode, TraceStage } from "../api/types";
import { sectionHash } from "../app/routes";
import { aFlowEdge, aFlowGap, aFlowNode, aFlowView, aTrace, aTraceCard, aTraceNode, tenGapRows, aTreeNode, aTreeView } from "../test/builders";
import { aGroupRead, aLine, flowClient, TYPE_A, TYPE_B } from "../test/flowStub";
import { renderApp } from "../test/render";
import { FlowView, type FlowState } from "./FlowView";
import { FLOW_DASHES, dashOf, edgeLine, edgeWords, shortWhen, typeColour, typesOf, WHEN_SHOWN } from "./labels";
import { BOX_WIDTH, COLUMN_GAP, LANE_PAD } from "./layout";

// docs/features/flow-canvas.md on the stub client: AC-03 nothing lifted, AC-05 and AC-06 on the
// canvas, AC-08 to AC-11 the rows, the panel, the states and the cap, AC-13 the keys, motion, width.

afterEach(() => {
  Reflect.deleteProperty(window, "matchMedia");
});

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

function hasBox(name: string): boolean {
  return boxes().some((element) => element.dataset.id === `n:${name}`);
}

async function drawn(name: string): Promise<HTMLElement> {
  await waitFor(() => {
    expect(hasBox(name)).toBe(true);
  });
  return box(name);
}

function caret(name: string): Element {
  const found = box(name).querySelector("[data-flow-toggle]");
  if (found === null) {
    throw new Error(`no caret on ${name}`);
  }
  return found;
}

function key(name: string, pressed: string) {
  const element = box(name);
  element.focus();
  fireEvent.keyDown(element, { key: pressed });
}

function stubNarrow(narrow: boolean) {
  Object.defineProperty(window, "matchMedia", {
    value: (media: string) => ({ media, matches: narrow && media === "(max-width: 48rem)" }) as unknown as MediaQueryList,
    configurable: true,
    writable: true,
  });
}

function gapsRegion(): HTMLElement {
  return screen.getByRole("region", { name: "Gaps" });
}

function gapRow(code: string): HTMLElement {
  const row = within(gapsRegion()).getByText(code).closest("li");
  if (row === null) {
    throw new Error(`no row ${code}`);
  }
  return row;
}

/** A read held until the test lets it land. */
function deferred<T>() {
  let settle: (value: T) => void = () => undefined;
  const promise = new Promise<T>((resolve) => {
    settle = resolve;
  });
  return {
    promise,
    resolve: (value: T) => {
      settle(value);
    },
  };
}

async function top(): Promise<void> {
  await screen.findByRole("region", { name: "Gaps" });
}

const steps = aFlowView(aLine(["A", "B", "C"]), [aFlowEdge({ from: "A", to: "B" }), aFlowEdge({ from: "B", to: "C" })]);

describe("nothing lifted (AC-03)", () => {
  it("draws a top edge's own count with its group open, whatever the group's read holds", async () => {
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 2 }), aFlowNode({ id: "AFTER", distance: 1 })], [aFlowEdge({ from: "G", to: "AFTER", count: 7, type: TYPE_A })]);
    const group = aGroupRead("G", aLine(["X", "Y"]), [aFlowEdge({ from: "X", to: "Y", count: 2 })]);
    renderApp(flowClient(reads, { G: group }), "#/alpha/flow");
    await drawn("X");
    expect(screen.getAllByText("x7")).toHaveLength(1);
    expect(screen.getAllByText("x2")).toHaveLength(1);
  });

  it("asks no read but its own: a group's edges are never counted up from what is open", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 })], []), { G: aGroupRead("G", aLine(["X"])) });
    renderApp(client, "#/alpha/flow");
    await drawn("X");
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([{ depth: 0, trace: true }, { root: "G", depth: 1, trace: true }]);
  });
});

describe("a box on the canvas (AC-04, AC-05)", () => {
  it("names its words, marks and gaps: Start at the top, Entry in a lane, End", async () => {
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 2, start: true, gaps: 3, title: "A group" }), aFlowNode({ id: "F", distance: 1, end: true, title: "The last" }), aFlowNode({ id: "KEPT", distance: 2, holds: 1 })], []);
    const group = aGroupRead("G", [aFlowNode({ id: "X", distance: 0, start: true, title: "Way in" })]);
    renderApp(flowClient(reads, { G: group }), "#/alpha/flow");
    await drawn("G");
    expect(box("F").getAttribute("aria-label")).toBe("F, The last, widget, End");
    fireEvent.click(caret("G"));
    await drawn("X");
    expect(box("X").getAttribute("aria-label")).toBe("X, Way in, widget, Entry");
    expect(box("X").textContent).toContain("Entry");
    expect(box("G").getAttribute("aria-label")).toBe("G, A group, widget, Start, open group, 2 documents");
    expect(box("G").querySelector(".flow-mark-gap")).toBeNull();
    expect(box("G").textContent).toContain("Start");
  });

  it("shows a closed group's caret, what it holds and its served gaps", async () => {
    renderApp(flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 28, gaps: 15 }), aFlowNode({ id: "H", distance: 1, holds: 1 })], [])), "#/alpha/flow");
    await drawn("G");
    expect(box("G").textContent).toContain("28 documents");
    expect(box("G").textContent).toContain("15 with gaps");
    expect(box("H").textContent).toContain("1 document");
    expect(box("H").textContent).not.toContain("with gaps");
    expect(box("G").getAttribute("aria-label")).toContain("closed group, 28 documents, 15 with gaps");
  });

  it("marks an open group only when a row names it, never for what it holds", async () => {
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1, gaps: 1 })], [], { gaps: tenGapRows([aFlowGap("unreachable", [["G"]])]) });
    renderApp(flowClient(reads, { G: aGroupRead("G", aLine(["X"])) }), "#/alpha/flow");
    await drawn("X");
    expect(box("G").querySelector(".flow-mark-gap")?.textContent).toBe("Gap");
    expect(box("G").getAttribute("aria-label")).toContain("named by a gap row");
  });

  it("puts a closed group's gap mark first on its line, so the line's end cannot cut it", async () => {
    renderApp(flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 28, gaps: 15, start: true, end: true }), aFlowNode({ id: "H", distance: 1, holds: 1 })], [])), "#/alpha/flow");
    await drawn("G");
    expect(box("G").querySelector(".flow-box-marks")?.firstElementChild?.textContent).toContain("15 with gaps");
  });

  it("marks a drawn box a top row names, by icon and word, no count on a step", async () => {
    const reads = aFlowView(aLine(["A", "B"]), [], { gaps: tenGapRows([aFlowGap("unreachable", [["B"]])]) });
    renderApp(flowClient(reads), "#/alpha/flow");
    await drawn("B");
    expect(box("B").querySelector(".flow-mark-gap")?.textContent).toBe("Gap");
    expect(box("A").querySelector(".flow-mark-gap")).toBeNull();
    expect(box("B").getAttribute("aria-label")).toContain("named by a gap row");
  });

  it("draws a title as text: markup an author pasted never becomes an element", async () => {
    const hostile = "<img src=x onerror=boom()>";
    renderApp(flowClient(aFlowView([aFlowNode({ id: "A", distance: 0, title: hostile })], [])), "#/alpha/flow");
    await drawn("A");
    expect(box("A").textContent).toContain(hostile);
    expect(box("A").querySelector("img")).toBeNull();
  });

  it("gives a box with no title its name, and a band its caption", async () => {
    renderApp(flowClient(aFlowView([aFlowNode({ id: "A", distance: 0, title: null, kind: null }), aFlowNode({ id: "DEF", distance: null })], [])), "#/alpha/flow");
    await drawn("A");
    expect(box("A").querySelector(".flow-box-title")?.textContent).toBe("A");
    expect(box("A").getAttribute("aria-label")).toBe("A");
    expect(screen.getByText("Not on the walk")).toBeTruthy();
  });
});

describe("forms (AC-06)", () => {
  const edge = (fields: Partial<FlowEdge>) => aFlowEdge({ from: "A", to: "B", ...fields });

  it("keeps seven distinct dashes, one dotted, and that one for a repeat whatever types a project holds", () => {
    expect(new Set(FLOW_DASHES).size).toBe(7);
    const dotted = (dash: string) => dash !== "none" && Number(dash.split(" ")[0]) < 3;
    expect(FLOW_DASHES.filter(dotted)).toHaveLength(1);
    for (const types of [[], [TYPE_A], [TYPE_A, TYPE_B], ["a", "b", "c", "d"], ["a", "b", "c", "d", "e", "f"]]) {
      for (const type of [TYPE_A, ...types]) {
        expect(dotted(dashOf(edge({ type, kind: "repeat" }), types))).toBe(true);
      }
    }
  });

  it("gives a plain edge its type's dash, by place, never dotted; a kind its own; types and kinds apart", () => {
    const types = ["a", "b", "c", "d"];
    const plain = types.map((type) => dashOf(edge({ type }), types));
    expect(new Set(plain).size).toBe(4);
    const kinds = ["alternative", "parallel", "repeat"].map((kind) => dashOf(edge({ type: "a", kind }), types));
    expect(new Set([...plain, ...kinds]).size).toBe(7);
    expect(plain.every((dash) => dash === "none" || Number(dash.split(" ")[0]) >= 3)).toBe(true);
  });

  it("shows an unknown kind raw, with its type's dash", () => {
    const unknown = edge({ type: "b", kind: "wander" });
    expect(dashOf(unknown, ["a", "b"])).toBe(dashOf(edge({ type: "b" }), ["a", "b"]));
    expect(edgeWords(unknown)).toEqual(["wander"]);
  });

  it("orders types by name over the top read's edges and each node's inner, so folding moves no dash", () => {
    const folded = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 3, inner: [{ type: "zebra", count: 2 }] })], [aFlowEdge({ from: "G", to: "G", type: "yak" })]);
    expect(typesOf(folded)).toEqual(["yak", "zebra"]);
    expect(typeColour("zebra", ["yak", "zebra"])).toBe("var(--flow-edge-1)");
    expect(typeColour("nobody", ["yak", "zebra"])).toBe("var(--flow-edge-0)");
  });

  it("colours and dashes an edge on the canvas: a repeat dotted on a curve and on an arc", async () => {
    const reads = aFlowView(aLine(["A", "B", "C"]), [
      aFlowEdge({ from: "A", to: "B", kind: "repeat", type: TYPE_B }),
      aFlowEdge({ from: "C", to: "A", kind: "repeat", type: TYPE_A }),
      aFlowEdge({ from: "B", to: "C", type: TYPE_A }),
    ]);
    renderApp(flowClient(reads), "#/alpha/flow");
    await drawn("C");
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__edge-path")).toHaveLength(3);
    });
    const paths = Array.from(document.querySelectorAll<SVGPathElement>(".react-flow__edge-path"));
    const dashes = paths.map((path) => path.style.strokeDasharray);
    expect(dashes.filter((dash) => dash === "1 7")).toHaveLength(2);
    expect(paths.filter((path) => path.style.strokeWidth === "var(--flow-repeat-width)")).toHaveLength(2);
    expect(paths.map((path) => path.style.stroke)).toEqual(expect.arrayContaining(["var(--flow-edge-0)", "var(--flow-edge-1)"]));
  });

  it("says an edge's kind, its when clipped, its count; a plain edge says nothing", () => {
    expect(edgeWords(edge({ kind: "alternative", when: "after dark", count: 1 }))).toEqual(["alternative", "after dark"]);
    expect(edgeWords(edge({ count: 3 }))).toEqual(["x3"]);
    expect(edgeWords(edge({}))).toEqual([]);
    const long = String.fromCodePoint(0x437).repeat(WHEN_SHOWN + 5);
    expect(Array.from(shortWhen(long))).toHaveLength(WHEN_SHOWN + 1);
    expect(shortWhen("a".repeat(WHEN_SHOWN))).toBe("a".repeat(WHEN_SHOWN));
  });

  it("names each type and kind in the legend, in the daemon's words, with the arc's meaning", async () => {
    renderApp(flowClient(aFlowView(aLine(["A", "B"]), [aFlowEdge({ from: "A", to: "B", type: TYPE_B }), aFlowEdge({ from: "B", to: "A", type: TYPE_A })])), "#/alpha/flow");
    await drawn("B");
    const legend = within(screen.getByRole("region", { name: "Legend" }));
    for (const word of [TYPE_A, TYPE_B, "alternative", "parallel", "repeat"]) {
      expect(legend.getByText(word)).toBeTruthy();
    }
    expect(legend.getByText(/runs against the walk$/)).toBeTruthy();
    expect(legend.getByText("Drawn with React Flow (MIT)")).toBeTruthy();
  });
});

describe("gap rows (AC-08)", () => {
  it("lists the ten as served, in words, unknown states last, a row's meaning under its code", async () => {
    const gaps = tenGapRows([aFlowGap("two-starts", [["A", "B"]]), aFlowGap("unreachable", [], { state: "weighed", count: null })]);
    renderApp(flowClient(aFlowView(aLine(["A", "B"]), [], { gaps })), "#/alpha/flow");
    await top();
    const codes = Array.from(gapsRegion().querySelectorAll(".flow-gap-code")).map((element) => element.textContent);
    expect(codes).toEqual(["no-start", "two-starts", "no-way-to-end", "trap-cycle", "unjoined-parallel", "cycle-no-repeat", "definition-unused", "definition-once", "supply-unfed", "unreachable"]);
    expect(within(gapRow("no-start")).getByText("None")).toBeTruthy();
    expect(within(gapRow("two-starts")).getByText("1 found")).toBeTruthy();
    expect(within(gapRow("unreachable")).getByText("weighed").closest(".badge")?.getAttribute("data-tone")).toBe("flow-gap-unknown");
    expect(within(gapRow("no-start")).getByText("Nothing starts the flow.")).toBeTruthy();
  });

  it("reads a found row's remedy verbatim under its meaning; a row at None holds none (AC-02 of flow-authoring)", async () => {
    const gaps = tenGapRows([aFlowGap("two-starts", [["A", "B"]])]);
    renderApp(flowClient(aFlowView(aLine(["A"]), [], { gaps })), "#/alpha/flow");
    await top();
    const cause = gapRow("two-starts").querySelector(".flow-gap-cause");
    expect(cause?.textContent).toBe("more than one start: narrow [[flow.start]] to one");
    expect(cause?.previousElementSibling?.textContent).toBe("The flow starts in more than one place.");
    expect(gapRow("no-start").querySelector(".flow-gap-cause")).toBeNull();
  });

  it("reads Not configured with its cause verbatim and no number, and says how many rows need rules", async () => {
    const cause = "no [[flow.start]] rule: add one naming the first step by paths";
    const gaps = tenGapRows([aFlowGap("no-start", [], { state: "not_configured", count: null, cause }), aFlowGap("two-starts", [], { state: "not_configured", count: null, cause })]);
    renderApp(flowClient(aFlowView(aLine(["A"]), [], { gaps })), "#/alpha/flow");
    await top();
    const row = gapRow("no-start");
    expect(within(row).getByText("Not configured")).toBeTruthy();
    expect(within(row).getByText(cause)).toBeTruthy();
    expect(row.textContent).not.toMatch(/\d/);
    expect(within(gapsRegion()).getByText("2 rows need [flow] rules in specengine.toml")).toBeTruthy();
  });

  it("reads Cannot measure in cannot-verify's colour and icon, the cause verbatim, never a number or None's look", async () => {
    const cause = "specengine.toml:14: `flow.start`: a rule takes at least one selector";
    const gaps = tenGapRows([aFlowGap("unreachable", [], { state: "cannot", count: null, cause })]);
    renderApp(flowClient(aFlowView(aLine(["A"]), [], { gaps })), "#/alpha/flow");
    await top();
    const row = gapRow("unreachable");
    const badge = within(row).getByText("Cannot measure").closest(".badge");
    expect(badge?.getAttribute("data-tone")).toBe("sync-cannot-verify");
    expect(badge?.querySelector("path")?.getAttribute("d")).toContain("M4.2 11.8l7.6-7.6");
    expect(within(row).getByText(cause)).toBeTruthy();
    expect(row.textContent.replace(cause, "")).not.toMatch(/\d/);
    expect(within(gapsRegion()).queryByText(/need \[flow\] rules/)).toBeNull();
  });

  it("never reads a row with no count as none: a measured row with no number shows raw", async () => {
    const gaps = tenGapRows([aFlowGap("trap-cycle", [], { count: null })]);
    renderApp(flowClient(aFlowView(aLine(["A"]), [], { gaps })), "#/alpha/flow");
    await top();
    const row = gapRow("trap-cycle");
    expect(within(row).queryByText("None")).toBeNull();
    expect(within(row).getByText("measured").closest(".badge")?.getAttribute("data-tone")).toBe("flow-gap-unknown");
  });

  it("shows no group read's rows", async () => {
    const group = aGroupRead("G", aLine(["X"]), [], { gaps: tenGapRows([aFlowGap("unreachable", [["X"], ["OTHER"]])]) });
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 })], [], { gaps: tenGapRows([aFlowGap("trap-cycle", [["G"]])]) });
    renderApp(flowClient(reads, { G: group }), "#/alpha/flow");
    await drawn("X");
    expect(within(gapRow("unreachable")).getByText("None")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "OTHER" })).toBeNull();
  });

  it("shows eight items, then Show all <n>; members joined by +; each a button, but no-start's scope", async () => {
    const items = Array.from({ length: 11 }, (_, index) => [`N-${String(index)}`]);
    const gaps = tenGapRows([aFlowGap("no-start", [["all"]]), aFlowGap("unreachable", items), aFlowGap("trap-cycle", [["T-A", "T-B"]])]);
    renderApp(flowClient(aFlowView(aLine(["A"]), [], { gaps })), "#/alpha/flow");
    await top();
    expect(within(gapRow("unreachable")).getAllByRole("button").map((button) => button.textContent)).toEqual([...items.slice(0, 8).flat(), "Show all 11 of unreachable"]);
    fireEvent.click(within(gapRow("unreachable")).getByRole("button", { name: /^Show all 11/ }));
    expect(within(gapRow("unreachable")).getAllByRole("button")).toHaveLength(11);
    expect(gapRow("trap-cycle").querySelector(".flow-gap-item")?.textContent).toBe("T-A + T-B");
    expect(within(gapRow("trap-cycle")).getAllByRole("button")).toHaveLength(2);
    expect(within(gapRow("no-start")).getByText("all")).toBeTruthy();
    expect(within(gapRow("no-start")).queryByRole("button")).toBeNull();
  });

  it("changes no status or control: every row at a count leaves the picture's controls as they were (ADR-0012)", async () => {
    const gaps = tenGapRows([aFlowGap("trap-cycle", [["A"]])]);
    renderApp(flowClient(aFlowView(aLine(["A", "B"]), [], { gaps })), "#/alpha/flow");
    await drawn("B");
    expect(document.querySelector("[aria-disabled='true']")).toBeNull();
    expect(screen.getByRole("tab", { name: "List" }).getAttribute("aria-disabled")).toBeNull();
  });
});

describe("gap selection (AC-09)", () => {
  const treeRows = [aTreeNode({ id: "G", depth: 0 }), aTreeNode({ id: "H", depth: 1, parent: "G" }), aTreeNode({ id: "DEEP", depth: 2, parent: "H" })];
  const reads = () => ({
    top: aFlowView([aFlowNode({ id: "G", distance: 0, holds: 3, gaps: 1 }), aFlowNode({ id: "N", distance: 1 }), aFlowNode({ id: "KEPT", distance: 2, holds: 1 })], [], { gaps: tenGapRows([aFlowGap("trap-cycle", [["DEEP", "N"]])]) }),
    groups: { G: aGroupRead("G", [aFlowNode({ id: "H", distance: 0, holds: 1, gaps: 1 })]), H: aGroupRead("H", [aFlowNode({ id: "DEEP", distance: 0, gaps: 1 })]) },
  });

  it("shows the closed group's gaps, then calls getTree once, runs both group reads, routes the first member, marks all, keeps focus", async () => {
    const { top: flow, groups } = reads();
    const client = flowClient(flow, groups);
    client.getTree.mockImplementation(() => Promise.resolve(aTreeView(treeRows)));
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    expect(box("G").textContent).toContain("1 with gaps");
    const member = within(gapRow("trap-cycle")).getByRole("button", { name: "DEEP" });
    member.focus();
    fireEvent.click(member);
    await drawn("DEEP");
    expect(client.getTree).toHaveBeenCalledTimes(1);
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([{ depth: 0, trace: true }, { root: "G", depth: 1, trace: true }, { root: "H", depth: 1, trace: true }]);
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/DEEP");
    });
    expect(box("DEEP").getAttribute("aria-label")).toContain("marked");
    expect(box("N").getAttribute("aria-label")).toContain("marked");
    expect(box("DEEP").getAttribute("aria-label")).toContain("selected");
    expect(document.activeElement).toBe(within(gapRow("trap-cycle")).getByRole("button", { name: "DEEP" }));
  });

  it("reads no tree for a member already drawn", async () => {
    const reads2 = aFlowView(aLine(["A", "B"]), [], { gaps: tenGapRows([aFlowGap("unreachable", [["B"]])]) });
    const client = flowClient(reads2);
    renderApp(client, "#/alpha/flow");
    await drawn("B");
    fireEvent.click(within(gapRow("unreachable")).getByRole("button", { name: "B" }));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/flow/B");
    });
    expect(client.getTree).not.toHaveBeenCalled();
    expect(box("B").getAttribute("aria-label")).toContain("selected");
  });

  it("keeps the picture when the tree fails, with a line in the daemon's words", async () => {
    const { top: flow, groups } = reads();
    const client = flowClient(flow, groups);
    client.getTree.mockImplementation(() => Promise.reject(new ClientError({ status: 503, message: "spec: the index is locked" })));
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(within(gapRow("trap-cycle")).getByRole("button", { name: "DEEP" }));
    expect(await screen.findByText(/could not be read: spec: the index is locked/)).toBeTruthy();
    expect(hasBox("G")).toBe(true);
    expect(hasBox("DEEP")).toBe(false);
    expect(client.getFlow).toHaveBeenCalledTimes(1);
  });

  it("scrolls the List to the row, on the List", async () => {
    const reads2 = aFlowView(aLine(["A", "B"]), [], { gaps: tenGapRows([aFlowGap("unreachable", [["B"]])]) });
    renderApp(flowClient(reads2), "#/alpha/flow");
    await drawn("B");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const scrolled = vi.spyOn(Element.prototype, "scrollIntoView");
    fireEvent.click(within(gapRow("unreachable")).getByRole("button", { name: "B" }));
    await waitFor(() => {
      expect(scrolled).toHaveBeenCalled();
    });
    expect((scrolled.mock.contexts[0] as HTMLElement).dataset.flowRow).toBe("B");
  });
});

describe("the panel (AC-10)", () => {
  const client = () => {
    const stub = flowClient(aFlowView([aFlowNode({ id: "G", distance: 2, holds: 4, gaps: 2, inner: [{ type: TYPE_A, count: 3 }] }), aFlowNode({ id: "S", distance: 0, path: "docs/spec/s.md", line: 7, title: "The step" }), aFlowNode({ id: "KEPT", distance: 3, holds: 1 })], []));
    return stub;
  };

  it("routes a click, shows the box's words, place, text, links and the ways out; only reads go out", async () => {
    const stub = client();
    renderApp(stub, "#/alpha/flow");
    await drawn("S");
    fireEvent.click(box("S"));
    const heading = await screen.findByRole("heading", { level: 2, name: "S" });
    expect(window.location.hash).toBe("#/alpha/flow/S");
    const panel = heading.closest("aside");
    if (panel === null) {
      throw new Error("no panel");
    }
    const inside = within(panel);
    expect(inside.getByText("The step")).toBeTruthy();
    expect(inside.getByText("docs/spec/s.md:7")).toBeTruthy();
    expect(inside.getByRole("link", { name: "Open in Spec tree" }).getAttribute("href")).toBe(sectionHash("alpha", "tree", "S"));
    expect(inside.getByRole("link", { name: "Open in Graph" }).getAttribute("href")).toBe(sectionHash("alpha", "graph", "S"));
    expect(inside.getByText("Changes go through your AI, as proposals.")).toBeTruthy();
    await waitFor(() => {
      expect(stub.getNode).toHaveBeenCalledWith("alpha", "S", { with: ["links"] }, expect.any(AbortSignal));
    });
    expect(stub.stageDecision).not.toHaveBeenCalled();
    expect(stub.unstageDecision).not.toHaveBeenCalled();
    expect(stub.stageRoundAnswers).not.toHaveBeenCalled();
  });

  it("shows a group's inner edges and opens it from there", async () => {
    const stub = client();
    const real = stub.getFlow.getMockImplementation();
    stub.getFlow.mockImplementation((project, options, signal) => (options.root === "G" ? Promise.resolve(aGroupRead("G", aLine(["X"]))) : (real?.(project, options, signal) ?? Promise.reject(new Error("no")))));
    renderApp(stub, "#/alpha/flow/G");
    await drawn("G");
    const open = await screen.findByRole("button", { name: "Open the group" });
    expect(within(document.querySelector<HTMLElement>(".flow-details") ?? document.body).getByText(TYPE_A).closest("li")?.textContent).toBe(`${TYPE_A} x3 within`);
    expect(screen.getByText("4 documents, 2 with gaps")).toBeTruthy();
    fireEvent.click(open);
    await waitFor(() => {
      expect(stub.getFlow).toHaveBeenCalledWith("alpha", { root: "G", depth: 1, trace: true }, expect.any(AbortSignal));
    });
    expect(await screen.findByRole("button", { name: "Close the group" })).toBeTruthy();
  });

  it("closes on Esc, the route back to the flow, focus back on the box", async () => {
    renderApp(client(), "#/alpha/flow");
    await drawn("S");
    key("S", "Enter");
    const heading = await screen.findByRole("heading", { level: 2, name: "S" });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
    fireEvent.keyDown(heading, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("heading", { level: 2, name: "S" })).toBeNull();
    });
    expect(window.location.hash).toBe("#/alpha/flow");
    expect(document.activeElement).toBe(box("S"));
  });
});

describe("states (AC-11)", () => {
  it("loading: a skeleton, aria-busy, until the top lands", async () => {
    const client = flowClient(steps);
    const held = deferred<FlowAnswer>();
    client.getFlow.mockImplementation(() => held.promise);
    renderApp(client, "#/alpha/flow");
    expect(await screen.findByRole("status", { name: "Reading the flow of alpha" })).toBeTruthy();
    expect(document.querySelector(".flow-results")?.getAttribute("aria-busy")).toBe("true");
    await act(async () => {
      held.resolve(steps);
      await held.promise;
    });
    await drawn("A");
  });

  it("a group opening stays closed, says Opening, then opens; focus stays", async () => {
    const client = flowClient(steps);
    const held = deferred<FlowAnswer>();
    client.getFlow.mockImplementation((_project, options) =>
      options.root === undefined ? Promise.resolve(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "H", distance: 1, holds: 1 })], [])) : held.promise,
    );
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    key("G", " ");
    await waitFor(() => {
      expect(box("G").textContent).toContain("Opening");
    });
    expect(box("G").getAttribute("aria-label")).toContain("closed group");
    expect(box("G").getAttribute("aria-label")).toContain("opening");
    await act(async () => {
      held.resolve(aGroupRead("G", aLine(["X"])));
      await held.promise;
    });
    await drawn("X");
    expect(box("G").textContent).not.toContain("Opening");
    expect(document.activeElement).toBe(box("G"));
  });

  it("a failed top: the error panel, the daemon's message, Retry", async () => {
    const client = flowClient(steps);
    client.getFlow.mockImplementationOnce(() => Promise.reject(new ClientError({ status: 503, message: "spec: index locked" })));
    renderApp(client, "#/alpha/flow");
    expect(await screen.findByText("spec: index locked")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await drawn("A");
  });

  it("a failed group read keeps the picture, with a line in the daemon's words and Retry", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "K", distance: 1, holds: 1 })], []), { G: aGroupRead("G", aLine(["X"])) });
    const real = client.getFlow.getMockImplementation();
    client.getFlow.mockImplementationOnce(real ?? (() => Promise.reject(new Error("no")))).mockImplementationOnce(() => Promise.reject(new ClientError({ status: 503, message: "spec: group read failed" })));
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(caret("G"));
    expect(await screen.findByText(/could not be read: spec: group read failed/)).toBeTruthy();
    expect(hasBox("K")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: /^Retry/ }));
    await drawn("X");
    expect(screen.queryByText(/could not be read/)).toBeNull();
  });

  it("empty: the meaning and the next step; the rows stay", async () => {
    renderApp(flowClient(aFlowView([], [])), "#/alpha/flow");
    expect(await screen.findByText("No flow in this project yet. Next: have your AI add flow links.")).toBeTruthy();
    expect(gapRow("no-start")).toBeTruthy();
    expect(screen.queryByRole("tab", { name: "Canvas" })).toBeNull();
  });

  it("a name no read or chain finds: a note and a Tree link; the picture stays", async () => {
    const client = flowClient(steps);
    renderApp(client, "#/alpha/flow/NOPE");
    expect(await screen.findByText(/No box here is named/)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Open NOPE in the Spec tree" }).getAttribute("href")).toBe(sectionHash("alpha", "tree", "NOPE"));
    expect(hasBox("A")).toBe(true);
  });

  it("reads the tree once for a deep link, and only after the sole group's read landed: a child of it needs no chain", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 })], []), { G: aGroupRead("G", aLine(["X"])) });
    renderApp(client, "#/alpha/flow/X");
    await drawn("X");
    expect(await screen.findByRole("heading", { level: 2, name: "X" })).toBeTruthy();
    expect(client.getTree).not.toHaveBeenCalled();
  });

  it("reopens nothing when a group round the selected box is closed; the panel stays", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 })], []), { G: aGroupRead("G", aLine(["X"])) });
    renderApp(client, "#/alpha/flow/X");
    await drawn("X");
    fireEvent.click(caret("G"));
    await waitFor(() => {
      expect(hasBox("X")).toBe(false);
    });
    expect(screen.getByRole("heading", { level: 2, name: "X" })).toBeTruthy();
    expect(client.getTree).not.toHaveBeenCalled();
    await act(async () => {
      await Promise.resolve();
    });
    expect(hasBox("X")).toBe(false);
  });

  it("a failed re-read keeps the picture: a line in the daemon's words, Retry", async () => {
    const client = flowClient(steps);
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    client.getFlow.mockImplementationOnce(() => Promise.reject(new ClientError({ status: 503, message: "spec: index locked" })));
    act(() => {
      emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1", commit: "abc" } });
    });
    expect(await screen.findByText(/could not be read again: spec: index locked/)).toBeTruthy();
    expect(hasBox("A")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(screen.queryByText(/could not be read again/)).toBeNull();
    });
  });

  it("a group read with a reason closes, with a note in the daemon's words", async () => {
    const refused = aFlowView([], [], { root: "G", reason: "`G` resolves to no ID and no alias", gaps: [] });
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "H", distance: 1, holds: 1 })], []), { G: refused });
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(caret("G"));
    expect(await screen.findByText("`G` resolves to no ID and no alias")).toBeTruthy();
    expect(box("G").getAttribute("aria-label")).toContain("closed group");
    expect(box("G").getAttribute("aria-label")).not.toContain("opening");
  });

  it("says what was left out and the daemon's notes", async () => {
    renderApp(flowClient(aFlowView(aLine(["A"]), [], { notes: ["no document under [paths] spec"], left_out: { generated: 2, tier3: 1 } })), "#/alpha/flow");
    await drawn("A");
    expect(screen.getByText("no document under [paths] spec")).toBeTruthy();
    expect(screen.getByText("Left out: 2 generated, 1 archived")).toBeTruthy();
  });
});

describe("a seek", () => {
  const tree = [aTreeNode({ id: "G", depth: 0 }), aTreeNode({ id: "H", depth: 1, parent: "G" }), aTreeNode({ id: "DEEP", depth: 2, parent: "H" })];
  const refusal = "`G` resolves to no ID and no alias";
  const refused = aFlowView([], [], { root: "G", reason: refusal, gaps: [] });
  const flow = () => aFlowView([aFlowNode({ id: "G", distance: 0, holds: 2 }), aFlowNode({ id: "K", distance: 1, holds: 1 })], [], { gaps: tenGapRows([aFlowGap("trap-cycle", [["DEEP"]])]) });
  const readsOfG = (client: ReturnType<typeof flowClient>) => client.getFlow.mock.calls.filter((call) => call[1].root === "G");

  it("ends at a group whose read has a reason: it stays closed, the note says so, the name is lost, the view lives", async () => {
    const client = flowClient(flow(), { G: refused });
    client.getTree.mockImplementation(() => Promise.resolve(aTreeView(tree)));
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(within(gapRow("trap-cycle")).getByRole("button", { name: "DEEP" }));
    expect(await screen.findByText(refusal)).toBeTruthy();
    expect(await screen.findByText(/No box here is named/)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Open DEEP in the Spec tree" })).toBeTruthy();
    expect(box("G").getAttribute("aria-label")).toContain("closed group");
    expect(readsOfG(client)).toHaveLength(1);
    expect(client.getTree).toHaveBeenCalledTimes(1);
  });

  it("does the same from a deep link", async () => {
    const client = flowClient(flow(), { G: refused });
    client.getTree.mockImplementation(() => Promise.resolve(aTreeView(tree)));
    renderApp(client, "#/alpha/flow/DEEP");
    expect(await screen.findByText(/No box here is named/)).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Flow" })).toBeTruthy();
    expect(readsOfG(client)).toHaveLength(1);
  });

  it("is released once the chain is run out: the owner's next close of a group on it stands", async () => {
    const client = flowClient(flow(), { G: aGroupRead("G", [aFlowNode({ id: "H", distance: 0, holds: 1 })]), H: aGroupRead("H", aLine(["OTHER"])) });
    client.getTree.mockImplementation(() => Promise.resolve(aTreeView(tree)));
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(within(gapRow("trap-cycle")).getByRole("button", { name: "DEEP" }));
    await drawn("OTHER");
    expect(await screen.findByText(/No box here is named/)).toBeTruthy();
    fireEvent.click(caret("H"));
    await waitFor(() => {
      expect(hasBox("OTHER")).toBe(false);
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(hasBox("OTHER")).toBe(false);
  });

  it("ends at the owner's close: a read landing after it opens nothing more", async () => {
    const heldH = deferred<FlowAnswer>();
    const client = flowClient(flow());
    client.getFlow.mockImplementation((_project, options) => {
      if (options.root === undefined) {
        return Promise.resolve(flow());
      }
      return options.root === "G" ? Promise.resolve(aGroupRead("G", [aFlowNode({ id: "H", distance: 0, holds: 1 })])) : heldH.promise;
    });
    client.getTree.mockImplementation(() => Promise.resolve(aTreeView(tree)));
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(within(gapRow("trap-cycle")).getByRole("button", { name: "DEEP" }));
    await drawn("H");
    await waitFor(() => {
      expect(box("H").textContent).toContain("Opening");
    });
    fireEvent.click(caret("G"));
    await waitFor(() => {
      expect(hasBox("H")).toBe(false);
    });
    await act(async () => {
      heldH.resolve(aGroupRead("H", aLine(["DEEP"])));
      await heldH.promise;
    });
    expect(box("G").getAttribute("aria-label")).toContain("closed group");
    expect(hasBox("DEEP")).toBe(false);
    expect(screen.queryByText(/No box here is named/)).toBeNull();
  });

  it("says a refused group's note only while its box is in the reads shown", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "P", distance: 0, holds: 2 })], []), { P: aGroupRead("P", [aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "Q", distance: 1 })]), G: refused });
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(caret("G"));
    expect(await screen.findByText(refusal)).toBeTruthy();
    fireEvent.click(caret("P"));
    await waitFor(() => {
      expect(screen.queryByText(refusal)).toBeNull();
    });
  });
});

describe("focus after a Retry", () => {
  const twoGroups = () => aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "K", distance: 1, holds: 1 })], []);
  const results = () => document.querySelector<HTMLElement>(".flow-results");

  /** A client whose live tail the test can speak on. */
  function live(client: ReturnType<typeof flowClient>) {
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    return () => {
      act(() => {
        emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1", commit: "abc" } });
      });
    };
  }

  it("goes to the results after the top re-read's Retry, on an answer equal to the last", async () => {
    const client = flowClient(steps);
    const applied = live(client);
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    client.getFlow.mockImplementationOnce(() => Promise.reject(new ClientError({ status: 503, message: "spec: index locked" })));
    applied();
    const retry = await screen.findByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(screen.queryByText(/could not be read again/)).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(results());
    });
  });

  it("goes to the group's head after its Retry worked, the line and the button kept up while it runs", async () => {
    const second = deferred<FlowAnswer>();
    const client = flowClient(twoGroups());
    client.getFlow.mockImplementation((_project, options) => (options.root === undefined ? Promise.resolve(twoGroups()) : Promise.reject(new ClientError({ status: 503, message: "spec: group read failed" }))));
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(caret("G"));
    const retry = await screen.findByRole("button", { name: /^Retry/ });
    retry.focus();
    client.getFlow.mockImplementation((_project, options) => (options.root === undefined ? Promise.resolve(twoGroups()) : second.promise));
    fireEvent.click(retry);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /^Retrying/ })).toBeTruthy();
    });
    expect(document.activeElement).toBe(retry);
    expect(screen.getByText(/could not be read: spec: group read failed/)).toBeTruthy();
    await act(async () => {
      second.resolve(aGroupRead("G", aLine(["X"])));
      await second.promise;
    });
    await drawn("X");
    await waitFor(() => {
      expect(document.activeElement).toBe(box("G"));
    });
  });

  it("goes to the results on the List, where no box stands", async () => {
    const client = flowClient(twoGroups());
    let fails = true;
    client.getFlow.mockImplementation((_project, options) => {
      if (options.root === undefined) {
        return Promise.resolve(twoGroups());
      }
      return fails ? Promise.reject(new ClientError({ status: 503, message: "spec: group read failed" })) : Promise.resolve(aGroupRead("G", aLine(["X"])));
    });
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    fireEvent.click(await screen.findByRole("button", { name: "Open G" }));
    const retry = await screen.findByRole("button", { name: /^Retry/ });
    retry.focus();
    fails = false;
    fireEvent.click(retry);
    await waitFor(() => {
      expect(document.activeElement).toBe(results());
    });
  });

  it("stays on Retry when it failed again", async () => {
    const client = flowClient(twoGroups());
    client.getFlow.mockImplementation((_project, options) => (options.root === undefined ? Promise.resolve(twoGroups()) : Promise.reject(new ClientError({ status: 503, message: "spec: group read failed" }))));
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(caret("G"));
    const retry = await screen.findByRole("button", { name: /^Retry/ });
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(client.getFlow.mock.calls.filter((call) => call[1].root === "G")).toHaveLength(2);
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: /^Retry/ }));
  });

  it("hands a focused top box's focus to the tab stop when a re-read takes it away", async () => {
    let shown = steps;
    const client = flowClient(steps);
    client.getFlow.mockImplementation(() => Promise.resolve(shown));
    const applied = live(client);
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    box("C").focus();
    shown = aFlowView(aLine(["A", "B"]), [aFlowEdge({ from: "A", to: "B" })]);
    applied();
    await waitFor(() => {
      expect(hasBox("C")).toBe(false);
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(box("A"));
    });
  });
});

describe("a read that said no, or failed", () => {
  const refusal = "`G` resolves to no ID and no alias";
  const refused = aFlowView([], [], { root: "G", reason: refusal, gaps: [] });
  const flow = () => aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "K", distance: 1, holds: 1 })], []);
  const readsOfG = (client: ReturnType<typeof flowClient>) => client.getFlow.mock.calls.filter((call) => call[1].root === "G");

  it("reads a refused group again on a live event: the note goes, and Space opens it", async () => {
    let answer = refused;
    const client = flowClient(flow());
    client.getFlow.mockImplementation((_project, options) => Promise.resolve(options.root === undefined ? flow() : answer));
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(caret("G"));
    expect(await screen.findByText(refusal)).toBeTruthy();
    answer = aGroupRead("G", aLine(["X"]));
    act(() => {
      emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1", commit: "abc" } });
    });
    await waitFor(() => {
      expect(screen.queryByText(refusal)).toBeNull();
    });
    expect(readsOfG(client)).toHaveLength(2);
    expect(hasBox("X")).toBe(false);
    key("G", " ");
    await drawn("X");
    expect(readsOfG(client)).toHaveLength(2);
  });

  it("reads again when Open is pressed after a failed read: the group is not closed, the line stays until it lands", async () => {
    const client = flowClient(flow());
    let fails = true;
    client.getFlow.mockImplementation((_project, options) => {
      if (options.root === undefined) {
        return Promise.resolve(flow());
      }
      return fails ? Promise.reject(new ClientError({ status: 503, message: "spec: group read failed" })) : Promise.resolve(aGroupRead("G", aLine(["X"])));
    });
    renderApp(client, "#/alpha/flow/G");
    await drawn("G");
    fireEvent.click(await screen.findByRole("button", { name: "Open the group" }));
    expect(await screen.findByText(/could not be read: spec: group read failed/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open the group" }));
    await waitFor(() => {
      expect(readsOfG(client)).toHaveLength(2);
    });
    expect(await screen.findByText(/could not be read: spec: group read failed/)).toBeTruthy();
    fails = false;
    key("G", " ");
    await drawn("X");
    expect(readsOfG(client)).toHaveLength(3);
    expect(screen.queryByText(/could not be read/)).toBeNull();
  });
});

describe("the canvas's width changing", () => {
  const line = aFlowView(aLine(["A", "B", "C", "D", "E", "F"]), []);

  function viewport(): { x: number; y: number; zoom: number } {
    const transform = document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform ?? "";
    const match = /translate\(\s*(-?[\d.]+)px,\s*(-?[\d.]+)px\)\s*scale\(([\d.]+)\)/.exec(transform);
    return { x: Number(match?.[1]), y: Number(match?.[2]), zoom: Number(match?.[3]) };
  }

  /** The canvas's measured width, as React Flow reads it on a resize. */
  function canvasWidth(initial: number) {
    let width = initial;
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(() => width);
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(() => 700);
    return async (next: number) => {
      width = next;
      await act(async () => {
        window.dispatchEvent(new Event("resize"));
        await Promise.resolve();
      });
    };
  }

  it("refits a view the owner has not moved", async () => {
    const resize = canvasWidth(3000);
    renderApp(flowClient(line), "#/alpha/flow");
    await drawn("F");
    await waitFor(() => {
      expect(viewport().zoom).toBe(1);
    });
    await resize(600);
    await waitFor(() => {
      expect(viewport().zoom).toBe(0.6);
    });
    expect(viewport().x).toBe(32);
  });

  it("leaves a view the owner has moved where it is while no box is selected", async () => {
    const resize = canvasWidth(1200);
    renderApp(flowClient(line), "#/alpha/flow");
    await drawn("F");
    await waitFor(() => {
      expect(viewport().zoom).toBe(0.6);
    });
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    await waitFor(() => {
      expect(viewport().zoom).toBeGreaterThan(0.6);
    });
    key("A", "c");
    const before = await waitFor(() => {
      const seen = viewport();
      expect(seen.zoom).toBeGreaterThan(0.6);
      return seen;
    });
    await resize(800);
    await act(async () => {
      await Promise.resolve();
    });
    expect(viewport()).toEqual(before);
  });

  it("brings the selected box back into sight when the panel's column narrows the canvas", async () => {
    window.localStorage.setItem("specengine.flow.gaps-hidden", "on");
    const resize = canvasWidth(2000);
    renderApp(flowClient(line), "#/alpha/flow");
    await drawn("D");
    await waitFor(() => {
      expect(viewport().zoom).toBeGreaterThan(0.6);
    });
    key("D", "c");
    await act(async () => {
      await Promise.resolve();
    });
    fireEvent.click(box("D"));
    await screen.findByRole("heading", { level: 2, name: "D" });
    const wide = viewport();
    const left = (from: { x: number; zoom: number }) => from.x + (LANE_PAD + 3 * (BOX_WIDTH + COLUMN_GAP)) * from.zoom;
    expect(left(wide)).toBeGreaterThan(700);
    await resize(700);
    await waitFor(() => {
      const seen = viewport();
      expect(seen.x).not.toBe(wide.x);
      expect(left(seen)).toBeGreaterThanOrEqual(0);
      expect(left(seen) + BOX_WIDTH * seen.zoom).toBeLessThanOrEqual(700);
    });
  });
});

describe("the side column", () => {
  const found = aFlowView(aLine(["A", "B"]), [aFlowEdge({ from: "A", to: "B" })], { gaps: tenGapRows([aFlowGap("unreachable", [["B"]])]) });

  it("hides the gaps and shows them again; the choice is the viewer's, kept across a reload", async () => {
    const client = flowClient(found);
    const first = renderApp(client, "#/alpha/flow");
    await drawn("A");
    const hide = screen.getByRole("button", { name: "Hide gaps" });
    expect(hide.getAttribute("aria-expanded")).toBe("true");
    hide.focus();
    fireEvent.click(hide);
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
    expect(document.querySelector(".flow-stage")?.classList.contains("no-side")).toBe(true);
    expect(document.activeElement).toBe(hide);
    expect(hide.textContent).toBe("Show gaps");
    first.unmount();
    renderApp(flowClient(found), "#/beta/flow");
    await drawn("A");
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show gaps" }));
    expect(screen.getByRole("region", { name: "Gaps" })).toBeTruthy();
    expect(window.localStorage.length).toBe(0);
  });

  it("keeps the panel's column while a box is selected", async () => {
    window.localStorage.setItem("specengine.flow.gaps-hidden", "on");
    renderApp(flowClient(found), "#/alpha/flow/B");
    await screen.findByRole("heading", { level: 2, name: "B" });
    const stage = document.querySelector(".flow-stage");
    expect([stage?.classList.contains("no-gaps"), stage?.classList.contains("no-side")]).toEqual([true, false]);
  });

  it("shows the rows on an empty flow whatever was chosen: they are all there is", async () => {
    window.localStorage.setItem("specengine.flow.gaps-hidden", "on");
    renderApp(flowClient(aFlowView([], [])), "#/alpha/flow");
    expect(await screen.findByRole("region", { name: "Gaps" })).toBeTruthy();
  });

  it("works when the browser refuses its storage", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied");
    });
    renderApp(flowClient(found), "#/alpha/flow");
    await drawn("A");
    fireEvent.click(screen.getByRole("button", { name: "Hide gaps" }));
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
  });
});

describe("a clean flow (docs/features/screen-fit.md)", () => {
  const CLEAR_LINE = "No gaps: every row reads None";
  const clean = aFlowView(aLine(["A", "B"]), []);
  const withRows = (rows: FlowGap[]) => aFlowView(aLine(["A", "B"]), [], { gaps: tenGapRows(rows) });
  const oneFound = withRows([aFlowGap("unreachable", [["B"]])]);
  const results = () => document.querySelector<HTMLElement>(".flow-results");

  /** Whether the flow's stage holds `no-gaps` and `no-side`. */
  function sideClasses(): [boolean | undefined, boolean | undefined] {
    const stage = document.querySelector(".flow-stage");
    return [stage?.classList.contains("no-gaps"), stage?.classList.contains("no-side")];
  }

  function emitApplied(tail: ReturnType<typeof tailOf>) {
    act(() => {
      tail.emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
  }

  it("says one line in the tools, after the stage filter, in place of the rows and their button; the side column goes (AC-03)", async () => {
    renderApp(flowClient(clean), "#/alpha/flow");
    await drawn("A");
    const tools = document.querySelector<HTMLElement>(".flow-tools");
    expect(tools).not.toBeNull();
    const line = within(tools ?? document.body).getByRole("status");
    expect(line.textContent).toBe(CLEAR_LINE);
    expect(line.querySelector("svg")).not.toBeNull();
    expect(line.previousElementSibling?.classList.contains("flow-stage-filter")).toBe(true);
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
    expect(sideClasses()).toEqual([true, true]);
    expect(screen.queryByRole("button", { name: "Hide gaps" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Show gaps" })).toBeNull();
  });

  it("keeps the line on every tab: the rows stood outside them", async () => {
    renderApp(stagedClient(clean, [aStage("print", { no_card: 1 })]), "#/alpha/flow");
    await drawn("A");
    for (const tab of ["List", "Timeline"]) {
      fireEvent.click(screen.getByRole("tab", { name: tab }));
      expect(screen.getByText(CLEAR_LINE)).toBeTruthy();
    }
  });

  it("one row found: all ten as today, no line, the button back, the column kept (AC-04)", async () => {
    renderApp(flowClient(oneFound), "#/alpha/flow");
    await drawn("A");
    expect(gapsRegion().querySelectorAll(".flow-gap")).toHaveLength(10);
    expect(screen.queryByText(CLEAR_LINE)).toBeNull();
    expect(screen.getByRole("button", { name: "Hide gaps" })).toBeTruthy();
    expect(sideClasses()).toEqual([false, false]);
  });

  it.each([
    ["Not configured", aFlowGap("no-start", [], { state: "not_configured", count: null, cause: "no [[flow.start]] rule: add one naming the first step by paths" })],
    ["Cannot measure", aFlowGap("supply-unfed", [], { state: "cannot", count: null, cause: "a cause" })],
    ["an unknown state", aFlowGap("trap-cycle", [], { state: "weighed", count: 0 })],
    ["a measured row with no count", aFlowGap("trap-cycle", [], { count: null })],
  ])("draws the rows beside %s: a row that says no None is something to say", async (_name, row) => {
    renderApp(flowClient(withRows([row])), "#/alpha/flow");
    await drawn("A");
    expect(gapsRegion().querySelectorAll(".flow-gap")).toHaveLength(10);
    expect(screen.queryByText(CLEAR_LINE)).toBeNull();
  });

  it("keeps the panel's column for a selected box: no-gaps alone (AC-04)", async () => {
    renderApp(flowClient(clean), "#/alpha/flow/B");
    await screen.findByRole("heading", { level: 2, name: "B" });
    expect(sideClasses()).toEqual([true, false]);
    expect(screen.getByText(CLEAR_LINE)).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
  });

  it("a failed re-read of a clean flow: every row Not read, never the line; a read that lands brings the line back (AC-05)", async () => {
    const client = flowClient(clean);
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await screen.findByText(CLEAR_LINE);
    client.getFlow.mockRejectedValue(new ClientError({ status: 503, message: "spec: index locked" }));
    emitApplied(tail);
    await screen.findByText(/The flow could not be read again/);
    expect(screen.queryByText(CLEAR_LINE)).toBeNull();
    const states = Array.from(gapsRegion().querySelectorAll(".flow-gap .badge"), (badge) => badge.textContent);
    expect(states).toEqual(Array.from({ length: 10 }, () => "State: Not read"));
    expect(sideClasses()).toEqual([false, false]);
    client.getFlow.mockImplementation(() => Promise.resolve(clean));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText(CLEAR_LINE)).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
  });

  it("brings the rows back through the same read when a row turns up, with no reload", async () => {
    let shown = clean;
    const client = flowClient(clean);
    client.getFlow.mockImplementation(() => Promise.resolve(shown));
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await screen.findByText(CLEAR_LINE);
    shown = oneFound;
    emitApplied(tail);
    expect(await screen.findByRole("region", { name: "Gaps" })).toBeTruthy();
    expect(screen.queryByText(CLEAR_LINE)).toBeNull();
    expect(screen.getByRole("button", { name: "Hide gaps" })).toBeTruthy();
    expect(sideClasses()).toEqual([false, false]);
  });

  it("leaves a stored Hide gaps as it is; it holds again when a row is found", async () => {
    window.localStorage.setItem("specengine.flow.gaps-hidden", "on");
    let shown = clean;
    const client = flowClient(clean);
    client.getFlow.mockImplementation(() => Promise.resolve(shown));
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await screen.findByText(CLEAR_LINE);
    expect(window.localStorage.getItem("specengine.flow.gaps-hidden")).toBe("on");
    shown = oneFound;
    emitApplied(tail);
    expect(await screen.findByRole("button", { name: "Show gaps" })).toBeTruthy();
    expect(screen.queryByText(CLEAR_LINE)).toBeNull();
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
    expect(window.localStorage.getItem("specengine.flow.gaps-hidden")).toBe("on");
  });

  it("an empty flow keeps its rows, whatever was chosen: no tools to hold the line", async () => {
    window.localStorage.setItem("specengine.flow.gaps-hidden", "on");
    renderApp(flowClient(aFlowView([], [])), "#/alpha/flow");
    expect(await screen.findByRole("region", { name: "Gaps" })).toBeTruthy();
    expect(screen.queryByText(CLEAR_LINE)).toBeNull();
    expect(sideClasses()).toEqual([false, false]);
  });

  it("reads Top's rows alone: a group's found row draws no rows and leaves the line", async () => {
    const group = aGroupRead("G", aLine(["X"]), [], { gaps: tenGapRows([aFlowGap("unreachable", [["X"]])]) });
    renderApp(flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 })], []), { G: group }), "#/alpha/flow");
    await drawn("X");
    expect(screen.getByText(CLEAR_LINE)).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Gaps" })).toBeNull();
  });

  it("hands focus to the results when a clean read takes the button that had it, never to the body", async () => {
    let shown = oneFound;
    const client = flowClient(oneFound);
    client.getFlow.mockImplementation(() => Promise.resolve(shown));
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    screen.getByRole("button", { name: "Hide gaps" }).focus();
    shown = clean;
    emitApplied(tail);
    await screen.findByText(CLEAR_LINE);
    await waitFor(() => {
      expect(document.activeElement).toBe(results());
    });
  });

  it("takes no focus on a clean first read", async () => {
    renderApp(flowClient(clean), "#/alpha/flow");
    await screen.findByText(CLEAR_LINE);
    await act(async () => {
      await Promise.resolve();
    });
    expect(document.activeElement).not.toBe(results());
  });
});

describe("a group asked open and not drawn open", () => {
  const room = aFlowView(
    [aFlowNode({ id: "G", distance: 0, holds: 1 }), ...Array.from({ length: 199 }, (_, index) => aFlowNode({ id: `N-${String(index)}`, distance: 1 })), aFlowNode({ id: "K", distance: 2, holds: 1 })],
    [],
  );

  it("reads as closed while its read is on its way: aria-expanded and the words say so, a press adds nothing", async () => {
    const held = deferred<FlowAnswer>();
    const flow = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "K", distance: 1, holds: 1 })], []);
    const client = flowClient(flow);
    client.getFlow.mockImplementation((_project, options) => (options.root === undefined ? Promise.resolve(flow) : held.promise));
    renderApp(client, "#/alpha/flow/G");
    const button = await screen.findByRole("button", { name: "Open the group" });
    fireEvent.click(button);
    await waitFor(() => {
      expect(box("G").textContent).toContain("Opening");
    });
    expect(screen.getByRole("button", { name: "Open the group" }).getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(screen.getByRole("button", { name: "Open the group" }));
    expect(box("G").textContent).toContain("Opening");
    await act(async () => {
      held.resolve(aGroupRead("G", aLine(["X"])));
      await held.promise;
    });
    await drawn("X");
    expect(screen.getByRole("button", { name: "Close the group" }).getAttribute("aria-expanded")).toBe("true");
  });

  it("reads as closed with no room left; pressing Open says why and asks nothing", async () => {
    const client = flowClient(room, { G: aGroupRead("G", aLine(["X"])) });
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const panel = await screen.findByRole("tabpanel", { name: "List" });
    fireEvent.click(within(panel).getByRole("button", { name: "Open G" }));
    await waitFor(() => {
      expect(within(panel).getByRole("link", { name: "X" })).toBeTruthy();
    });
    const again = within(panel).getByRole("button", { name: "Open G" });
    expect(again.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(again);
    expect(within(panel).getByRole("link", { name: "X" })).toBeTruthy();
    expect(client.getFlow.mock.calls.filter((call) => call[1].root === "G")).toHaveLength(1);
  });
});

describe("the sole top group", () => {
  it("starts open beside plain boxes; two groups do not", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "S", distance: 1 })], []), { G: aGroupRead("G", aLine(["X"])) });
    renderApp(client, "#/alpha/flow");
    await drawn("X");
    expect(box("G").getAttribute("aria-label")).toContain("open group");
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([{ depth: 0, trace: true }, { root: "G", depth: 1, trace: true }]);
  });

  it("leaves two top groups closed: no group read on load", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "H", distance: 1, holds: 1 })], []));
    renderApp(client, "#/alpha/flow");
    await drawn("H");
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([{ depth: 0, trace: true }]);
  });

  it("stays closed once closed, for the page's life", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 })], []), { G: aGroupRead("G", aLine(["X"])) });
    renderApp(client, "#/alpha/flow");
    await drawn("X");
    fireEvent.click(caret("G"));
    await waitFor(() => {
      expect(hasBox("X")).toBe(false);
    });
    fireEvent.click(within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Tasks" }));
    window.location.hash = "#/alpha/tasks";
    await screen.findByRole("heading", { level: 1, name: "Tasks" });
    window.location.hash = "#/alpha/flow";
    await drawn("G");
    expect(hasBox("X")).toBe(false);
  });
});

describe("the List", () => {
  it("holds each read, its boxes, a closed group's Open, each edge as spec flow prints it", async () => {
    const edge = aFlowEdge({ from: "G", to: "S", type: TYPE_A, count: 2, kind: "alternative", path: "docs/spec/s.md", line: 9 });
    const inner = aFlowEdge({ from: "X", to: "Y", when: "a very long condition that the canvas clips", path: "docs/spec/y.md", line: 4 });
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 2 }), aFlowNode({ id: "S", distance: 1 }), aFlowNode({ id: "K", distance: 2, holds: 1 })], [edge]), { G: aGroupRead("G", aLine(["X", "Y"]), [inner]) });
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const panel = await screen.findByRole("tabpanel", { name: "List" });
    expect(within(panel).getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual(["Top (3 boxes, 1 edge)"]);
    expect(within(panel).getByText("G -> S | alpha_link | x2 | kind alternative | docs/spec/s.md:9")).toBeTruthy();
    fireEvent.click(within(panel).getByRole("button", { name: "Open G" }));
    await waitFor(() => {
      expect(within(panel).getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual(["Top (3 boxes, 1 edge)", "G (2 boxes, 1 edge)"]);
    });
    expect(within(panel).getByText(edgeLine(inner))).toBeTruthy();
    expect(within(panel).getByRole("link", { name: "X" }).getAttribute("href")).toBe(sectionHash("alpha", "flow", "X"));
    expect(within(panel).getByRole("button", { name: "Close G" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "Open K" })).toBeTruthy();
  });

  it("marks what the canvas marks: a step a row names, a closed group's count, an open group only if named", async () => {
    const gaps = tenGapRows([aFlowGap("unreachable", [["S"], ["G"]])]);
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 2, gaps: 2 }), aFlowNode({ id: "S", distance: 1 }), aFlowNode({ id: "K", distance: 2, holds: 3, gaps: 2 }), aFlowNode({ id: "O", distance: 3, holds: 1, gaps: 1 })], [], { gaps });
    const client = flowClient(reads, { G: aGroupRead("G", aLine(["X"])), O: aGroupRead("O", aLine(["Y"])) });
    renderApp(client, "#/alpha/flow");
    await drawn("O");
    fireEvent.click(caret("G"));
    await drawn("X");
    fireEvent.click(caret("O"));
    await drawn("Y");
    const canvas = ["S", "K", "G", "O"].map((name) => box(name).querySelector(".flow-mark-gap")?.textContent ?? null);
    expect(canvas).toEqual(["Gap", "2 with gaps", "Gap", null]);
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const panel = await screen.findByRole("tabpanel", { name: "List" });
    const rows = new Map(Array.from(panel.querySelectorAll<HTMLElement>("[data-flow-row]")).map((row) => [row.dataset.flowRow, row.querySelector(".flow-mark-gap")?.textContent ?? null]));
    expect(["S", "K", "G", "O"].map((name) => rows.get(name))).toEqual(canvas);
  });

  it("marks the selected row as the canvas does, by word and icon", async () => {
    renderApp(flowClient(steps), "#/alpha/flow/B");
    await drawn("B");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const panel = await screen.findByRole("tabpanel", { name: "List" });
    const row = Array.from(panel.querySelectorAll<HTMLElement>("[data-flow-row]")).find((element) => element.dataset.flowRow === "B");
    expect(row?.getAttribute("aria-current")).toBe("true");
    expect(row?.querySelector(".flow-mark-selected")?.textContent).toBe("Selected");
    expect(row?.querySelector(".flow-mark-selected svg")).toBeTruthy();
  });

  it("holds a group's own edge to a child, which the canvas's lane leaves out", async () => {
    const own = aFlowEdge({ from: "G", to: "X", path: "docs/spec/x.md", line: 2 });
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 })], []), { G: aGroupRead("G", aLine(["X"]), [own]) });
    renderApp(client, "#/alpha/flow");
    await drawn("X");
    expect(document.querySelectorAll(".react-flow__edge")).toHaveLength(0);
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(await screen.findByText(edgeLine(own))).toBeTruthy();
  });
});

describe("keyboard (AC-13)", () => {
  const nodes: FlowNode[] = [aFlowNode({ id: "G", distance: 0, holds: 2 }), aFlowNode({ id: "AFTER", distance: 1 }), aFlowNode({ id: "NEXT", distance: 1 })];
  const flow = () => flowClient(aFlowView([...nodes, aFlowNode({ id: "OTHER", distance: 2, holds: 1 })], [aFlowEdge({ from: "G", to: "AFTER" })]), { G: aGroupRead("G", aLine(["X", "Y"])) });

  it("holds one tab stop among the boxes, the first box's, and arrows only move it", async () => {
    renderApp(flow(), "#/alpha/flow");
    await drawn("G");
    await waitFor(() => {
      expect(boxes().filter((element) => element.tabIndex === 0).map((element) => element.dataset.id)).toEqual(["n:G"]);
    });
    key("G", "ArrowRight");
    expect(document.activeElement).toBe(box("AFTER"));
    await waitFor(() => {
      expect(boxes().filter((element) => element.tabIndex === 0).map((element) => element.dataset.id)).toEqual(["n:AFTER"]);
    });
    fireEvent.keyDown(box("AFTER"), { key: "ArrowDown" });
    expect(document.activeElement).toBe(box("NEXT"));
    fireEvent.keyDown(box("NEXT"), { key: "ArrowUp" });
    expect(document.activeElement).toBe(box("AFTER"));
    expect(window.location.hash).toBe("#/alpha/flow");
  });

  it("opens a group with Space, focus kept; goes in by Right, out by Left; closes with Space, focus on its head", async () => {
    const client = flow();
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    key("G", " ");
    await drawn("X");
    expect(document.activeElement).toBe(box("G"));
    key("G", "ArrowRight");
    expect(document.activeElement).toBe(box("X"));
    fireEvent.keyDown(box("X"), { key: "ArrowLeft" });
    expect(document.activeElement).toBe(box("G"));
    fireEvent.keyDown(box("G"), { key: " " });
    await waitFor(() => {
      expect(hasBox("X")).toBe(false);
    });
    expect(document.activeElement).toBe(box("G"));
  });

  it("closes and reopens with no new read", async () => {
    const client = flow();
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    key("G", " ");
    await drawn("X");
    fireEvent.keyDown(box("G"), { key: " " });
    await waitFor(() => {
      expect(hasBox("X")).toBe(false);
    });
    fireEvent.keyDown(box("G"), { key: " " });
    await drawn("X");
    expect(client.getFlow).toHaveBeenCalledTimes(2);
  });

  it("does nothing on Space on a step", async () => {
    const client = flow();
    renderApp(client, "#/alpha/flow");
    await drawn("AFTER");
    key("AFTER", " ");
    expect(client.getFlow).toHaveBeenCalledTimes(1);
  });

  it("hands focus to the enclosing head when a focused box leaves the picture", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "G", distance: 0, holds: 2 })], []), { G: aGroupRead("G", aLine(["X", "Y"])) });
    renderApp(client, "#/alpha/flow");
    await drawn("Y");
    box("Y").focus();
    expect(document.activeElement).toBe(box("Y"));
    // The caret is clicked while focus sits on a child: the mouse takes no focus in jsdom.
    fireEvent.click(caret("G"));
    await waitFor(() => {
      expect(hasBox("Y")).toBe(false);
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(box("G"));
    });
  });

  it("selects with Enter, focus to the panel heading; the Tree with o; a modifier does nothing", async () => {
    renderApp(flow(), "#/alpha/flow");
    await drawn("AFTER");
    box("AFTER").focus();
    fireEvent.keyDown(box("AFTER"), { key: "Enter", metaKey: true });
    fireEvent.keyDown(box("AFTER"), { key: "o", ctrlKey: true });
    expect(window.location.hash).toBe("#/alpha/flow");
    fireEvent.keyDown(box("AFTER"), { key: "Enter" });
    const heading = await screen.findByRole("heading", { level: 2, name: "AFTER" });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
    fireEvent.keyDown(box("AFTER"), { key: "o" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/tree/AFTER");
    });
  });

  it("keeps `c` on the box: it moves no route and no focus", async () => {
    renderApp(flow(), "#/alpha/flow");
    await drawn("AFTER");
    key("AFTER", "c");
    expect(window.location.hash).toBe("#/alpha/flow");
    expect(document.activeElement).toBe(box("AFTER"));
  });

  it("lists the keys on ?", async () => {
    renderApp(flow(), "#/alpha/flow");
    await drawn("G");
    key("G", "?");
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText("Canvas: open or close the group, focus kept")).toBeTruthy();
    expect(within(dialog).getByText("Canvas: select the box, focus to its panel")).toBeTruthy();
  });

  it("wires React Flow's own keys and words off: no key option left, every word replaced", async () => {
    renderApp(flow(), "#/alpha/flow");
    await drawn("G");
    expect(screen.getByRole("application", { name: "Flow canvas" })).toBeTruthy();
    expect(document.querySelector(".react-flow__attribution")).toBeNull();
    expect(screen.getByRole("button", { name: "Fit the flow to the view, its text readable" })).toBeTruthy();
  });
});

describe("width and memory (AC-13)", () => {
  it("under 48rem: the List first", async () => {
    stubNarrow(true);
    renderApp(flowClient(steps), "#/alpha/flow");
    expect(await screen.findByRole("tabpanel", { name: "List" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "List" }).getAttribute("aria-selected")).toBe("true");
    expect(hasBox("A")).toBe(false);
  });

  it("over it the canvas, and a tab chosen is kept per project, under 48rem too", async () => {
    renderApp(flowClient(steps), "#/alpha/flow");
    await drawn("A");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    window.location.hash = "#/alpha/tasks";
    await screen.findByRole("heading", { level: 1, name: "Tasks" });
    stubNarrow(true);
    window.location.hash = "#/alpha/flow";
    expect(await screen.findByRole("tabpanel", { name: "List" })).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Canvas" }));
    await drawn("A");
    window.location.hash = "#/alpha/tasks";
    await screen.findByRole("heading", { level: 1, name: "Tasks" });
    window.location.hash = "#/alpha/flow";
    await drawn("A");
    window.location.hash = "#/beta/flow";
    expect(await screen.findByRole("tabpanel", { name: "List" })).toBeTruthy();
  });
});

const COUNTS = { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 0 };

function aStage(stage: string | null, counts: Partial<TraceCounts> = {}, distance: number | null = 0): TraceStage {
  return { stage, distance, counts: { ...COUNTS, ...counts } };
}

/** A flow whose lanes the summary read serves, and a read per lane. */
function stagedClient(top: FlowAnswer, stages: TraceStage[], lanes: Record<string, TraceNode[]> = {}, groups: Record<string, FlowAnswer> = {}) {
  const client = flowClient(top, groups);
  client.getTrace.mockImplementation((_project, options) => {
    const counts = stages.reduce((sum, lane) => ({ no_card: sum.no_card + lane.counts.no_card, in_work: sum.in_work + lane.counts.in_work, suspect: sum.suspect + lane.counts.suspect, departed: sum.departed + lane.counts.departed, confirmed: sum.confirmed + lane.counts.confirmed }), COUNTS);
    return Promise.resolve(aTrace(options?.stage === undefined ? [] : (lanes[options.stage] ?? []), { stages, counts }));
  });
  return client;
}

function openTimeline() {
  fireEvent.click(screen.getByRole("tab", { name: "Timeline" }));
}

function laneOf(stage: string): HTMLElement {
  const found = screen.getByRole("button", { name: new RegExp(`^${stage}\\b`) }).closest("li");
  if (found === null) {
    throw new Error(`no lane ${stage}`);
  }
  return found;
}

/** The live tail's `emit`, the client answering through it. */
function tailOf(client: ReturnType<typeof flowClient>) {
  const tail: { emit: (event: QueueEvent) => void } = { emit: () => undefined };
  client.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
    tail.emit = onEvent;
    return () => undefined;
  });
  return tail;
}

describe("flow badges (trace-views)", () => {
  it("always sends trace, and says a box's rollup in a label and an icon; its accessible name says it too", async () => {
    const reads = aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "confirmed", counts: { ...COUNTS, confirmed: 1 } }), aFlowNode({ id: "B", distance: 1, rollup: "in work" })], []);
    const client = flowClient(reads);
    renderApp(client, "#/alpha/flow");
    await drawn("B");
    const badge = box("A").querySelector(".flow-box-state .badge");
    expect([badge?.textContent, badge?.getAttribute("data-tone"), badge?.querySelector("svg") !== null]).toEqual(["Confirmed", "trace-confirmed", true]);
    expect(box("A").getAttribute("aria-label")).toContain("Confirmed");
    expect(box("B").querySelector(".badge")?.textContent).toBe("In work");
    expect(client.getFlow.mock.calls.map((call) => call[1])).toEqual([{ depth: 0, trace: true }]);
  });

  it("an unknown rollup is a neutral badge, raw; no rollup read, no badge at all", async () => {
    renderApp(flowClient(aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "pondering" }), aFlowNode({ id: "B", distance: 1 })], [])), "#/alpha/flow");
    await drawn("B");
    const badge = box("A").querySelector(".badge");
    expect([badge?.textContent, badge?.getAttribute("data-tone")]).toEqual(["pondering", "trace-unknown"]);
    expect(box("B").querySelector(".badge")).toBeNull();
    expect(box("B").getAttribute("aria-label")).toBe("B, Title of B, widget");
  });

  it("draws a step with no card hollow, and a step with one not", async () => {
    renderApp(flowClient(aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "no card" }), aFlowNode({ id: "B", distance: 1, rollup: "in work" })], [])), "#/alpha/flow");
    await drawn("B");
    expect(box("A").querySelector(".flow-box")?.classList.contains("is-hollow")).toBe(true);
    expect(box("B").querySelector(".flow-box")?.classList.contains("is-hollow")).toBe(false);
  });

  it("draws a closed group's served counts, zero parts left out; a stub of 7 draws 7 whatever its read holds (AC-04)", async () => {
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 2, rollup: "no card", counts: { ...COUNTS, confirmed: 7 } }), aFlowNode({ id: "H", distance: 1, holds: 1, counts: { ...COUNTS, confirmed: 3, suspect: 1, no_card: 2 } })], []);
    const client = flowClient(reads, { G: aGroupRead("G", aLine(["X", "Y"])) });
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    expect(box("G").querySelector(".flow-box-state")?.textContent).toBe("7 confirmed");
    expect(box("H").querySelector(".flow-box-state")?.textContent).toBe("3 confirmed, 1 suspect, 2 no card");
    expect(box("G").getAttribute("aria-label")).toContain("7 confirmed");
    expect(box("G").querySelector(".badge")).toBeNull();
    expect(client.getFlow.mock.calls.filter((call) => call[1].root !== undefined)).toEqual([]);
  });

  it("gives an open group no state of its own: what it holds shows on its boxes", async () => {
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1, rollup: "no card", counts: { ...COUNTS, confirmed: 1 } })], []);
    renderApp(flowClient(reads, { G: aGroupRead("G", [aFlowNode({ id: "X", distance: 0, rollup: "confirmed" })]) }), "#/alpha/flow");
    await drawn("X");
    expect(box("G").querySelector(".flow-box-state, .badge")).toBeNull();
    expect(box("X").querySelector(".badge")?.textContent).toBe("Confirmed");
  });

  it("says a closed group's Start and End as served, Entry in a group's lane", async () => {
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 2, start: true }), aFlowNode({ id: "H", distance: 1, holds: 2, end: true })], []);
    renderApp(flowClient(reads), "#/alpha/flow");
    await drawn("H");
    expect(box("G").querySelector(".flow-mark-start")?.textContent).toBe("Start");
    expect(box("H").querySelector(".flow-mark-end")?.textContent).toBe("End");
  });

  it("the List says the same, a group's counts, a step's rollup, its stage, in words", async () => {
    const reads = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1, counts: { ...COUNTS, confirmed: 2 } }), aFlowNode({ id: "A", distance: 1, rollup: "suspect", stage: "\u043f\u0435\u0447\u0430\u0442\u044c" })], []);
    renderApp(flowClient(reads), "#/alpha/flow");
    await drawn("A");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const row = (name: string) => document.querySelector(`[data-flow-row="${name}"]`);
    expect(row("G")?.querySelector(".flow-list-counts")?.textContent).toBe("2 confirmed");
    expect(row("A")?.querySelector(".badge")?.textContent).toBe("Suspect");
    expect(row("A")?.textContent).toContain("\u043f\u0435\u0447\u0430\u0442\u044c");
  });

  it("a card event reads the boxes again, with no reload (a rollup moves with its card)", async () => {
    let rollup = "in work";
    const client = flowClient(aFlowView([], []));
    client.getFlow.mockImplementation(() => Promise.resolve(aFlowView([aFlowNode({ id: "A", distance: 0, rollup })], [])));
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    rollup = "confirmed";
    act(() => {
      tail.emit({ seq: 9, type: "card.accepted", payload: { id: "C-0001" } });
    });
    await waitFor(() => {
      expect(box("A").querySelector(".badge")?.textContent).toBe("Confirmed");
    });
  });

  it("changes no control: a state is what a read says, and nothing is held for it (ADR-0012)", async () => {
    const plain = aFlowView(aLine(["A", "B"]), []);
    const marked = aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "departed" }), aFlowNode({ id: "B", distance: 1, rollup: "suspect" })], []);
    const controls = () => Array.from(document.querySelectorAll("button, a, select"), (element) => element.textContent);
    const first = renderApp(flowClient(plain), "#/alpha/flow");
    await drawn("B");
    const before = controls();
    first.unmount();
    renderApp(flowClient(marked), "#/alpha/flow");
    await drawn("B");
    expect(controls()).toEqual(before);
  });

  it("the panel says the box's stage and holds its cards", async () => {
    const client = flowClient(aFlowView([aFlowNode({ id: "A", distance: 0, stage: "print" })], []));
    client.getTrace.mockResolvedValue(aTrace([aTraceNode("A", [aTraceCard({ id: "C-0001", title: "Its card" })], { rollup: "in work" })]));
    renderApp(client, "#/alpha/flow/A");
    const panel = await screen.findByRole("complementary", { name: "A" });
    expect(within(panel).getByText("Stage").closest("div")?.textContent).toBe("Stageprint");
    expect((await within(panel).findByRole("link", { name: /C-0001/ })).getAttribute("href")).toBe("#/alpha/board/C-0001");
  });
});

describe("the Timeline (trace-views)", () => {
  const flow = aFlowView(aLine(["A", "B", "C"]), []);

  it("on load reads the summary, opening a lane its stage; never a whole read (AC-05)", async () => {
    const stages = [aStage("gather", { confirmed: 1, no_card: 1 }), aStage(null, { no_card: 1 }, null)];
    const client = stagedClient(flow, stages, { gather: [aTraceNode("A", [], { distance: 0, rollup: "confirmed" })], "-": [aTraceNode("C", [], { rollup: "no card" })] });
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    await waitFor(() => {
      expect(client.getTrace).toHaveBeenCalled();
    });
    expect(client.getTrace.mock.calls.map((call) => call[1])).toEqual([{ summary: true }]);
    openTimeline();
    fireEvent.click(await screen.findByRole("button", { name: /^gather/ }));
    await within(laneOf("gather")).findByRole("link", { name: "A" });
    fireEvent.click(screen.getByRole("button", { name: /^No stage/ }));
    await within(laneOf("No stage")).findByRole("link", { name: "C" });
    // The summary once more as the tab mounts, so the score goes with the steps; never a whole read.
    expect(client.getTrace.mock.calls.map((call) => call[1])).toEqual([{ summary: true }, { summary: true }, { stage: "gather" }, { stage: "-" }]);
  });

  it("draws lanes in the served order, each with its stage, its score and each part in words", async () => {
    renderApp(stagedClient(flow, [aStage("print", { confirmed: 2, in_work: 1, suspect: 1, no_card: 2 }), aStage("\u043f\u0435\u0447\u0430\u0442\u044c", { confirmed: 1 }), aStage(null, { no_card: 3 }, null)]), "#/alpha/flow");
    await drawn("C");
    openTimeline();
    const heads = await screen.findAllByRole("button", { expanded: false, name: /confirmed$/ });
    expect(heads.map((head) => head.textContent)).toEqual(["print: 2 of 6 confirmed", "\u043f\u0435\u0447\u0430\u0442\u044c: 1 of 1 confirmed", "No stage: 0 of 3 confirmed"]);
    const lane = laneOf("print");
    expect(Array.from(lane.querySelectorAll(".timeline-parts .badge"), (badge) => badge.textContent)).toEqual(["2 confirmed", "1 in work", "1 suspect", "2 no card"]);
    expect(lane.querySelectorAll(".trace-bar-part")).toHaveLength(4);
    expect(lane.querySelectorAll(".trace-bar-part.is-hollow")).toHaveLength(1);
  });

  it("lists a lane's steps by distance, a step with no card in its place, hollow, \"No card\": the gaps stay visible (AC-06)", async () => {
    const steps = [aTraceNode("LAST", [], { distance: 3, rollup: "confirmed" }), aTraceNode("OFF", [], { distance: null, rollup: "no card" }), aTraceNode("GAP", [], { distance: 2, rollup: "no card" }), aTraceNode("FIRST", [], { distance: 1, rollup: "in work" })];
    renderApp(stagedClient(flow, [aStage("print", { in_work: 1, confirmed: 1, no_card: 2 })], { print: steps }), "#/alpha/flow");
    await drawn("C");
    openTimeline();
    fireEvent.click(await screen.findByRole("button", { name: /^print/ }));
    const list = await screen.findByRole("list", { name: "Steps of print" });
    const rows = Array.from(list.querySelectorAll("li"));
    expect(rows.map((row) => row.querySelector("a")?.textContent)).toEqual(["FIRST", "GAP", "LAST", "OFF"]);
    expect(rows[1]?.className).toContain("is-hollow");
    expect(rows[1]?.textContent).toContain("No card");
    expect(rows[0]?.className).not.toContain("is-hollow");
    expect(rows.map((row) => row.querySelector(".timeline-step-place")?.textContent)).toEqual(["Step 2", "Step 3", "Step 4", "Step unknown"]);
    expect(list.textContent).not.toContain("Distance");
    expect(rows[1]?.querySelector("a")?.getAttribute("href")).toBe("#/alpha/flow/GAP");
  });

  it("fills as cards are accepted: a card.accepted event reads the lanes and what is open, to <n> of <n>, no reload (AC-06)", async () => {
    let done = false;
    const client = flowClient(flow);
    client.getTrace.mockImplementation((_project, options) => {
      const counts = done ? { ...COUNTS, confirmed: 2 } : { ...COUNTS, confirmed: 1, no_card: 1 };
      const rows = [aTraceNode("A", [], { distance: 0, rollup: "confirmed" }), aTraceNode("B", [], { distance: 1, rollup: done ? "confirmed" : "no card" })];
      return Promise.resolve(aTrace(options?.stage === undefined ? [] : rows, { stages: [{ stage: "print", distance: 0, counts }], counts }));
    });
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    expect((await screen.findByRole("button", { name: /^print/ })).textContent).toContain("1 of 2 confirmed");
    fireEvent.click(screen.getByRole("button", { name: /^print/ }));
    const list = await screen.findByRole("list", { name: "Steps of print" });
    expect(list.querySelectorAll(".is-hollow")).toHaveLength(1);
    done = true;
    act(() => {
      tail.emit({ seq: 9, type: "card.accepted", payload: { id: "C-0001", verdict: "briefed" } });
    });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /^print/ }).textContent).toContain("2 of 2 confirmed");
    });
    await waitFor(() => {
      expect(screen.getByRole("list", { name: "Steps of print" }).querySelectorAll(".is-hollow")).toHaveLength(0);
    });
  });

  it("keeps the gap rows beside it", async () => {
    renderApp(stagedClient({ ...flow, gaps: tenGapRows([aFlowGap("unreachable", [["C"]])]) }, [aStage("print", { no_card: 1 })]), "#/alpha/flow");
    await drawn("C");
    openTimeline();
    await screen.findByRole("button", { name: /^print/ });
    expect(gapsRegion()).toBeTruthy();
  });

  it("no card anywhere: lanes at 0 of <n>, and the next step", async () => {
    renderApp(stagedClient(flow, [aStage("print", { no_card: 2 })]), "#/alpha/flow");
    await drawn("C");
    openTimeline();
    expect(await screen.findByText("No step has a card yet. Next: open a step and press New card.")).toBeTruthy();
    expect(screen.getByRole("button", { name: /^print/ }).textContent).toContain("0 of 2 confirmed");
  });

  it("no `stage:` anywhere: one lane, and what would make others; a filter of All and No stage", async () => {
    renderApp(stagedClient(flow, [aStage(null, { no_card: 3 }, null)]), "#/alpha/flow");
    await drawn("C");
    await waitFor(() => {
      expect(within(screen.getByLabelText("Stage")).getAllByRole("option").map((option) => option.textContent)).toEqual(["All stages", "No stage"]);
    });
    openTimeline();
    expect(await screen.findByText("Next: give documents a `stage:` key")).toBeTruthy();
    expect(screen.getByRole("button", { name: /^No stage/ })).toBeTruthy();
  });

  it("no lane at all says the same next step, and offers no empty list", async () => {
    renderApp(stagedClient(flow, []), "#/alpha/flow");
    await drawn("C");
    openTimeline();
    expect(await screen.findByText("Next: give documents a `stage:` key")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /confirmed$/ })).toBeNull();
  });

  it("hides the Stage select on the Timeline: lanes are the stages there; the other tabs keep it", async () => {
    renderApp(stagedClient(flow, [aStage("print", { no_card: 1 })]), "#/alpha/flow");
    await drawn("C");
    expect(screen.getByLabelText("Stage")).toBeTruthy();
    openTimeline();
    expect(screen.queryByLabelText("Stage")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(screen.getByLabelText("Stage")).toBeTruthy();
  });

  it("reads the summary again each time it mounts, so the score goes with the open lanes' steps", async () => {
    let done = false;
    const client = flowClient(flow);
    client.getTrace.mockImplementation((_project, options) => {
      const counts = done ? { ...COUNTS, confirmed: 1 } : { ...COUNTS, no_card: 1 };
      return Promise.resolve(aTrace(options?.stage === undefined ? [] : [aTraceNode("A", [], { distance: 0, rollup: done ? "confirmed" : "no card" })], { stages: [{ stage: "print", distance: 0, counts }], counts }));
    });
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    fireEvent.click(await screen.findByRole("button", { name: /^print/ }));
    await screen.findByRole("list", { name: "Steps of print" });
    fireEvent.click(screen.getByRole("tab", { name: "Canvas" }));
    done = true;
    openTimeline();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /^print/ }).textContent).toContain("1 of 1 confirmed");
    });
    await waitFor(() => {
      expect(screen.getByRole("list", { name: "Steps of print" }).textContent).toContain("Confirmed");
    });
  });

  it("closing a panel opened from a step returns focus to that step's link", async () => {
    renderApp(stagedClient(flow, [aStage("print", { no_card: 1 })], { print: [aTraceNode("A", [], { distance: 0, rollup: "no card" })] }), "#/alpha/flow");
    await drawn("C");
    openTimeline();
    fireEvent.click(await screen.findByRole("button", { name: /^print/ }));
    const link = await within(await screen.findByRole("list", { name: "Steps of print" })).findByRole("link", { name: "A" });
    window.location.hash = "#/alpha/flow/A";
    const panel = await screen.findByRole("complementary", { name: "A" });
    fireEvent.click(within(panel).getByRole("button", { name: /^Close/ }));
    await waitFor(() => {
      expect(document.activeElement).toBe(link);
    });
  });

  it("a failed re-read of the lanes says the score is the last read's, beside the daemon's words, and of a lane's steps too", async () => {
    const client = stagedClient(flow, [aStage("print", { confirmed: 1 })], { print: [aTraceNode("A", [], { distance: 0, rollup: "confirmed" })] });
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    fireEvent.click(await screen.findByRole("button", { name: /^print/ }));
    await screen.findByRole("list", { name: "Steps of print" });
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    act(() => {
      tail.emit({ seq: 9, type: "card.moved", payload: { id: "C-0001" } });
    });
    const panel = await screen.findByRole("tabpanel", { name: "Timeline" });
    await waitFor(() => {
      expect(within(panel).getAllByText(/could not be read again: the queue is locked/)).toHaveLength(2);
    });
    for (const line of within(panel).getAllByText(/could not be read again: the queue is locked/)) {
      expect(line.textContent).toContain("What stands below is from the last read.");
    }
  });

  it("says a note once: one the flow's reads show already does not stand again over the lanes", async () => {
    const note = "C-0009: `GONE` no longer resolves";
    const client = stagedClient(aFlowView(aLine(["A"]), [], { notes: [note] }), [aStage("print", { no_card: 1 })]);
    const lanes = client.getTrace.getMockImplementation();
    client.getTrace.mockImplementation(async (project, options, signal) => ({ ...(await (lanes as NonNullable<typeof lanes>)(project, options, signal)), notes: [note, "only the stages say this"] }));
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    openTimeline();
    expect(await screen.findByText("only the stages say this")).toBeTruthy();
    expect(screen.getAllByText(note)).toHaveLength(1);
  });

  it("a bad [flow]: lanes by name, no distance, the daemon's note", async () => {
    const note = "specengine.toml:9: [flow]: `start` names no field";
    const client = stagedClient(flow, [aStage("alpha", { no_card: 1 }, null), aStage("beta", { no_card: 1 }, null)], { alpha: [aTraceNode("A", [], { rollup: "no card" })] });
    const lanes = client.getTrace.getMockImplementation();
    client.getTrace.mockImplementation(async (project, options, signal) => ({ ...(await (lanes as NonNullable<typeof lanes>)(project, options, signal)), notes: [note] }));
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    expect(await screen.findByText(note)).toBeTruthy();
    expect(screen.getAllByRole("button", { name: /confirmed$/ }).map((head) => head.textContent)).toEqual(["alpha: 0 of 1 confirmed", "beta: 0 of 1 confirmed"]);
    fireEvent.click(screen.getByRole("button", { name: /^alpha/ }));
    expect((await screen.findByRole("list", { name: "Steps of alpha" })).textContent).toContain("Step unknown");
  });

  it("loading: a skeleton, aria-busy, until the lanes land", async () => {
    const held = deferred<ReturnType<typeof aTrace>>();
    const client = flowClient(flow);
    client.getTrace.mockImplementationOnce(() => held.promise);
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    expect(document.querySelector('[aria-busy="true"] .skeleton')).not.toBeNull();
    act(() => {
      held.resolve(aTrace([], { stages: [aStage("print", { no_card: 1 })] }));
    });
    expect(await screen.findByRole("button", { name: /^print/ })).toBeTruthy();
    expect(document.querySelector('[role="tabpanel"] .skeleton')).toBeNull();
  });

  it("a failed summary: the Timeline says so in the daemon's words, and Retry reads it", async () => {
    const client = flowClient(flow);
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    const panel = await screen.findByRole("tabpanel", { name: "Timeline" });
    expect((await within(panel).findByRole("alert")).textContent).toContain("the queue is locked");
    client.getTrace.mockResolvedValue(aTrace([], { stages: [aStage("print", { no_card: 1 })] }));
    const retry = within(panel).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    expect(await within(panel).findByRole("button", { name: /^print/ })).toBeTruthy();
    // Retry went with the failure: focus lands on the lanes, never the body.
    await waitFor(() => {
      expect(document.activeElement).toBe(panel.querySelector(".timeline-lanes"));
    });
  });

  it("a failed re-read of the lanes: its Retry hands focus to the lanes", async () => {
    const client = stagedClient(flow, [aStage("print", { confirmed: 1 })]);
    const lanes = client.getTrace.getMockImplementation();
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    const panel = await screen.findByRole("tabpanel", { name: "Timeline" });
    await within(panel).findByRole("button", { name: /^print/ });
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    act(() => {
      tail.emit({ seq: 9, type: "card.moved", payload: { id: "C-0001" } });
    });
    await within(panel).findByRole("alert");
    client.getTrace.mockImplementation(lanes as NonNullable<typeof lanes>);
    const retry = within(panel).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(document.activeElement).toBe(panel.querySelector(".timeline-lanes"));
    });
  });

  it("a failed lane: its Retry hands focus to the lanes", async () => {
    const client = stagedClient(flow, [aStage("print", { no_card: 1 })], { print: [aTraceNode("A", [], { rollup: "no card" })] });
    const lanes = client.getTrace.getMockImplementation();
    client.getTrace.mockImplementation((project, options, signal) => (options?.stage === "print" ? Promise.reject(new ClientError({ status: 503, message: "the index is locked" })) : (lanes as NonNullable<typeof lanes>)(project, options, signal)));
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    fireEvent.click(await screen.findByRole("button", { name: /^print/ }));
    await within(laneOf("print")).findByRole("alert");
    client.getTrace.mockImplementation(lanes as NonNullable<typeof lanes>);
    const retry = within(laneOf("print")).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await screen.findByRole("list", { name: "Steps of print" });
    await waitFor(() => {
      expect(document.activeElement).toBe(document.querySelector(".timeline-lanes"));
    });
  });

  it("a failed lane read: a line in the daemon's words and Retry, the other lanes standing", async () => {
    const client = stagedClient(flow, [aStage("print", { no_card: 1 }), aStage("bind", { no_card: 1 })], { print: [aTraceNode("A", [], { rollup: "no card" })] });
    const lanes = client.getTrace.getMockImplementation();
    let fail = true;
    client.getTrace.mockImplementation((project, options, signal) => (fail && options?.stage === "print" ? Promise.reject(new ClientError({ status: 503, message: "the index is locked" })) : (lanes as NonNullable<typeof lanes>)(project, options, signal)));
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    fireEvent.click(await screen.findByRole("button", { name: /^print/ }));
    expect((await within(laneOf("print")).findByRole("alert")).textContent).toContain("the index is locked");
    expect(screen.getByRole("button", { name: /^bind/ })).toBeTruthy();
    fail = false;
    fireEvent.click(within(laneOf("print")).getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("list", { name: "Steps of print" })).toBeTruthy();
  });

  it("closing a lane and opening it again reads nothing unless an event marked it", async () => {
    const client = stagedClient(flow, [aStage("print", { no_card: 1 })], { print: [aTraceNode("A", [], { rollup: "no card" })] });
    renderApp(client, "#/alpha/flow");
    await drawn("C");
    openTimeline();
    const head = await screen.findByRole("button", { name: /^print/ });
    fireEvent.click(head);
    await screen.findByRole("list", { name: "Steps of print" });
    fireEvent.click(head);
    expect(screen.queryByRole("list", { name: "Steps of print" })).toBeNull();
    fireEvent.click(head);
    await screen.findByRole("list", { name: "Steps of print" });
    expect(client.getTrace.mock.calls.filter((call) => call[1]?.stage === "print")).toHaveLength(1);
  });
});

describe("when the rollup read fails (trace-views)", () => {
  it("draws the canvas, boxes without badges, a line with the daemon's words", async () => {
    const client = flowClient(aFlowView(aLine(["A", "B"]), []));
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "the queue is of a newer schema" }));
    renderApp(client, "#/alpha/flow");
    await drawn("B");
    expect((await screen.findByRole("alert")).textContent).toContain("the queue is of a newer schema");
    expect(box("B").querySelector(".badge")).toBeNull();
    expect(hasBox("A")).toBe(true);
    client.getTrace.mockResolvedValue(aTrace([], { stages: [aStage("print")] }));
    const retry = screen.getByRole("button", { name: /^Retry/ });
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(screen.queryByRole("alert")).toBeNull();
    });
    // Retry went with the line: focus lands on the results, never the body.
    await waitFor(() => {
      expect(document.activeElement).toBe(document.querySelector(".flow-results"));
    });
  });

  it("a newer queue serves no state and a note: the note shows, every box bare, and nothing anywhere says No card", async () => {
    const note = "rollups not shown: the queue is of a newer schema";
    const reads = aFlowView([aFlowNode({ id: "A", distance: 0 }), aFlowNode({ id: "G", distance: 1, holds: 2 })], [], { notes: [note] });
    expect([reads.nodes[0]?.rollup, reads.nodes[0]?.counts]).toEqual([null, null]);
    renderApp(flowClient(reads), "#/alpha/flow");
    await drawn("G");
    expect(screen.getByText(note)).toBeTruthy();
    expect(document.querySelector(".flow-box-state, .flow-box-bar, .is-hollow")).toBeNull();
    expect(screen.queryByText("No card")).toBeNull();
    expect(box("A").getAttribute("aria-label")).not.toContain("No card");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(document.querySelector("[data-flow-row] .badge, .flow-list-counts")).toBeNull();
    expect(screen.queryByText("No card")).toBeNull();
  });

  it("names each state in the legend once a read served one", async () => {
    renderApp(flowClient(aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "in work" })], [])), "#/alpha/flow");
    await drawn("A");
    const legend = screen.getByRole("region", { name: "Legend" });
    expect(Array.from(legend.querySelectorAll(".badge"), (badge) => badge.textContent)).toEqual(["Confirmed", "In work", "Suspect", "Departed", "No card"]);
  });
});

describe("a read that failed again (trace-views)", () => {
  const COUNTED = { ...COUNTS, confirmed: 2 };

  it("takes the top's badges and counts away, says why beside the daemon's words, and a Retry brings them back", async () => {
    const reads = aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "confirmed" }), aFlowNode({ id: "G", distance: 1, holds: 2, counts: COUNTED })], []);
    const client = flowClient(reads);
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("G");
    expect(box("A").querySelector(".badge")?.textContent).toBe("Confirmed");
    client.getFlow.mockRejectedValue(new ClientError({ status: 503, message: "spec: index locked" }));
    act(() => {
      tail.emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    const line = await screen.findByText(/The flow could not be read again: spec: index locked/);
    expect(line.textContent).toContain("Top-level boxes show no state until it reads.");
    expect(box("A").querySelector(".badge")).toBeNull();
    expect(box("G").querySelector(".flow-box-state, .flow-box-bar")).toBeNull();
    expect(box("A").getAttribute("aria-label")).not.toContain("Confirmed");
    client.getFlow.mockImplementation(() => Promise.resolve(reads));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(box("A").querySelector(".badge")?.textContent).toBe("Confirmed");
    });
  });

  it("takes an open group's boxes' badges away, the top's staying", async () => {
    const top = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "A", distance: 1, rollup: "in work" })], []);
    const group = aGroupRead("G", [aFlowNode({ id: "X", distance: 0, rollup: "confirmed" })]);
    const client = flowClient(top, { G: group });
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("X");
    expect(box("X").querySelector(".badge")?.textContent).toBe("Confirmed");
    client.getFlow.mockImplementation((_project, options) => (options.root === undefined ? Promise.resolve(top) : Promise.reject(new ClientError({ status: 503, message: "spec: index locked" }))));
    act(() => {
      tail.emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    await waitFor(() => {
      expect(box("X").querySelector(".badge")).toBeNull();
    });
    expect(box("A").querySelector(".badge")?.textContent).toBe("In work");
  });

  /** A flow of one step, read once, then its re-read failed; the client to answer again. */
  async function failedAfterRead(reads: FlowAnswer) {
    const client = flowClient(reads);
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    client.getFlow.mockRejectedValue(new ClientError({ status: 503, message: "spec: index locked" }));
    act(() => {
      tail.emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    await screen.findByText(/The flow could not be read again/);
    return client;
  }

  it("takes a None row's state away: never the clean word or icon beside a read that failed, and a read brings it back", async () => {
    const reads = aFlowView(aLine(["A"]), [], { gaps: tenGapRows([aFlowGap("unreachable", [["A"]])]) });
    const client = flowClient(reads);
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    const badge = () => gapRow("no-start").querySelector(".badge");
    expect([badge()?.textContent, badge()?.getAttribute("data-tone")]).toEqual(["State: None", "flow-gap-none"]);
    client.getFlow.mockRejectedValue(new ClientError({ status: 503, message: "spec: index locked" }));
    act(() => {
      tail.emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    await screen.findByText(/The flow could not be read again/);
    expect([badge()?.textContent, badge()?.getAttribute("data-tone")]).toEqual(["State: Not read", "flow-gap-unknown"]);
    expect(within(gapsRegion()).queryByText("None")).toBeNull();
    client.getFlow.mockImplementation(() => Promise.resolve(reads));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(badge()?.getAttribute("data-tone")).toBe("flow-gap-none");
    });
  });

  it("takes a found row's count, items and cause away with it, and the rules line", async () => {
    const notConfigured = "no [[flow.start]] rule: add one naming the first step by paths";
    const reads = aFlowView(aLine(["A"]), [], { gaps: tenGapRows([aFlowGap("unreachable", [["A"]]), { code: "no-start", state: "not_configured", count: null, items: [], cause: notConfigured }]) });
    await failedAfterRead(reads);
    expect(gapRow("unreachable").querySelector(".badge")?.textContent).toBe("State: Not read");
    expect(within(gapRow("unreachable")).queryByRole("button", { name: "A" })).toBeNull();
    expect(screen.queryByText(notConfigured)).toBeNull();
    expect(gapRow("unreachable").querySelector(".flow-gap-cause")).toBeNull();
    expect(screen.queryByText(/rules in specengine.toml/)).toBeNull();
  });

  it("takes the gap marks off the top's boxes too: a mark from the last read is no mark now", async () => {
    await failedAfterRead(aFlowView(aLine(["A"]), [], { gaps: tenGapRows([aFlowGap("unreachable", [["A"]])]) }));
    expect(box("A").querySelector(".flow-mark-gap")).toBeNull();
    expect(box("A").getAttribute("aria-label")).not.toContain("gap");
  });

  it("says nothing of boxes on the Timeline, where none stands; the failure line and the unread rows stay", async () => {
    const reads = aFlowView(aLine(["A"]), []);
    const client = stagedClient(reads, [aStage("print", { confirmed: 1 })]);
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    openTimeline();
    await screen.findByRole("button", { name: /^print/ });
    client.getFlow.mockRejectedValue(new ClientError({ status: 503, message: "spec: index locked" }));
    act(() => {
      tail.emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    const line = await screen.findByText(/The flow could not be read again/);
    expect(line.textContent).not.toContain("boxes");
    expect(gapRow("no-start").querySelector(".badge")?.textContent).toBe("State: Not read");
  });

  it("scopes its sentence to the top level: an open group whose read landed keeps its badges beside it", async () => {
    const top = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "A", distance: 1, rollup: "in work" })], []);
    const group = aGroupRead("G", [aFlowNode({ id: "X", distance: 0, rollup: "confirmed" })]);
    const client = flowClient(top, { G: group });
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("X");
    client.getFlow.mockImplementation((_project, options) => (options.root === undefined ? Promise.reject(new ClientError({ status: 503, message: "spec: index locked" })) : Promise.resolve(group)));
    act(() => {
      tail.emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    const line = await screen.findByText(/The flow could not be read again/);
    expect(line.textContent).toContain("Top-level boxes");
    expect(box("A").querySelector(".badge")).toBeNull();
    expect(box("X").querySelector(".badge")?.textContent).toBe("Confirmed");
  });

  it("the List says no state either, and the legend names none", async () => {
    const reads = aFlowView([aFlowNode({ id: "A", distance: 0, rollup: "confirmed" })], []);
    const client = flowClient(reads);
    const tail = tailOf(client);
    renderApp(client, "#/alpha/flow");
    await drawn("A");
    client.getFlow.mockRejectedValue(new ClientError({ status: 503, message: "spec: index locked" }));
    act(() => {
      tail.emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    await screen.findByText(/could not be read again/);
    expect(screen.queryByText("Confirmed")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(document.querySelector('[data-flow-row="A"] .badge')).toBeNull();
  });
});

describe("a group read's notes (trace-views)", () => {
  it("stand with the top's, each once, while its read is shown", async () => {
    const shared = "rollups not shown: the queue is of a newer schema";
    const top = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 }), aFlowNode({ id: "K", distance: 1, holds: 1 })], [], { notes: [shared] });
    const group = aGroupRead("G", aLine(["X"]), [], { notes: [shared, "only the group says this"] });
    renderApp(flowClient(top, { G: group }), "#/alpha/flow");
    await drawn("K");
    expect(screen.queryByText("only the group says this")).toBeNull();
    fireEvent.click(caret("G"));
    await drawn("X");
    expect(screen.getByText("only the group says this")).toBeTruthy();
    expect(screen.getAllByText(shared)).toHaveLength(1);
  });
});

describe("a stage filter over groups, a book of parts (trace-views)", () => {
  // A nested read: BOOK folds two parts, neither with a stage of its own; `print` cuts across them.
  const top = aFlowView([aFlowNode({ id: "BOOK", distance: 0, holds: 6 })], []);
  const book = aGroupRead("BOOK", [aFlowNode({ id: "PART-ONE", distance: 0, holds: 2 }), aFlowNode({ id: "PART-TWO", distance: 1, holds: 2 })]);
  const partOne = aGroupRead("PART-ONE", [aFlowNode({ id: "STEP-SET", distance: 0, stage: "print" }), aFlowNode({ id: "STEP-CALL", distance: 1, stage: "gather" })]);
  const lanes = [aStage("gather", { no_card: 1 }), aStage("print", { no_card: 1 }, 1), aStage(null, { no_card: 1 }, 2)];

  async function bookOpen(value: string) {
    renderApp(stagedClient(top, lanes, {}, { BOOK: book, "PART-ONE": partOne }), "#/alpha/flow");
    await drawn("PART-TWO");
    await waitFor(() => {
      expect(within(screen.getByLabelText("Stage")).getAllByRole("option")).toHaveLength(4);
    });
    fireEvent.change(screen.getByLabelText("Stage"), { target: { value } });
  }

  const rows = () => Array.from(document.querySelectorAll<HTMLElement>("[data-flow-row]"), (row) => row.dataset.flowRow);

  it("keeps the parts and their Open in the List, and opening one shows only its steps of the stage", async () => {
    await bookOpen("stage:print");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(rows()).toEqual(["BOOK", "PART-ONE", "PART-TWO"]);
    expect(screen.getByRole("button", { name: "Open PART-ONE" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "BOOK (2 boxes, 0 edges)" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open PART-ONE" }));
    await waitFor(() => {
      expect(rows()).toContain("STEP-SET");
    });
    expect(rows()).not.toContain("STEP-CALL");
  });

  it("dims neither part on the Canvas, and no name says it is outside: what it holds is not known here", async () => {
    await bookOpen("stage:print");
    await waitFor(() => {
      expect(box("PART-ONE").getAttribute("aria-label")).not.toContain("outside");
    });
    expect(box("PART-TWO").getAttribute("aria-label")).not.toContain("outside");
    expect(document.querySelector(".is-dimmed")).toBeNull();
    fireEvent.click(caret("PART-ONE"));
    await drawn("STEP-CALL");
    expect(box("STEP-CALL").querySelector(".flow-box")?.classList.contains("is-dimmed")).toBe(true);
    expect(box("STEP-CALL").getAttribute("aria-label")).toContain("outside the stage shown");
    expect(box("STEP-SET").querySelector(".flow-box")?.classList.contains("is-dimmed")).toBe(false);
  });

  it("No stage singles out no group: the parts look as under any other stage", async () => {
    await bookOpen("none");
    await waitFor(() => {
      expect(screen.getByLabelText<HTMLSelectElement>("Stage").value).toBe("none");
    });
    expect(document.querySelector(".is-dimmed")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(rows()).toEqual(["BOOK", "PART-ONE", "PART-TWO"]);
  });

  it("a group with a stage of its own other than the one shown dims, and says so; its row and Open stay", async () => {
    const staged = aGroupRead("BOOK", [aFlowNode({ id: "PART-ONE", distance: 0, holds: 2, stage: "gather" }), aFlowNode({ id: "PART-TWO", distance: 1, holds: 2 })]);
    renderApp(stagedClient(top, lanes, {}, { BOOK: staged }), "#/alpha/flow");
    await drawn("PART-TWO");
    await waitFor(() => {
      expect(within(screen.getByLabelText("Stage")).getAllByRole("option")).toHaveLength(4);
    });
    fireEvent.change(screen.getByLabelText("Stage"), { target: { value: "stage:print" } });
    await waitFor(() => {
      expect(box("PART-ONE").getAttribute("aria-label")).toContain("outside the stage shown");
    });
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(rows()).toContain("PART-ONE");
  });

  it("keeps an edge between two kept groups, and a group's own edge to a child, in the List", async () => {
    const wired = aGroupRead("BOOK", [aFlowNode({ id: "PART-ONE", distance: 0, holds: 2 }), aFlowNode({ id: "PART-TWO", distance: 1, holds: 2 }), aFlowNode({ id: "LOOSE", distance: 2, stage: "gather" })], [aFlowEdge({ from: "PART-ONE", to: "PART-TWO" }), aFlowEdge({ from: "BOOK", to: "PART-ONE" }), aFlowEdge({ from: "PART-TWO", to: "LOOSE" })]);
    renderApp(stagedClient(top, lanes, {}, { BOOK: wired }), "#/alpha/flow");
    await drawn("LOOSE");
    await waitFor(() => {
      expect(within(screen.getByLabelText("Stage")).getAllByRole("option")).toHaveLength(4);
    });
    fireEvent.change(screen.getByLabelText("Stage"), { target: { value: "stage:print" } });
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const lines = Array.from(document.querySelectorAll(".graph-edge-lines li"), (line) => line.textContent);
    expect(lines.filter((line) => line.startsWith("PART-ONE -> PART-TWO"))).toHaveLength(1);
    expect(lines.filter((line) => line.startsWith("BOOK -> PART-ONE"))).toHaveLength(1);
    expect(lines.filter((line) => line.includes("LOOSE"))).toHaveLength(0);
  });
});

describe("the stage filter (trace-views)", () => {
  const reads = aFlowView([aFlowNode({ id: "A", distance: 0, stage: "gather" }), aFlowNode({ id: "B", distance: 1, stage: "print" }), aFlowNode({ id: "C", distance: 2 })], [aFlowEdge({ from: "A", to: "B" }), aFlowEdge({ from: "B", to: "C" })]);
  const stages = [aStage("gather", { no_card: 1 }), aStage("print", { no_card: 1 }, 1), aStage(null, { no_card: 1 }, null)];

  async function filtered(value: string) {
    renderApp(stagedClient(reads, stages), "#/alpha/flow");
    await drawn("C");
    await waitFor(() => {
      expect(within(screen.getByLabelText("Stage")).getAllByRole("option")).toHaveLength(4);
    });
    fireEvent.change(screen.getByLabelText("Stage"), { target: { value } });
  }

  it("offers All, each stage, No stage, from the served lanes", async () => {
    await filtered("all");
    expect(within(screen.getByLabelText("Stage")).getAllByRole("option").map((option) => option.textContent)).toEqual(["All stages", "gather", "print", "No stage"]);
  });

  it("dims the other boxes on the Canvas, keeping the layout; every box keeps its place and its name says why (AC-08)", async () => {
    await filtered("all");
    const place = () => boxes().map((element) => element.style.transform);
    const before = place();
    fireEvent.change(screen.getByLabelText("Stage"), { target: { value: "stage:print" } });
    await waitFor(() => {
      expect(box("A").querySelector(".flow-box")?.classList.contains("is-dimmed")).toBe(true);
    });
    expect(box("B").querySelector(".flow-box")?.classList.contains("is-dimmed")).toBe(false);
    expect(box("C").querySelector(".flow-box")?.classList.contains("is-dimmed")).toBe(true);
    expect(box("A").getAttribute("aria-label")).toContain("outside the stage shown");
    expect(place()).toEqual(before);
  });

  it("No stage picks what has none; All takes the dimming off", async () => {
    await filtered("none");
    await waitFor(() => {
      expect(box("C").querySelector(".flow-box")?.classList.contains("is-dimmed")).toBe(false);
    });
    expect(box("A").querySelector(".flow-box")?.classList.contains("is-dimmed")).toBe(true);
    fireEvent.change(screen.getByLabelText("Stage"), { target: { value: "all" } });
    await waitFor(() => {
      expect(document.querySelector(".is-dimmed")).toBeNull();
    });
  });

  it("hides the others in the List, and the edges they end (AC-08)", async () => {
    await filtered("stage:print");
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const rows = () => Array.from(document.querySelectorAll<HTMLElement>("[data-flow-row]"), (row) => row.dataset.flowRow);
    expect(rows()).toEqual(["B"]);
    expect(document.querySelectorAll(".graph-edge-lines li")).toHaveLength(0);
    expect(screen.getByRole("heading", { level: 2, name: "Top (1 box, 0 edges)" })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Stage"), { target: { value: "all" } });
    expect(rows()).toEqual(["A", "B", "C"]);
    expect(document.querySelectorAll(".graph-edge-lines li")).toHaveLength(2);
  });

  it("is remembered per project with the tab and the open lanes, and a stage no lane serves now shows all", async () => {
    const remembered = new Map<string, FlowState>();
    const memory = { recall: (project: string) => remembered.get(project), remember: (project: string, state: FlowState) => remembered.set(project, state) };
    const client = stagedClient(reads, stages);
    const view = render(
      <ApiProvider client={client}>
        <FlowView project="alpha" nodeRef={null} memory={memory} />
      </ApiProvider>,
    );
    await drawn("C");
    await waitFor(() => {
      expect(within(screen.getByLabelText("Stage")).getAllByRole("option")).toHaveLength(4);
    });
    fireEvent.change(screen.getByLabelText("Stage"), { target: { value: "stage:print" } });
    expect(remembered.get("alpha")?.stage).toBe("stage:print");
    view.unmount();
    render(
      <ApiProvider client={client}>
        <FlowView project="alpha" nodeRef={null} memory={memory} />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(screen.getByLabelText<HTMLSelectElement>("Stage").value).toBe("stage:print");
    });
  });

  it("an edge between a group and its child stays undrawn, and in the List (AC-08)", async () => {
    const top = aFlowView([aFlowNode({ id: "G", distance: 0, holds: 1 })], []);
    const group = aGroupRead("G", aLine(["X"]), [aFlowEdge({ from: "G", to: "X" })]);
    renderApp(flowClient(top, { G: group }), "#/alpha/flow");
    await drawn("X");
    expect(document.querySelector(".react-flow__edge")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(document.querySelector(".graph-edge-lines")?.textContent).toContain("G -> X");
  });
});

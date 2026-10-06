import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { GraphEdge, GraphNode, GraphView } from "../api/types";
import { aGraphEdge, aGraphNode, aGraphView } from "../test/builders";
import { graphClient, graphOf, HOSTILE, HOSTILE_LINK } from "../test/graphStub";
import { renderApp } from "../test/render";
import { FIT_MARGIN, FIT_MIN_ZOOM } from "./geometry";
import { COLUMN_STEP, patternAt } from "./layout";

// AC-01, AC-03, AC-04 and AC-06 to AC-12 of docs/features/ui-graph.md, on the stub client: the
// route and its one read, the options, the read-only canvas (boxes, stubs, details, keys, words),
// the List, the states, hostile text.

function boxes(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".react-flow__node"));
}

function box(id: string): HTMLElement {
  const found = boxes().find((element) => element.dataset.id === id);
  if (found === undefined) {
    throw new Error(`no box ${id}`);
  }
  return found;
}

function boxLabelled(label: string): HTMLElement {
  const found = boxes().find((element) => element.getAttribute("aria-label") === label);
  if (found === undefined) {
    throw new Error(`no box labelled ${label}`);
  }
  return found;
}

async function drawn(): Promise<void> {
  await waitFor(() => {
    expect(boxes().length).toBeGreaterThan(0);
  });
}

function results(): HTMLElement {
  const element = document.querySelector<HTMLElement>(".graph-results");
  if (element === null) {
    throw new Error("no results region");
  }
  return element;
}

function lastCall(client: ReturnType<typeof graphClient>) {
  return client.getGraph.mock.calls[client.getGraph.mock.calls.length - 1]?.[1];
}

async function calls(client: ReturnType<typeof graphClient>, count: number) {
  await waitFor(() => {
    expect(client.getGraph).toHaveBeenCalledTimes(count);
  });
}

/** Browser Back; jsdom traverses history in a task of its own, so wait for the hashchange. */
async function goBack() {
  const traversed = new Promise<void>((resolve) => {
    window.addEventListener(
      "hashchange",
      () => {
        resolve();
      },
      { once: true },
    );
  });
  await act(async () => {
    window.history.back();
    await traversed;
  });
}

/** A link clicked as the browser follows it (jsdom moves the hash only a task later). */
function follow(link: HTMLElement) {
  fireEvent.click(link);
  window.location.hash = link.getAttribute("href") ?? "";
}

/** A box's column, from where React Flow placed it. */
function columnOf(element: HTMLElement): number {
  const x = /translate\(\s*(-?[\d.]+)px/.exec(element.style.transform)?.[1];
  return Number(x) / COLUMN_STEP;
}

/** The canvas's pan and zoom, from the viewport's transform. */
function viewport(): { x: number; y: number; zoom: number } {
  const transform = document.querySelector<HTMLElement>(".react-flow__viewport")?.style.transform ?? "";
  const match = /translate\(\s*(-?[\d.]+)px,\s*(-?[\d.]+)px\)\s*scale\(([\d.]+)\)/.exec(transform);
  return { x: Number(match?.[1]), y: Number(match?.[2]), zoom: Number(match?.[3]) };
}

/** A client answering every read with `view` filtered to the types asked for. */
function clientOf(view: GraphView) {
  const client = graphClient();
  client.getGraph.mockImplementation((_project, options) => {
    if (options.types === undefined) {
      return Promise.resolve(view);
    }
    const followed = new Set(options.types);
    return Promise.resolve({
      ...view,
      types: view.types.filter((entry) => followed.has(entry.type)),
      edges: view.edges.filter((edge) => followed.has(edge.type)),
    });
  });
  return client;
}

function chips(): HTMLElement[] {
  return within(screen.getByRole("group", { name: "Types followed" })).getAllByRole("button");
}

function chip(name: string): HTMLElement {
  const found = chips().find((button) => button.textContent.replace(/\s+/g, " ").trim().startsWith(name));
  if (found === undefined) {
    throw new Error(`no chip ${name}`);
  }
  return found;
}

describe("the route and its one read (AC-01)", () => {
  it("reads one graph for #/harbor-sim/graph/MEC-TIDES: the REF and depth 2, nothing else", async () => {
    const client = graphClient();
    client.getProjects.mockResolvedValue([{ slug: "harbor-sim", name: "Harbor Sim", root: "/work/harbor-sim", branch: "main" }]);
    renderApp(client, "#/harbor-sim/graph/MEC-TIDES");
    expect(await screen.findByRole("heading", { level: 1, name: "Graph: MEC-TIDES" })).toBeTruthy();
    await drawn();
    expect(client.getGraph.mock.calls).toEqual([["harbor-sim", { ref: "MEC-TIDES", depth: 2 }]]);
    for (const read of [client.getTree, client.getNode, client.search, client.getBundle, client.getInbox]) {
      expect(read).not.toHaveBeenCalled();
    }
  });

  it("reads nothing at #/harbor-sim/graph and puts focus in the REF field", async () => {
    const client = graphClient();
    client.getProjects.mockResolvedValue([{ slug: "harbor-sim", name: "Harbor Sim", root: "/work/harbor-sim", branch: "main" }]);
    renderApp(client, "#/harbor-sim/graph");
    expect(await screen.findByRole("heading", { level: 1, name: "Graph" })).toBeTruthy();
    const field = screen.getByLabelText("REF");
    await waitFor(() => {
      expect(document.activeElement).toBe(field);
    });
    expect(screen.getByText("Give a REF to draw its links")).toBeTruthy();
    expect(field.getAttribute("aria-describedby")).toBe(screen.getByText("Give a REF to draw its links").id);
    expect(client.getGraph).not.toHaveBeenCalled();
  });

  it("shows the trimmed REF's graph; a blank one says so and reads nothing", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph");
    const field = await screen.findByLabelText("REF");
    fireEvent.change(field, { target: { value: "   " } });
    fireEvent.submit(screen.getByRole("form", { name: "Graph options" }));
    expect(screen.getByRole("alert").textContent).toBe("Type a REF: an ID, slug/ID, ID#SECTION or a root-relative .md path.");
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(client.getGraph).not.toHaveBeenCalled();
    fireEvent.change(field, { target: { value: "  R-0 " } });
    fireEvent.submit(screen.getByRole("form", { name: "Graph options" }));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/graph/R-0");
    });
    await calls(client, 1);
    expect(lastCall(client)).toEqual({ ref: "R-0", depth: 2 });
  });
});

describe("options: one read per change (AC-03)", () => {
  it("reads once for each change of REF, mode, chip, depth or archive; an equal value reads nothing", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    await calls(client, 1);

    const depth = screen.getByLabelText("Depth");
    fireEvent.change(depth, { target: { value: "3" } });
    await calls(client, 2);
    expect(lastCall(client)).toEqual({ ref: "R-0", depth: 3 });
    fireEvent.change(depth, { target: { value: "3" } });

    fireEvent.click(screen.getByLabelText("Include archive"));
    await calls(client, 3);
    expect(lastCall(client)).toEqual({ ref: "R-0", depth: 3, archive: true });

    fireEvent.click(screen.getByLabelText("Impact"));
    await calls(client, 4);
    expect(lastCall(client)).toEqual({ ref: "R-0", impact: true, depth: 3, archive: true });
    fireEvent.click(screen.getByLabelText("Impact"));

    fireEvent.change(depth, { target: { value: "all" } });
    await calls(client, 5);
    expect(lastCall(client)).toEqual({ ref: "R-0", impact: true, archive: true });

    await waitFor(() => {
      expect(chip("zeta_type").getAttribute("aria-pressed")).toBe("true");
    });
    fireEvent.click(chip("zeta_type"));
    await calls(client, 6);
    expect(lastCall(client)).toEqual({ ref: "R-0", impact: true, types: ["gamma_type"], archive: true });

    const field = screen.getByLabelText("REF");
    fireEvent.change(field, { target: { value: "A1" } });
    fireEvent.submit(screen.getByRole("form", { name: "Graph options" }));
    await calls(client, 7);
    expect(lastCall(client)).toEqual({ ref: "A1", impact: true, types: ["gamma_type"], archive: true });
    fireEvent.submit(screen.getByRole("form", { name: "Graph options" }));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(client.getGraph).toHaveBeenCalledTimes(7);
  });

  it("keeps the previous answer up, marked busy, while another is read", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    let release: () => void = () => undefined;
    client.getGraph.mockImplementationOnce(
      (_project, options) =>
        new Promise<GraphView>((resolve) => {
          release = () => {
            resolve(graphOf(options));
          };
        }),
    );
    fireEvent.change(screen.getByLabelText("Depth"), { target: { value: "4" } });
    await calls(client, 2);
    expect(results().getAttribute("aria-busy")).toBe("true");
    expect(box("n:R-0")).toBeTruthy();
    await act(async () => {
      release();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(results().getAttribute("aria-busy")).toBe("false");
    });
  });
});

describe("modes, chips, legend (AC-04)", () => {
  it("shows the mode's unfiltered types in the answer's order, then mentions released; all but mentions send no types", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    await waitFor(() => {
      expect(chips()).toHaveLength(4);
    });
    expect(chips().map((button) => button.textContent.replace(/\s+/g, " ").trim())).toEqual([
      "zeta_type outgoing",
      "alpha_type outgoing",
      "gamma_type outgoing",
      "mentions",
    ]);
    expect(chips().map((button) => button.getAttribute("aria-pressed"))).toEqual(["true", "true", "true", "false"]);
    expect(lastCall(client)).toEqual({ ref: "R-0", depth: 2 });

    fireEvent.click(chip("mentions"));
    await calls(client, 2);
    expect(lastCall(client)?.types).toEqual(["zeta_type", "alpha_type", "gamma_type", "mentions"]);
    fireEvent.click(chip("mentions"));
    await calls(client, 3);
    expect(lastCall(client)?.types).toBeUndefined();
  });

  it("keeps one type followed: the last pressed chip is aria-disabled and says why", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    await waitFor(() => {
      expect(chips()).toHaveLength(4);
    });
    fireEvent.click(chip("zeta_type"));
    await calls(client, 2);
    fireEvent.click(chip("alpha_type"));
    await calls(client, 3);
    expect(lastCall(client)?.types).toEqual(["gamma_type"]);
    const last = chip("gamma_type");
    expect(last.getAttribute("aria-disabled")).toBe("true");
    expect(document.getElementById(last.getAttribute("aria-describedby") ?? "")?.textContent).toBe("At least one type is followed");
    fireEvent.click(last);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(client.getGraph).toHaveBeenCalledTimes(3);
    expect(last.getAttribute("aria-pressed")).toBe("true");
  });

  it("sends impact: true under Impact, resets the chips, labels them with the answer's directions", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    await waitFor(() => {
      expect(chips()).toHaveLength(4);
    });
    fireEvent.click(chip("zeta_type"));
    await calls(client, 2);
    fireEvent.click(screen.getByLabelText("Impact"));
    await calls(client, 3);
    expect(lastCall(client)).toEqual({ ref: "R-0", impact: true, depth: 2 });
    await waitFor(() => {
      expect(chips().map((button) => button.textContent.replace(/\s+/g, " ").trim())).toEqual([
        "gamma_type incoming",
        "zeta_type outgoing",
        "mentions",
      ]);
    });
    expect(chips().map((button) => button.getAttribute("aria-pressed"))).toEqual(["true", "true", "false"]);
    fireEvent.click(chip("mentions"));
    await calls(client, 4);
    expect(lastCall(client)?.types).toEqual(["gamma_type", "zeta_type", "mentions"]);
    await waitFor(() => {
      expect(chip("mentions").textContent.replace(/\s+/g, " ").trim()).toBe("mentions incoming");
    });
  });

  it("gives the legend each followed type in order: pattern, direction icon, words; and the credit as plain text", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    fireEvent.click(screen.getByLabelText("Impact"));
    await calls(client, 2);
    const legend = await screen.findByRole("region", { name: "Legend" });
    await waitFor(() => {
      expect(within(legend).getByText("gamma_type (incoming)")).toBeTruthy();
    });
    const items = Array.from(legend.querySelectorAll(".graph-legend-list")[0]?.querySelectorAll("li") ?? []);
    expect(items.map((item) => item.textContent)).toEqual(["gamma_type (incoming)", "zeta_type (outgoing)"]);
    items.forEach((item, position) => {
      expect(item.querySelector("line")?.style.strokeDasharray).toBe(patternAt(position));
      expect(item.querySelectorAll("svg.icon")).toHaveLength(1);
    });
    const credit = within(legend).getByText("Drawn with React Flow (MIT)");
    expect(credit.closest("a")).toBeNull();
    expect(within(legend).getByText("Arrows point as the link is written; columns are the distance.")).toBeTruthy();
  });

  it("keeps each type's pattern when another chip is released: the mode's chip order, not the answer's", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    await waitFor(() => {
      expect(chips()).toHaveLength(4);
    });
    const legendPatterns = () =>
      Object.fromEntries(
        Array.from(screen.getByRole("region", { name: "Legend" }).querySelectorAll(".graph-legend-list:first-of-type li"), (item) => [
          item.textContent,
          item.querySelector("line")?.style.strokeDasharray,
        ]),
      );
    const edgePattern = (label: string) =>
      document.querySelector<SVGElement>(`.react-flow__edge[aria-label="${label}"] .react-flow__edge-path`)?.style.strokeDasharray;
    expect(legendPatterns()["gamma_type (outgoing)"]).toBe(patternAt(2));
    expect(edgePattern("A3 gamma_type B2")).toBe(patternAt(2));
    fireEvent.click(chip("alpha_type"));
    await calls(client, 2);
    await waitFor(() => {
      expect(Object.keys(legendPatterns())).toEqual(["zeta_type (outgoing)", "gamma_type (outgoing)"]);
    });
    expect(legendPatterns()).toEqual({ "zeta_type (outgoing)": patternAt(0), "gamma_type (outgoing)": patternAt(2) });
    expect(edgePattern("A3 gamma_type B2")).toBe(patternAt(2));
    expect(edgePattern("R-0 zeta_type A1")).toBe(patternAt(0));
  });

  it("names mentions' direction only from an answer of the mode shown, never the previous mode's", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    // Impact's chips learned at depth 2, then Outgoing at depth 3 with mentions pressed.
    fireEvent.click(screen.getByLabelText("Impact"));
    await calls(client, 2);
    await waitFor(() => {
      expect(chips()).toHaveLength(3);
    });
    fireEvent.click(screen.getByLabelText("Outgoing"));
    fireEvent.change(screen.getByLabelText("Depth"), { target: { value: "3" } });
    await waitFor(() => {
      expect(lastCall(client)).toEqual({ ref: "R-0", depth: 3 });
    });
    await waitFor(() => {
      expect(chips()).toHaveLength(4);
    });
    fireEvent.click(chip("mentions"));
    await waitFor(() => {
      expect(chip("mentions").textContent.replace(/\s+/g, " ").trim()).toBe("mentions outgoing");
    });
    // Impact at depth 3 is read anew: the Outgoing answer stays up meanwhile.
    const before = client.getGraph.mock.calls.length;
    client.getGraph.mockImplementationOnce(() => new Promise<GraphView>(() => undefined));
    fireEvent.click(screen.getByLabelText("Impact"));
    await calls(client, before + 1);
    expect(lastCall(client)).toEqual({ ref: "R-0", impact: true, depth: 3 });
    expect(results().getAttribute("aria-busy")).toBe("true");
    expect(chips().map((button) => button.textContent.replace(/\s+/g, " ").trim())).toEqual(["gamma_type incoming", "zeta_type outgoing", "mentions"]);
  });
});

describe("boxes, stubs, details (AC-06, AC-07)", () => {
  it("draws a null end as an unresolved stub: written, state label and icon, reason verbatim, no link", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    const stub = boxLabelled(`${HOSTILE_LINK}, not walked: Dangling`);
    expect(stub.textContent).toContain(HOSTILE_LINK);
    expect(stub.textContent).toContain("Dangling");
    expect(stub.querySelector(".badge svg.icon")).not.toBeNull();
    fireEvent.click(stub);
    const details = await screen.findByRole("complementary");
    expect(within(details).getByRole("heading", { level: 2 }).textContent).toBe(HOSTILE_LINK);
    expect(within(details).getByText("Dangling")).toBeTruthy();
    expect(within(details).getByText("`R-404` resolves to no ID and no alias")).toBeTruthy();
    expect(details.querySelectorAll("a[href]")).toHaveLength(0);
    expect(within(details).queryByText(/Centre here/)).toBeNull();
    expect(stub.querySelectorAll("a[href]")).toHaveLength(0);
  });

  it("draws an end outside the nodes as one section stub per name, never a link", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    expect(boxes().filter((element) => element.dataset.id === "s:SEC-R")).toHaveLength(1);
    // SEC-R is walked from toward A1 (distance 1): one column before it, beside the focus.
    expect([columnOf(box("s:SEC-R")), columnOf(box("n:R-0")), columnOf(box("n:A1"))]).toEqual([0, 0, 1]);
    const stub = boxLabelled("SEC-R, not walked: a section");
    fireEvent.click(stub);
    const details = await screen.findByRole("complementary");
    expect(within(details).getByText("Not walked: a section")).toBeTruthy();
    expect(within(details).getByText("SEC-R --gamma_type--> A1 | docs/spec/r-0.md:12")).toBeTruthy();
    expect(details.querySelectorAll("a[href]")).toHaveLength(0);
  });

  it("keeps a selected stub's id across answers: never another stub's details or tab stop", async () => {
    const view = aGraphView(
      [aGraphNode({ id: "R-0", distance: 0 }), aGraphNode({ id: "A1", distance: 1 })],
      [
        aGraphEdge({ src: "R-0", type: "alpha_type", dst: null, written: "W-1", line: 3, state: "dangling" }),
        aGraphEdge({ src: "R-0", type: "zeta_type", dst: null, written: "W-2", line: 2, state: "dangling" }),
        aGraphEdge({ src: "R-0", type: "zeta_type", dst: "A1", line: 3 }),
      ],
    );
    const client = clientOf(view);
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    fireEvent.click(boxLabelled("W-1, not walked: Dangling"));
    const details = await screen.findByRole("complementary");
    expect(within(details).getByRole("heading", { level: 2 }).textContent).toBe("W-1");
    await waitFor(() => {
      expect(boxLabelled("W-1, not walked: Dangling").getAttribute("tabindex")).toBe("0");
    });
    // alpha_type released: W-1 leaves the answer, W-2 now comes first.
    await waitFor(() => {
      expect(chips()).toHaveLength(3);
    });
    fireEvent.click(chip("alpha_type"));
    await calls(client, 2);
    await waitFor(() => {
      expect(boxes().some((element) => element.getAttribute("aria-label") === "W-1, not walked: Dangling")).toBe(false);
    });
    expect(screen.queryByRole("complementary")).toBeNull();
    expect(boxLabelled("W-2, not walked: Dangling").getAttribute("tabindex")).toBe("-1");
    expect(boxes().filter((element) => element.getAttribute("tabindex") === "0").map((element) => element.dataset.id)).toEqual(["n:R-0"]);
  });

  it("marks Focus and Selected by text and icon; the selected box's edges take the emphasis", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    const focus = within(box("n:R-0")).getByText("Focus");
    expect(focus.querySelector("svg.icon")).not.toBeNull();
    expect(within(box("n:A1")).queryByText("Selected")).toBeNull();
    fireEvent.click(box("n:A1"));
    await waitFor(() => {
      expect(within(box("n:A1")).getByText("Selected").querySelector("svg.icon")).not.toBeNull();
    });
    const emphasised = Array.from(document.querySelectorAll(".react-flow__edge.is-emphasis")).map((edge) => edge.getAttribute("aria-label"));
    expect(emphasised.sort()).toEqual(["A1 zeta_type B1", "R-0 zeta_type A1", "SEC-R gamma_type A1"]);
    expect(document.querySelectorAll(".react-flow__edge:not(.is-emphasis)")).toHaveLength(6);
    const details = screen.getByRole("complementary");
    expect(within(details).getByText("Selected").querySelector("svg.icon")).not.toBeNull();
  });

  it("opens the spec tree and centres here by their hashes; Centre here reads once, options kept; Back restores", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    fireEvent.change(screen.getByLabelText("Depth"), { target: { value: "3" } });
    fireEvent.click(screen.getByLabelText("Impact"));
    await calls(client, 3);
    await drawn();
    fireEvent.click(box("n:A1"));
    const details = await screen.findByRole("complementary");
    expect(within(details).getByRole("link", { name: "Open in spec tree" }).getAttribute("href")).toBe("#/alpha/tree/A1");
    const centre = within(details).getByRole("link", { name: "Centre here" });
    expect(centre.getAttribute("href")).toBe("#/alpha/graph/A1");
    follow(centre);
    expect(await screen.findByRole("heading", { level: 1, name: "Graph: A1" })).toBeTruthy();
    await calls(client, 4);
    expect(lastCall(client)).toEqual({ ref: "A1", impact: true, depth: 3 });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(client.getGraph).toHaveBeenCalledTimes(4);
    await goBack();
    expect(await screen.findByRole("heading", { level: 1, name: "Graph: R-0" })).toBeTruthy();
    expect(screen.getByLabelText<HTMLSelectElement>("Depth").value).toBe("3");
    expect(screen.getByLabelText<HTMLInputElement>("Impact").checked).toBe(true);
    expect(screen.getByLabelText<HTMLInputElement>("REF").value).toBe("R-0");
  });
});

describe("keyboard (AC-08)", () => {
  it("gives exactly one box the tab stop: the REF's", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    expect(boxes().filter((element) => element.getAttribute("tabindex") === "0").map((element) => element.dataset.id)).toEqual(["n:R-0"]);
  });

  it("moves the tab stop with the arrows, j and k, Home and End, within and across columns", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    // Columns: R-0 and SEC-R (walked from toward A1); A1 A2 A3; B1 B2 and a stub; a stub.
    const steps: [string, string][] = [
      ["ArrowRight", "n:A1"],
      ["ArrowDown", "n:A2"],
      ["j", "n:A3"],
      ["k", "n:A2"],
      ["End", "n:A3"],
      ["Home", "n:A1"],
      ["ArrowRight", "n:B1"],
      ["ArrowDown", "n:B2"],
      ["ArrowLeft", "n:A2"],
      ["ArrowLeft", "s:SEC-R"],
      ["ArrowUp", "n:R-0"],
      ["ArrowUp", "n:R-0"],
      ["End", "s:SEC-R"],
    ];
    box("n:R-0").focus();
    for (const [key, id] of steps) {
      fireEvent.keyDown(document.activeElement ?? document.body, { key });
      await waitFor(() => {
        expect([key, (document.activeElement as HTMLElement | null)?.dataset.id]).toEqual([key, id]);
      });
      await waitFor(() => {
        expect(boxes().filter((element) => element.getAttribute("tabindex") === "0").map((element) => element.dataset.id)).toEqual([id]);
      });
    }
  });

  it("opens the details with Enter, closes them with Esc and puts focus back on the box", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    box("n:A1").focus();
    fireEvent.keyDown(box("n:A1"), { key: "Enter" });
    const details = await screen.findByRole("complementary");
    await waitFor(() => {
      expect(document.activeElement).toBe(within(details).getByRole("heading", { level: 2 }));
    });
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("complementary")).toBeNull();
    });
    expect(document.activeElement).toBe(box("n:A1"));
    fireEvent.keyDown(box("n:A1"), { key: "Enter" });
    await screen.findByRole("complementary");
    box("n:A1").focus();
    fireEvent.keyDown(box("n:A1"), { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("complementary")).toBeNull();
    });
  });

  it("centres here with c and opens the spec tree with o", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    box("n:A1").focus();
    fireEvent.keyDown(box("n:A1"), { key: "c" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/graph/A1");
    });
    await screen.findByRole("heading", { level: 1, name: "Graph: A1" });
    await drawn();
    box("n:B1").focus();
    fireEvent.keyDown(box("n:B1"), { key: "o" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/tree/B1");
    });
  });

  it("acts on nothing with Ctrl, Meta or Alt held, nor from the REF field", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    box("n:R-0").focus();
    for (const modifier of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }]) {
      fireEvent.keyDown(box("n:R-0"), { key: "ArrowRight", ...modifier });
      fireEvent.keyDown(box("n:R-0"), { key: "c", ...modifier });
      fireEvent.keyDown(box("n:R-0"), { key: "Enter", ...modifier });
    }
    const field = screen.getByLabelText("REF");
    field.focus();
    for (const key of ["j", "k", "c", "o", "?"]) {
      fireEvent.keyDown(field, { key });
    }
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(window.location.hash).toBe("#/alpha/graph/R-0");
    expect(boxes().filter((element) => element.getAttribute("tabindex") === "0").map((element) => element.dataset.id)).toEqual(["n:R-0"]);
    expect(screen.queryByRole("complementary")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("lists the keys with ?", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    box("n:R-0").focus();
    fireEvent.keyDown(box("n:R-0"), { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText("Canvas: next box in the column")).toBeTruthy();
    expect(within(dialog).getByText("Canvas: centre the graph on the box, options kept")).toBeTruthy();
  });

  // Amended by docs/features/ui-home.md "Amendment": exactly the shell's one listener, the chord's.
  it("adds no key listener to the document or the window when the canvas mounts, but the shell's chord listener", async () => {
    const onDocument = vi.spyOn(document, "addEventListener");
    const onWindow = vi.spyOn(window, "addEventListener");
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    const keyListeners = [
      ...onDocument.mock.calls.map(([type, , options]) => ["document", type, options]),
      ...onWindow.mock.calls.map(([type, , options]) => ["window", type, options]),
    ].filter(([, type]) => typeof type === "string" && /^key/.test(type));
    expect(keyListeners).toEqual([["document", "keydown", true]]);
  });
});

describe("accessible names and words (AC-09)", () => {
  it("names each box by its name, kind, title and distance, focus and archived marked", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    expect(screen.getByRole("group", { name: "A1, gadget, First gadget, distance 1" })).toBe(box("n:A1"));
    expect(box("n:R-0").getAttribute("aria-label")).toBe("R-0, widget, The root widget, distance 0, focus");
    expect(box("n:A3").getAttribute("aria-label")).toBe("A3, gadget, Third gadget, distance 1, archived");
    expect(box("n:B2").getAttribute("aria-label")).toBe("B2, no kind, no title, distance 2");
    const canvas = screen.getByRole("application", { name: "Graph canvas" });
    expect(document.getElementById(canvas.getAttribute("aria-describedby") ?? "")?.textContent).toBe(
      "The List view holds every node and edge as text.",
    );
  });

  it("never speaks of deleting, moving, dragging or connecting on the canvas", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    const canvas = screen.getByRole("application", { name: "Graph canvas" });
    const words = [canvas.textContent];
    for (const element of [canvas, ...Array.from(canvas.querySelectorAll("*"))]) {
      for (const attribute of ["aria-label", "title", "aria-description"]) {
        words.push(element.getAttribute(attribute) ?? "");
      }
    }
    expect(words.filter((word) => /delete|move|drag|connect/i.test(word))).toEqual([]);
  });

  it("lists the whole answer: nodes by distance, stubs, every edge line, each link to #/", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const panel = await screen.findByRole("tabpanel", { name: "List" });
    expect(within(panel).getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual([
      "Distance 0 (1)",
      "Distance 1 (3)",
      "Distance 2 (2)",
      "Not walked (3)",
      "Edges (9)",
    ]);
    const lines = Array.from(panel.querySelectorAll("section:last-of-type li"), (item) => item.textContent);
    expect(lines).toHaveLength(9);
    expect(lines).toContain(`A2 --zeta_type--> ${HOSTILE_LINK} | docs/spec/a2.md:3 | dangling: \`R-404\` resolves to no ID and no alias`);
    expect(lines).toContain("B2 --alpha_type--> other:R-9 | docs/spec/b2.md:3 | skipped");
    expect(within(panel).getAllByRole("link", { name: /^Centre here/ })).toHaveLength(6);
    expect(within(panel).getByRole("link", { name: "A1" }).getAttribute("href")).toBe("#/alpha/tree/A1");
    for (const anchor of Array.from(panel.querySelectorAll("a[href]"))) {
      expect(anchor.getAttribute("href")?.startsWith("#/")).toBe(true);
    }
  });
});

function fan(count: number): GraphView {
  const nodes: GraphNode[] = [aGraphNode({ id: "R-0", distance: 0 })];
  const edges: GraphEdge[] = [];
  for (let at = 0; at < count; at += 1) {
    const id = `N-${String(at).padStart(3, "0")}`;
    nodes.push(aGraphNode({ id, distance: 1 }));
    edges.push(aGraphEdge({ src: "R-0", type: "zeta_type", dst: id }));
  }
  return aGraphView(nodes, edges);
}

describe("limits (AC-10)", () => {
  it("collapses a column past 24 into '+k more'; Show in List opens its distance", async () => {
    const client = graphClient();
    client.getGraph.mockImplementation(() => Promise.resolve(fan(30)));
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    expect(boxes().filter((element) => element.dataset.id?.startsWith("n:N-"))).toHaveLength(24);
    const more = boxLabelled("+6 more at distance 1; every node is in the List");
    fireEvent.click(more);
    const details = await screen.findByRole("complementary");
    expect(within(details).getByText("The column at distance 1 holds 6 more boxes than the canvas draws; the List holds every one.")).toBeTruthy();
    fireEvent.click(within(details).getByRole("button", { name: "Show in List" }));
    const heading = await screen.findByRole("heading", { level: 2, name: "Distance 1 (30)" });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
    expect(screen.getByRole("tab", { name: "List" }).getAttribute("aria-selected")).toBe("true");
  });

  it.each([
    ["a column of stubs only (no distance heading)", 0, 30],
    ["a column whose hidden boxes are stubs", 20, 10],
  ])("Show in List focuses Not walked for %s, never the page", async (_case, walked, dangling) => {
    const view = fan(walked);
    const ends = Array.from({ length: dangling }, (_, at) =>
      aGraphEdge({ src: "R-0", type: "zeta_type", dst: null, written: `GONE-${String(at)}`, line: 40 + at, state: "dangling" }),
    );
    const client = graphClient();
    client.getGraph.mockImplementation(() => Promise.resolve({ ...view, edges: [...view.edges, ...ends] }));
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    fireEvent.click(box("m:1"));
    const details = await screen.findByRole("complementary");
    fireEvent.click(within(details).getByRole("button", { name: "Show in List" }));
    const heading = await screen.findByRole("heading", { level: 2, name: `Not walked (${String(dangling)})` });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
    expect(document.activeElement).not.toBe(document.body);
  });

  it("draws whole columns up to 200 boxes and says what the canvas leaves to the List", async () => {
    const nodes: GraphNode[] = [];
    const edges: GraphEdge[] = [];
    for (let distance = 0; distance < 12; distance += 1) {
      for (let at = 0; at < 20; at += 1) {
        const id = `D${String(distance)}-${String(at)}`;
        nodes.push(aGraphNode({ id, distance }));
        if (distance > 0) {
          edges.push(aGraphEdge({ src: `D${String(distance - 1)}-${String(at)}`, type: "zeta_type", dst: id }));
        }
      }
    }
    const client = graphClient();
    client.getGraph.mockImplementation(() => Promise.resolve(aGraphView(nodes, edges)));
    renderApp(client, "#/alpha/graph/D0-0");
    await drawn();
    expect(boxes()).toHaveLength(200);
    expect(screen.getByText("The canvas shows distance 0-9: 200 of 240 nodes; see the List.")).toBeTruthy();
  });

  it("says when the daemon cut the answer", async () => {
    const client = graphClient();
    client.getGraph.mockImplementation((_project, options) => Promise.resolve({ ...graphOf(options), truncated: true }));
    renderApp(client, "#/alpha/graph/R-0");
    expect(
      await screen.findByText("The daemon cut this answer: some nodes or edges are not in it. Lower the depth or follow fewer types."),
    ).toBeTruthy();
  });
});

describe("states (AC-11)", () => {
  it("loading: a skeleton, busy", async () => {
    const client = graphClient();
    client.getGraph.mockImplementation(() => new Promise<GraphView>(() => undefined));
    renderApp(client, "#/alpha/graph/R-0");
    const skeleton = await screen.findByRole("status", { name: "Drawing the graph of R-0" });
    expect(skeleton.getAttribute("aria-busy")).toBe("true");
    expect(results().getAttribute("aria-busy")).toBe("true");
  });

  it("an unknown REF: its reason and notes verbatim, Retry reads once more", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-404");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("`R-404` resolves to no ID and no alias");
    expect(screen.getByText("the index was refreshed before the walk")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await calls(client, 2);
  });

  it("a refusal: the daemon's message verbatim, Retry reads once more", async () => {
    const client = graphClient();
    client.getGraph.mockRejectedValueOnce(new ClientError({ status: 503, message: "spec: index locked (code 5)" }));
    renderApp(client, "#/alpha/graph/R-0");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("spec: index locked (code 5)");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await calls(client, 2);
    await drawn();
  });

  it("no edge: what it means and what to try next", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/LONE");
    expect(await screen.findByRole("heading", { level: 2, name: "No link to draw" })).toBeTruthy();
    expect(screen.getByText(/No link of the followed types leaves LONE\. 1 node, 0 edges\./)).toBeTruthy();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Follow mentions too" })).toBeTruthy();
    });
    expect(screen.getByRole("button", { name: "Include archive" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Show Impact" }));
    await calls(client, 2);
    expect(lastCall(client)).toEqual({ ref: "LONE", impact: true, depth: 2 });
  });

  it("shows the same note twice when the daemon says it twice", async () => {
    const client = graphClient();
    client.getGraph.mockImplementation((_project, options) =>
      Promise.resolve({ ...graphOf(options), notes: ["the index was refreshed", "the index was refreshed"] }),
    );
    renderApp(client, "#/alpha/graph/R-0");
    const notes = await screen.findByRole("list", { name: "Notes from the daemon" });
    expect(within(notes).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["the index was refreshed", "the index was refreshed"]);
  });

  it("an answer: the count spoken politely; Left out names only its non-zero parts", async () => {
    const client = graphClient();
    renderApp(client, "#/alpha/graph/R-0");
    await drawn();
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("6 nodes, 9 edges");
    });
    expect(screen.getByText("6 nodes, 9 edges", { selector: ".graph-count" })).toBeTruthy();
    const leftOut = screen.getByText(/^Left out:/);
    expect(leftOut.textContent).toBe("Left out: 1 generated, 2 archived (Include archive)");
    fireEvent.click(within(leftOut).getByRole("button", { name: "Include archive" }));
    await calls(client, 2);
    await waitFor(() => {
      expect(screen.getByText(/^Left out:/).textContent).toBe("Left out: 1 generated");
    });
  });
});

describe("a link from a section to itself", () => {
  it("lists it once under its stub", async () => {
    const view = aGraphView(
      [aGraphNode({ id: "R-0", distance: 0 }), aGraphNode({ id: "A1", distance: 1 })],
      [
        aGraphEdge({ src: "R-0", type: "zeta_type", dst: "A1" }),
        aGraphEdge({ src: "SEC-Z", type: "zeta_type", dst: "SEC-Z", path: "docs/spec/r-0.md", line: 20 }),
      ],
    );
    renderApp(clientOf(view), "#/alpha/graph/R-0");
    await drawn();
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const group = (await screen.findByRole("heading", { level: 2, name: "Not walked (1)" })).closest("section");
    expect(Array.from(group?.querySelectorAll(".graph-edge-lines li") ?? [], (item) => item.textContent)).toEqual([
      "SEC-Z --zeta_type--> SEC-Z | docs/spec/r-0.md:20",
    ]);
  });
});

describe("fit, zoom and the wheel (Canvas)", () => {
  it("fits each answer at a readable zoom, the focus at the margin; the Fit button does it again", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    // jsdom measures nothing: React Flow takes a 500 x 500 canvas, too small for four columns at 0.6.
    await waitFor(() => {
      expect(viewport().zoom).toBe(FIT_MIN_ZOOM);
    });
    expect(viewport().x).toBe(FIT_MARGIN);
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    await waitFor(() => {
      expect(viewport().zoom).toBeGreaterThan(FIT_MIN_ZOOM);
    });
    fireEvent.click(screen.getByRole("button", { name: "Fit the graph to the view, its text readable" }));
    await waitFor(() => {
      expect(viewport()).toEqual(expect.objectContaining({ x: FIT_MARGIN, zoom: FIT_MIN_ZOOM }));
    });
  });

  it("leaves the wheel to the page: no zoom, no prevented scroll", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    await waitFor(() => {
      expect(viewport().zoom).toBe(FIT_MIN_ZOOM);
    });
    const before = viewport();
    const pane = document.querySelector<HTMLElement>(".react-flow__pane");
    expect(pane).not.toBeNull();
    const event = new WheelEvent("wheel", { deltaY: 120, bubbles: true, cancelable: true });
    pane?.dispatchEvent(event);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(event.defaultPrevented).toBe(false);
    expect(viewport()).toEqual(before);
  });
});

describe("hostile text and links (AC-12)", () => {
  it("shows a hostile title as text, draws no img, links only to #/, shows no attribution link", async () => {
    renderApp(graphClient(), "#/alpha/graph/R-0");
    await drawn();
    expect(within(box("n:A2")).getByText(HOSTILE)).toBeTruthy();
    fireEvent.click(box("n:A2"));
    const details = await screen.findByRole("complementary");
    expect(within(details).getByText(HOSTILE)).toBeTruthy();
    expect(document.querySelector("img")).toBeNull();
    expect(document.querySelector(".react-flow__attribution")).toBeNull();
    for (const anchor of Array.from(document.querySelectorAll("a[href]"))) {
      expect([anchor.textContent, anchor.getAttribute("href")?.startsWith("#")]).toEqual([anchor.textContent, true]);
    }
    for (const anchor of Array.from(screen.getByRole("main").querySelectorAll("a[href]"))) {
      expect(anchor.getAttribute("href")?.startsWith("#/")).toBe(true);
    }
  });
});

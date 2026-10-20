import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import { aRoadmap, aRoadmapGroup, aRoadmapItem } from "../test/builders";
import { renderApp } from "../test/render";
import { aPlan, EVERY_TASK_LABEL, EVERY_TASK_STATE, PLAN_LINE, roadmapClient } from "../test/roadmapStub";
import { argsOf } from "../test/stubClient";

// UI-3 of docs/features/roadmap-screen.md on the stub client: every mark of the Lanes tab is text
// plus an icon, never a colour alone (Current, MVP cut, In a cycle, Waits on, unestimated, every
// task state, an unknown one raw); the links (the ID to the Tree, a wait to the Roadmap, a task to
// Tasks); selection by route; the unlisted-ID note; the header line, the cut's rule, the unplaced
// releases; the states (loading, failure, a failed re-read, no items, all done); the one read.

function lane(name: string): HTMLElement {
  const heading = screen.getByRole("heading", { level: 2, name });
  const section = heading.closest("section");
  if (section === null) {
    throw new Error(`no lane ${name}`);
  }
  return section;
}

function card(id: string): HTMLElement {
  const found = Array.from(document.querySelectorAll<HTMLElement>("[data-item]")).find((element) => element.dataset.item === id);
  if (found === undefined) {
    throw new Error(`no card ${id}`);
  }
  return found;
}

/** A mark: its visible label and whether an icon sits inside it. */
function marked(element: Element | null | undefined): [string, boolean] {
  return [element?.querySelector(".badge-label")?.textContent ?? element?.textContent ?? "", element?.querySelector("svg.icon") !== null && element !== null];
}

function badges(scope: HTMLElement, tone: string): Element[] {
  return Array.from(scope.querySelectorAll(`.badge.tone-${tone}`));
}

async function openRoadmap(hash = "#/alpha/roadmap", client = roadmapClient()) {
  renderApp(client, hash);
  await screen.findByRole("heading", { level: 1, name: "Roadmap" });
  await screen.findByRole("tab", { name: "Lanes" });
  return client;
}

/** A link clicked as the browser follows it (jsdom moves the hash only a task later). */
async function follow(link: HTMLElement) {
  fireEvent.click(link);
  await act(async () => {
    window.location.hash = link.getAttribute("href") ?? "";
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

describe("the read and the header (UI-3)", () => {
  it("reads the whole plan once, sending no option, with the query's signal", async () => {
    const client = await openRoadmap();
    expect(argsOf(client.getRoadmap)).toEqual([["alpha"]]);
    expect(client.getRoadmap.mock.calls[0]?.[1]).toBeUndefined();
    expect(client.getRoadmap.mock.calls[0]?.[2]).toBeInstanceOf(AbortSignal);
  });

  it("shows the header line as the CLI's first line less `roadmap: `, and Read again reads once more", async () => {
    const client = await openRoadmap();
    expect(document.querySelector(".roadmap-line")?.textContent).toBe(PLAN_LINE);
    fireEvent.click(screen.getByRole("button", { name: "Read again" }));
    await waitFor(() => {
      expect(client.getRoadmap).toHaveBeenCalledTimes(2);
    });
  });

  it("writes the header's sums by the CLI's rule: the bound alone when equal, no unit unset, no unestimated at 0, current none", async () => {
    const plan = aRoadmap([aRoadmapGroup("release", "beta", [aRoadmapItem({ id: "RM-0001", estimate: { min: 3, max: 3 } })])], {
      unit: null,
      current: null,
      cut: { release: "beta", min: 3, max: 3, unestimated: 0 },
    });
    await openRoadmap("#/alpha/roadmap", roadmapClient(plan));
    expect(document.querySelector(".roadmap-line")?.textContent).toBe("1 open, 0 done; current none; to beta 3; total 3");
  });

  it("opens on Lanes, each group a lane in served order, the notes last, verbatim", async () => {
    await openRoadmap();
    expect(screen.getAllByRole("tab").map((tab) => [tab.textContent, tab.getAttribute("aria-selected")])).toEqual([
      ["Lanes", "true"],
      ["Chain", "false"],
    ]);
    expect(screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual(["alpha", "beta", "launch", "Pool", "Unplaced", "Notes"]);
    expect(Array.from(lane("Notes").querySelectorAll("li"), (item) => item.textContent)).toEqual([
      'RM-0050: release "someday" is not in [roadmap] releases',
      "depends_on cycle: RM-0020, RM-0021",
    ]);
    expect(document.querySelector(".roadmap-view")?.textContent).not.toMatch(/block/i);
  });

  it("moves focus to a lane's heading from its jump button; no in-page link", async () => {
    await openRoadmap();
    const jumps = within(screen.getByRole("navigation", { name: "Lanes of the plan" }));
    expect(jumps.getAllByRole("button").map((button) => button.textContent)).toEqual(["alpha", "beta", "launch", "Pool", "Unplaced", "Notes"]);
    expect(jumps.queryAllByRole("link")).toEqual([]);
    fireEvent.click(jumps.getByRole("button", { name: "launch" }));
    expect(document.activeElement).toBe(screen.getByRole("heading", { level: 2, name: "launch" }));
    expect(window.location.hash).toBe("#/alpha/roadmap");
  });
});

describe("marks are text plus an icon, never colour alone (UI-3)", () => {
  it("marks the current release and the MVP cut on beta, each with its icon; the cut's rule follows beta with the cut's sum", async () => {
    await openRoadmap();
    expect(badges(lane("beta"), "roadmap-current").map(marked)).toEqual([["Current", true]]);
    expect(badges(lane("beta"), "roadmap-cut").map(marked)).toEqual([["MVP cut", true]]);
    for (const other of ["alpha", "launch", "Pool", "Unplaced"]) {
      expect([other, badges(lane(other), "roadmap-current").length, badges(lane(other), "roadmap-cut").length]).toEqual([other, 0, 0]);
    }
    const rule = document.querySelector(".roadmap-cut-rule");
    expect(marked(rule)).toEqual(["MVP cut: 10-12 days, 1 unestimated", true]);
    expect(rule?.previousElementSibling).toBe(lane("beta"));
  });

  it("says each lane's counts and sum as text, its bar beside it hidden from readers; the pool and unplaced have no bar", async () => {
    await openRoadmap();
    expect(lane("beta").querySelector(".roadmap-lane-counts")?.textContent).toBe("4 open, 1 done");
    expect(lane("beta").querySelector(".roadmap-sum-text")?.textContent).toBe("10-12 days, 1 unestimated");
    expect(lane("alpha").querySelector(".roadmap-sum-text")?.textContent).toBe("0 days");
    expect(lane("Unplaced").querySelector(".roadmap-sum-text")?.textContent).toBe("2-6 days, 1 unestimated");
    for (const name of ["alpha", "beta", "launch"]) {
      expect([name, lane(name).querySelector(".roadmap-bar")?.getAttribute("aria-hidden")]).toEqual([name, "true"]);
    }
    expect(lane("Pool").querySelector(".roadmap-bar")).toBeNull();
    expect(lane("Unplaced").querySelector(".roadmap-bar")).toBeNull();
    // Scale: the largest release max, 12; beta solid to 10, striped to 12.
    expect(lane("beta").querySelector<HTMLElement>(".roadmap-bar-solid")?.style.width).toBe(`${String((10 / 12) * 100)}%`);
  });

  it("draws no bar when every release's max is 0; the text stays", async () => {
    const plan = aRoadmap([aRoadmapGroup("release", "beta", [aRoadmapItem({ id: "RM-0001" })], { current: true })]);
    await openRoadmap("#/alpha/roadmap", roadmapClient(plan));
    expect(document.querySelector(".roadmap-bar")).toBeNull();
    expect(lane("beta").querySelector(".roadmap-sum-text")?.textContent).toBe("0 days, 1 unestimated");
  });

  it("marks a cycle, a wait and an unestimated item with their words and icons", async () => {
    await openRoadmap();
    expect(badges(card("RM-0021"), "roadmap-cycle").map(marked)).toEqual([["In a cycle", true]]);
    expect(badges(card("RM-0060"), "roadmap-cycle").map(marked)).toEqual([["In a cycle", true]]);
    expect(badges(card("RM-0030"), "roadmap-cycle")).toEqual([]);
    expect(badges(card("RM-0011"), "roadmap-waits").map(marked)).toEqual([["Waits on", true]]);
    expect(badges(card("RM-0017"), "roadmap-waits")).toEqual([]);
    expect(badges(card("RM-0013"), "roadmap-unestimated").map(marked)).toEqual([["unestimated", true]]);
    expect(card("RM-0017").querySelector(".roadmap-estimate")?.textContent).toBe("5 days");
    expect(card("RM-0010").querySelector(".roadmap-estimate")?.textContent).toBe("2-4 days");
  });

  it("shows every task state by its label and icon, an unknown one raw, each task linked to Tasks", async () => {
    await openRoadmap();
    const cells = [...Array.from(card("RM-0017").querySelectorAll("[data-task]")), ...Array.from(card("RM-0010").querySelectorAll("[data-task]"))];
    expect(cells.map((cell) => cell.getAttribute("data-task"))).toEqual(EVERY_TASK_STATE.map((task) => task.id));
    expect(cells.map((cell) => marked(cell.querySelector(".badge")))).toEqual(EVERY_TASK_LABEL.map((label) => [label, true]));
    expect(cells.at(-1)?.querySelector(".badge")?.className).toContain("tone-task-unknown");
    expect(cells.map((cell) => cell.querySelector("a")?.getAttribute("href"))).toEqual(EVERY_TASK_STATE.map((task) => `#/alpha/tasks/${task.id}`));
  });

  it("shows an item's status raw in a neutral badge, its priority raw, `-` for either when absent; the item's in-progress is not a task's", async () => {
    await openRoadmap();
    expect(marked(card("RM-0013").querySelector(".tone-roadmap-status"))).toEqual(["status parked", true]);
    expect(marked(card("RM-0010").querySelector(".tone-roadmap-status"))).toEqual(["status in-progress", true]);
    expect(card("RM-0010").querySelector(".tone-roadmap-status")?.className).not.toContain("tone-task");
    expect(marked(card("RM-0041").querySelector(".tone-roadmap-status"))).toEqual(["status -", true]);
    expect(card("RM-0017").querySelector(".roadmap-priority")?.textContent).toBe("priority must");
    expect(card("RM-0041").querySelector(".roadmap-priority")?.textContent).toBe("priority -");
    expect(card("RM-0041").querySelector(".roadmap-card-name")?.textContent).toBe("-");
  });
});

describe("cards and links (UI-3)", () => {
  it("numbers release items, links each ID to its Tree node, shows its path; pool and unplaced items have no number", async () => {
    await openRoadmap();
    expect(Array.from(lane("beta").querySelectorAll(".roadmap-position"), (position) => position.textContent)).toEqual(["1.", "2.", "3.", "4."]);
    expect(lane("Pool").querySelectorAll(".roadmap-position")).toHaveLength(0);
    expect(within(card("RM-0017")).getByRole("link", { name: "RM-0017" }).getAttribute("href")).toBe("#/alpha/tree/RM-0017");
    expect(card("RM-0017").querySelector(".roadmap-path")?.textContent).toBe("plan/RM-0017.md");
  });

  it("links each wait to the Roadmap, across lanes too", async () => {
    await openRoadmap();
    const waits = Array.from(card("RM-0022").querySelectorAll<HTMLAnchorElement>("[data-waits-on]"));
    expect(waits.map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
      ["RM-0030", "#/alpha/roadmap/RM-0030"],
      ["RM-0020", "#/alpha/roadmap/RM-0020"],
      ["RM-0011", "#/alpha/roadmap/RM-0011"],
    ]);
    expect(card("RM-0022").querySelector(".roadmap-waits")?.textContent).toBe("Waits onRM-0030, RM-0020, RM-0011");
  });

  it("shows an unplaced item's release as written, or `release not a string`", async () => {
    await openRoadmap();
    expect(card("RM-0050").querySelector(".roadmap-release")?.textContent).toBe("release someday");
    expect(card("RM-0051").querySelector(".roadmap-release")?.textContent).toBe("release not a string");
    expect(card("RM-0017").querySelector(".roadmap-release")).toBeNull();
  });

  it("lists the pool's tasks after its items: ID to Tasks, state, title or -, targets or -", async () => {
    await openRoadmap();
    const rows = Array.from(lane("Pool").querySelectorAll<HTMLElement>("[data-pool-task]"));
    expect(rows.map((row) => row.dataset.poolTask)).toEqual(["T-0020", "T-0021"]);
    expect(rows.map((row) => [row.querySelector("a")?.getAttribute("href"), marked(row.querySelector(".badge"))])).toEqual([
      ["#/alpha/tasks/T-0020", ["Ready", true]],
      ["#/alpha/tasks/T-0021", ["Draft", true]],
    ]);
    expect(rows.map((row) => [row.querySelector(".roadmap-pool-task-title")?.textContent, row.querySelector(".roadmap-pool-task-targets")?.textContent])).toEqual([
      ["Harbor zone pass", "BAY-HARBOR, docs/spec/bay.md"],
      ["-", "-"],
    ]);
    expect(lane("Pool").querySelector(".roadmap-items")?.compareDocumentPosition(rows[0] ?? document.body)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  });
});

describe("selection by the route (UI-3)", () => {
  it("marks the route's card current and focuses it; a wait's link selects that card", async () => {
    await openRoadmap("#/alpha/roadmap/RM-0011");
    await waitFor(() => {
      expect(document.activeElement).toBe(card("RM-0011"));
    });
    expect(Array.from(document.querySelectorAll('[aria-current="true"][data-item]'), (element) => element.getAttribute("data-item"))).toEqual(["RM-0011"]);
    await follow(within(card("RM-0011")).getByRole("link", { name: "RM-0010" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(card("RM-0010"));
    });
    expect(card("RM-0011").getAttribute("aria-current")).toBeNull();
    expect(card("RM-0010").getAttribute("aria-current")).toBe("true");
    expect(screen.getByRole("heading", { level: 1, name: "Roadmap" })).toBeTruthy();
  });

  it("says an ID no group lists is not an open item, links its Tree node, selects nothing, reads nothing more", async () => {
    const client = await openRoadmap("#/alpha/roadmap/RM-0099");
    const note = document.querySelector(".roadmap-unlisted");
    expect(note?.textContent).toBe("RM-0099 is not an open item; done items are counted, not listedOpen RM-0099 in the Spec tree");
    expect(within(note as HTMLElement).getByRole("link", { name: "Open RM-0099 in the Spec tree" }).getAttribute("href")).toBe("#/alpha/tree/RM-0099");
    expect(note?.querySelector("svg.icon")).not.toBeNull();
    expect(document.querySelectorAll('[aria-current="true"][data-item]')).toHaveLength(0);
    expect(client.getRoadmap).toHaveBeenCalledTimes(1);
  });

  it("keeps the chosen tab per project while the page lives", async () => {
    await openRoadmap();
    fireEvent.click(screen.getByRole("tab", { name: "Chain" }));
    await follow(within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Tasks" }));
    await screen.findByRole("heading", { level: 1, name: "Tasks" });
    await follow(within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Roadmap" }));
    const chain = await screen.findByRole("tab", { name: "Chain" });
    expect(chain.getAttribute("aria-selected")).toBe("true");
  });

  it("lists the general keys and the Roadmap's intro under ?", async () => {
    await openRoadmap();
    fireEvent.keyDown(screen.getByRole("heading", { level: 1, name: "Roadmap" }), { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(dialog.querySelector(".dialog-text")?.textContent).toBe(
      "No key moves an item: the Roadmap is read only, and an item moves when its file's release, priority or depends_on changes. Keys act outside a text field, without Ctrl, Alt or Cmd; Cmd-K or Ctrl-K excepted: it opens Jump to from anywhere, a text field included.",
    );
    expect(Array.from(dialog.querySelectorAll(".shortcut dd"), (item) => item.textContent)).toEqual([
      "Jump to a section, task, proposal, node or project",
      "Show this list",
      "Close a dialog",
    ]);
  });
});

describe("states (UI-3)", () => {
  it("shows a skeleton with aria-busy while the plan is read", async () => {
    const client = roadmapClient();
    client.getRoadmap.mockReturnValue(new Promise(() => undefined));
    renderApp(client, "#/alpha/roadmap");
    const skeleton = await screen.findByRole("status", { name: "Reading the plan of alpha" });
    expect(skeleton.getAttribute("aria-busy")).toBe("true");
  });

  it("shows a failure in the daemon's words with Retry and the fixed line; Retry reads again", async () => {
    const client = roadmapClient();
    const message = "spec: specengine.toml has no [roadmap]";
    client.getRoadmap.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/roadmap");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The plan could not be read${message}`);
    expect(document.querySelector(".roadmap-no-plan")?.textContent).toBe("A project has a plan once its specengine.toml has a [roadmap] table");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("tab", { name: "Lanes" })).toBeTruthy();
    expect(client.getRoadmap).toHaveBeenCalledTimes(2);
  });

  it("keeps the plan when a re-read fails, with the re-read line and Retry", async () => {
    const client = await openRoadmap();
    client.getRoadmap.mockRejectedValueOnce(new ClientError({ status: 503, message: "spec: the index is locked" }));
    fireEvent.click(screen.getByRole("button", { name: "Read again" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("The plan could not be read again: spec: the index is locked");
    expect(card("RM-0017")).toBeTruthy();
    fireEvent.click(within(document.querySelector(".roadmap-reread-failed") as HTMLElement).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(screen.queryByRole("alert")).toBeNull();
    });
    expect(client.getRoadmap).toHaveBeenCalledTimes(3);
  });

  it("says no plan items yet, naming the dir and the prefix, above the tabs; the lanes still drawn", async () => {
    const plan = aRoadmap([aRoadmapGroup("release", "beta", []), aRoadmapGroup("pool", null, []), aRoadmapGroup("unplaced", null, [])], { dir: "docs/roadmap" });
    await openRoadmap("#/alpha/roadmap", roadmapClient(plan));
    const line = document.querySelector(".roadmap-empty");
    expect(line?.textContent).toBe("No plan items yet: an item is a file in docs/roadmap whose id starts with RM-");
    expect(line?.compareDocumentPosition(screen.getByRole("tablist"))).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual(["beta", "Pool", "Unplaced"]);
  });

  it("says every item is done, with the count, above the tabs; the lanes still drawn", async () => {
    const plan = aRoadmap([aRoadmapGroup("release", "beta", [], { done: 4 }), aRoadmapGroup("pool", null, [], { done: 1 })]);
    await openRoadmap("#/alpha/roadmap", roadmapClient(plan));
    expect(document.querySelector(".roadmap-empty")?.textContent).toBe("Every item is done (5)");
    expect(screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual(["beta", "Pool"]);
  });

  it("says nothing of the kind for a plan with open items", async () => {
    await openRoadmap();
    expect(document.querySelector(".roadmap-empty")).toBeNull();
    expect(aPlan().counts.open).toBe(13);
  });
});

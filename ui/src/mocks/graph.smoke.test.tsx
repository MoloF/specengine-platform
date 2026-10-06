import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The Graph over the real mock, as `pnpm dev` shows it: "Show in graph" from the tree, MEC-TIDES'
// stubs, and the `large` scenario's Impact from DOM-GEN-01: AC-10's collapsed column at depth 2,
// AC-09's whole List under All (docs/features/ui-graph.md).

function renderMock(scenario: Scenario, hash: string) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return client;
}

function boxes(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".react-flow__node"));
}

async function drawn(): Promise<void> {
  await waitFor(() => {
    expect(boxes().length).toBeGreaterThan(0);
  });
}

describe("the Graph over the mock", () => {
  it("opens from the tree's node pane with Show in graph", async () => {
    renderMock("normal", "#/harbor-sim/tree/MEC-TIDES");
    const link = await screen.findByRole("link", { name: "Show in graph" });
    expect(link.getAttribute("href")).toBe("#/harbor-sim/graph/MEC-TIDES");
    fireEvent.click(link);
    window.location.hash = "#/harbor-sim/graph/MEC-TIDES";
    expect(await screen.findByRole("heading", { level: 1, name: "Graph: MEC-TIDES" })).toBeTruthy();
    await drawn();
    const labels = boxes().map((element) => element.getAttribute("aria-label"));
    expect(labels).toContain("MEC-TIDES, mechanic, Tide cycle, distance 0, focus");
    // The adopts link is written in MEC-TIDES' nested section: an end named by itself.
    expect(labels).toContain("RULE-TIDE-WINDOW, not walked: a section");
    expect(labels).toContain("TERM-SLACK-WATER, not walked: Dangling");
    expect(labels).toContain("harbor-ops:MEC-SHIFTS, not walked: Skipped: another project");
  });

  it("large, Impact from DOM-GEN-01, depth 2: the distance-2 column shows 24 boxes and +78 more", async () => {
    renderMock("large", "#/harbor-sim/graph/DOM-GEN-01");
    expect(await screen.findByRole("heading", { level: 2, name: "No link to draw" })).toBeTruthy();
    fireEvent.click(screen.getByLabelText("Impact"));
    await drawn();
    const atDistance = (distance: number) =>
      boxes().filter((element) => element.getAttribute("aria-label")?.includes(`distance ${String(distance)}`) === true && element.dataset.id?.startsWith("n:") === true);
    await waitFor(() => {
      expect(atDistance(2)).toHaveLength(24);
    });
    expect(atDistance(1)).toHaveLength(17);
    expect(boxes().find((element) => element.dataset.id === "m:2")?.textContent).toContain("+78 more at distance 2");
    expect(screen.getByText("120 nodes, 119 edges", { selector: ".graph-count" })).toBeTruthy();
  });

  it("large, Impact from DOM-GEN-01, All: the List holds 516 nodes and every edge, each link to #/", async () => {
    renderMock("large", "#/harbor-sim/graph/DOM-GEN-01");
    await screen.findByRole("heading", { level: 2, name: "No link to draw" });
    fireEvent.click(screen.getByLabelText("Impact"));
    fireEvent.change(screen.getByLabelText("Depth"), { target: { value: "all" } });
    await waitFor(() => {
      expect(screen.getByText("516 nodes, 515 edges", { selector: ".graph-count" })).toBeTruthy();
    });
    expect(boxes().length).toBeLessThanOrEqual(200);
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const panel = await screen.findByRole("tabpanel", { name: "List" });
    expect(within(panel).getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual([
      "Distance 0 (1)",
      "Distance 1 (17)",
      "Distance 2 (102)",
      "Distance 3 (396)",
      "Edges (515)",
    ]);
    expect(panel.querySelectorAll(".graph-list-node")).toHaveLength(516);
    expect(panel.querySelectorAll(".graph-list-group:last-child li")).toHaveLength(515);
    const anchors = Array.from(panel.querySelectorAll("a[href]"));
    expect(anchors).toHaveLength(1032);
    expect(anchors.every((anchor) => anchor.getAttribute("href")?.startsWith("#/") === true)).toBe(true);
    await act(async () => {
      await Promise.resolve();
    });
  });
});

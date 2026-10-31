import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The Flow over the real mock, as `pnpm dev` shows it (docs/features/flow-canvas.md): AC-02's reads
// on harbor-sim, AC-08 on ledger-api, AC-09's item two levels down, AC-11's `large` past the cap.

function renderMock(scenario: Scenario, hash: string) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  const getFlow = vi.spyOn(client, "getFlow");
  const getTree = vi.spyOn(client, "getTree");
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { getFlow, getTree };
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

async function drawn(name: string) {
  await waitFor(() => {
    expect(boxes().some((element) => element.dataset.id === `n:${name}`)).toBe(true);
  });
}

function caret(name: string): Element {
  const found = box(name).querySelector("[data-flow-toggle]");
  if (found === null) {
    throw new Error(`no caret on ${name}`);
  }
  return found;
}

function options(getFlow: ReturnType<typeof renderMock>["getFlow"]) {
  return getFlow.mock.calls.map((call) => call[1]);
}

describe("the Flow over the mock", () => {
  it("harbor-sim: reads the top and the sole group, never the whole flow; opening adds a read, closing and reopening none (AC-02)", async () => {
    const { getFlow } = renderMock("normal", "#/harbor-sim/flow");
    await drawn("DOM-BERTHS");
    expect(options(getFlow)).toEqual([{ depth: 0 }, { root: "DOM-HARBOR", depth: 1 }]);
    fireEvent.click(caret("DOM-BERTHS"));
    await drawn("MEC-MOORING");
    expect(options(getFlow)).toEqual([{ depth: 0 }, { root: "DOM-HARBOR", depth: 1 }, { root: "DOM-BERTHS", depth: 1 }]);
    fireEvent.click(caret("DOM-BERTHS"));
    await waitFor(() => {
      expect(boxes().some((element) => element.dataset.id === "n:MEC-MOORING")).toBe(false);
    });
    fireEvent.click(caret("DOM-BERTHS"));
    await drawn("MEC-MOORING");
    expect(getFlow).toHaveBeenCalledTimes(3);
    expect(options(getFlow).every((asked) => asked.depth !== undefined)).toBe(true);
  });

  it("harbor-sim: draws arcs, a loop and a band; the child with no flow link lies in it", async () => {
    renderMock("normal", "#/harbor-sim/flow");
    await drawn("DOM-BERTHS");
    fireEvent.click(caret("DOM-BERTHS"));
    await drawn("docs/spec/berths/draft-limits.md");
    expect(screen.getByText("Not on the walk")).toBeTruthy();
    expect(box("DOM-BERTHS").textContent).toContain("Berths and moorings");
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__edge").length).toBeGreaterThan(5);
    });
  });

  it("a gap item two levels down: its closed group shows its gaps; one tree read, both group reads, focus kept (AC-09)", async () => {
    const { getFlow, getTree } = renderMock("normal", "#/harbor-sim/flow");
    await drawn("DOM-WATER");
    expect(box("DOM-WATER").textContent).toContain("2 with gaps");
    const member = await screen.findByRole("button", { name: "MEC-TIDE-TABLES" });
    member.focus();
    fireEvent.click(member);
    await drawn("MEC-TIDE-TABLES");
    expect(getTree).toHaveBeenCalledTimes(1);
    expect(options(getFlow)).toEqual([{ depth: 0 }, { root: "DOM-HARBOR", depth: 1 }, { root: "DOM-WATER", depth: 1 }, { root: "MEC-TIDES", depth: 1 }]);
    await waitFor(() => {
      expect(window.location.hash).toBe("#/harbor-sim/flow/MEC-TIDE-TABLES");
    });
    expect(box("MEC-TIDE-TABLES").getAttribute("aria-label")).toContain("marked");
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "MEC-TIDE-TABLES" }));
  });

  it("a deep link selects: the same chain, the panel shows its text and links", async () => {
    const { getTree } = renderMock("normal", "#/harbor-sim/flow/MEC-TIDE-TABLES");
    await drawn("MEC-TIDE-TABLES");
    expect(getTree).toHaveBeenCalledTimes(1);
    const panel = (await screen.findByRole("heading", { level: 2, name: "MEC-TIDE-TABLES" })).closest("aside");
    expect(panel?.textContent).toContain("Changes go through your AI, as proposals.");
  });

  it("ledger-api: the canvas drawn, rows Not configured, cause verbatim, no digit in them (AC-08)", async () => {
    renderMock("normal", "#/ledger-api/flow");
    const gaps = await screen.findByRole("region", { name: "Gaps" });
    expect(within(gaps).getByText("7 rows need [flow] rules in specengine.toml")).toBeTruthy();
    expect(within(gaps).getAllByText("Not configured")).toHaveLength(7);
    const row = within(gaps).getByText("no-start").closest("li");
    expect(row?.textContent).toContain("no [[flow.start]] rule: what starts a flow is the project's to declare");
    expect(row?.textContent).not.toMatch(/\d/);
    await waitFor(() => {
      expect(boxes().length).toBeGreaterThan(0);
    });
  });

  it("empty: the meaning and the next step, the rows stay", async () => {
    renderMock("empty", "#/harbor-sim/flow");
    expect(await screen.findByText("No flow in this project yet. Next: have your AI add flow links.")).toBeTruthy();
    expect(within(screen.getByRole("region", { name: "Gaps" })).getByText("Cannot measure")).toBeTruthy();
  });

  it("error: the daemon's message, Retry", async () => {
    renderMock("error", "#/harbor-sim/flow");
    expect(await screen.findByText(/database is locked/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  });

  it("large, opened past 200: 200 boxes, the line, the List whole (AC-11)", async () => {
    renderMock("large", "#/harbor-sim/flow");
    await drawn("ZONE-GEN-25");
    expect(boxes()).toHaveLength(25);
    for (let group = 1; group <= 10; group += 1) {
      const name = `ZONE-GEN-${String(group).padStart(2, "0")}`;
      fireEvent.click(caret(name));
      await drawn(`${name}-${group === 10 ? "01" : "19"}`);
    }
    await screen.findByText("15 of 215 boxes not drawn: close a group, or read the List");
    await waitFor(() => {
      expect(boxes()).toHaveLength(200);
    });
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const panel = await screen.findByRole("tabpanel", { name: "List" });
    expect(panel.querySelectorAll("[data-flow-row]")).toHaveLength(215);
    expect(within(panel).getAllByRole("heading", { level: 2 })).toHaveLength(11);
    await act(async () => {
      await Promise.resolve();
    });
  });
});

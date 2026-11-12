import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The Overview and the palette over the real mock, as `pnpm dev` shows them
// (docs/features/department-overview.md "Acceptance criteria", DO3, DO4, DO6, DO12; docs/features/ui-home.md
// "Owner's manual check" for `large`). harbor-sim serves three departments: galley (copy, its sections
// list Flow), foundry (plates, Health and no Flow) and the harbourmaster (every flow): each shows its
// blocks by config alone, and the harbourmaster sets them side by side.

const METHODS = ["getProjects", "getInbox", "getTree", "getNode", "search", "getBundle", "getGraph", "getTasks", "getTask", "stageDecision", "unstageDecision", "getCardCounts", "getFlow", "getHealth"] as const;

function renderMock(scenario: Scenario, hash: string, delayMs = 0) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs, now: () => Date.parse("2026-10-06T12:00:00Z") });
  const spies = Object.fromEntries(METHODS.map((name) => [name, vi.spyOn(client, name)]));
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, spies };
}

function calls(spies: Record<string, { mock: { calls: unknown[] } }>): Record<string, number> {
  return Object.fromEntries(Object.entries(spies).flatMap(([name, spy]) => (spy.mock.calls.length > 0 ? [[name, spy.mock.calls.length]] : [])));
}

/** The Overview's blocks, settled: the counts and any Gaps read answered. */
async function settled() {
  await waitFor(() => {
    expect(document.querySelectorAll('.department-overview[aria-busy="false"]')).toHaveLength(1);
    expect(document.querySelectorAll('.department-block[aria-busy="true"]')).toHaveLength(0);
  });
}

/** harbor-sim's Overview in a department (galley, the first, when none is chosen). */
async function loadedHome(scenario: Scenario = "normal", hash = "#/harbor-sim", department: string | null = null) {
  if (department !== null) {
    window.localStorage.setItem("specengine.workspace.harbor-sim", department);
  }
  const rendered = renderMock(scenario, hash);
  await screen.findByRole("heading", { level: 1, name: "Overview" });
  await settled();
  return rendered;
}

function blocks(): string[] {
  return Array.from(document.querySelectorAll(".department-overview h2"), (heading) => heading.textContent);
}

function rowsOf(name: string): string[][] {
  const table = screen.getByRole("table", { name });
  return Array.from(table.querySelectorAll("tr"), (row) => Array.from(row.children, (cell) => cell.textContent));
}

describe("the Overview over the mock (DO12)", () => {
  it("#/ lands on harbor-sim's home in foundry, which lands on the Overview: the counts read once, no inbox, no agent tasks, no flow, no health (DO3)", async () => {
    // foundry lands on the Overview; galley, the first, on the Flow (docs/features/workspaces.md WS-11).
    window.localStorage.setItem("specengine.workspace.harbor-sim", "foundry");
    const { spies } = renderMock("normal", "#/");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    expect(window.location.hash).toBe("#/harbor-sim");
    await settled();
    expect(calls(spies)).toEqual({ getProjects: 1, getCardCounts: 1 });
  });

  it.each([
    ["galley", ["Work", "Waiting on this department", "Gaps in the flow"]],
    ["foundry", ["Work", "Waiting on this department"]],
    ["harbourmaster", ["Work", "Waiting on this department", "Gaps in the flow", "Departments side by side"]],
  ])("%s shows its blocks by its sections and flows alone (DO6)", async (department, expected) => {
    await loadedHome("normal", "#/harbor-sim", department);
    expect(blocks()).toEqual(expected);
  });

  it("galley's Work: the copy desk by status as the Board heads its columns; what waits on it, each to its page", async () => {
    await loadedHome();
    expect(rowsOf("Tasks by status, per flow")).toEqual([
      ["Flow", "To do", "In progress", "Waiting for review", "Returned", "Done"],
      ["Copy desk", "3", "3", "1", "0", "4"],
    ]);
    expect(Array.from(document.querySelectorAll(".department-aside li"), (item) => item.textContent)).toEqual(["1 cancelled"]);
    const waiting = screen.getByRole("region", { name: "Waiting on this department" });
    expect(Array.from(waiting.querySelectorAll(".property-row"), (row) => [row.querySelector("dt")?.textContent, row.querySelector(".department-count")?.textContent ?? null])).toEqual([
      ["Waiting for review", "1"],
      ["Returned subtasks", "0"],
      ["Ready to accept", "1"],
      ["Open questions", "1"],
      ["Agent tasks awaiting you", "1"],
    ]);
    expect(within(waiting).getAllByRole("link").map((link) => link.getAttribute("href"))).toEqual(["#/harbor-sim/board", "#/harbor-sim/board", "#/harbor-sim/board", "#/harbor-sim/inbox", "#/harbor-sim/tasks"]);
    expect(within(screen.getByRole("region", { name: "Gaps in the flow" })).getByRole("link", { name: "Open Flow" }).getAttribute("href")).toBe("#/harbor-sim/flow");
  });

  it("the harbourmaster's side table: a row per department in served order, then All flows, each flow once", async () => {
    await loadedHome("normal", "#/harbor-sim", "harbourmaster");
    const rows = rowsOf("Tasks by status, per department");
    expect(rows.map((row) => row[0])).toEqual(["Department", "Galley", "Foundry", "Harbourmaster", "All flows"]);
    expect(rows[1]?.slice(1, 6)).toEqual(["3", "3", "1", "0", "4"]);
    // The lead's every flow is All flows.
    expect(rows[3]?.slice(1)).toEqual(rows[4]?.slice(1));
  });

  it("local-only: the hub's line, no digit in the agent task row (DO4)", async () => {
    await loadedHome("local-only");
    const row = Array.from(screen.getByRole("region", { name: "Waiting on this department" }).querySelectorAll(".property-row")).at(-1);
    expect(row?.querySelector("dd")?.textContent).toBe("Local only: agent tasks live on each terminalOpen Agent tasks");
  });

  it("slow: busy with a skeleton until the counts answer", async () => {
    window.history.replaceState(null, "", "/?scenario=slow#/harbor-sim");
    render(<App client={new MockClient("slow", { delayMs: 50 })} scenario="slow" />);
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    await waitFor(() => {
      expect(document.querySelector('.department-overview[aria-busy="true"] [role="status"]')).not.toBeNull();
    });
    await settled();
    expect(blocks()).toEqual(["Work", "Waiting on this department", "Gaps in the flow"]);
  });

  it("empty: no flow, so the empty state and its next steps", async () => {
    await loadedHome("empty");
    expect(screen.getByRole("heading", { level: 2, name: "No task flow yet" })).toBeTruthy();
    expect(blocks()[0]).toBe("No task flow yet");
  });

  it("error: the counts in the daemon's words with their own Retry", async () => {
    renderMock("error", "#/harbor-sim");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    const title = await screen.findByText("The task counts could not be read");
    const panel = title.closest<HTMLElement>(".error-panel");
    expect(panel?.querySelector(".error-message")?.textContent).toMatch(/^spec index unavailable/);
    expect(within(panel as HTMLElement).getByRole("button", { name: "Retry" })).toBeTruthy();
  });
});

describe("the palette over the mock (AC-13)", () => {
  function combobox(): HTMLInputElement {
    return screen.getByRole("combobox", { name: "Jump to a section, agent task, proposal, node or project" });
  }

  async function open() {
    fireEvent.keyDown(document.body, { key: "k", ctrlKey: true });
    await screen.findByRole("dialog", { name: "Jump to" });
  }

  it("jumps to T-0108 on Enter", async () => {
    await loadedHome();
    await open();
    // The palette reads the agent tasks on opening: the Overview reads none (DO3).
    fireEvent.change(combobox(), { target: { value: "T-010" } });
    await waitFor(() => {
      expect(screen.getAllByRole("option").some((option) => option.textContent.startsWith("T-0108"))).toBe(true);
    });
    fireEvent.change(combobox(), { target: { value: "T-0108" } });
    fireEvent.keyDown(combobox(), { key: "Enter" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/harbor-sim/tasks/T-0108");
    });
  });

  it("opens the REF of the spec's Data as a document over the screen, with no search (DM-16 of doc-modal)", async () => {
    const { spies } = await loadedHome();
    const before = calls(spies);
    await open();
    fireEvent.change(combobox(), { target: { value: "MEC-TIDES#RULE-TIDE-WINDOW" } });
    fireEvent.click(screen.getByRole("option", { name: "Open document 'MEC-TIDES#RULE-TIDE-WINDOW'" }));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/harbor-sim?doc=MEC-TIDES%23RULE-TIDE-WINDOW");
    });
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(calls(spies).search).toBeUndefined();
    expect(before.search).toBeUndefined();
  });

  it("large: T-0 lists ten of harbor-sim's 329 tasks", async () => {
    await loadedHome("large");
    await open();
    fireEvent.change(combobox(), { target: { value: "T-0" } });
    const group = screen.getAllByRole("group").find((element) => element.textContent.startsWith("Agent tasks"));
    expect(document.getElementById(group?.getAttribute("aria-labelledby") ?? "")?.textContent).toBe("Agent tasks, 10 of 329");
  });
});

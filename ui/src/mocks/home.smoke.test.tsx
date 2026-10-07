import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The home and the palette over the real mock, as `pnpm dev` shows them (docs/features/ui-home.md
// "Data", AC-02, AC-03, AC-04, AC-06, AC-13; "Owner's manual check" for `large`).

const METHODS = ["getProjects", "getInbox", "getTree", "getNode", "search", "getBundle", "getGraph", "getTasks", "getTask", "stageDecision", "unstageDecision"] as const;

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

function region(name: "Tasks" | "Inbox"): HTMLElement {
  return screen.getByRole("region", { name });
}

async function loadedHome(scenario: Scenario = "normal", hash = "#/harbor-sim") {
  const rendered = renderMock(scenario, hash);
  await screen.findByRole("heading", { level: 1, name: "Overview" });
  await waitFor(() => {
    expect(document.querySelectorAll('.home-region[aria-busy="false"]')).toHaveLength(2);
  });
  return rendered;
}

function tally(element: HTMLElement, heading: string): string[] {
  const block = within(element).getByRole("heading", { level: 3, name: heading }).parentElement;
  return Array.from(block?.querySelectorAll(".home-tally > li") ?? [], (item) => item.textContent);
}

function sum(lines: string[]): number {
  return lines.reduce((total, line) => total + Number(/(\d+)$/.exec(line)?.[1] ?? Number.NaN), 0);
}

describe("the home over the mock", () => {
  it("#/ lands on harbor-sim's home; #/harbor-sim reads the projects, the inbox and the tasks once each (AC-02)", async () => {
    const { spies } = renderMock("normal", "#/");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    expect(window.location.hash).toBe("#/harbor-sim");
    await waitFor(() => {
      expect(document.querySelectorAll('.home-region[aria-busy="false"]')).toHaveLength(2);
    });
    expect(calls(spies)).toEqual({ getProjects: 1, getInbox: 1, getTasks: 1 });
  });

  it("shows harbor-sim's Tasks region as the spec's Data says (AC-03)", async () => {
    await loadedHome();
    const tasks = region("Tasks");
    const rows = Array.from(tasks.querySelectorAll(".home-block-waiting .home-row"));
    expect(rows.map((row) => [row.querySelector("a")?.getAttribute("href"), row.querySelector(".badge")?.textContent])).toEqual([
      ["#/harbor-sim/tasks/T-0107", "State: Plan review"],
      ["#/harbor-sim/tasks/T-0108", "State: Ready"],
    ]);
    expect(rows.map((row) => row.textContent.includes("Spec changed"))).toEqual([false, true]);
    expect(tally(tasks, "Other open tasks")).toEqual(["Draft: 1", "Changes requested: 1", "Ready: 2", "In progress: 1"]);
    const notes = Array.from(within(tasks).getByRole("list", { name: "Notes from the daemon on the task list" }).querySelectorAll("li"), (item) => item.textContent);
    expect(notes).toContain("T-0106: unreadable row (bad JSON in criteria); skipped");
  });

  it.each(["harbor-sim", "ledger-api"])("counts %s's queue as the Inbox lists it (AC-04)", async (slug) => {
    renderMock("normal", `#/${slug}/inbox`);
    const list = await screen.findByRole("listbox", { name: "Proposals by severity, then age" });
    const queue = within(list)
      .getAllByRole("option")
      .map((option) => option.dataset.proposal ?? "");
    cleanup();
    await loadedHome("normal", `#/${slug}`);
    const inbox = region("Inbox");
    expect(inbox.querySelector(".home-total")?.textContent).toBe(`${String(queue.length)} ${queue.length === 1 ? "proposal" : "proposals"} in the queue`);
    const first = Array.from(inbox.querySelectorAll(".home-row a"), (link) => /^(\S+):/.exec(link.textContent)?.[1]);
    expect(first).toEqual(queue.slice(0, 5));
    expect(sum(tally(inbox, "By status"))).toBe(queue.length);
    expect(sum(tally(inbox, "By kind"))).toBe(queue.length);
  });

  it("slow: both regions busy with a skeleton until their reads answer (AC-06)", async () => {
    window.history.replaceState(null, "", "/?scenario=slow#/harbor-sim");
    render(<App client={new MockClient("slow", { delayMs: 50 })} scenario="slow" />);
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    for (const name of ["Tasks", "Inbox"] as const) {
      expect(region(name).getAttribute("aria-busy")).toBe("true");
      expect(within(region(name)).getByRole("status").getAttribute("aria-busy")).toBe("true");
    }
    await waitFor(() => {
      expect(document.querySelectorAll('.home-region[aria-busy="false"]')).toHaveLength(2);
    });
  });

  it("empty: both regions say so with their next step (AC-06)", async () => {
    await loadedHome("empty");
    expect(within(region("Tasks")).getByText("No tasks yet.")).toBeTruthy();
    expect(within(region("Inbox")).getByText("The queue is clear: no proposal waits for your decision.")).toBeTruthy();
  });

  it("error: each region in the daemon's words with its own Retry (AC-06)", async () => {
    renderMock("error", "#/harbor-sim");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    // The projects fail too: the route's project cannot be checked, so its home is shown.
    for (const name of ["Tasks", "Inbox"] as const) {
      expect(await within(region(name)).findByText(/^spec index unavailable/)).toBeTruthy();
      expect(within(region(name)).getByRole("button", { name: "Retry" })).toBeTruthy();
    }
  });
});

describe("the palette over the mock (AC-13)", () => {
  function combobox(): HTMLInputElement {
    return screen.getByRole("combobox", { name: "Jump to a section, task, proposal, node or project" });
  }

  async function open() {
    fireEvent.keyDown(document.body, { key: "k", ctrlKey: true });
    await screen.findByRole("dialog", { name: "Jump to" });
  }

  it("jumps to T-0108 on Enter", async () => {
    await loadedHome();
    await open();
    fireEvent.change(combobox(), { target: { value: "T-0108" } });
    fireEvent.keyDown(combobox(), { key: "Enter" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/harbor-sim/tasks/T-0108");
    });
  });

  it("opens the REF of the spec's Data in the tree, with no call", async () => {
    const { spies } = await loadedHome();
    const before = calls(spies);
    await open();
    fireEvent.change(combobox(), { target: { value: "MEC-TIDES#RULE-TIDE-WINDOW" } });
    fireEvent.click(screen.getByRole("option", { name: "Open 'MEC-TIDES#RULE-TIDE-WINDOW' in the spec tree" }));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/harbor-sim/tree/MEC-TIDES%23RULE-TIDE-WINDOW");
    });
    expect(calls(spies).search).toBeUndefined();
    expect(before.search).toBeUndefined();
  });

  it("large: T-0 lists ten of harbor-sim's 329 tasks", async () => {
    await loadedHome("large");
    await open();
    fireEvent.change(combobox(), { target: { value: "T-0" } });
    const group = screen.getAllByRole("group").find((element) => element.textContent.startsWith("Tasks"));
    expect(document.getElementById(group?.getAttribute("aria-labelledby") ?? "")?.textContent).toBe("Tasks, 10 of 329");
  });
});

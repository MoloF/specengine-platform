import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { CUT_NOTICE, TOTAL_CUT_NOTICE } from "../tasks/SpecChangesPanel";
import { argsOf } from "../test/stubClient";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The Tasks screen over the real mock, as `pnpm dev` shows it (docs/features/ui-tasks.md): AC-01
// (what a route reads), AC-04 (harbor-sim's groups), AC-06 (the four staleness displays, the spec
// changes as sent), AC-08 (proposals split by task_id; a decision read again; the Inbox card links
// the task its review document names, again since ui-live-tasks AC-12), AC-11 (slow, empty, error,
// an unknown T), AC-14 (large; T-0200's diffs past the package's total).

function renderMock(scenario: Scenario, hash: string, delayMs = 0) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs, now: () => Date.parse("2026-10-06T12:00:00Z") });
  const getTasks = vi.spyOn(client, "getTasks");
  const getTask = vi.spyOn(client, "getTask");
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, getTasks, getTask };
}

function listbox(): HTMLElement {
  return screen.getByRole("listbox", { name: "Tasks, what waits for you first" });
}

function rowIds(): string[] {
  return within(listbox())
    .getAllByRole("option")
    .map((item) => item.dataset.task ?? "");
}

function groups(): [string, string[]][] {
  return within(listbox())
    .getAllByRole("group")
    .map((group) => [
      group.querySelector(".task-group-title > span:not(.task-group-count)")?.textContent ?? "",
      within(group)
        .getAllByRole("option")
        .map((item) => item.dataset.task ?? ""),
    ]);
}

function chip(name: RegExp): HTMLElement {
  return within(screen.getByRole("group", { name: "Filter the tasks" })).getByRole("button", { name });
}

async function openTask(id: string, scenario: Scenario = "normal") {
  const spies = renderMock(scenario, `#/harbor-sim/tasks/${id}`);
  await screen.findByRole("tab", { name: "Overview", selected: true });
  return spies;
}

/** The staleness slot of the open task: its badge's text, or the words standing in for one. */
function staleness(): string {
  const badges = document.querySelector(".task-badges");
  const badge = Array.from(badges?.querySelectorAll(".badge") ?? []).find((element) => element.textContent.startsWith("Spec since approval"));
  return badge?.textContent ?? badges?.querySelector(".frozen-note")?.textContent ?? "";
}

/** Browser Back; jsdom traverses history in a task of its own, so wait for the hashchange. */
async function goTo(hash: string) {
  const changed = new Promise<void>((resolve) => {
    window.addEventListener(
      "hashchange",
      () => {
        resolve();
      },
      { once: true },
    );
  });
  await act(async () => {
    window.location.hash = hash;
    await changed;
  });
}

describe("what a route reads over the mock (AC-01)", () => {
  it("#/harbor-sim/tasks: one list read, no package; .../T-0107: one of each", async () => {
    const list = renderMock("normal", "#/harbor-sim/tasks");
    await screen.findByRole("listbox");
    expect([list.getTasks.mock.calls.length, list.getTask.mock.calls.length]).toEqual([1, 0]);
    fireEvent.click(chip(/^Closed/));
    fireEvent.change(screen.getByLabelText("Filter by ID, title or target"), { target: { value: "berth" } });
    expect([list.getTasks.mock.calls.length, list.getTask.mock.calls.length]).toEqual([1, 0]);
  });

  it("#/harbor-sim/tasks/T-0107 reads the list and that package once each", async () => {
    const { getTasks, getTask } = await openTask("T-0107");
    expect([getTasks.mock.calls.length, argsOf(getTask)]).toEqual([1, [["harbor-sim", "T-0107"]]]);
  });
});

describe("harbor-sim's list (AC-04)", () => {
  it("waits on T-0107 and T-0108 first, then the states; T-0104 and T-0105 only with Closed", async () => {
    renderMock("normal", "#/harbor-sim/tasks");
    await screen.findByRole("listbox");
    expect(groups()).toEqual([
      ["Waiting for you", ["T-0107", "T-0108"]],
      ["Draft", ["T-0113"]],
      ["Changes requested", ["T-0110"]],
      ["Ready", ["T-0111", "T-0112"]],
      ["In progress", ["T-0109"]],
    ]);
    expect(screen.getByRole("list", { name: "Notes from the daemon on the list" }).textContent).toMatch(/^T-0106: .*; skipped/);
    fireEvent.click(chip(/^Closed/));
    expect(groups().at(-1)).toEqual(["Closed", ["T-0104", "T-0105"]]);
  });
});

describe("staleness and spec changes (AC-06)", () => {
  it.each([
    ["T-0108", "Spec since approval: Spec changed since approval"],
    ["T-0112", "Spec since approval: Unchanged since approval"],
    ["T-0111", "Spec since approval: Unknown"],
    ["T-0113", "The spec is frozen at approval."],
  ])("%s: %s", async (id, shown) => {
    await openTask(id);
    expect(staleness()).toBe(shown);
  });

  it("T-0111's unknown comes with the daemon's note", async () => {
    await openTask("T-0111");
    expect(screen.getByRole("list", { name: "Notes from the daemon on this task" }).textContent).toBe(
      "snapshot place /work/harbor-sim/T-0111 is gone (worktree removed); stale unknown",
    );
    fireEvent.click(screen.getByRole("tab", { name: "Spec changes" }));
    expect(screen.getByText("Unknown: the spec could not be compared.")).toBeTruthy();
    expect(document.querySelectorAll(".snapshot-diff")).toHaveLength(0);
  });

  it("T-0108's changes: exactly its diff entries, in order, as sent; a removal for the gone node", async () => {
    const { client } = await openTask("T-0108");
    fireEvent.click(screen.getByRole("tab", { name: /^Spec changes/ }));
    const answer = await client.getTask("harbor-sim", "T-0108");
    const entries = "snapshot_diff" in answer ? (answer.snapshot_diff ?? []) : [];
    const shown = Array.from(document.querySelectorAll<HTMLElement>(".snapshot-diff"));
    expect(shown.map((item) => item.dataset.node)).toEqual(["MEC-NIGHT-PASSAGE", "RULE-NIGHT-LIGHTS"]);
    shown.forEach((item, index) => {
      // The hunks as sent, line by line; the spoken "Added: " and "Removed: " are for screen readers only.
      const lines = Array.from(item.querySelectorAll(".diff-line")).map((line) => {
        const copy = line.cloneNode(true) as HTMLElement;
        copy.querySelectorAll(".sr-only").forEach((spoken) => {
          spoken.remove();
        });
        return copy.textContent;
      });
      expect(lines.join("")).toBe(entries[index]?.diff);
    });
    expect(document.querySelector(".cut-note")).toBeNull();
    expect(screen.getByRole("figure", { name: "Changes to RULE-NIGHT-LIGHTS since approval" })).toBeTruthy();
    // The gone node is named, not linked: the tree has nothing to open (`docs/canon/task-package.md` "Package": a gone target).
    const heads = shown.map((item) => [item.querySelector(".snapshot-diff-head a")?.textContent ?? null, item.querySelector(".plain-tag")?.textContent ?? null]);
    expect(heads).toEqual([
      ["MEC-NIGHT-PASSAGE", null],
      [null, "Removed since approval"],
    ]);
  });

  it("T-0109's one diff is cut, and says so", async () => {
    await openTask("T-0109");
    fireEvent.click(screen.getByRole("tab", { name: /^Spec changes/ }));
    expect(document.querySelectorAll(".snapshot-diff")).toHaveLength(1);
    expect(screen.getByText("Diff cut by SpecEngine at 8 192 bytes; the file in the worktree holds the rest.")).toBeTruthy();
  });

  it("T-0112's spec is unchanged: no diff", async () => {
    await openTask("T-0112");
    fireEvent.click(screen.getByRole("tab", { name: /^Spec changes/ }));
    expect(screen.getByText("No change since approval.")).toBeTruthy();
  });
});

describe("proposals and the Inbox (AC-08)", () => {
  function listed(title: string): string[] {
    const part = screen.getByRole("heading", { level: 2, name: new RegExp(`^${title}`) }).closest("section");
    return Array.from(part?.querySelectorAll(".task-proposal") ?? []).map((item) => item.querySelector("a")?.textContent ?? "");
  }

  it("splits T-0107's proposals by task_id alone, each linked to its Inbox card", async () => {
    await openTask("T-0107");
    fireEvent.click(screen.getByRole("tab", { name: /^Proposals/ }));
    expect(listed("Raised by this task")).toEqual(["PR-0041", "PR-0042"]);
    expect(listed("On its nodes")).toEqual(["PR-0044"]);
    expect(screen.getByRole("link", { name: "PR-0041" }).getAttribute("href")).toBe("#/harbor-sim/inbox/PR-0041");
  });

  it("links PR-0041's Inbox card to T-0107; after PR-0041 is rejected, T-0107 is read again without it", async () => {
    const { getTask } = renderMock("normal", "#/harbor-sim/tasks/T-0107");
    await screen.findByRole("tab", { name: "Overview", selected: true });
    expect(within(document.querySelector(".task-assumptions") ?? document.body).getByText("PR-0041")).toBeTruthy();
    expect(getTask).toHaveBeenCalledTimes(1);

    await goTo("#/harbor-sim/inbox/PR-0041");
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    const task = within(card).getByRole("link", { name: "T-0107" });
    expect(task.getAttribute("href")).toBe("#/harbor-sim/tasks/T-0107");
    expect(task.closest(".fact")?.querySelector("dt")?.textContent).toBe("Task");
    fireEvent.click(within(card).getByRole("button", { name: "Reject" }));
    const dialog = await screen.findByRole("dialog", { name: "Reject PR-0041" });
    fireEvent.change(within(dialog).getByLabelText("Reason (required)"), { target: { value: "The window stays." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Reject" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    await goTo("#/harbor-sim/tasks/T-0107");
    await waitFor(() => {
      expect(getTask).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(document.querySelector(".task-assumptions")?.textContent).not.toContain("PR-0041");
    });
    fireEvent.click(screen.getByRole("tab", { name: /^Proposals/ }));
    expect(listed("Raised by this task")).toEqual(["PR-0042"]);
  });
});

describe("states over the mock (AC-11)", () => {
  it("slow: both regions show a busy skeleton", async () => {
    renderMock("slow", "#/harbor-sim/tasks/T-0107", 40);
    expect((await screen.findByLabelText("Loading the tasks of harbor-sim")).getAttribute("aria-busy")).toBe("true");
    expect((await screen.findByLabelText("Loading T-0107")).getAttribute("aria-busy")).toBe("true");
    expect(await screen.findByRole("tab", { name: "Overview", selected: true }, { timeout: 2000 })).toBeTruthy();
  });

  it("empty: no tasks yet, for both projects", async () => {
    renderMock("empty", "#/ledger-api/tasks");
    expect((await screen.findByText(/Create one:/)).textContent).toBe("No tasks yet. Create one: spec task new --nodes REF...");
  });

  it("error: the mock's message verbatim and a Retry", async () => {
    const { getTasks } = renderMock("error", "#/harbor-sim/tasks");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("spec index unavailable: the database is locked by another process (mock scenario: error)");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(getTasks).toHaveBeenCalledTimes(2);
    });
  });

  it("an unknown T: the mock's reason and Back to tasks", async () => {
    renderMock("normal", "#/harbor-sim/tasks/T-0999");
    expect(await screen.findByText("no task T-0999 in this repository")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Back to tasks" }).getAttribute("href")).toBe("#/harbor-sim/tasks");
  });
});

describe("the large scenario (AC-14)", () => {
  it("lists all 329 tasks from one read with Closed pressed, and opens T-0200 at its caps", async () => {
    const { getTasks } = renderMock("large", "#/harbor-sim/tasks/T-0200");
    await screen.findByRole("listbox");
    fireEvent.click(chip(/^Closed/));
    expect(rowIds()).toHaveLength(329);
    expect(getTasks).toHaveBeenCalledTimes(1);
    await screen.findByRole("tab", { name: "Overview", selected: true });
    fireEvent.click(screen.getByRole("tab", { name: "Package" }));
    const text = document.querySelector(".task-json")?.textContent ?? "";
    expect((JSON.parse(text) as { targets: unknown[] }).targets).toHaveLength(64);
  });

  it("shows T-0200's 32 diffs as sent and its 96 left out past the total as quiet lines, the daemon's note above", async () => {
    await openTask("T-0200", "large");
    expect(screen.getByRole("list", { name: "Notes from the daemon on this task" }).textContent).toBe(
      "snapshot_diff: 96 diff(s) past 262144 B left out",
    );
    fireEvent.click(screen.getByRole("tab", { name: /^Spec changes/ }));
    const panel = screen.getByRole("tabpanel");
    const entries = Array.from(panel.querySelectorAll<HTMLElement>(".snapshot-diff"));
    const notes = entries.map((entry) => entry.querySelector(".cut-note")?.textContent);
    expect([entries.length, within(panel).getAllByRole("figure").length]).toEqual([128, 32]);
    expect(notes.slice(0, 32).every((note) => note === CUT_NOTICE)).toBe(true);
    expect(notes.slice(32).every((note) => note === TOTAL_CUT_NOTICE)).toBe(true);
    expect(entries.slice(32).every((entry) => entry.querySelector(".diff") === null)).toBe(true);
    expect(within(panel).queryByText("No section diff attached.")).toBeNull();
  });

  it("shows a generated task in progress claimed, its run 1 running", async () => {
    renderMock("large", "#/harbor-sim/tasks/T-0207");
    await screen.findByRole("tab", { name: "Overview", selected: true });
    expect(document.querySelector(".task-badges .badge")?.textContent).toBe("State: In progress");
    fireEvent.click(screen.getByRole("tab", { name: /^Runs/ }));
    expect(screen.queryByText("Not claimed")).toBeNull();
    expect(Array.from(document.querySelectorAll(".pairs dd.mono")).map((item) => item.textContent)).toContain("/work/harbor-sim/T-0207");
    expect(document.querySelector(".running-tag")?.textContent).toBe("Running");
  });
});

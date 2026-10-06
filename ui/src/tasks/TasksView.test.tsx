import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { TaskPackage } from "../api/types";
import { App } from "../app/App";
import { sectionHash } from "../app/routes";
import { aTaskEntry, aTaskPackage, aTaskProposal, aTaskRun } from "../test/builders";
import { renderApp } from "../test/render";
import { SOME_TASKS, taskClient } from "../test/taskStub";

// docs/features/ui-tasks.md on a stub client: AC-01 (what a route reads), AC-03 (an unknown state
// in the DOM), AC-05 (filters), AC-07 (verbatim text, links), AC-09 (commands and Copy), AC-10
// (Package), AC-11 (states), AC-12 (keyboard), AC-13 (hostile text); a node its diff removes
// (task-package G4) as text; opening a task in the stacked layout.

/** Markup an author pasted; shown as text, never parsed (escaped so no source line spells a dialog call). */
const HOSTILE = "<img src=x onerror=\u0061lert(1)>";

function listbox(): HTMLElement {
  return screen.getByRole("listbox", { name: "Tasks, what waits for you first" });
}

function rows(): HTMLElement[] {
  return within(listbox()).getAllByRole("option");
}

function rowIds(): string[] {
  return rows().map((item) => item.dataset.task ?? "");
}

function row(id: string): HTMLElement {
  const found = rows().find((item) => item.dataset.task === id);
  if (found === undefined) {
    throw new Error(`no row ${id}`);
  }
  return found;
}

function groupTitles(): string[] {
  return within(listbox())
    .getAllByRole("group")
    .map((group) => group.querySelector(".task-group-title > span:not(.task-group-count)")?.textContent ?? "");
}

function chip(name: RegExp): HTMLElement {
  return within(screen.getByRole("group", { name: "Filter the tasks" })).getByRole("button", { name });
}

function heading(): HTMLElement {
  return screen.getByRole("heading", { level: 1 });
}

function commands(): HTMLElement {
  return screen.getByRole("region", { name: "Owner commands" });
}

function politeRegion(): HTMLElement {
  const region = document.querySelector<HTMLElement>('[aria-live="polite"]');
  if (region === null) {
    throw new Error("no polite live region");
  }
  return region;
}

async function openList(client = taskClient(SOME_TASKS), hash = "#/alpha/tasks") {
  renderApp(client, hash);
  await screen.findByRole("listbox", { name: "Tasks, what waits for you first" });
  return client;
}

async function openTask(pkg: TaskPackage, entries = [aTaskEntry({ id: pkg.id, status: pkg.status, title: pkg.title })]) {
  const client = taskClient(entries, [pkg]);
  renderApp(client, `#/alpha/tasks/${encodeURIComponent(pkg.id)}`);
  await screen.findByRole("tab", { name: /^(Overview|Package)/, selected: true });
  return client;
}

function showTab(name: RegExp) {
  fireEvent.click(screen.getByRole("tab", { name }));
}

async function focused(element: HTMLElement) {
  await waitFor(() => {
    expect(document.activeElement).toBe(element);
  });
}

/** A route change from outside the list (an address typed, Back); waits for its hashchange. */
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

/** Runs the tasks jsdom queued: a clicked link's navigation, a hashchange. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");

function stubClipboard(writeText: (text: string) => Promise<void>) {
  const spy = vi.fn(writeText);
  Object.defineProperty(navigator, "clipboard", { value: { writeText: spy }, configurable: true });
  return spy;
}

afterEach(() => {
  if (clipboard === undefined) {
    Reflect.deleteProperty(navigator, "clipboard");
  } else {
    Object.defineProperty(navigator, "clipboard", clipboard);
  }
  Reflect.deleteProperty(window, "matchMedia");
});

/**
 * Gives jsdom (no `matchMedia`, no layout) the answer a browser gives for the view's breakpoint:
 * `stacked` is a narrow window or 200 % zoom, the task under the list.
 */
function stubLayout(stacked: boolean) {
  const query = vi.fn((media: string) => ({ media, matches: stacked && media === "(max-width: 48em)" }) as unknown as MediaQueryList);
  Object.defineProperty(window, "matchMedia", { value: query, configurable: true, writable: true });
  return query;
}

/** The elements `scrollIntoView` was called on, with its options, in order. */
function scrollSpy() {
  const spy = vi.spyOn(Element.prototype, "scrollIntoView");
  return () => spy.mock.contexts.map((element, index) => [element, spy.mock.calls[index]?.[0]]);
}

describe("what a route reads (AC-01)", () => {
  it("reads the list once and no package for #/alpha/tasks; nothing is opened by itself", async () => {
    const client = await openList();
    expect(client.getTasks).toHaveBeenCalledTimes(1);
    expect(client.getTasks).toHaveBeenCalledWith("alpha");
    expect(client.getTask).not.toHaveBeenCalled();
    expect(heading().textContent).toBe("Tasks");
    expect(rows().filter((item) => item.getAttribute("aria-selected") === "true")).toEqual([]);
  });

  it("reads the list and one package for #/alpha/tasks/T-0002", async () => {
    const client = await openList(taskClient(SOME_TASKS), "#/alpha/tasks/T-0002");
    await screen.findByRole("tab", { name: "Overview" });
    expect(client.getTasks).toHaveBeenCalledTimes(1);
    expect(client.getTask).toHaveBeenCalledTimes(1);
    expect(client.getTask).toHaveBeenCalledWith("alpha", "T-0002");
    expect(row("T-0002").getAttribute("aria-selected")).toBe("true");
  });
});

describe("the list (AC-03, AC-04)", () => {
  it("groups what waits for you first; Closed hidden until its chip; the unknown state last, raw", async () => {
    await openList();
    expect(groupTitles()).toEqual(["Waiting for you", "Draft", "Changes requested", "Ready", "In progress", "Other states"]);
    expect(rowIds()).toEqual(["T-0002", "T-0003", "T-0005", "T-0007", "T-0004", "T-0006", "T-0009"]);
    const odd = row("T-0009").querySelector<HTMLElement>(".badge");
    expect(odd?.dataset.tone).toBe("task-unknown");
    expect(odd?.textContent).toBe("State: triage");
    expect(odd?.querySelector("svg")).not.toBeNull();
    fireEvent.click(chip(/^Closed/));
    expect(groupTitles()).toEqual(["Waiting for you", "Draft", "Changes requested", "Ready", "In progress", "Closed", "Other states"]);
    expect(rowIds().slice(-3)).toEqual(["T-0001", "T-0008", "T-0009"]);
  });

  it("shows a row's ID, state, title or Untitled, three targets then +k, Spec changed only when stale is true", async () => {
    await openList();
    expect(row("T-0002").querySelector(".task-row-targets")?.textContent).toBe("Targets: R-1, R-2, R-3 +2");
    expect(row("T-0005").querySelector(".task-row-title")?.textContent).toBe("Untitled");
    expect(row("T-0003").textContent).toContain("Spec changed");
    expect(row("T-0006").textContent).toContain("Spec changed");
    expect(row("T-0004").textContent).not.toContain("Spec changed");
    expect(row("T-0002").querySelector(".badge")?.textContent).toBe("State: Plan review");
    expect(row("T-0002").querySelector("time")?.getAttribute("datetime")).toBe("2026-10-01T10:00:00Z");
  });

  it("puts the daemon's notes above, verbatim, and counts what waits", async () => {
    const note = "T-0010: unreadable row (bad JSON in criteria); skipped";
    await openList(taskClient(SOME_TASKS, [], [note]));
    expect(screen.getByRole("list", { name: "Notes from the daemon on the list" }).textContent).toBe(note);
    expect(screen.getByText(/waiting for you, 9 tasks in all/).textContent).toBe("2 waiting for you, 9 tasks in all");
  });
});

describe("filters (AC-05)", () => {
  it("press and release chips with counts over the whole answer, and read nothing again", async () => {
    const client = await openList();
    expect(chip(/^Ready/).textContent).toContain("2");
    expect(chip(/^Closed/).getAttribute("aria-pressed")).toBe("false");
    expect(chip(/^Draft/).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(chip(/^Ready/));
    expect(chip(/^Ready/).getAttribute("aria-pressed")).toBe("false");
    expect(rowIds()).not.toContain("T-0004");
    expect(rowIds()).not.toContain("T-0003");
    fireEvent.click(chip(/^Spec changed/));
    expect(rowIds()).toEqual(["T-0006"]);
    fireEvent.change(screen.getByLabelText("Filter by ID, title or target"), { target: { value: "under" } });
    expect(rowIds()).toEqual(["T-0006"]);
    expect(chip(/^Ready/).textContent).toContain("2");
    expect(client.getTasks).toHaveBeenCalledTimes(1);
    expect(client.getTask).not.toHaveBeenCalled();
  });

  it("finds a precomposed title with a decomposed query", async () => {
    const title = "Nord \u00e9cluse";
    await openList(taskClient([aTaskEntry({ id: "T-0001", status: "draft", title }), aTaskEntry({ id: "T-0002", status: "draft" })]));
    fireEvent.change(screen.getByLabelText("Filter by ID, title or target"), { target: { value: "E\u0301CLUSE" } });
    expect(rowIds()).toEqual(["T-0001"]);
  });

  it("says when nothing is left and Clear filters restores the defaults, focus on the list", async () => {
    await openList();
    fireEvent.click(chip(/^Closed/));
    fireEvent.change(screen.getByLabelText("Filter by ID, title or target"), { target: { value: "nothing like it" } });
    expect(screen.getByText("No task matches the filters.")).toBeTruthy();
    const clear = screen.getByRole("button", { name: "Clear filters" });
    clear.focus();
    fireEvent.click(clear);
    expect(rowIds()).toEqual(["T-0002", "T-0003", "T-0005", "T-0007", "T-0004", "T-0006", "T-0009"]);
    expect(chip(/^Closed/).getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByLabelText("Filter by ID, title or target")).toHaveProperty("value", "");
    await focused(row("T-0002"));
  });
});

describe("the task's text and links (AC-07, AC-13)", () => {
  const plan = "\n\n  1. First step   \n\n\tindented\n- not a list item **not bold**  ";
  const pkg = aTaskPackage({
    id: "T-0042",
    status: "review",
    title: `${HOSTILE} title`,
    goal: "  Goal with spaces kept  \nand a second line ",
    plan,
    targets: [
      { id: "R-1", path: "docs/spec/r.md", kind: "widget", title: "Evil title" },
      { id: null, path: "docs/spec/idless.md", kind: "widget", title: null },
      { id: "R-GONE", path: "docs/spec/gone.md", kind: null, title: null },
    ],
    criteria: [
      { ref: "R-1#AC-01", text: "  Criterion text\n  kept  " },
      { ref: null, text: "Free criterion" },
      { ref: "R-9", text: null },
    ],
    affected_nodes: ["R-2"],
    assumptions: [{ proposal: "PR-0007", text: "Working answer kept" }],
    owner_notes: [{ at: "2026-10-02T09:00:00Z", note: `${HOSTILE}\n  second line ` }],
    open_proposals: [aTaskProposal({ id: "PR-0007", task_id: "T-0042", summary: "  Summary kept  " })],
    runs: [aTaskRun({ run: 1, role: HOSTILE, ended_at: "2026-10-02T10:00:00Z", outcome: "completed", summary: " Run summary\n" })],
  });

  it("shows goal, criterion text, note and the plan exactly as sent", async () => {
    await openTask(pkg);
    expect(document.querySelector(".task-goal")?.textContent).toBe(pkg.goal);
    expect(Array.from(document.querySelectorAll(".criterion-text")).map((element) => element.textContent)).toEqual([
      "  Criterion text\n  kept  ",
      "Free criterion",
    ]);
    expect(screen.getByText("Free text")).toBeTruthy();
    expect(screen.getByText("Not found in the compared place")).toBeTruthy();
    expect(document.querySelector(".owner-note-text")?.textContent).toBe(`${HOSTILE}\n  second line `);
    showTab(/^Plan/);
    expect(document.querySelector(".task-plan")?.textContent).toBe(plan);
    showTab(/^Proposals/);
    expect(document.querySelector(".task-proposal-summary")?.textContent).toBe("  Summary kept  ");
    showTab(/^Runs/);
    expect(document.querySelector(".run-summary")?.textContent).toBe(" Run summary\n");
  });

  it("links targets, criteria, affected nodes and assumptions from sectionHash; a gone target is text alone", async () => {
    await openTask(pkg);
    const pane = screen.getByRole("article");
    const hrefs = Array.from(pane.querySelectorAll("a[href]")).map((anchor) => anchor.getAttribute("href"));
    expect(hrefs).toEqual([
      sectionHash("alpha", "tree", "R-1#AC-01"),
      sectionHash("alpha", "tree", "R-9"),
      sectionHash("alpha", "tree", "R-1"),
      sectionHash("alpha", "graph", "R-1"),
      sectionHash("alpha", "tree", "docs/spec/idless.md"),
      sectionHash("alpha", "graph", "docs/spec/idless.md"),
      sectionHash("alpha", "tree", "R-2"),
      sectionHash("alpha", "inbox", "PR-0007"),
    ]);
    const gone = Array.from(pane.querySelectorAll(".task-target")).find((item) => item.textContent.includes("R-GONE"));
    expect(gone?.querySelector("a")).toBeNull();
    expect(hrefs.some((href) => href?.includes("Evil") === true)).toBe(false);
  });

  it("parts the ID from the title with a colon read aloud, a space drawn; Untitled likewise", async () => {
    await openTask(aTaskPackage({ id: "T-0044", status: "draft", title: null }));
    expect(heading().textContent).toBe("T-0044: Untitled");
    const colon = heading().querySelector(".sr-only");
    expect([colon?.textContent, colon?.previousElementSibling?.textContent, colon?.nextElementSibling?.textContent]).toEqual([":", "T-0044", "Untitled"]);
    expect(screen.getByRole("heading", { level: 1, name: "T-0044: Untitled" })).toBe(heading());
  });

  it("shows a node its diff removes as text in Spec changes, every other node a link (task-package G4)", async () => {
    const kept = { id: "R-1", path: "docs/spec/r.md", span_hash: "b3:kept" };
    const gone = { id: "R-GONE", path: "docs/spec/gone.md", span_hash: "b3:gone" };
    await openTask(
      aTaskPackage({
        id: "T-0043",
        status: "ready",
        stale: true,
        spec_snapshot: { at: "2026-10-01T09:30:00Z", place: { worktree: "/work/alpha", root_rel: "", branch: "main", commit: "c0ffee" }, nodes: [kept, gone] },
        snapshot_diff: [
          { ...kept, diff: "--- snapshot docs/spec/r.md\n+++ current docs/spec/r.md\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n", cut: false },
          { ...gone, diff: "--- snapshot docs/spec/gone.md\n+++ current docs/spec/gone.md\n@@ -1,2 +0,0 @@\n-## R-GONE: Gone\n-Text.\n", cut: false },
        ],
      }),
    );
    showTab(/^Spec changes/);
    const panel = screen.getByRole("tabpanel");
    const hrefs = Array.from(panel.querySelectorAll("a[href]")).map((anchor) => anchor.getAttribute("href"));
    expect(hrefs).toEqual([sectionHash("alpha", "tree", "R-1"), sectionHash("alpha", "tree", "R-1")]);
    const frozen = Array.from(panel.querySelectorAll(".snapshot-node-list > li")).find((item) => item.textContent.startsWith("R-GONE"));
    const entry = panel.querySelector('.snapshot-diff[data-node="R-GONE"] .snapshot-diff-head');
    for (const place of [frozen, entry]) {
      expect(place?.querySelector("a")).toBeNull();
      expect(place?.querySelector(".plain-tag")?.textContent).toBe("Removed since approval");
    }
    expect(panel.querySelector('.snapshot-diff[data-node="R-1"] .plain-tag')).toBeNull();
    expect(screen.getByRole("figure", { name: "Changes to R-GONE since approval" })).toBeTruthy();
  });

  it("shows hostile title, note and role as text, never as markup", async () => {
    await openTask(pkg);
    expect(heading().textContent).toBe(`T-0042: ${HOSTILE} title`);
    expect(screen.getByRole("article", { name: `T-0042: ${HOSTILE} title` })).toBeTruthy();
    showTab(/^Runs/);
    expect(document.querySelector(".run-role")?.textContent).toBe(HOSTILE);
    expect(document.querySelector("img")).toBeNull();
  });
});

describe("owner commands and Copy (AC-09)", () => {
  it.each([
    ["review", ["approve", "changes", "cancel", "show"]],
    ["draft", ["approve", "cancel", "show"]],
    ["changes_requested", ["approve", "cancel", "show"]],
    ["ready", ["approve", "cancel", "show"]],
    ["in_progress", ["cancel", "show"]],
    ["done", ["show"]],
    ["cancelled", ["show"]],
    ["accepted", ["show"]],
    ["triage", ["show"]],
  ])("%s: exactly its row of the table, then show", async (status, actions) => {
    await openTask(aTaskPackage({ id: "T-0107", status }));
    const items = within(commands()).getAllByRole("listitem");
    expect(items.map((item) => item.dataset.action)).toEqual(actions);
    expect(within(commands()).getAllByRole("button", { name: /^Copy/ })).toHaveLength(actions.length);
  });

  it("says where owner actions run, words T-0107's commands exactly", async () => {
    await openTask(aTaskPackage({ id: "T-0107", status: "review" }));
    expect(within(commands()).getByText(/^Owner actions run on a terminal/).textContent).toBe(
      "Owner actions run on a terminal, which asks [y/N]; nothing changes here. Approval freezes the spec in the worktree where you run it.",
    );
    expect(Array.from(commands().querySelectorAll("code")).map((code) => code.textContent)).toEqual([
      "spec task approve T-0107",
      'spec task changes T-0107 --note "..."',
      "spec task cancel T-0107",
      "spec task show T-0107",
    ]);
  });

  it("copies the exact command and says Copied, politely", async () => {
    const writeText = stubClipboard(() => Promise.resolve());
    await openTask(aTaskPackage({ id: "T-0107", status: "review" }));
    fireEvent.click(within(commands()).getByRole("button", { name: 'Copy the command: spec task changes T-0107 --note "..."' }));
    await waitFor(() => {
      expect(politeRegion().textContent).toBe("Copied");
    });
    expect(writeText).toHaveBeenCalledWith('spec task changes T-0107 --note "..."');
    expect(within(commands()).getByText("Copied")).toBeTruthy();
  });

  it("says how to copy by hand when the clipboard refuses", async () => {
    stubClipboard(() => Promise.reject(new Error("denied")));
    await openTask(aTaskPackage({ id: "T-0107", status: "review" }));
    fireEvent.click(within(commands()).getByRole("button", { name: "Copy the command: spec task show T-0107" }));
    expect(await within(commands()).findByText("Copy failed: select the command and copy it")).toBeTruthy();
    expect(politeRegion().textContent).toBe("Copy failed: select the command and copy it");
  });

  it("offers no command and no Copy for an ID that is not a task ID", async () => {
    const id = "T-1; rm -rf ~";
    await openTask(aTaskPackage({ id, status: "review" }));
    expect(within(commands()).getByText("No command for this ID")).toBeTruthy();
    expect(within(commands()).queryByRole("button")).toBeNull();
    expect(commands().querySelector("code")).toBeNull();
  });
});

describe("the Package tab and the version guard (AC-10)", () => {
  const pkg = aTaskPackage({
    id: "T-0050",
    status: "ready",
    stale: false,
    spec_snapshot: { at: "2026-10-01T09:30:00Z", place: { worktree: "/w", root_rel: "", branch: "main", commit: "c0ffee" }, nodes: [] },
    snapshot_diff: [],
  });

  it("holds every key of the answer, nulls and empty lists kept, and copies that text", async () => {
    const writeText = stubClipboard(() => Promise.resolve());
    await openTask(pkg);
    showTab(/^Package/);
    const text = document.querySelector(".task-json")?.textContent ?? "";
    expect(JSON.parse(text)).toStrictEqual(pkg);
    expect(Object.keys(JSON.parse(text) as object)).toHaveLength(25);
    expect(text).toContain('"goal": null');
    expect(text).toContain('"bindings": []');
    fireEvent.click(screen.getByRole("button", { name: "Copy package of T-0050 as JSON" }));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(text);
    });
  });

  it("shows only the Package tab, with a notice naming the version, for schema_version 2", async () => {
    const next = { ...aTaskPackage({ id: "T-0051", status: "review", title: "Another shape" }), schema_version: 2 };
    await openTask(next);
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Package"]);
    expect(screen.getByRole("note").textContent).toContain("schema_version 2");
    expect(heading().textContent).toBe("T-0051");
    expect(JSON.parse(document.querySelector(".task-json")?.textContent ?? "")).toStrictEqual(next);
    expect(document.querySelector(".task-badges")).toBeNull();
    expect(Array.from(commands().querySelectorAll("code")).map((code) => code.textContent)).toEqual(["spec task show T-0051"]);
  });
});

describe("states (AC-11)", () => {
  it("slow: a busy skeleton in the list and in the task", async () => {
    const client = taskClient(SOME_TASKS);
    client.getTasks.mockImplementation(() => new Promise(() => undefined));
    client.getTask.mockImplementation(() => new Promise(() => undefined));
    renderApp(client, "#/alpha/tasks/T-0002");
    const list = await screen.findByLabelText("Loading the tasks of alpha");
    expect(list.getAttribute("aria-busy")).toBe("true");
    expect(list.closest(".tasks-region")?.getAttribute("aria-busy")).toBe("true");
    expect((await screen.findByLabelText("Loading T-0002")).getAttribute("aria-busy")).toBe("true");
    expect(screen.getByRole("article").getAttribute("aria-busy")).toBe("true");
    expect(heading().textContent).toBe("T-0002");
  });

  it("never shows the previous task's package while the next one is read", async () => {
    const first = aTaskPackage({ id: "T-0002", status: "review", title: "First package", plan: "First plan" });
    const client = taskClient(SOME_TASKS, [first]);
    client.getTask.mockImplementation((_project, id) => (id === "T-0002" ? Promise.resolve(first) : new Promise(() => undefined)));
    await openList(client, "#/alpha/tasks/T-0002");
    await screen.findByText("First package");
    row("T-0004").focus();
    fireEvent.keyDown(row("T-0004"), { key: "Enter" });
    await waitFor(() => {
      expect(heading().textContent).toBe("T-0004");
    });
    expect(screen.queryByText("First package")).toBeNull();
    expect(screen.queryByText("First plan")).toBeNull();
    expect(screen.getByLabelText("Loading T-0004").getAttribute("aria-busy")).toBe("true");
  });

  it("empty: no tasks yet and the command to create one", async () => {
    renderApp(taskClient([]), "#/alpha/tasks");
    const empty = await screen.findByText(/Create one:/);
    expect(empty.textContent).toBe("No tasks yet. Create one: spec task new --nodes REF...");
    expect(empty.querySelector("code")?.textContent).toBe("spec task new --nodes REF...");
  });

  it("error: the daemon's words, and Retry reads the list once more, focus then on the list", async () => {
    const client = taskClient(SOME_TASKS);
    const message = "spec index unavailable: the database is locked (code 5)";
    client.getTasks.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/tasks");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The tasks could not be loaded${message}`);
    const retry = screen.getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await screen.findByRole("listbox");
    expect(client.getTasks).toHaveBeenCalledTimes(2);
    await focused(row("T-0002"));
  });

  it("an unknown T: the reason verbatim and Back to tasks, focus then on the list", async () => {
    const client = await openList(taskClient(SOME_TASKS), "#/alpha/tasks/T-0999");
    expect(await screen.findByText("no task T-0999 in this repository")).toBeTruthy();
    expect(heading().textContent).toBe("T-0999");
    const back = screen.getByRole("link", { name: "Back to tasks" });
    expect(back.getAttribute("href")).toBe("#/alpha/tasks");
    back.focus();
    fireEvent.click(back);
    await settle();
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/tasks");
    });
    expect(document.activeElement).toBe(row("T-0002"));
    expect(client.getTask).toHaveBeenCalledTimes(1);
  });

  it("Back to tasks opened elsewhere (Cmd-, Ctrl-, Shift- or Alt-click) leaves focus on the link", async () => {
    await openList(taskClient(SOME_TASKS), "#/alpha/tasks/T-0999");
    const back = await screen.findByRole("link", { name: "Back to tasks" });
    for (const modifier of [{ metaKey: true }, { ctrlKey: true }, { shiftKey: true }, { altKey: true }]) {
      back.focus();
      fireEvent.click(back, modifier);
      expect(document.activeElement).toBe(back);
    }
    await settle();
    expect(listbox().contains(document.activeElement)).toBe(false);
  });

  it("a task that fails to render leaves the list working", async () => {
    const broken = { ...aTaskPackage({ id: "T-0002", status: "review" }), targets: null } as unknown as TaskPackage;
    const client = taskClient(SOME_TASKS, [broken]);
    window.history.replaceState(null, "", "/#/alpha/tasks/T-0002");
    render(<App client={client} scenario={null} />, { onCaughtError: () => undefined });
    expect(await screen.findByText(/The task could not be shown/)).toBeTruthy();
    expect(heading().textContent).toBe("T-0002");
    expect(rowIds()).toContain("T-0004");
    fireEvent.click(row("T-0004"));
    expect(await screen.findByRole("tab", { name: "Overview" })).toBeTruthy();
  });

  it("a detail's read failing says so in the daemon's words with a Retry", async () => {
    const client = taskClient(SOME_TASKS);
    client.getTask.mockRejectedValueOnce(new ClientError({ status: 503, message: "bundle budget unreadable" }));
    await openList(client, "#/alpha/tasks/T-0002");
    expect((await screen.findByRole("alert")).textContent).toBe("T-0002 could not be readbundle budget unreadable");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("tab", { name: "Overview" })).toBeTruthy();
    expect(client.getTask).toHaveBeenCalledTimes(2);
  });
});

describe("keyboard (AC-12)", () => {
  it("gives the list one Tab stop and moves it with the arrows, j and k, Home and End; nothing opens", async () => {
    const client = await openList();
    expect(rows().filter((item) => item.tabIndex === 0).map((item) => item.dataset.task)).toEqual(["T-0002"]);
    const before = window.history.length;
    row("T-0002").focus();
    fireEvent.keyDown(row("T-0002"), { key: "ArrowDown" });
    await focused(row("T-0003"));
    fireEvent.keyDown(row("T-0003"), { key: "j" });
    await focused(row("T-0005"));
    fireEvent.keyDown(row("T-0005"), { key: "k" });
    await focused(row("T-0003"));
    fireEvent.keyDown(row("T-0003"), { key: "ArrowUp" });
    await focused(row("T-0002"));
    fireEvent.keyDown(row("T-0002"), { key: "End" });
    await focused(row("T-0009"));
    fireEvent.keyDown(row("T-0009"), { key: "Home" });
    await focused(row("T-0002"));
    expect(rows().filter((item) => item.tabIndex === 0)).toHaveLength(1);
    expect(window.history.length).toBe(before);
    expect(client.getTask).not.toHaveBeenCalled();
  });

  it("opens with Enter: one history entry, the task beside the list, focus kept on the row", async () => {
    const client = await openList();
    const before = window.history.length;
    row("T-0004").focus();
    fireEvent.keyDown(row("T-0004"), { key: "Enter" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/tasks/T-0004");
    });
    expect(window.history.length).toBe(before + 1);
    await screen.findByRole("tab", { name: "Overview" });
    expect(client.getTask).toHaveBeenCalledWith("alpha", "T-0004");
    expect(document.activeElement).toBe(row("T-0004"));
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
  });

  it("stacked (narrow, 200 %): Enter scrolls the opened task's heading into view; focus stays on the row", async () => {
    stubLayout(true);
    const scrolled = scrollSpy();
    await openList();
    row("T-0004").focus();
    fireEvent.keyDown(row("T-0004"), { key: "Enter" });
    await screen.findByRole("tab", { name: "Overview" });
    expect(heading().textContent).toBe("T-0004: Calm");
    expect(scrolled()).toEqual([[heading(), { block: "start" }]]);
    expect(document.activeElement).toBe(row("T-0004"));
    fireEvent.keyDown(row("T-0004"), { key: "Enter" });
    expect(scrolled()).toEqual([
      [heading(), { block: "start" }],
      [heading(), { block: "start" }],
    ]);
    expect(document.activeElement).toBe(row("T-0004"));
  });

  it("stacked: a click scrolls the opened task's heading into view likewise, focus on the row; another route change scrolls nothing", async () => {
    stubLayout(true);
    const scrolled = scrollSpy();
    await openList();
    fireEvent.click(row("T-0007"));
    await waitFor(() => {
      expect(heading().textContent).toBe("T-0007: Sent back");
    });
    expect(scrolled()).toEqual([[heading(), { block: "start" }]]);
    await focused(row("T-0007"));
    fireEvent.click(row("T-0007"));
    expect(scrolled()).toHaveLength(2);
    await goTo("#/alpha/tasks/T-0002");
    await waitFor(() => {
      expect(heading().textContent).toBe("T-0002: Plan to read");
    });
    expect(scrolled()).toHaveLength(2);
  });

  it("side by side, Enter or a click scrolls nothing", async () => {
    const layout = stubLayout(false);
    const scrolled = scrollSpy();
    await openList();
    row("T-0004").focus();
    fireEvent.keyDown(row("T-0004"), { key: "Enter" });
    await screen.findByRole("tab", { name: "Overview" });
    expect(layout).toHaveBeenCalledWith("(max-width: 48em)");
    fireEvent.click(row("T-0007"));
    await waitFor(() => {
      expect(heading().textContent).toBe("T-0007: Sent back");
    });
    expect(scrolled()).toEqual([]);
  });

  it("goes back to the task's row with Esc in the task, to the list's Tab stop when it is filtered out", async () => {
    await openList(taskClient(SOME_TASKS), "#/alpha/tasks/T-0004");
    const tab = await screen.findByRole("tab", { name: "Overview" });
    tab.focus();
    fireEvent.keyDown(tab, { key: "Escape" });
    await focused(row("T-0004"));
    fireEvent.click(chip(/^Ready/));
    const plan = screen.getByRole("tab", { name: "Plan" });
    plan.focus();
    fireEvent.keyDown(plan, { key: "Escape" });
    await focused(row("T-0002"));
  });

  it("ignores keys with Ctrl, Meta or Alt and keys typed in the filter field", async () => {
    await openList();
    row("T-0002").focus();
    for (const modifier of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }]) {
      fireEvent.keyDown(row("T-0002"), { key: "ArrowDown", ...modifier });
      fireEvent.keyDown(row("T-0002"), { key: "Enter", ...modifier });
    }
    const field = screen.getByLabelText("Filter by ID, title or target");
    field.focus();
    for (const key of ["j", "k", "ArrowDown", "Home", "End", "Enter", "?"]) {
      fireEvent.keyDown(field, { key });
    }
    await settle();
    expect(window.location.hash).toBe("#/alpha/tasks");
    expect(rows().filter((item) => item.tabIndex === 0).map((item) => item.dataset.task)).toEqual(["T-0002"]);
    expect(document.activeElement).toBe(field);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("lists the keys with ?", async () => {
    await openList();
    row("T-0002").focus();
    fireEvent.keyDown(row("T-0002"), { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText("Task list: next task")).toBeTruthy();
    expect(within(dialog).getByText("In a task: back to its row in the list; a dialog: close it")).toBeTruthy();
    expect(within(dialog).getByText(/No key changes a task/)).toBeTruthy();
  });

  it("adds no key listener to the document or the window", async () => {
    const onDocument = vi.spyOn(document, "addEventListener");
    const onWindow = vi.spyOn(window, "addEventListener");
    await openList(taskClient(SOME_TASKS), "#/alpha/tasks/T-0002");
    await screen.findByRole("tab", { name: "Overview" });
    const keyListeners = [...onDocument.mock.calls, ...onWindow.mock.calls].map(([type]) => type).filter((type) => /^key/.test(type));
    expect(keyListeners).toEqual([]);
  });

  it("opens with a click: one history entry and focus on the row", async () => {
    await openList();
    const before = window.history.length;
    fireEvent.click(row("T-0007"));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/tasks/T-0007");
    });
    expect(window.history.length).toBe(before + 1);
    await focused(row("T-0007"));
  });
});

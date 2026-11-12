import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { CardCounts, QueueEvent, WorkspaceDocument } from "../api/types";
import { aColumnCount, aFlowCount, aFlowGap, aFlowView, aModule, aProposal, aWorkspaces, tenGapRows } from "../test/builders";
import { PINNED_WORKSPACES, pinnedCountsJson, pinnedHubCountsJson, pinnedWorkspace } from "../test/countsStub";
import { callsOf } from "../test/homeStub";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";
import { SOME_TASKS } from "../test/taskStub";

// docs/features/department-overview.md "Acceptance criteria" on a stub client: what the Overview
// reads and never reads (DO3), the hub's agent tasks (DO4), ages against the viewer's day with no
// read (DO5), the blocks by `sections` and `flows` alone (DO6), the Overview's states and live reads
// (DO10, DO12); with docs/features/ui-home.md's route (AC-01) and its nav and keys (AC-08), which
// still hold.

/** Markup an author pasted; shown as text, never parsed (escaped so no source line spells a dialog call). */
const HOSTILE = "<img src=x onerror=\u0061lert(1)>";

/** The scenario's departments, `words` rooted at DOC-1 by its module `chapters`. */
function scenarioWorkspaces(): WorkspaceDocument {
  return aWorkspaces(
    PINNED_WORKSPACES.map((workspace) => (workspace.name === "words" ? { ...workspace, module: "chapters" } : workspace)),
    [aModule({ name: "chapters", root: "DOC-1" })],
  );
}

/** A stub serving the spec's scenario, `chosen` the viewer's department; the Flow read's gaps all measured at 0. */
function homeClient(chosen: string | null = "words", counts: CardCounts = pinnedCountsJson()): StubClient {
  const client = stubClient();
  client.getWorkspaces.mockResolvedValue(scenarioWorkspaces());
  client.getCardCounts.mockResolvedValue(counts);
  if (chosen !== null) {
    window.localStorage.setItem("specengine.workspace.alpha", chosen);
  }
  return client;
}

function overview(): HTMLElement {
  const found = document.querySelector<HTMLElement>(".department-overview");
  if (found === null) {
    throw new Error("no Overview blocks");
  }
  return found;
}

/** The Overview of `#/alpha`, its counts and its Gaps read settled. */
async function openHome(client: StubClient = homeClient(), hash = "#/alpha") {
  renderApp(client, hash);
  await screen.findByRole("heading", { level: 1, name: "Overview" });
  await waitFor(() => {
    expect(overview().getAttribute("aria-busy")).toBe("false");
    expect(document.querySelectorAll('.department-block[aria-busy="true"]')).toHaveLength(0);
  });
  return client;
}

/** The blocks' headings, in order. */
function blocks(): string[] {
  return within(overview())
    .getAllByRole("heading", { level: 2 })
    .map((heading) => heading.textContent);
}

/** A table's rows, each its header and cells' text. */
function rowsOf(name: string): string[][] {
  const table = screen.getByRole("table", { name });
  return Array.from(table.querySelectorAll("tr"), (row) => Array.from(row.children, (cell) => cell.textContent));
}

/** The Waiting block's rows, each its label and its value's text. */
function waiting(): [string, string][] {
  const list = screen.getByRole("region", { name: "Waiting on this department" }).querySelector(".property-rows");
  return Array.from(list?.querySelectorAll(".property-row") ?? [], (row) => [row.querySelector("dt")?.textContent ?? "", row.querySelector("dd")?.textContent ?? ""]);
}

/** A stub whose live tail the test drives: the returned function emits one event. */
function live(client: StubClient): (event: QueueEvent) => void {
  let emit: (event: QueueEvent) => void = () => undefined;
  client.subscribe = (_project, onEvent) => {
    emit = onEvent;
    return () => undefined;
  };
  return (event) => {
    act(() => {
      emit(event);
    });
  };
}

/** Runs the tasks jsdom queued: a hashchange, a late answer. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function hold<T>() {
  let release: (value: T) => void = () => undefined;
  const promise = new Promise<T>((resolve) => {
    release = resolve;
  });
  return { promise, release };
}

describe("the home route (AC-01 of ui-home)", () => {
  it("lands #/ on the first project's home without a history entry, focus at the top", async () => {
    const before = window.history.length;
    // The scenario's first department, words, lands on the Overview.
    renderApp(homeClient(null), "#/");
    expect(await screen.findByRole("heading", { level: 1, name: "Overview" })).toBeTruthy();
    expect(window.location.hash).toBe("#/alpha");
    expect(window.history.length).toBe(before);
    expect(screen.getByText("alpha").closest("p")?.textContent).toBe("What waits for you in alpha.");
    expect(document.activeElement).toBe(document.body);
  });

  it("opens #/alpha/ as the home too", async () => {
    await openHome(undefined, "#/alpha/");
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
  });

  it("says there is no project gamma for #/gamma", async () => {
    renderApp(homeClient(), "#/gamma");
    expect(await screen.findByRole("heading", { level: 1, name: "Not found" })).toBeTruthy();
    expect(screen.getByText("There is no project gamma.")).toBeTruthy();
  });
});

describe("what the Overview reads (DO3)", () => {
  it("with the implicit department: the counts and Flow's top read once each, never the inbox, the agent tasks or health", async () => {
    const client = await openHome(stubClient());
    await settle();
    // The shell's one identity read: a local page's `{login: null}`, so no sync read follows (docs/features/hub-git.md "Status and the screen").
    expect(callsOf(client)).toEqual({ getProjects: 1, getIdentity: 1, getWorkspaces: 1, getCardCounts: 1, getFlow: 1 });
    expect(argsOf(client.getCardCounts)).toEqual([["alpha"]]);
    expect(argsOf(client.getFlow)).toEqual([["alpha", { depth: 0, in_flow: true, trace: true }]]);
  });

  it("print, whose sections list Health and not Flow: no health read and no flow read (M: a health read on the Overview; the flow read made whatever the sections)", async () => {
    const client = await openHome(homeClient("print"));
    await settle();
    expect(client.getHealth).toHaveBeenCalledTimes(0);
    expect(client.getFlow).toHaveBeenCalledTimes(0);
    expect(callsOf(client)).toEqual({ getProjects: 1, getIdentity: 1, getWorkspaces: 1, getCardCounts: 1 });
  });

  it("words, whose sections list Flow: one flow read, at its rooted module's root", async () => {
    const client = await openHome(homeClient("words"));
    await settle();
    expect(argsOf(client.getFlow)).toEqual([["alpha", { root: "DOC-1", depth: 1, in_flow: true, trace: true }]]);
    expect(client.getHealth).toHaveBeenCalledTimes(0);
  });

  it("press, its module none: the top read", async () => {
    const client = await openHome(homeClient("press"));
    expect(argsOf(client.getFlow)).toEqual([["alpha", { depth: 0, in_flow: true, trace: true }]]);
  });

  it("reads no flow and shows no block while the departments are out, then the chosen one's", async () => {
    const client = homeClient("print");
    const read = hold<WorkspaceDocument>();
    client.getWorkspaces.mockReturnValue(read.promise);
    renderApp(client, "#/alpha");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    await waitFor(() => {
      expect(within(overview()).getByRole("status", { name: "Loading the numbers of alpha" })).toBeTruthy();
    });
    await settle();
    expect(screen.queryByRole("heading", { level: 2 })).toBeNull();
    expect(client.getFlow).toHaveBeenCalledTimes(0);
    await act(async () => {
      read.release(scenarioWorkspaces());
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(blocks()).toEqual(["Work", "Waiting on this department"]);
    });
    expect(client.getFlow).toHaveBeenCalledTimes(0);
  });

  it("with the departments' read failed: every flow, Gaps shown, no side table", async () => {
    const client = homeClient(null);
    client.getWorkspaces.mockRejectedValue(new ClientError({ status: 503, message: "workspaces: bad config" }));
    await openHome(client);
    expect(blocks()).toEqual(["Work", "Waiting on this department", "Gaps in the flow"]);
    expect(rowsOf("Tasks by status, per flow").map((row) => row[0])).toEqual(["Flow", "Copy", "Plates", "Sound", "All flows"]);
  });
});

describe("the blocks by config alone (DO6, DO12)", () => {
  it("words: Work, Waiting and Gaps, no side table; its name under the heading", async () => {
    await openHome(homeClient("words"));
    expect(blocks()).toEqual(["Work", "Waiting on this department", "Gaps in the flow"]);
    expect(screen.queryByRole("table", { name: "Tasks by status, per department" })).toBeNull();
    expect(document.querySelectorAll(".home-view .view-about")[1]?.textContent).toBe("Department: Words");
  });

  it("print: Work and Waiting alone (M: the side table shown for a department with flows set)", async () => {
    await openHome(homeClient("print"));
    expect(blocks()).toEqual(["Work", "Waiting on this department"]);
  });

  it("press: every block, the side table one row per department in served order, then All flows, each flow once", async () => {
    vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 10, 12, 12, 0) });
    await openHome(homeClient("press"));
    expect(blocks()).toEqual(["Work", "Waiting on this department", "Gaps in the flow", "Departments side by side"]);
    expect(rowsOf("Tasks by status, per department")).toEqual([
      ["Department", "To do", "In progress", "Waiting for review", "Returned", "Done", "Open questions", "Oldest question", "Ready to accept"],
      ["Words", "0", "1", "1", "1", "0", "3", "oldest 10 days", "1"],
      ["Print", "1", "1", "1", "1", "1", "3", "oldest 10 days", "1"],
      ["Press room", "2", "1", "1", "1", "1", "3", "oldest 10 days", "1"],
      ["All flows", "2", "1", "1", "1", "1", "3", "oldest 10 days", "1"],
    ]);
    const table = screen.getByRole("table", { name: "Tasks by status, per department" });
    expect(Array.from(table.querySelectorAll("tbody th bdi"), (title) => title.textContent)).toEqual(["Words", "Print", "Press room"]);
  });

  it("one department alone: no side table, and no name under the heading", async () => {
    const client = homeClient(null);
    client.getWorkspaces.mockResolvedValue(aWorkspaces([pinnedWorkspace("press")]));
    await openHome(client);
    expect(blocks()).toEqual(["Work", "Waiting on this department", "Gaps in the flow"]);
    expect(document.querySelectorAll(".home-view .view-about")).toHaveLength(1);
  });
});

describe("Work and what waits (DO2, DO12)", () => {
  it("print's Work: a row per flow by its served title, each head with its category's icon, All flows, then not in the flow and cancelled", async () => {
    await openHome(homeClient("print"));
    expect(rowsOf("Tasks by status, per flow")).toEqual([
      ["Flow", "To do", "In progress", "Waiting for review", "Returned", "Done"],
      ["Copy", "0", "1", "1", "1", "0"],
      ["Plates", "1", "0", "0", "0", "1"],
      ["All flows", "1", "1", "1", "1", "1"],
    ]);
    const heads = screen.getByRole("table", { name: "Tasks by status, per flow" }).querySelectorAll("thead th");
    expect(Array.from(heads, (head) => head.querySelector("svg") !== null)).toEqual([false, true, true, true, true, true]);
    expect(Array.from(document.querySelectorAll(".department-aside li"), (item) => item.textContent)).toEqual(["1 not in the flow", "1 cancelled"]);
  });

  it("words' Work: one flow, no All flows", async () => {
    await openHome(homeClient("words"));
    expect(rowsOf("Tasks by status, per flow")).toEqual([
      ["Flow", "To do", "In progress", "Waiting for review", "Returned", "Done"],
      ["Copy", "0", "1", "1", "1", "0"],
    ]);
  });

  it("words' Waiting: each row a count and the page that lists it; Ready to accept in the owner's word; the oldest question's age", async () => {
    vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 10, 12, 12, 0) });
    await openHome(homeClient("words"));
    expect(waiting()).toEqual([
      ["Waiting for review", "1Open Board"],
      ["Returned subtasks", "1Open Board"],
      ["Ready to accept", "1Open Board"],
      ["Open questions", "3oldest 10 daysOpen Inbox"],
      ["Agent tasks awaiting you", "1Open Agent tasks"],
    ]);
    const region = screen.getByRole("region", { name: "Waiting on this department" });
    expect(within(region).getAllByRole("link").map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
      ["Open Board", "#/alpha/board"],
      ["Open Board", "#/alpha/board"],
      ["Open Board", "#/alpha/board"],
      ["Open Inbox", "#/alpha/inbox"],
      ["Open Agent tasks", "#/alpha/tasks"],
    ]);
    // Every count is a pointer: no control on the Overview but its links.
    expect(within(overview()).queryAllByRole("button")).toEqual([]);
  });

  it("names a flow no [card] lists by its name raw, tagged English, under a department whose flows is null", async () => {
    const counts = pinnedCountsJson();
    counts.columns.push(aColumnCount({ flow: "ink", column: "mixing", open: 2 }));
    counts.by_flow.push(aFlowCount({ flow: "ink" }));
    await openHome(homeClient("press", counts));
    const ink = screen.getByRole("table", { name: "Tasks by status, per flow" }).querySelector('tr[data-flow="ink"] th bdi');
    expect([ink?.textContent, ink?.getAttribute("lang")]).toEqual(["ink", "en"]);
    expect(Array.from(document.querySelectorAll(".department-aside li"), (item) => item.textContent)).toEqual(["3 not in the flow", "1 cancelled"]);
  });

  it("shows a hostile title as text", async () => {
    const counts = pinnedCountsJson();
    counts.flows = counts.flows.map((flow) => (flow.name === "copy" ? { ...flow, title: HOSTILE } : flow));
    await openHome(homeClient("words", counts));
    expect(document.querySelector(".department-overview img")).toBeNull();
    expect(screen.getByRole("table", { name: "Tasks by status, per flow" }).textContent).toContain(HOSTILE);
  });
});

describe("agent tasks on a hub (DO4)", () => {
  it("says local only in words, no digit in its row (M: null shown as 0)", async () => {
    await openHome(homeClient("words", pinnedHubCountsJson()));
    const row = waiting().find(([label]) => label === "Agent tasks awaiting you");
    expect(row).toEqual(["Agent tasks awaiting you", "Local only: agent tasks live on each terminalOpen Agent tasks"]);
    expect(row?.[1]).not.toMatch(/\d/);
  });

  it("shows a word the page does not know raw, tagged English", async () => {
    await openHome(homeClient("words", { ...pinnedCountsJson(), agent_tasks: "sampled" }));
    expect(screen.getByText("sampled").getAttribute("lang")).toBe("en");
  });
});

describe("no clock (DO5)", () => {
  it("turns oldest 10 days into oldest 12 days when the viewer's day moves from 11-12 to 11-14, with no read (M: the age from Date.now())", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "Date"], now: new Date(2026, 10, 12, 12, 0) });
    const client = await openHome(homeClient("words"));
    expect(screen.getByText("oldest 10 days")).toBeTruthy();
    vi.setSystemTime(new Date(2026, 10, 14, 12, 0));
    act(() => {
      vi.advanceTimersByTime(30_000);
    });
    expect(screen.getByText("oldest 12 days")).toBeTruthy();
    expect(client.getCardCounts).toHaveBeenCalledTimes(1);
  });

  it("says oldest today on the day the question was asked", async () => {
    vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 10, 2, 18, 0) });
    await openHome(homeClient("words"));
    expect(screen.getByText("oldest today")).toBeTruthy();
  });
});

describe("Gaps (DO3, DO12)", () => {
  it("lists each row that found something, in its words, then the rows not measured, then Open Flow", async () => {
    const client = homeClient("words");
    client.getFlow.mockResolvedValue(
      aFlowView([], [], {
        gaps: tenGapRows([aFlowGap("unreachable", [["DOC-2"], ["DOC-3"]]), aFlowGap("trap-cycle", [], { state: "not_configured", count: null }), aFlowGap("supply-unfed", [], { state: "cannot", count: null })]),
      }),
    );
    await openHome(client);
    const region = screen.getByRole("region", { name: "Gaps in the flow" });
    expect(Array.from(region.querySelectorAll(".department-gap"), (row) => row.textContent)).toEqual(["2 foundSteps no path from a start reaches."]);
    expect(within(region).getByText("2 rows not measured")).toBeTruthy();
    expect(within(region).queryByText("No gap found.")).toBeNull();
  });

  it("says no gap found when every row is at 0, and links Flow", async () => {
    await openHome(homeClient("words"));
    const region = screen.getByRole("region", { name: "Gaps in the flow" });
    expect(within(region).getByText("No gap found.")).toBeTruthy();
    expect(within(region).queryByText(/not measured/)).toBeNull();
    expect(within(region).getByRole("link", { name: "Open Flow" }).getAttribute("href")).toBe("#/alpha/flow");
  });

  it("fails alone with its own Retry, which reads the flow again and nothing else", async () => {
    const client = homeClient("words");
    client.getFlow.mockRejectedValueOnce(new ClientError({ status: 503, message: "flow: the index is locked" }));
    renderApp(client, "#/alpha");
    const region = await screen.findByRole("region", { name: "Gaps in the flow" });
    expect(await within(region).findByText("flow: the index is locked")).toBeTruthy();
    expect(await screen.findByRole("table", { name: "Tasks by status, per flow" })).toBeTruthy();
    fireEvent.click(within(region).getByRole("button", { name: "Retry" }));
    await within(region).findByText("No gap found.");
    expect([client.getFlow.mock.calls.length, client.getCardCounts.mock.calls.length]).toEqual([2, 1]);
  });
});

describe("the Overview's states (DO12)", () => {
  it("shows a skeleton and aria-busy while the counts are on the way, Gaps its own", async () => {
    const client = homeClient("words");
    const counts = hold<CardCounts>();
    client.getCardCounts.mockReturnValue(counts.promise);
    renderApp(client, "#/alpha");
    await waitFor(() => {
      expect(overview().getAttribute("aria-busy")).toBe("true");
    });
    expect(within(overview()).getByRole("status", { name: "Loading the numbers of alpha" })).toBeTruthy();
    await within(overview()).findByText("No gap found.");
    await act(async () => {
      counts.release(pinnedCountsJson());
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(overview().getAttribute("aria-busy")).toBe("false");
    });
    expect(blocks()).toEqual(["Work", "Waiting on this department", "Gaps in the flow"]);
  });

  it("with no flow and no column served: an empty state linking Agent tasks and the Inbox", async () => {
    await openHome(homeClient("words", { ...pinnedCountsJson(), flows: [], columns: [], by_flow: [] }));
    const title = screen.getByRole("heading", { level: 2, name: "No task flow yet" });
    const box = title.closest(".empty-state-box");
    expect(box?.textContent).toBe("No task flow yetTasks are counted once the project configures a flow. Meanwhile, Agent tasks and Inbox hold what waits for you.");
    expect(Array.from(box?.querySelectorAll("a") ?? [], (link) => [link.textContent, link.getAttribute("href")])).toEqual([
      ["Agent tasks", "#/alpha/tasks"],
      ["Inbox", "#/alpha/inbox"],
    ]);
    expect(blocks()).toEqual(["No task flow yet", "Gaps in the flow"]);
  });

  it("fails in the daemon's words, tagged English; Retry reads the counts again and nothing else, then focus to Work", async () => {
    const client = homeClient("print");
    const message = "no route /api/projects/alpha/cards/counts: the routes are /api/projects and the web UI";
    client.getCardCounts.mockRejectedValueOnce(new ClientError({ status: 404, message }));
    renderApp(client, "#/alpha");
    const shown = await screen.findByText(message);
    expect(shown.getAttribute("lang")).toBe("en");
    expect(screen.getByRole("alert").textContent).toBe(`The task counts could not be read${message}`);
    const retry = screen.getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await screen.findByRole("table", { name: "Tasks by status, per flow" });
    expect(callsOf(client)).toEqual({ getProjects: 1, getIdentity: 1, getWorkspaces: 1, getCardCounts: 2 });
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("heading", { level: 2, name: "Work" }));
    });
  });

  it("keeps the last numbers under the panel when a read again fails", async () => {
    const client = homeClient("print");
    const emit = live(client);
    await openHome(client);
    client.getCardCounts.mockRejectedValueOnce(new ClientError({ status: 503, message: "spec card counts: a bad [card] table" }));
    // A task event reads the counts again (useLiveQueue).
    emit({ seq: 1, type: "task.updated", payload: { id: "T-0001" } });
    expect(await screen.findByText("spec card counts: a bad [card] table")).toBeTruthy();
    expect(rowsOf("Tasks by status, per flow")[1]).toEqual(["Copy", "0", "1", "1", "1", "0"]);
    expect(client.getCardCounts).toHaveBeenCalledTimes(2);
  });

  it("lists the daemon's notes verbatim, tagged English", async () => {
    const note = "C-0009: its `tasks` does not read; its agent tasks are not counted";
    await openHome(homeClient("words", { ...pinnedCountsJson(), notes: [note, "  spaced  note "] }));
    const notes = screen.getByRole("list", { name: "Notes from the daemon on the counts" });
    expect(Array.from(notes.querySelectorAll("li"), (item) => [item.textContent, item.getAttribute("lang")])).toEqual([
      [note, "en"],
      ["  spaced  note ", "en"],
    ]);
  });
});

describe("live reads (DO10)", () => {
  it.each([
    ["proposal.created", "PR-0001", 2],
    ["proposal.applied", "PR-0001", 2],
    ["proposal.approved", "PR-0001", 2],
    ["proposal.deferred", "PR-0001", 2],
    ["task.updated", "T-0001", 2],
    ["card.moved", "C-0001", 2],
    ["proposal.staged", "PR-0001", 1],
    ["proposal.unstaged", "PR-0001", 1],
  ])("on %s reads the counts %i time(s) in all", async (type, id, reads) => {
    const client = homeClient("print");
    const emit = live(client);
    await openHome(client);
    emit({ seq: 1, type, payload: { id } });
    await settle();
    await settle();
    expect([type, client.getCardCounts.mock.calls.length]).toEqual([type, reads]);
  });
});

describe("nav, switcher and keys (AC-08 of ui-home)", () => {
  function navLinks(): HTMLElement[] {
    return within(screen.getByRole("navigation", { name: "Sections" })).getAllByRole("link");
  }

  /** The implicit department over a queue and agent tasks: every screen has something to focus. */
  function busyClient(): StubClient {
    const client = stubClient([aProposal({ id: "PR-1" })]);
    client.getTasks.mockResolvedValue({ tasks: SOME_TASKS, notes: [] });
    return client;
  }

  it("lists nine entries, Overview first to the home, current only there; Tasks off the nav (docs/features/workspaces.md WS-1)", async () => {
    await openHome(stubClient());
    await waitFor(() => {
      expect(navLinks()).toHaveLength(9);
    });
    expect(navLinks().map((link) => [link.textContent, link.getAttribute("href"), link.getAttribute("aria-current")])).toEqual([
      ["Overview", "#/alpha", "page"],
      ["Inbox", "#/alpha/inbox", null],
      ["Board", "#/alpha/board", null],
      ["Roadmap", "#/alpha/roadmap", null],
      ["Flow", "#/alpha/flow", null],
      ["Spec tree", "#/alpha/tree", null],
      ["Graph", "#/alpha/graph", null],
      ["Health", "#/alpha/health", null],
      ["Questions", "#/alpha/questions", null],
    ]);
  });

  it("switches the home to the other project's home", async () => {
    await openHome(stubClient());
    const select = screen.getByLabelText("Project");
    await waitFor(() => {
      expect((select as HTMLSelectElement).disabled).toBe(false);
    });
    expect((select as HTMLSelectElement).value).toBe("alpha");
    fireEvent.change(select, { target: { value: "beta" } });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/beta");
    });
    expect(await screen.findByRole("heading", { level: 1, name: "Overview" })).toBeTruthy();
    expect(screen.getByText("beta").closest("p")?.textContent).toBe("What waits for you in beta.");
  });

  it("lists the chord, ? and Esc on ? pressed on a home link, never the Inbox's keys", async () => {
    await openHome();
    const [link] = within(screen.getByRole("region", { name: "Waiting on this department" })).getAllByRole("link", { name: "Open Board" });
    if (link === undefined) {
      throw new Error("no Open Board link");
    }
    link.focus();
    fireEvent.keyDown(link, { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(Array.from(dialog.querySelectorAll(".shortcut"), (row) => row.textContent)).toEqual([
      "Cmd-K or Ctrl-KJump to a section, agent task, proposal, node or project",
      "?Show this list",
      "EscClose a dialog",
    ]);
    expect(within(dialog).queryByText("Next proposal")).toBeNull();
  });

  it("opens nothing for ? with Ctrl, Alt or Cmd held", async () => {
    await openHome();
    const link = within(screen.getByRole("region", { name: "Gaps in the flow" })).getByRole("link", { name: "Open Flow" });
    expect(fireEvent.keyDown(link, { key: "?", ctrlKey: true })).toBe(true);
    expect(fireEvent.keyDown(link, { key: "?", altKey: true })).toBe(true);
    expect(fireEvent.keyDown(link, { key: "?", metaKey: true })).toBe(true);
    await settle();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it.each([
    ["#/alpha", "a home link", ".department-open"],
    ["#/alpha/inbox", "the queue", '[role="option"][aria-selected="true"]'],
    ["#/alpha/tasks", "the task list", '[role="option"][tabindex="0"]'],
    ["#/alpha/tree", "the tree", '[role="treeitem"][tabindex="0"]'],
    ["#/alpha/graph", "the view's heading", "main h1"],
  ])("lists the chord on %s (? on %s)", async (hash, _where, selector) => {
    renderApp(busyClient(), hash);
    await waitFor(() => {
      expect(document.querySelector(selector)).not.toBeNull();
    });
    const target = document.querySelector<HTMLElement>(selector);
    if (target === null) {
      throw new Error(`nothing at ${selector}`);
    }
    target.focus();
    fireEvent.keyDown(target, { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText("Jump to a section, agent task, proposal, node or project")).toBeTruthy();
    // The intro's "without Ctrl, Alt or Cmd" names the chord as its exception.
    expect(dialog.querySelector(".dialog-text")?.textContent).toContain(
      "without Ctrl, Alt or Cmd; Cmd-K or Ctrl-K excepted: it opens Jump to from anywhere, a text field included.",
    );
  });
});

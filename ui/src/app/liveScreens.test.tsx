import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HttpClient } from "../api/http";
import type { CheckReport, GraphView, TaskList, TaskNotFound, TaskPackage } from "../api/types";
import { NO_DIFF_NOTICE } from "../tasks/SpecChangesPanel";
import {
  aCheckFinding,
  aCheckReport,
  aGraphEdge,
  aGraphNode,
  aGraphView,
  aProposal,
  aTaskEntry,
  aTaskPackage,
  aTaskProposal,
  aTaskRun,
  entryOf,
} from "../test/builders";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { renderApp } from "../test/render";

// AC-13 of docs/features/ui-live.md: Health and Graph over HttpClient, the daemon a stubbed
// `fetch`. Each shows the daemon's answer as data (every check verdict a 200 report, the graph's
// 404 the exit-1 document) or its refusal in its own words with Retry; never "Not built yet".
// AC-11 of docs/features/ui-live-tasks.md: Tasks, a task, Home's task panel and the palette's
// tasks the same way, from `/tasks` and `/tasks/:id`; AC-12: the Inbox card's Task fact from the
// served review document.

const NOT_BUILT = "Not built yet: the daemon has no endpoint for this read";

/** How the stubbed daemon answers the read under test (the check or the graph). */
type Answer = { status: number; body: CheckReport | GraphView } | { status: number; message: string };

/** The daemon: one project with an empty inbox, `read` answering `answer`; anything else its 404. */
function daemon(read: "check" | "graph", answer: Answer) {
  return stubFetch((url) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: [], notes: [] });
    }
    if (url.startsWith(`/api/projects/alpha/${read}`)) {
      return "message" in answer ? errorAnswer(answer.status, answer.message) : jsonAnswer(answer.status, answer.body);
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

function region(name: string): HTMLElement {
  return screen.getByRole("region", { name });
}

/** Health with its four regions answered. */
async function openHealth() {
  renderApp(new HttpClient(), "#/alpha/health");
  await screen.findByRole("heading", { level: 1, name: "Health" });
  await waitFor(() => {
    expect(document.querySelectorAll('.health-region[aria-busy="false"]')).toHaveLength(4);
  });
}

function verdictLabel(): string | undefined {
  return region("Check").querySelector(".check-verdict .badge .badge-label")?.textContent;
}

function findingRows(): HTMLElement[] {
  return Array.from(region("Findings").querySelectorAll<HTMLElement>("[data-finding]"));
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Health over the daemon (AC-13 of ui-live)", () => {
  it("a 200 `blocked` report: Fails the check, its finding listed, the check read once; never Not built yet", async () => {
    const report = aCheckReport({
      verdict: "blocked",
      counts: { documents: 9, errors: 1, worst_w_bytes: 24576 },
      findings: [aCheckFinding({ code: "key-missing", path: "docs/spec/movement/sprint.md", line: 1, subject: "status", message: "the key `status` is missing" })],
    });
    const fetchStub = daemon("check", { status: 200, body: report });
    await openHealth();
    expect(verdictLabel()).toBe("Fails the check");
    expect(findingRows()).toHaveLength(1);
    expect(findingRows()[0]?.textContent).toContain("the key `status` is missing");
    expect(region("Check").querySelector(".check-worst-w")?.textContent).toBe("24576 B");
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
    expect(screen.queryAllByRole("alert")).toEqual([]);
    expect(urlsOf(fetchStub).filter((url) => url.includes("/check"))).toEqual(["/api/projects/alpha/check"]);
  });

  it("a 200 `cannot-check` report: Could not check, the causes verbatim, W not measured", async () => {
    const report = aCheckReport({
      verdict: "cannot-check",
      counts: { documents: 0, worst_w_bytes: 0 },
      cannot_check: [
        { path: "", message: "today `x` is not a YYYY-MM-DD date" },
        { path: ".spec-debt.toml", message: "line 3: `expires` is not a YYYY-MM-DD date" },
      ],
    });
    daemon("check", { status: 200, body: report });
    await openHealth();
    expect(verdictLabel()).toBe("Could not check");
    // As the CLI prints a cause: `.` for no path (core check/report.rs `shown`).
    expect(Array.from(region("Check").querySelectorAll(".check-causes li"), (item) => item.textContent)).toEqual([
      ".: today `x` is not a YYYY-MM-DD date",
      ".spec-debt.toml: line 3: `expires` is not a YYYY-MM-DD date",
    ]);
    expect(region("Check").querySelector(".check-worst-w")?.textContent).toBe("Not measured");
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("a 503: the daemon's words in an alert, with Retry; never Not built yet", async () => {
    const message = "spec: specengine.toml: `[check] mode` must be one of observe, enforce-introduced, enforce";
    daemon("check", { status: 503, message });
    renderApp(new HttpClient(), "#/alpha/health");
    const alert = await within(await screen.findByRole("region", { name: "Check" })).findByRole("alert");
    expect(alert.textContent).toBe(`The check could not be read${message}`);
    expect(within(region("Check")).getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });
});

describe("Graph over the daemon (AC-13 of ui-live)", () => {
  it("a 404 document: its reason, the REF and the default depth as the query; never Not built yet", async () => {
    const reason = "`R-404` resolves to no ID and no alias";
    const fetchStub = daemon("graph", { status: 404, body: aGraphView([], [], { ref: "R-404", reason, types: [], depth: null }) });
    renderApp(new HttpClient(), "#/alpha/graph/R-404");
    expect(await screen.findByText(reason)).toBeTruthy();
    expect(screen.getByText("R-404 names nothing to draw")).toBeTruthy();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
    expect(urlsOf(fetchStub).filter((url) => url.includes("/graph"))).toEqual(["/api/projects/alpha/graph?ref=R-404&depth=2"]);
  });

  it("a 200: the walk drawn and counted", async () => {
    const answer = aGraphView(
      [aGraphNode({ id: "MEC-TIDES", distance: 0 }), aGraphNode({ id: "RULE-TIDE-WINDOW", distance: 1 })],
      [aGraphEdge({ src: "MEC-TIDES", type: "zeta_type", dst: "RULE-TIDE-WINDOW" })],
    );
    daemon("graph", { status: 200, body: answer });
    renderApp(new HttpClient(), "#/alpha/graph/MEC-TIDES");
    await waitFor(() => {
      expect(document.querySelector(".graph-count")?.textContent).toBe("2 nodes, 1 edge");
    });
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__node").length).toBeGreaterThan(0);
    });
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("a 503: the daemon's words in an alert, with Retry", async () => {
    // Non-Latin text from the daemon, verbatim (escaped here: ADR-0024).
    const message = "spec: `MEC-\u0417\u0435` mixes scripts in an ID; IDs are Latin only";
    daemon("graph", { status: 503, message });
    renderApp(new HttpClient(), "#/alpha/graph/MEC-TIDES");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The graph of MEC-TIDES could not be read${message}`);
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });
});

/** harbor-sim's served list: two waiting, a draft, one in progress, one closed. */
const SERVED_LIST: TaskList = {
  tasks: [
    aTaskEntry({ id: "T-0104", status: "done", title: "Mooring crew of four", targets: ["MEC-MOORING"], stale: null }),
    aTaskEntry({ id: "T-0107", status: "review", title: "Queue deep ships at the outer anchorage", targets: ["MEC-ANCHORAGE"] }),
    aTaskEntry({ id: "T-0108", status: "ready", title: "Speed limit on night passages", targets: ["MEC-TIDES"], stale: true }),
    aTaskEntry({ id: "T-0109", status: "in_progress", title: "Tide tables at the berth", targets: ["MEC-TIDES"], stale: false }),
    aTaskEntry({ id: "T-0113", status: "draft", title: null }),
  ],
  notes: ["T-0106: a corrupt row skipped (bad JSON in criteria)"],
};

/** T-0107 as served: approved before, its target since changed; the bundle could not be made, git made no diff. */
const T_0107: TaskPackage = aTaskPackage({
  id: "T-0107",
  project: "harbor-sim",
  status: "review",
  title: "Queue deep ships at the outer anchorage",
  stale: true,
  plan: "Queue the deep ships.",
  open_proposals: [aTaskProposal({ id: "PR-0041", kind: "discrepancy", task_id: "T-0107", summary: "Entry only inside the tide window" })],
  spec_snapshot: {
    at: "2026-10-05T09:00:00Z",
    place: { worktree: "/work/harbor-sim", root_rel: "", branch: "main", commit: "c0ffee1" },
    nodes: [{ id: "MEC-ANCHORAGE", path: "docs/spec/anchorage.md", span_hash: "b3:0" }],
  },
  snapshot_diff: [{ id: "MEC-ANCHORAGE", path: "docs/spec/anchorage.md", span_hash: "b3:0", diff: null, cut: false }],
  runs: [aTaskRun({ run: 1, role: "harbour-dev", ended_at: "2026-10-05T12:00:00Z", outcome: "partial", summary: "Half done" })],
  bundle: { node_ids: ["MEC-ANCHORAGE"], budget: 10000, bundle_hash: null },
  notes: ["bundle: `MEC-ANCHORAGE` resolves to no ID", "snapshot_diff: no diff of `MEC-ANCHORAGE`: git exited 128"],
});

const NO_T_0099: TaskNotFound = { id: "T-0099", reason: "no task T-0099 in this repository" };

/** The daemon of harbor-sim's tasks: the list (or `listAnswer`), T-0107, any other T its 404 document; an empty inbox. */
function taskDaemon(listAnswer: { status: number; message: string } | null = null) {
  return stubFetch((url) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "harbor-sim", name: "Harbor sim", root: "/work/harbor-sim", branch: "main" }]);
    }
    if (url === "/api/projects/harbor-sim/inbox") {
      return jsonAnswer(200, { proposals: [], notes: [] });
    }
    if (url === "/api/projects/harbor-sim/tasks") {
      return listAnswer === null ? jsonAnswer(200, SERVED_LIST) : errorAnswer(listAnswer.status, listAnswer.message);
    }
    const task = /^\/api\/projects\/harbor-sim\/tasks\/([^/?]+)$/.exec(url)?.[1];
    if (task !== undefined) {
      return task === "T-0107" ? jsonAnswer(200, T_0107) : jsonAnswer(404, { ...NO_T_0099, id: task, reason: `no task ${task} in this repository` });
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

/** The URLs of the task reads, in order. */
function taskReads(fetchStub: ReturnType<typeof stubFetch>): string[] {
  return urlsOf(fetchStub).filter((url) => url.includes("/tasks"));
}

describe("Tasks over the daemon (AC-11 of ui-live-tasks)", () => {
  it("lists the served entries in their groups from one `/tasks` read, its note verbatim; never Not built yet", async () => {
    const fetchStub = taskDaemon();
    renderApp(new HttpClient(), "#/harbor-sim/tasks");
    const list = await screen.findByRole("listbox", { name: "Tasks, what waits for you first" });
    const groups = within(list)
      .getAllByRole("group")
      .map((group) => [
        group.querySelector(".task-group-title > span:not(.task-group-count)")?.textContent ?? "",
        within(group)
          .getAllByRole("option")
          .map((option) => option.dataset.task ?? ""),
      ]);
    expect(groups).toEqual([
      ["Waiting for you", ["T-0107", "T-0108"]],
      ["Draft", ["T-0113"]],
      ["In progress", ["T-0109"]],
    ]);
    expect(screen.getByText("T-0106: a corrupt row skipped (bad JSON in criteria)")).toBeTruthy();
    expect(taskReads(fetchStub)).toEqual(["/api/projects/harbor-sim/tasks"]);
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("shows T-0107's tabs from the served package: a diff git could not make as the quiet line, `bundle_hash` null as sent", async () => {
    const fetchStub = taskDaemon();
    renderApp(new HttpClient(), "#/harbor-sim/tasks/T-0107");
    await screen.findByRole("tab", { name: "Overview", selected: true });
    const pane = screen.getByRole("article", { name: "T-0107: Queue deep ships at the outer anchorage" });
    expect(within(pane).getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Overview", "Plan", "Spec changes 1", "Proposals 1", "Runs 1", "Package"]);

    fireEvent.click(within(pane).getByRole("tab", { name: /^Spec changes/ }));
    const changes = within(pane).getByRole("tabpanel");
    const entry = changes.querySelector('.snapshot-diff[data-node="MEC-ANCHORAGE"]');
    expect([entry?.querySelector(".diff"), entry?.querySelector(".cut-note")?.textContent]).toEqual([null, NO_DIFF_NOTICE]);

    fireEvent.click(within(pane).getByRole("tab", { name: "Package" }));
    const json = within(pane).getByRole("tabpanel").querySelector(".task-json")?.textContent ?? "";
    expect(JSON.parse(json)).toEqual(T_0107);
    expect(json).toContain('"bundle_hash": null');
    expect(taskReads(fetchStub).filter((url) => url.includes("/tasks/"))).toEqual(["/api/projects/harbor-sim/tasks/T-0107"]);
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("an unknown T: the 404 document's reason verbatim and Back to tasks", async () => {
    taskDaemon();
    renderApp(new HttpClient(), "#/harbor-sim/tasks/T-0099");
    expect(await screen.findByText(NO_T_0099.reason)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Back to tasks" }).getAttribute("href")).toBe("#/harbor-sim/tasks");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("a 503: the daemon's words in an alert, Retry reads `/tasks` again", async () => {
    const message = "spec: /work/harbor-sim is in no git worktree: the tasks need git";
    const fetchStub = taskDaemon({ status: 503, message });
    renderApp(new HttpClient(), "#/harbor-sim/tasks");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The tasks could not be loaded${message}`);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(taskReads(fetchStub)).toEqual(["/api/projects/harbor-sim/tasks", "/api/projects/harbor-sim/tasks"]);
    });
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("Home's task panel lists what waits for you from `/tasks`", async () => {
    const fetchStub = taskDaemon();
    renderApp(new HttpClient(), "#/harbor-sim");
    const tasks = await screen.findByRole("region", { name: "Tasks" });
    await waitFor(() => {
      expect(tasks.getAttribute("aria-busy")).toBe("false");
    });
    const rows = Array.from(tasks.querySelectorAll(".home-block-waiting .home-row"), (row) => row.querySelector("a")?.getAttribute("href"));
    expect(rows).toEqual(["#/harbor-sim/tasks/T-0107", "#/harbor-sim/tasks/T-0108"]);
    expect(taskReads(fetchStub)).toEqual(["/api/projects/harbor-sim/tasks"]);
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("the palette's tasks come from `/tasks`, read once on opening it elsewhere", async () => {
    const fetchStub = taskDaemon();
    renderApp(new HttpClient(), "#/harbor-sim/inbox");
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    expect(taskReads(fetchStub)).toEqual([]);
    fireEvent.keyDown(document.body, { key: "k", code: "KeyK", metaKey: true });
    const dialog = await screen.findByRole("dialog", { name: "Jump to" });
    fireEvent.change(within(dialog).getByRole("combobox"), { target: { value: "T-01" } });
    const group = await within(dialog).findByRole("group", { name: "Tasks" });
    // The Tasks screen's order, Closed last.
    expect(within(group).getAllByRole("option").map((option) => option.textContent.split(":")[0])).toEqual(["T-0107", "T-0108", "T-0113", "T-0109", "T-0104"]);
    expect(taskReads(fetchStub)).toEqual(["/api/projects/harbor-sim/tasks"]);
    expect(within(dialog).queryByText(NOT_BUILT)).toBeNull();
  });
});

describe("the Inbox card's Task fact over the daemon (AC-12 of ui-live-tasks)", () => {
  it("links PR-0041's task from its served review document; an unbound one says No task", async () => {
    const bound = aProposal({ id: "PR-0041", project: "harbor-sim", kind: "discrepancy", summary: "Entry only inside the tide window", task_id: "T-0107" });
    const unbound = aProposal({ id: "PR-0044", project: "harbor-sim", summary: "Berth draft", task_id: null });
    stubFetch((url) => {
      if (url === "/api/projects") {
        return jsonAnswer(200, [{ slug: "harbor-sim", name: "Harbor sim", root: "/work/harbor-sim", branch: "main" }]);
      }
      if (url === "/api/projects/harbor-sim/inbox") {
        // The inbox entry: the daemon's 11 keys, no task.
        return jsonAnswer(200, { proposals: [entryOf(bound), entryOf(unbound)], notes: [] });
      }
      if (url === "/api/projects/harbor-sim/proposals/PR-0041" || url === "/api/projects/harbor-sim/proposals/PR-0044") {
        return jsonAnswer(200, url.endsWith("PR-0041") ? bound : unbound);
      }
      return errorAnswer(404, `no route ${url}`);
    });
    renderApp(new HttpClient(), "#/harbor-sim/inbox/PR-0041");
    const card = await screen.findByRole("article");
    const task = await within(card).findByRole("link", { name: "T-0107" });
    expect(task.getAttribute("href")).toBe("#/harbor-sim/tasks/T-0107");
    expect(task.closest(".fact")?.querySelector("dt")?.textContent).toBe("Task");

    fireEvent.click(within(screen.getByRole("listbox")).getByText("Berth draft"));
    await waitFor(() => {
      expect(screen.getByRole("article").querySelector(".card-id")?.textContent).toBe("PR-0044");
    });
    const other = screen.getByRole("article");
    await within(other).findByRole("heading", { level: 3, name: "Provenance" });
    const fact = Array.from(other.querySelectorAll(".fact")).find((item) => item.querySelector("dt")?.textContent === "Task");
    expect(fact?.querySelector("dd")?.textContent).toBe("No task");
  });
});

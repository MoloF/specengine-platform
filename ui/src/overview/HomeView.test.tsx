import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import { App } from "../app/App";
import type { Inbox, Proposal, TaskList } from "../api/types";
import { aProposal, aTaskEntry } from "../test/builders";
import { callsOf, homeClient } from "../test/homeStub";
import { renderApp } from "../test/render";
import { SOME_TASKS } from "../test/taskStub";

// docs/features/ui-home.md on a stub client: AC-01 (the route, `#/`), AC-02 (what the home reads),
// AC-03 (the Tasks region), AC-04 (the Inbox region's counts), AC-05 (only the daemon's keys),
// AC-06 (states per region), AC-07 (text as data, times as stored), AC-08 (nav, switcher, keys).

/** Markup an author pasted; shown as text, never parsed (escaped so no source line spells a dialog call). */
const HOSTILE = "<img src=x onerror=\u0061lert(1)>";

const QUEUE: Proposal[] = [
  aProposal({ id: "PR-1", severity: "low", kind: "update", status: "open", summary: "Low and old", created_at: "2026-10-01T08:00:00Z" }),
  aProposal({ id: "PR-2", severity: "high", kind: "question", status: "deferred", summary: "High", created_at: "2026-10-03T09:30:00Z" }),
  aProposal({ id: "PR-3", severity: "normal", kind: "update", status: "open", summary: null, rationale: "First line\nsecond", created_at: "2026-10-02T10:00:00Z" }),
];

function region(name: "Tasks" | "Inbox"): HTMLElement {
  return screen.getByRole("region", { name });
}

async function openHome(client = homeClient(SOME_TASKS, QUEUE), hash = "#/alpha") {
  renderApp(client, hash);
  await screen.findByRole("heading", { level: 1, name: "Overview" });
  await waitFor(() => {
    expect(region("Tasks").getAttribute("aria-busy")).toBe("false");
    expect(region("Inbox").getAttribute("aria-busy")).toBe("false");
  });
  return client;
}

/** Runs the tasks jsdom queued: a hashchange, a late answer. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function tally(element: HTMLElement, heading: string): string[] {
  const block = within(element).getByRole("heading", { level: 3, name: heading }).parentElement;
  return Array.from(block?.querySelectorAll(".home-tally > li") ?? [], (item) => item.textContent);
}

function hold<T>() {
  let release: (value: T) => void = () => undefined;
  const promise = new Promise<T>((resolve) => {
    release = resolve;
  });
  return { promise, release };
}

describe("the home route (AC-01)", () => {
  it("lands #/ on the first project's home without a history entry, focus at the top", async () => {
    const before = window.history.length;
    renderApp(homeClient(SOME_TASKS, QUEUE), "#/");
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
    renderApp(homeClient([]), "#/gamma");
    expect(await screen.findByRole("heading", { level: 1, name: "Not found" })).toBeTruthy();
    expect(screen.getByText("There is no project gamma.")).toBeTruthy();
  });
});

describe("what the home reads (AC-02)", () => {
  it("reads the projects, the inbox and the tasks once each, nothing else", async () => {
    const client = await openHome();
    await settle();
    expect(callsOf(client)).toEqual({ getProjects: 1, getInbox: 1, getTasks: 1 });
    expect(client.getInbox.mock.calls).toEqual([["alpha"]]);
    expect(client.getTasks.mock.calls).toEqual([["alpha"]]);
  });
});

describe("the Tasks region (AC-03)", () => {
  it("lists what waits for you, the other open groups but Closed, the notes verbatim", async () => {
    const note = "T-0010: unreadable row (bad JSON in criteria); skipped";
    await openHome(homeClient(SOME_TASKS, QUEUE, { taskNotes: [note, "  spaced  note "] }));
    const tasks = region("Tasks");
    expect(within(tasks).getByRole("heading", { level: 3, name: "Waiting for you: 2" })).toBeTruthy();
    expect(within(tasks).getByRole("heading", { level: 3, name: "Waiting for you: 2" }).parentElement?.className).toBe(
      "home-block home-block-waiting",
    );
    const rows = Array.from(tasks.querySelectorAll(".home-block-waiting .home-row"));
    expect(rows.map((row) => row.querySelector("a")?.textContent)).toEqual(["T-0002: Plan to read", "T-0003: Changed under it"]);
    expect(rows.map((row) => row.querySelector("a")?.getAttribute("href"))).toEqual(["#/alpha/tasks/T-0002", "#/alpha/tasks/T-0003"]);
    expect(rows.map((row) => Array.from(row.querySelectorAll(".badge"), (badge) => badge.textContent))).toEqual([
      ["State: Plan review"],
      ["State: Ready", "Spec changed"],
    ]);
    expect(tally(tasks, "Other open tasks")).toEqual(["Draft: 1", "Changes requested: 1", "Ready: 1", "In progress: 1", "Other states: 1"]);
    expect(tasks.textContent).not.toMatch(/Closed/);
    const notes = within(tasks).getByRole("list", { name: "Notes from the daemon on the task list" });
    expect(Array.from(notes.querySelectorAll("li"), (item) => item.textContent)).toEqual([note, "  spaced  note "]);
    expect(within(tasks).getByRole("link", { name: "Open Tasks" }).getAttribute("href")).toBe("#/alpha/tasks");
  });

  it("shows the first five of seven waiting and says so", async () => {
    const waiting = Array.from({ length: 7 }, (_, index) => aTaskEntry({ id: `T-${String(index + 1).padStart(4, "0")}`, status: "review" }));
    await openHome(homeClient(waiting));
    const tasks = region("Tasks");
    expect(tasks.querySelectorAll(".home-block-waiting .home-row")).toHaveLength(5);
    expect(within(tasks).getByText("5 of 7 shown")).toBeTruthy();
    expect(within(tasks).getByText("No other open task.")).toBeTruthy();
  });

  it("says nothing waits when no plan is in review and no approved spec changed", async () => {
    await openHome(homeClient([aTaskEntry({ id: "T-1", status: "ready", stale: false })]));
    expect(within(region("Tasks")).getByText("Nothing waits for you: no plan to review, no approved spec changed.")).toBeTruthy();
    expect(tally(region("Tasks"), "Other open tasks")).toEqual(["Ready: 1"]);
    // The block is emphasised only while something waits.
    const block = within(region("Tasks")).getByRole("heading", { level: 3, name: "Waiting for you: 0" }).parentElement;
    expect(block?.className).toBe("home-block");
    expect(region("Tasks").querySelector(".home-block-waiting")).toBeNull();
  });

  it("marks Spec changed only where stale is true, not on null or false", async () => {
    await openHome(
      homeClient([
        aTaskEntry({ id: "T-1", status: "review", stale: null }),
        aTaskEntry({ id: "T-2", status: "review", stale: false }),
        aTaskEntry({ id: "T-3", status: "ready", stale: true }),
      ]),
    );
    const rows = Array.from(region("Tasks").querySelectorAll(".home-row"));
    expect(rows.map((row) => row.textContent.includes("Spec changed"))).toEqual([false, false, true]);
  });
});

describe("the Inbox region (AC-04)", () => {
  it("counts the queue by status and by raw kind, each summing to the total, and lists the first five in queue order", async () => {
    const queue = [
      ...QUEUE,
      aProposal({ id: "PR-4", kind: "widget", status: "escalated", severity: "normal", created_at: "2026-10-04T00:00:00Z" }),
      aProposal({ id: "PR-5", kind: "update", status: "approved", severity: null }),
      aProposal({ id: "PR-6", kind: "question", status: "open", severity: "high", created_at: "2026-10-05T00:00:00Z" }),
    ];
    await openHome(homeClient([], queue, { inboxNotes: ["one note"] }));
    const inbox = region("Inbox");
    expect(inbox.querySelector(".home-total")?.textContent).toBe("6 proposals in the queue");
    expect(tally(inbox, "By status")).toEqual(["Open: 3", "Approved: 1", "Deferred: 1", "escalated: 1"]);
    const statuses = Array.from(within(inbox).getByRole("heading", { name: "By status" }).parentElement?.querySelectorAll(".badge") ?? []);
    expect(statuses.map((badge) => [badge.getAttribute("data-tone"), badge.querySelector("svg") !== null])).toEqual([
      ["proposal-open", true],
      ["proposal-approved", true],
      ["proposal-deferred", true],
      ["proposal-unknown", true],
    ]);
    expect(tally(inbox, "By kind")).toEqual(["update: 3", "question: 2", "widget: 1"]);
    const first = Array.from(inbox.querySelectorAll(".home-row a"), (link) => link.textContent);
    expect(first).toEqual(["PR-2: High", "PR-6: Summary of PR-6", "PR-3: First line", "PR-4: Summary of PR-4", "PR-1: Low and old"]);
    expect(within(inbox).getByRole("link", { name: "PR-2: High" }).getAttribute("href")).toBe("#/alpha/inbox/PR-2");
    expect(within(inbox).getByRole("list", { name: "Notes from the daemon on the queue" }).textContent).toBe("one note");
    expect(within(inbox).getByRole("link", { name: "Open Inbox" }).getAttribute("href")).toBe("#/alpha/inbox");
  });

  it("says 1 proposal for one", async () => {
    await openHome(homeClient([], [aProposal({ id: "PR-1" })]));
    expect(region("Inbox").querySelector(".home-total")?.textContent).toBe("1 proposal in the queue");
  });
});

describe("only the daemon's InboxEntry keys (AC-05)", () => {
  const NINE = ["id", "kind", "status", "target_id", "target_ids", "created_at", "rationale", "severity", "summary"] as const;

  function entryOf(proposal: Proposal): Proposal {
    return Object.fromEntries(NINE.map((key) => [key, proposal[key]])) as unknown as Proposal;
  }

  /** The region's markup without the ids React generates per render. */
  function markup(): string {
    return new XMLSerializer().serializeToString(region("Inbox")).replace(/\s(id|aria-labelledby)="[^"]*"/g, "");
  }

  it("renders the region from the nine keys exactly as from full proposals", async () => {
    const evidence = [{ file: "src/a.rs", qpath: null, lines: "1-2", observed: "T-0009 seen", documented: "doc" }];
    const full = [
      aProposal({ id: "PR-7", task_id: "T-0001", evidence, summary: null, rationale: null, target_id: "R-1" }),
      ...QUEUE.map((proposal) => ({ ...proposal, task_id: "T-0002", evidence })),
    ];
    await openHome(homeClient([], full));
    const fromFull = markup();
    cleanup();
    await openHome(homeClient([], full.map(entryOf)));
    expect(markup()).toBe(fromFull);
    const fromEntries = region("Inbox");
    expect(within(fromEntries).getByRole("link", { name: "PR-7: update on R-1" })).toBeTruthy();
    expect(fromEntries.textContent).not.toMatch(/T-000/);
  });
});

describe("each region's states (AC-06)", () => {
  it("shows a skeleton and aria-busy in both while their reads are on the way", async () => {
    const client = homeClient([]);
    const tasks = hold<TaskList>();
    const inbox = hold<Inbox>();
    client.getTasks.mockReturnValue(tasks.promise);
    client.getInbox.mockReturnValue(inbox.promise);
    renderApp(client, "#/alpha");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    await waitFor(() => {
      expect(client.getInbox).toHaveBeenCalledTimes(1);
    });
    for (const name of ["Tasks", "Inbox"] as const) {
      expect(region(name).getAttribute("aria-busy")).toBe("true");
      expect(within(region(name)).getByRole("status").getAttribute("aria-busy")).toBe("true");
    }
    await act(async () => {
      tasks.release({ tasks: [], notes: [] });
      inbox.release({ proposals: [], notes: [] });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(region("Tasks").getAttribute("aria-busy")).toBe("false");
    });
  });

  it("says what an empty project means and the next step in each region", async () => {
    await openHome(homeClient([], []));
    const tasks = region("Tasks");
    expect(within(tasks).getByText("No tasks yet.")).toBeTruthy();
    expect(within(tasks).getByText("spec task new --nodes REF...").tagName).toBe("CODE");
    const inbox = region("Inbox");
    expect(within(inbox).getByText("The queue is clear: no proposal waits for your decision.")).toBeTruthy();
    const next = within(inbox).getByRole("link", { name: "approve the tasks that are ready for development" });
    expect(next.closest("p")?.textContent).toBe("Next: approve the tasks that are ready for development.");
    expect(next.getAttribute("href")).toBe("#/alpha/tasks");
  });

  it("fails the Tasks region alone in the daemon's words; its Retry reads the tasks again, never the inbox", async () => {
    const client = homeClient(SOME_TASKS, QUEUE);
    client.getTasks.mockRejectedValueOnce(new ClientError({ status: 503, message: "x" }));
    renderApp(client, "#/alpha");
    const tasks = await screen.findByRole("region", { name: "Tasks" });
    expect(await within(tasks).findByText("x")).toBeTruthy();
    expect(within(tasks).getByRole("alert").textContent).toBe("The tasks could not be loadedx");
    await within(region("Inbox")).findByRole("link", { name: "PR-2: High" });
    const retry = within(tasks).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await within(tasks).findByRole("link", { name: "T-0002: Plan to read" });
    expect(callsOf(client)).toEqual({ getProjects: 1, getInbox: 1, getTasks: 2 });
    await waitFor(() => {
      expect(document.activeElement).toBe(within(tasks).getByRole("heading", { level: 2, name: "Tasks" }));
    });
  });

  it("fails the Inbox region alone in the daemon's words; its Retry reads the inbox again, never the tasks", async () => {
    const client = homeClient(SOME_TASKS, QUEUE);
    client.getInbox.mockRejectedValueOnce(new ClientError({ status: 503, message: "y" }));
    renderApp(client, "#/alpha");
    const inbox = await screen.findByRole("region", { name: "Inbox" });
    expect(await within(inbox).findByText("y")).toBeTruthy();
    await within(region("Tasks")).findByRole("link", { name: "T-0002: Plan to read" });
    fireEvent.click(within(inbox).getByRole("button", { name: "Retry" }));
    await within(inbox).findByRole("link", { name: "PR-2: High" });
    expect(callsOf(client)).toEqual({ getProjects: 1, getInbox: 2, getTasks: 1 });
  });

  it("keeps a region's render failure in that region", async () => {
    const client = homeClient(SOME_TASKS);
    client.getInbox.mockResolvedValue({ proposals: null, notes: [] } as unknown as Inbox);
    window.history.replaceState(null, "", "/#/alpha");
    const caught: unknown[] = [];
    render(<App client={client} scenario={null} />, {
      onCaughtError: (error: unknown) => {
        caught.push(error);
      },
    });
    const inbox = await screen.findByRole("region", { name: "Inbox" });
    expect(await within(inbox).findByText("The inbox overview could not be shown; the rest of the page still works.")).toBeTruthy();
    expect(await within(region("Tasks")).findByRole("link", { name: "T-0002: Plan to read" })).toBeTruthy();
    expect(caught.length).toBeGreaterThan(0);
  });
});

describe("text as data (AC-07)", () => {
  it("shows hostile titles and summaries as text, times exactly as stored, one h1, no hold wording", async () => {
    const stamp = "2026-10-04T16:10:00Z";
    const odd = "yesterday-ish";
    await openHome(
      homeClient(
        [aTaskEntry({ id: "T-1", status: "review", title: HOSTILE, updated_at: stamp }), aTaskEntry({ id: "T-2", status: "review", updated_at: odd })],
        [aProposal({ id: "PR-1", summary: HOSTILE, created_at: stamp })],
      ),
    );
    expect(document.querySelector("img")).toBeNull();
    expect(within(region("Tasks")).getByRole("link", { name: `T-1: ${HOSTILE}` })).toBeTruthy();
    expect(within(region("Inbox")).getByRole("link", { name: `PR-1: ${HOSTILE}` })).toBeTruthy();
    const times = Array.from(document.querySelectorAll(".home-view time"), (time) => [time.textContent, time.getAttribute("datetime")]);
    expect(times).toEqual([
      [stamp, stamp],
      [odd, odd],
      [stamp, stamp],
    ]);
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    const view = document.querySelector(".home-view");
    expect(view?.textContent).not.toMatch(/block/i);
    expect(view?.textContent).not.toMatch(/\bago\b|just now/);
  });
});

describe("nav, switcher and keys (AC-08)", () => {
  function navLinks(): HTMLElement[] {
    return within(screen.getByRole("navigation", { name: "Sections" })).getAllByRole("link");
  }

  it("lists seven entries, Overview first to the home, current only there", async () => {
    await openHome();
    expect(navLinks().map((link) => [link.textContent, link.getAttribute("href"), link.getAttribute("aria-current")])).toEqual([
      ["Overview", "#/alpha", "page"],
      ["Inbox", "#/alpha/inbox", null],
      ["Tasks", "#/alpha/tasks", null],
      ["Spec tree", "#/alpha/tree", null],
      ["Graph", "#/alpha/graph", null],
      ["Health", "#/alpha/health", null],
      ["Questions", "#/alpha/questions", null],
    ]);
  });

  it("switches the home to the other project's home", async () => {
    await openHome();
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
    const link = within(region("Tasks")).getByRole("link", { name: "T-0002: Plan to read" });
    link.focus();
    fireEvent.keyDown(link, { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(Array.from(dialog.querySelectorAll(".shortcut"), (row) => row.textContent)).toEqual([
      "Cmd-K or Ctrl-KJump to a section, task, proposal, node or project",
      "?Show this list",
      "EscClose a dialog",
    ]);
    expect(within(dialog).queryByText("Next proposal")).toBeNull();
  });

  it("opens nothing for ? with Ctrl, Alt or Cmd held", async () => {
    await openHome();
    const link = within(region("Tasks")).getByRole("link", { name: "Open Tasks" });
    expect(fireEvent.keyDown(link, { key: "?", ctrlKey: true })).toBe(true);
    expect(fireEvent.keyDown(link, { key: "?", altKey: true })).toBe(true);
    expect(fireEvent.keyDown(link, { key: "?", metaKey: true })).toBe(true);
    await settle();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it.each([
    ["#/alpha", "a home link", ".home-row a"],
    ["#/alpha/inbox", "the queue", '[role="option"][aria-selected="true"]'],
    ["#/alpha/tasks", "the task list", '[role="option"][tabindex="0"]'],
    ["#/alpha/tree", "the tree", '[role="treeitem"][tabindex="0"]'],
    ["#/alpha/graph", "the view's heading", "main h1"],
  ])("lists the chord on %s (? on %s)", async (hash, _where, selector) => {
    renderApp(homeClient(SOME_TASKS, QUEUE), hash);
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
    expect(within(dialog).getByText("Jump to a section, task, proposal, node or project")).toBeTruthy();
    // The intro's "without Ctrl, Alt or Cmd" names the chord as its exception.
    expect(dialog.querySelector(".dialog-text")?.textContent).toContain(
      "without Ctrl, Alt or Cmd; Cmd-K or Ctrl-K excepted: it opens Jump to from anywhere, a text field included.",
    );
  });
});

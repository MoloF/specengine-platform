import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { WorkspaceDocument } from "../api/types";
import { App } from "../app/App";
import { SECTIONS, sectionLabel } from "../app/sections";
import { argsOf } from "../test/stubClient";
import { en } from "../i18n/en";
import { enT } from "../test/text";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";
import { HARBOR_NOTES, LEDGER_NOTE, workspacesOf } from "./workspaces";

// docs/features/workspaces.md "Acceptance criteria", the UI's over the real mock, as `pnpm dev` shows
// it: harbor-sim serves galley and foundry and the modules berths and waters, ledger-api the implicit
// workspace, `config-notes` the notes, `error` a failed read. Counted reads spy on the mock's methods.
// WS-1 (the screen), WS-2, WS-3, WS-5, WS-9, WS-11, WS-13, WS-14; WS-15: workspaceText.smoke.test.tsx.

/**
 * Subtasks' rows once the children read lands: the part's one unnamed list, beside Relies on and
 * Cancelled subtasks (docs/features/handoff-progress.md "Acceptance criteria", HP-5, HP-6).
 */
async function subtaskRows(part: HTMLElement): Promise<HTMLElement[]> {
  return waitFor(() => {
    const rows = within(part)
      .getAllByRole("list")
      .find((list) => list.getAttribute("aria-labelledby") === null);
    if (rows === undefined) {
      throw new Error("no subtask rows yet");
    }
    return within(rows).getAllByRole("listitem");
  });
}

function renderMock(scenario: Scenario, hash: string, prepare: (client: MockClient) => void = () => undefined) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  // The counted reads, spied before the page asks anything.
  const spies = {
    getWorkspaces: vi.spyOn(client, "getWorkspaces"),
    getTree: vi.spyOn(client, "getTree"),
    getFlow: vi.spyOn(client, "getFlow"),
    getTrace: vi.spyOn(client, "getTrace"),
    getCards: vi.spyOn(client, "getCards"),
  };
  prepare(client);
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, spies };
}

/** A reload: the page goes, the next one reads the same storage. */
function reload(scenario: Scenario, hash: string, prepare?: (client: MockClient) => void) {
  cleanup();
  return renderMock(scenario, hash, prepare);
}

function nav(): HTMLElement {
  return screen.getByRole("navigation", { name: "Sections" });
}

function navTexts(): string[] {
  return within(nav())
    .queryAllByRole("link")
    .map((link) => link.textContent);
}

async function navIs(labels: readonly string[]) {
  await waitFor(() => {
    expect(navTexts()).toEqual(labels);
  });
}

function current(): string[] {
  return within(nav())
    .queryAllByRole("link")
    .filter((link) => link.getAttribute("aria-current") === "page")
    .map((link) => link.textContent);
}

function switcher(): HTMLSelectElement {
  return screen.getByRole<HTMLSelectElement>("combobox", { name: en.workspace.workspaceLabel });
}

function bar(): HTMLElement | null {
  return screen.queryByRole("group", { name: en.workspace.modulesLabel });
}

function pressed(): string[] {
  return within(bar() ?? document.body)
    .getAllByRole("button")
    .filter((button) => button.getAttribute("aria-pressed") === "true")
    .map((button) => button.textContent);
}

/** The module Flow's places pane names current: its bar's items are links to places (docs/features/writer-studio.md D4). */
function currentModule(): string[] {
  return within(bar() ?? document.body)
    .getAllByRole("link")
    .filter((link) => link.getAttribute("aria-current") === "true")
    .map((link) => link.textContent);
}

/** The Flow's Top reads, in order: its `depth=0` reads. */
function topReads(spy: { mock: { calls: readonly (readonly unknown[])[] } }): unknown[] {
  return argsOf(spy)
    .map((args) => args[1])
    .filter((options) => typeof options === "object" && options !== null && "depth" in options && options.depth === 0);
}

async function hashIs(hash: string) {
  await waitFor(() => {
    expect(window.location.hash).toBe(hash);
  });
}

/** Moves the hash as a followed link does, the view following it. */
async function go(hash: string) {
  await act(async () => {
    window.location.hash = hash;
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

const GALLEY_NAV = ["Overview", "Board", "Flow", "Spec tree", "Inbox"];
const FOUNDRY_NAV = ["Overview", "Board", "Health", "Graph", "Spec tree"];
const IMPLICIT_NAV = ["Overview", "Inbox", "Board", "Roadmap", "Flow", "Spec tree", "Graph", "Health", "Questions"];
const EVERY_NAV = ["Overview", ...SECTIONS.map((section) => sectionLabel(section.id, enT))];

describe("the implicit workspace on the screen (WS-1)", () => {
  it("ledger-api: Overview, then every section but Tasks; no Workspace control, no module bar on Tree or Flow", async () => {
    renderMock("normal", "#/ledger-api/tree");
    await navIs(IMPLICIT_NAV);
    expect(screen.queryByRole("combobox", { name: en.workspace.workspaceLabel })).toBeNull();
    await screen.findByRole("tree", { name: "Spec tree" });
    expect(bar()).toBeNull();
    await go("#/ledger-api/flow");
    await screen.findByRole("heading", { level: 1, name: "Flow" });
    expect(bar()).toBeNull();
    expect(current()).toEqual(["Flow"]);
  });

  it("the mock serves the spec's implicit document for ledger-api, byte for byte", () => {
    const implicit: WorkspaceDocument = {
      source: "implicit",
      notes: [],
      workspaces: [{ name: "all", title: "All sections", landing: "overview", sections: ["inbox", "board", "roadmap", "flow", "tree", "graph", "health", "questions"], flows: null, disciplines: [], module: null }],
      modules: [],
    };
    expect(JSON.stringify(workspacesOf("ledger-api", "normal"))).toBe(JSON.stringify(implicit));
  });
});

describe("Tasks is still a route (WS-2)", () => {
  it("#/ledger-api/tasks/T-0031 renders the task; T-0107, harbor-sim's, the Tasks view's no-task reason; no redirect", async () => {
    renderMock("normal", "#/ledger-api/tasks/T-0031");
    expect(await screen.findByRole("heading", { level: 1, name: /^T-0031/ })).toBeTruthy();
    await navIs(IMPLICIT_NAV);
    expect(current()).toEqual([]);
    expect(window.location.hash).toBe("#/ledger-api/tasks/T-0031");
    await go("#/ledger-api/tasks/T-0107");
    expect(await screen.findByRole("heading", { level: 2, name: "No such agent task" })).toBeTruthy();
    expect(screen.getByText("no task T-0107 in this repository")).toBeTruthy();
    expect(window.location.hash).toBe("#/ledger-api/tasks/T-0107");
  });

  it("the palette, typed Tasks, lists the section; the Inbox's link to the task renders it", async () => {
    renderMock("normal", "#/ledger-api/inbox/PR-0007");
    const card = await screen.findByRole("article");
    const task = await within(card).findByRole("link", { name: "T-0031" });
    expect(task.getAttribute("href")).toBe("#/ledger-api/tasks/T-0031");
    fireEvent.click(task);
    await go("#/ledger-api/tasks/T-0031");
    expect(await screen.findByRole("heading", { level: 1, name: /^T-0031/ })).toBeTruthy();
    fireEvent.keyDown(document.body, { key: "k", ctrlKey: true });
    const dialog = await screen.findByRole("dialog", { name: "Jump to" });
    fireEvent.change(within(dialog).getByRole("combobox"), { target: { value: "Agent tasks" } });
    const sections = await waitFor(() => {
      const found = within(dialog)
        .getAllByRole("group")
        .find((group) => document.getElementById(group.getAttribute("aria-labelledby") ?? "")?.textContent.startsWith("Sections") === true);
      expect(found).toBeDefined();
      return found as HTMLElement;
    });
    expect(within(sections).getByRole("option", { name: "Agent tasks" })).toBeTruthy();
  });
});

describe("choosing a workspace (WS-3)", () => {
  it("a first visit chooses galley; Foundry is stored per project, its nav shown, its landing pushed as one entry; a reload keeps it", async () => {
    renderMock("normal", "#/harbor-sim/board");
    await navIs(GALLEY_NAV);
    expect(switcher().value).toBe("galley");
    expect(within(switcher()).getAllByRole("option").map((option) => option.textContent)).toEqual(["Galley", "Foundry"]);
    // Named Workspace, its hint the description: a lens, no right (D6).
    expect(document.getElementById(switcher().getAttribute("aria-describedby") ?? "")?.textContent).toBe(en.workspace.workspaceHint);
    expect(switcher().title).toBe(en.workspace.workspaceHint);
    const before = window.history.length;
    fireEvent.change(switcher(), { target: { value: "foundry" } });
    expect(window.localStorage.getItem("specengine.workspace.harbor-sim")).toBe("foundry");
    expect(window.localStorage.getItem("specengine.workspace.ledger-api")).toBeNull();
    await hashIs("#/harbor-sim");
    expect(window.history.length).toBe(before + 1);
    await navIs(FOUNDRY_NAV);
    expect(await screen.findByRole("heading", { level: 1, name: "Overview" })).toBeTruthy();

    reload("normal", "#/harbor-sim/board");
    await navIs(FOUNDRY_NAV);
    expect(switcher().value).toBe("foundry");
  });

  it("with setItem throwing, Foundry holds for the page until a reload, which reads galley", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
    });
    renderMock("normal", "#/harbor-sim/board");
    await navIs(GALLEY_NAV);
    fireEvent.change(switcher(), { target: { value: "foundry" } });
    await navIs(FOUNDRY_NAV);
    await go("#/harbor-sim/tree");
    await navIs(FOUNDRY_NAV);
    expect(switcher().value).toBe("foundry");
    reload("normal", "#/harbor-sim/board");
    await navIs(GALLEY_NAV);
    expect(switcher().value).toBe("galley");
  });

  it("is reachable by keyboard: the select sits in the header's tab order, after the project's", async () => {
    renderMock("normal", "#/harbor-sim/board");
    await navIs(GALLEY_NAV);
    const project = screen.getByLabelText("Project");
    expect(switcher().tabIndex).toBe(0);
    expect(project.compareDocumentPosition(switcher()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(switcher().closest("header")).toBe(project.closest("header"));
  });
});

describe("a lens hides no route (WS-5)", () => {
  it("galley lacks Health: #/harbor-sim/health renders it, no redirect, no entry, no item current; the palette lists the nine", async () => {
    const before = window.history.length;
    renderMock("normal", "#/harbor-sim/health");
    expect(await screen.findByRole("heading", { level: 1, name: "Health" })).toBeTruthy();
    await navIs(GALLEY_NAV);
    expect(current()).toEqual([]);
    expect(window.location.hash).toBe("#/harbor-sim/health");
    expect(window.history.length).toBe(before);
    fireEvent.keyDown(document.body, { key: "k", ctrlKey: true });
    const dialog = await screen.findByRole("dialog", { name: "Jump to" });
    const group = within(dialog)
      .getAllByRole("group")
      .find((element) => document.getElementById(element.getAttribute("aria-labelledby") ?? "")?.textContent.startsWith("Sections") === true);
    expect(within(group ?? dialog).getAllByRole("option").map((option) => option.textContent)).toEqual(EVERY_NAV);
  });
});

describe("modules root the Spec tree and the Flow (WS-9)", () => {
  it("galley roots both at DOM-BERTHS; All modules unroots and stores the empty string; waters roots both at DOM-WATER, kept on a reload; Foundry removes the key", async () => {
    let { spies } = renderMock("normal", "#/harbor-sim/tree");
    await screen.findByRole("tree", { name: "Spec tree" });
    // The first tree read is rooted: none went before the module was known.
    expect(argsOf(spies.getTree)[0]).toEqual(["harbor-sim", { root: "DOM-BERTHS" }]);
    expect(bar()).not.toBeNull();
    expect(within(bar() ?? document.body).getAllByRole("button").map((button) => button.textContent)).toEqual(["All modules", "Berths", "Harbour waters"]);
    expect(pressed()).toEqual(["Berths"]);
    await go("#/harbor-sim/flow");
    await screen.findByRole("heading", { level: 1, name: "Flow" });
    // The module's root is Flow's lens root: its scope read, no top (docs/features/writer-studio.md D3).
    await waitFor(() => {
      expect(argsOf(spies.getFlow).map((args) => args[1])).toEqual([{ root: "DOM-BERTHS", depth: 1, in_flow: true, trace: true }]);
    });
    expect(currentModule()).toEqual(["Berths"]);

    fireEvent.click(within(bar() ?? document.body).getByRole("link", { name: "All modules" }));
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBe("");
    await hashIs("#/harbor-sim/flow?module=");
    expect(currentModule()).toEqual(["All modules"]);
    await waitFor(() => {
      expect(topReads(spies.getFlow).at(-1)).toEqual({ depth: 0, in_flow: true, trace: true });
    });
    await go("#/harbor-sim/tree");
    await waitFor(() => {
      expect(argsOf(spies.getTree).at(-1)).toEqual(["harbor-sim"]);
    });

    fireEvent.click(within(bar() ?? document.body).getByRole("button", { name: "Harbour waters" }));
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBe("waters");
    await waitFor(() => {
      expect(argsOf(spies.getTree).at(-1)).toEqual(["harbor-sim", { root: "DOM-WATER" }]);
    });
    ({ spies } = reload("normal", "#/harbor-sim/flow"));
    await screen.findByRole("heading", { level: 1, name: "Flow" });
    await waitFor(() => {
      expect(argsOf(spies.getFlow).map((args) => args[1])).toEqual([{ root: "DOM-WATER", depth: 1, in_flow: true, trace: true }]);
    });
    expect(currentModule()).toEqual(["Harbour waters"]);

    fireEvent.change(switcher(), { target: { value: "foundry" } });
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBeNull();
    await hashIs("#/harbor-sim");
    await go("#/harbor-sim/tree");
    await screen.findByRole("tree", { name: "Spec tree" });
    expect(argsOf(spies.getTree).at(-1)).toEqual(["harbor-sim"]);
    expect(pressed()).toEqual(["All modules"]);
    await go("#/harbor-sim/flow");
    await waitFor(() => {
      expect(topReads(spies.getFlow).at(-1)).toEqual({ depth: 0, in_flow: true, trace: true });
    });
  });

  it("roots Flow's reads at the module: its scope read at depth 1, its lanes under it; a root's tree footer goes back to the whole tree", async () => {
    window.localStorage.setItem("specengine.module.harbor-sim", "waters");
    const { spies } = renderMock("normal", "#/harbor-sim/flow");
    await screen.findByRole("heading", { level: 1, name: "Flow" });
    await waitFor(() => {
      expect(argsOf(spies.getTrace)).toContainEqual(["harbor-sim", { ref: "DOM-WATER", summary: true }]);
    });
    expect(topReads(spies.getFlow)).toEqual([]);
    for (const options of argsOf(spies.getFlow).map((args) => args[1])) {
      expect(options).toEqual({ root: expect.any(String) as string, depth: 1, in_flow: true, trace: true });
    }
    await go("#/harbor-sim/tree");
    await screen.findByRole("tree", { name: "Spec tree" });
    expect(screen.getByText("Rooted at").textContent).toBe("Rooted at DOM-WATER");
    fireEvent.click(screen.getByRole("button", { name: "Whole tree" }));
    await waitFor(() => {
      expect(argsOf(spies.getTree).at(-1)).toEqual(["harbor-sim"]);
    });
    // The view's root alone: the stored choice and the bar stay.
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBe("waters");
    expect(pressed()).toEqual(["Harbour waters"]);
  });

  it("shows no bar on the Board, the Inbox or Health, and the Board's read takes no module", async () => {
    const { spies } = renderMock("normal", "#/harbor-sim/board");
    await screen.findByText(enT("workspace.flowsShown", { shown: 1, count: 2 }));
    expect(bar()).toBeNull();
    expect(argsOf(spies.getCards)).toEqual([["harbor-sim", { marks: true }]]);
    await go("#/harbor-sim/inbox");
    await screen.findByRole("listbox");
    expect(bar()).toBeNull();
    await go("#/harbor-sim/health");
    await screen.findByRole("heading", { level: 1, name: "Health" });
    expect(bar()).toBeNull();
  });

  it("a root naming nothing is the read's own reason; the bar stays to choose another", async () => {
    const { spies } = renderMock("normal", "#/harbor-sim/tree", (client) => {
      const served = workspacesOf("harbor-sim", "normal");
      vi.spyOn(client, "getWorkspaces").mockResolvedValue({ ...served, modules: [...served.modules, { name: "reef", title: "Reef", root: "DOM-REEF", kinds: null }] });
    });
    await screen.findByRole("tree", { name: "Spec tree" });
    fireEvent.click(within(bar() ?? document.body).getByRole("button", { name: "Reef" }));
    expect(await screen.findByText("No tree from that root:")).toBeTruthy();
    expect(argsOf(spies.getTree).at(-1)).toEqual(["harbor-sim", { root: "DOM-REEF" }]);
    fireEvent.click(within(bar() ?? document.body).getByRole("button", { name: "Berths" }));
    await screen.findByRole("tree", { name: "Spec tree" });
    expect(pressed()).toEqual(["Berths"]);
  });
});

describe("the landing (WS-11)", () => {
  it("#/ is replaced, no entry of its own, by galley's landing once the read answers", async () => {
    const before = window.history.length;
    renderMock("normal", "#/");
    await hashIs("#/harbor-sim/flow");
    expect(window.history.length).toBe(before);
    expect(await screen.findByRole("heading", { level: 1, name: "Flow" })).toBeTruthy();
  });

  it("with the read failing, #/ goes to the first project's home", async () => {
    renderMock("normal", "#/", (client) => {
      vi.spyOn(client, "getWorkspaces").mockRejectedValue(new ClientError({ status: 503, message: "specengine.toml:3: unknown field `slugg`" }));
    });
    await hashIs("#/harbor-sim");
    expect(await screen.findByRole("heading", { level: 1, name: "Overview" })).toBeTruthy();
    await navIs(EVERY_NAV);
  });

  it("a typed #/harbor-sim renders the Overview, never redirected", async () => {
    const { spies } = renderMock("normal", "#/harbor-sim");
    expect(await screen.findByRole("heading", { level: 1, name: "Overview" })).toBeTruthy();
    await navIs(GALLEY_NAV);
    expect(spies.getWorkspaces).toHaveBeenCalledTimes(1);
    expect(window.location.hash).toBe("#/harbor-sim");
    expect(current()).toEqual(["Overview"]);
  });
});

describe("notes and failures on the screen (WS-13)", () => {
  it("config-notes: Config notes (2) beside the switcher lists both notes verbatim, in served order; Esc closes it, focus back", async () => {
    renderMock("config-notes", "#/harbor-sim/board");
    const button = await screen.findByRole("button", { name: enT("workspace.notesButton", { n: "2" }) });
    expect(switcher().value).toBe("galley");
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByText(HARBOR_NOTES[0] ?? "")).toBeNull();
    fireEvent.click(button);
    expect(button.getAttribute("aria-expanded")).toBe("true");
    const list = screen.getByRole("list", { name: en.workspace.notesTitle });
    expect(within(list).getAllByRole("listitem").map((item) => item.textContent)).toEqual([...HARBOR_NOTES]);
    expect(document.getElementById(button.getAttribute("aria-controls") ?? "")?.contains(list)).toBe(true);
    fireEvent.keyDown(list, { key: "Escape" });
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(button);
    // A note is never an error page or a blank nav.
    await navIs(GALLEY_NAV);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("config-notes: ledger-api's one workspace, the note in the switcher's place", async () => {
    renderMock("config-notes", "#/ledger-api/inbox");
    fireEvent.click(await screen.findByRole("button", { name: enT("workspace.notesButton", { n: "1" }) }));
    expect(within(screen.getByRole("list", { name: en.workspace.notesTitle })).getByRole("listitem").textContent).toBe(LEDGER_NOTE);
    expect(screen.queryByRole("combobox", { name: en.workspace.workspaceLabel })).toBeNull();
    await navIs(IMPLICIT_NAV);
  });

  it("error: the nine sections, the daemon's message verbatim and Retry in the disclosure; Retry reads again", async () => {
    const { spies } = renderMock("error", "#/harbor-sim/inbox");
    await navIs(EVERY_NAV);
    fireEvent.click(await screen.findByRole("button", { name: enT("workspace.notesButton", { n: "1" }) }));
    const panel = document.getElementById(screen.getByRole("button", { name: enT("workspace.notesButton", { n: "1" }) }).getAttribute("aria-controls") ?? "");
    expect(panel).not.toBeNull();
    expect(within(panel as HTMLElement).getByText(en.workspace.readFailed)).toBeTruthy();
    expect(within(panel as HTMLElement).getByText("spec index unavailable: the database is locked by another process (mock scenario: error)")).toBeTruthy();
    // Not spoken: the nav already shows every section.
    expect(within(panel as HTMLElement).queryByRole("alert")).toBeNull();
    expect(spies.getWorkspaces).toHaveBeenCalledTimes(1);
    fireEvent.click(within(panel as HTMLElement).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(spies.getWorkspaces).toHaveBeenCalledTimes(2);
    });
    expect(screen.queryByRole("group", { name: en.workspace.modulesLabel })).toBeNull();
  });

  it("pending: the nav is a skeleton, aria-busy, never empty and never the nine", async () => {
    renderMock("normal", "#/harbor-sim/inbox", (client) => {
      vi.spyOn(client, "getWorkspaces").mockReturnValue(new Promise<WorkspaceDocument>(() => undefined));
    });
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    expect(nav().getAttribute("aria-busy")).toBe("true");
    expect(within(nav()).getByRole("status", { name: en.workspace.navPending })).toBeTruthy();
    expect(navTexts()).toEqual([]);
  });

  it("pending: the Spec tree waits for its root, the Flow for its, both busy, neither reads", async () => {
    const { spies } = renderMock("normal", "#/harbor-sim/tree", (client) => {
      vi.spyOn(client, "getWorkspaces").mockReturnValue(new Promise<WorkspaceDocument>(() => undefined));
    });
    expect(await screen.findByRole("status", { name: "Loading the spec tree of harbor-sim" })).toBeTruthy();
    await go("#/harbor-sim/flow");
    expect(await screen.findByRole("status", { name: "Reading the flow of harbor-sim" })).toBeTruthy();
    expect(document.querySelector(".flow-studio")?.getAttribute("aria-busy")).toBe("true");
    expect(spies.getTree).not.toHaveBeenCalled();
    expect(topReads(spies.getFlow)).toEqual([]);
  });
});

describe("the Board's flows (WS-14)", () => {
  it("galley: no tab and the line; Show all flows gives both tabs and turns; Foundry resets it", async () => {
    renderMock("normal", "#/harbor-sim/board");
    expect(await screen.findByText("Showing 1 of 2 flows.")).toBeTruthy();
    expect(screen.queryAllByRole("tab")).toEqual([]);
    await waitFor(() => {
      expect(document.querySelector('[data-card="C-0001"]')).not.toBeNull();
    });
    expect(document.querySelector('[data-card="C-0008"]')).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show all flows" }));
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Copy desk", "plates"]);
    expect(screen.queryByText("Showing 1 of 2 flows.")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show this department's flows" }));
    expect(screen.queryAllByRole("tab")).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: "Show all flows" }));
    expect(screen.getAllByRole("tab")).toHaveLength(2);

    fireEvent.change(switcher(), { target: { value: "foundry" } });
    await hashIs("#/harbor-sim");
    await go("#/harbor-sim/board");
    expect(await screen.findByText("Showing 1 of 2 flows.")).toBeTruthy();
    expect(screen.queryAllByRole("tab")).toEqual([]);
    await waitFor(() => {
      expect(document.querySelector('[data-card="C-0008"]')).not.toBeNull();
    });
    expect(screen.getByRole("button", { name: "Show all flows" })).toBeTruthy();
  });

  it("a card the route names in a flow the workspace leaves out shows every flow, its own selected", async () => {
    renderMock("normal", "#/harbor-sim/board/C-0008");
    await waitFor(() => {
      expect(screen.getAllByRole("tab").map((tab) => [tab.textContent, tab.getAttribute("aria-selected")])).toEqual([
        ["Copy desk", "false"],
        ["plates", "true"],
      ]);
    });
    expect(screen.getByRole("button", { name: "Show this department's flows" })).toBeTruthy();
  });

  it("flows null: every tab and no line", async () => {
    renderMock("normal", "#/harbor-sim/board", (client) => {
      vi.spyOn(client, "getWorkspaces").mockResolvedValue(workspacesOf("ledger-api", "normal"));
    });
    await waitFor(() => {
      expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Copy desk", "plates"]);
    });
    expect(screen.queryByText(/flows\.$/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Show all flows" })).toBeNull();
  });
});

// docs/features/departments.md "Acceptance criteria" over the real mock: harbor-sim's departments
// Galley and Foundry, its rooted modules Berths and Harbour waters, its direction Mechanics by the
// kind `mechanic`, and its flows `copy` titled "Copy desk" and `plates` untitled.

function directionsBar(): HTMLElement | null {
  return screen.queryByRole("group", { name: en.workspace.directionsLabel });
}

/** Every pressed choice of the Spec tree's bar, both groups. */
function pressedAll(): string[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".module-bar button"))
    .filter((button) => button.getAttribute("aria-pressed") === "true")
    .map((button) => button.textContent);
}

function treeItems(): HTMLElement[] {
  return within(screen.getByRole("tree", { name: "Spec tree" })).getAllByRole("treeitem");
}

function treeNames(): string[] {
  return treeItems().map((item) => item.querySelector(".tree-row-name")?.textContent ?? "");
}

describe("the department's Board (DP-10 of departments)", () => {
  it("names the switcher Department; Galley's one flow, its line; every flow shown: tabs by title, Copy desk and plates, the board named by its title", async () => {
    renderMock("normal", "#/harbor-sim/board");
    expect(en.workspace.workspaceLabel).toBe("Department");
    expect(await screen.findByRole("combobox", { name: "Department" })).toBe(switcher());
    expect(await screen.findByText("Showing 1 of 2 flows.")).toBeTruthy();
    // One flow shown draws no tab (WS-14): the board is the Board.
    expect(screen.queryAllByRole("tab")).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: "Show all flows" }));
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Copy desk", "plates"]);
    expect(screen.getByRole("group", { name: "Board Copy desk" })).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "plates" }));
    expect(await screen.findByRole("group", { name: "Board plates" })).toBeTruthy();
  });

  it("Foundry's own flow is plates; a routed card of Galley's flow reaches it by Show all flows, every tab by title", async () => {
    renderMock("normal", "#/harbor-sim/board");
    await screen.findByText("Showing 1 of 2 flows.");
    fireEvent.change(switcher(), { target: { value: "foundry" } });
    await hashIs("#/harbor-sim");
    await go("#/harbor-sim/board/C-0001");
    await waitFor(() => {
      expect(screen.getAllByRole("tab").map((tab) => [tab.textContent, tab.getAttribute("aria-selected")])).toEqual([
        ["Copy desk", "true"],
        ["plates", "false"],
      ]);
    });
    expect(screen.getByRole("button", { name: "Show this department's flows" })).toBeTruthy();
  });

  it("a Parent lane head and the side Flow row show Copy desk; the Subtasks rows each child's flow title", async () => {
    window.localStorage.setItem("specengine.board.lanes.harbor-sim", "parent");
    renderMock("normal", "#/harbor-sim/board");
    fireEvent.click(await screen.findByRole("button", { name: "Show all flows" }));
    fireEvent.click(screen.getByRole("tab", { name: "plates" }));
    await waitFor(() => {
      expect(document.querySelector('[data-card="C-0013"]')).not.toBeNull();
    });
    // C-0013 stands in plates; its parent C-0001 in copy: the head names copy by its title.
    const head = Array.from(document.querySelectorAll<HTMLElement>(".board-lane-head")).find((candidate) => within(candidate).queryByRole("link", { name: "C-0001" }) !== null);
    expect(head?.querySelector(".board-lane-status bdi")?.textContent).toBe("Copy desk");
    expect(head?.textContent).not.toContain("copy/");
    await go("#/harbor-sim/board/C-0001");
    const modal = await screen.findByRole("dialog", { name: "Write the tide table foreword" });
    const flow = Array.from(modal.querySelectorAll("dt")).find((term) => term.textContent === "Flow");
    expect(flow?.nextElementSibling?.textContent).toBe("Copy desk");
    const subtasks = await subtaskRows(await within(modal).findByRole("region", { name: "Subtasks" }));
    expect(subtasks.map((item) => item.querySelector(".board-handoff-title + bdi")?.textContent)).toEqual(["Copy desk", "plates", "plates"]);
  });

  it("the hand-off's polite line and the document view's Tasks block keep the raw <flow>/<column> pair", async () => {
    window.localStorage.setItem("specengine.board.as", "Ivo Brandt");
    renderMock("normal", "#/harbor-sim/board/C-0001");
    const modal = await screen.findByRole("dialog", { name: "Write the tide table foreword" });
    fireEvent.click(await within(modal).findByRole("button", { name: "Hand off" }));
    const dialog = await screen.findByRole("dialog", { name: "Hand off C-0001" });
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Department" }), { target: { value: "galley" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toMatch(/^C-\d+ handed off to copy\/draft$/);
    });
    await go("#/harbor-sim/board?doc=MEC-TIDE-TABLES");
    const doc = await screen.findByRole("dialog", { name: /tide/i });
    const block = await within(doc).findByRole("region", { name: "Tasks" });
    await waitFor(() => {
      expect(within(block).getAllByText(/^copy\/[a-z-]+$/).length).toBeGreaterThan(0);
    });
    expect(within(block).queryByText(/Copy desk/)).toBeNull();
  });
});

describe("directions in the Spec tree (DP-11 of departments)", () => {
  it("holds group Modules (All modules, Berths, Harbour waters) and group Directions (Mechanics), one pressed across both", async () => {
    renderMock("normal", "#/harbor-sim/tree");
    await screen.findByRole("tree", { name: "Spec tree" });
    expect(within(bar() ?? document.body).getAllByRole("button").map((button) => button.textContent)).toEqual(["All modules", "Berths", "Harbour waters"]);
    expect(within(directionsBar() ?? document.body).getAllByRole("button").map((button) => button.textContent)).toEqual(["Mechanics"]);
    expect(pressedAll()).toEqual(["Berths"]);
  });

  it("Mechanics stores its name and reads getTree by kinds, no root; every row one level, none expandable, in served order; a reload keeps it (M: rows built on the served depths; a direction's null root passed as root)", async () => {
    let { client, spies } = renderMock("normal", "#/harbor-sim/tree");
    await screen.findByRole("tree", { name: "Spec tree" });
    fireEvent.click(within(directionsBar() ?? document.body).getByRole("button", { name: "Mechanics" }));
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBe("mechanics");
    await waitFor(() => {
      expect(argsOf(spies.getTree).at(-1)).toEqual(["harbor-sim", { kinds: ["mechanic"] }]);
    });
    expect(argsOf(spies.getTree).some((args) => typeof args[1] === "object" && args[1] !== null && "root" in args[1] && args[1].root === null)).toBe(false);
    const served = (await client.getTree("harbor-sim", { kinds: ["mechanic"] })).nodes;
    // The kinds read keeps the whole tree's depths: some served row is deeper than the first level.
    expect(served.some((node) => node.depth > 0)).toBe(true);
    await waitFor(() => {
      expect(treeNames()).toEqual(served.map((node) => node.id ?? node.path));
    });
    expect(treeItems().map((item) => item.getAttribute("aria-level"))).toEqual(served.map(() => "1"));
    expect(treeItems().filter((item) => item.hasAttribute("aria-expanded"))).toEqual([]);
    expect(pressedAll()).toEqual(["Mechanics"]);

    ({ client, spies } = reload("normal", "#/harbor-sim/tree"));
    await screen.findByRole("tree", { name: "Spec tree" });
    expect(argsOf(spies.getTree)[0]).toEqual(["harbor-sim", { kinds: ["mechanic"] }]);
    expect(pressedAll()).toEqual(["Mechanics"]);
  });

  it("Show as root on a row reads from it; Whole tree goes back to the list; All modules reads the whole tree", async () => {
    window.localStorage.setItem("specengine.module.harbor-sim", "mechanics");
    const { spies } = renderMock("normal", "#/harbor-sim/tree", (client) => {
      // A cut list, so the footer offers Show as root on its first row.
      vi.spyOn(client, "getTree").mockImplementation(async (project, options) => {
        const answer = await MockClient.prototype.getTree.call(client, project, options);
        return options?.kinds === undefined ? answer : { ...answer, truncated: true };
      });
    });
    await waitFor(() => {
      expect(treeItems().length).toBeGreaterThan(1);
    });
    const first = treeNames()[0] ?? "";
    fireEvent.click(screen.getByRole("button", { name: `Show ${first} as root` }));
    await waitFor(() => {
      expect(argsOf(spies.getTree).at(-1)).toEqual(["harbor-sim", { root: first }]);
    });
    expect((await screen.findByText("Rooted at")).textContent).toBe(`Rooted at ${first}`);
    fireEvent.click(screen.getByRole("button", { name: "Whole tree" }));
    await waitFor(() => {
      expect(screen.queryByText("Rooted at")).toBeNull();
    });
    await waitFor(() => {
      expect(treeItems().every((item) => item.getAttribute("aria-level") === "1")).toBe(true);
    });
    expect(pressedAll()).toEqual(["Mechanics"]);
    fireEvent.click(within(bar() ?? document.body).getByRole("button", { name: "All modules" }));
    await waitFor(() => {
      expect(argsOf(spies.getTree).at(-1)).toEqual(["harbor-sim"]);
    });
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBe("");
  });

  it("with directions only, Modules holds All modules alone; with rooted modules only, no Directions group", async () => {
    renderMock("normal", "#/harbor-sim/tree", (client) => {
      const served = workspacesOf("harbor-sim", "normal");
      vi.spyOn(client, "getWorkspaces").mockResolvedValue({ ...served, workspaces: served.workspaces.map((workspace) => ({ ...workspace, module: null })), modules: served.modules.filter((module) => module.root === null) });
    });
    await screen.findByRole("tree", { name: "Spec tree" });
    await waitFor(() => {
      expect(within(bar() ?? document.body).getAllByRole("button").map((button) => button.textContent)).toEqual(["All modules"]);
    });
    expect(within(directionsBar() ?? document.body).getAllByRole("button").map((button) => button.textContent)).toEqual(["Mechanics"]);
    cleanup();
    renderMock("normal", "#/harbor-sim/tree", (client) => {
      const served = workspacesOf("harbor-sim", "normal");
      vi.spyOn(client, "getWorkspaces").mockResolvedValue({ ...served, modules: served.modules.filter((module) => module.root !== null) });
    });
    await screen.findByRole("tree", { name: "Spec tree" });
    await waitFor(() => {
      expect(within(bar() ?? document.body).getAllByRole("button").map((button) => button.textContent)).toEqual(["All modules", "Berths", "Harbour waters"]);
    });
    expect(directionsBar()).toBeNull();
  });
});

describe("Flow takes rooted modules only (DP-12 of departments)", () => {
  it("the places pane lists Berths and Harbour waters, never Mechanics (M: the places pane listing every module)", async () => {
    renderMock("normal", "#/harbor-sim/flow");
    await screen.findByRole("heading", { level: 1, name: "Flow" });
    await waitFor(() => {
      expect(within(bar() ?? document.body).getAllByRole("link").map((link) => link.textContent)).toEqual(["All modules", "Berths", "Harbour waters"]);
    });
    expect(within(bar() ?? document.body).queryByRole("link", { name: "Mechanics" })).toBeNull();
  });

  it("stored mechanics: #/harbor-sim/flow reads as All modules, its Top, the key unchanged", async () => {
    window.localStorage.setItem("specengine.module.harbor-sim", "mechanics");
    const { spies } = renderMock("normal", "#/harbor-sim/flow");
    await screen.findByRole("heading", { level: 1, name: "Flow" });
    await waitFor(() => {
      expect(currentModule()).toEqual(["All modules"]);
    });
    await waitFor(() => {
      expect(topReads(spies.getFlow).at(-1)).toEqual({ depth: 0, in_flow: true, trace: true });
    });
    expect(argsOf(spies.getFlow).some((args) => typeof args[1] === "object" && args[1] !== null && "root" in args[1] && args[1].root === null)).toBe(false);
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBe("mechanics");
  });

  it("module=mechanics in the hash drops by one replace, as an unserved module does: no push, the place written with All modules", async () => {
    window.localStorage.setItem("specengine.module.harbor-sim", "mechanics");
    renderMock("normal", "#/harbor-sim/flow?module=mechanics");
    const before = window.history.length;
    await screen.findByRole("heading", { level: 1, name: "Flow" });
    await hashIs("#/harbor-sim/flow?module=");
    expect(window.history.length).toBe(before);
    expect(currentModule()).toEqual(["All modules"]);
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBe("mechanics");
  });
});

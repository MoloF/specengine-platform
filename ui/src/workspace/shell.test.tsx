import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import { HttpClient } from "../api/http";
import { aModule, aWorkspace, aWorkspaces, anImplicitWorkspaces } from "../test/builders";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { en } from "../i18n/en";
import { enT } from "../test/text";

// The shell's lens on a stub client (docs/features/workspaces.md "Rules and edge cases", "The UI"):
// one workspace, a workspace whose every section is unknown, titles of any script, a project of its
// own, a Retry that answers, and the daemon behind HttpClient before its route is built.

function nav(): HTMLElement {
  return screen.getByRole("navigation", { name: "Sections" });
}

async function navIs(labels: readonly string[]) {
  await waitFor(() => {
    expect(
      within(nav())
        .queryAllByRole("link")
        .map((link) => link.textContent),
    ).toEqual(labels);
  });
}

const NINE = ["Overview", "Inbox", "Tasks", "Board", "Roadmap", "Flow", "Spec tree", "Graph", "Health", "Questions"];

describe("the edge cases of the screen", () => {
  it("one workspace configured: no switcher, its sections and its landing apply", async () => {
    const client = stubClient();
    client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "w-solo", title: "Solo", landing: "graph", sections: ["graph", "inbox"] })]));
    renderApp(client, "#/");
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/graph");
    });
    await navIs(["Overview", "Graph", "Inbox"]);
    expect(screen.queryByRole("combobox", { name: en.workspace.workspaceLabel })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Config notes/ })).toBeNull();
  });

  it("a workspace whose every section is unknown: Overview alone and the notes, never the implicit list", async () => {
    const client = stubClient();
    const note = "specengine.toml:7: workspace `w-bare`: `stage` is no section (inbox, tasks, board, roadmap, flow, tree, graph, health, questions): left out";
    client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "w-bare", sections: [] })], [], { notes: [note] }));
    renderApp(client, "#/alpha/inbox");
    await navIs(["Overview"]);
    fireEvent.click(await screen.findByRole("button", { name: enT("workspace.notesButton", { n: "1" }) }));
    expect(screen.getByRole("list", { name: en.workspace.notesTitle }).textContent).toBe(note);
    // The section still renders at its hash.
    expect(await screen.findByRole("heading", { level: 1, name: "Inbox" })).toBeTruthy();
  });

  it("shows titles of any script whole: the options, the bar's buttons", async () => {
    const client = stubClient();
    // Escaped: a Cyrillic and a Greek title, and one mixing scripts (ADR-0024 covers the repository, not a project's config).
    const writers = "\u041f\u0438\u0441\u0430\u0442\u0435\u043b\u0438";
    const shore = "\u0391\u03ba\u03c4\u03ae shore";
    client.getWorkspaces.mockResolvedValue(
      aWorkspaces([aWorkspace({ name: "w-a", title: writers, module: "m-a" }), aWorkspace({ name: "w-b", title: "Leads" })], [aModule({ name: "m-a", title: shore, root: "DOC-1" })]),
    );
    renderApp(client, "#/alpha/tree");
    const select = await screen.findByRole<HTMLSelectElement>("combobox", { name: en.workspace.workspaceLabel });
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([writers, "Leads"]);
    const bar = await screen.findByRole("group", { name: en.workspace.modulesLabel });
    expect(within(bar).getByRole("button", { name: shore }).getAttribute("aria-pressed")).toBe("true");
    await waitFor(() => {
      expect(client.getTree.mock.calls[0]?.[1]).toEqual({ root: "DOC-1" });
    });
  });

  it("reads each project's own workspaces: the nav follows the project", async () => {
    const client = stubClient();
    client.getWorkspaces.mockImplementation((project) =>
      Promise.resolve(project === "beta" ? aWorkspaces([aWorkspace({ name: "w-beta", sections: ["health"] })]) : anImplicitWorkspaces()),
    );
    renderApp(client, "#/alpha/inbox");
    await navIs(["Overview", "Inbox", "Board", "Roadmap", "Flow", "Spec tree", "Graph", "Health", "Questions"]);
    const select = screen.getByLabelText("Project");
    await waitFor(() => {
      expect((select as HTMLSelectElement).disabled).toBe(false);
    });
    fireEvent.change(select, { target: { value: "beta" } });
    await navIs(["Overview", "Health"]);
    expect(client.getWorkspaces.mock.calls.map((call) => call[0])).toEqual(["alpha", "beta"]);
  });

  it("a Retry that answers takes its button away: focus goes to the nav, never the body", async () => {
    const client = stubClient();
    client.getWorkspaces.mockRejectedValueOnce(new ClientError({ status: 503, message: "specengine.toml:2: `[project] slug`: `Alpha` is not a slug" }));
    renderApp(client, "#/alpha/inbox");
    await navIs(NINE);
    fireEvent.click(await screen.findByRole("button", { name: enT("workspace.notesButton", { n: "1" }) }));
    const retry = screen.getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await navIs(["Overview", "Inbox", "Board", "Roadmap", "Flow", "Spec tree", "Graph", "Health", "Questions"]);
    expect(screen.queryByRole("button", { name: /^Config notes/ })).toBeNull();
    await waitFor(() => {
      expect(document.activeElement).toBe(within(nav()).getByRole("link", { name: "Overview" }));
    });
  });

  it("a read again failing with the last answer kept: the nav keeps its sections, the notes say so and hold the daemon's words", async () => {
    const client = stubClient();
    const message = "specengine.toml:2: `[project] slug`: `Alpha` is not a slug";
    client.getWorkspaces.mockResolvedValueOnce(aWorkspaces([aWorkspace({ name: "w-solo", sections: ["graph", "inbox"] })])).mockRejectedValue(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/inbox");
    await navIs(["Overview", "Graph", "Inbox"]);
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });
    const button = await screen.findByRole("button", { name: enT("workspace.notesButton", { n: "1" }) });
    await navIs(["Overview", "Graph", "Inbox"]);
    fireEvent.click(button);
    const panel = document.getElementById(button.getAttribute("aria-controls") ?? "") as HTMLElement;
    expect(within(panel).getByText(en.workspace.readFailedKept)).toBeTruthy();
    expect(within(panel).queryByText(en.workspace.readFailed)).toBeNull();
    expect(within(panel).getByText(message)).toBeTruthy();
  });

  it("reads the workspaces again on window focus, nothing else for it", async () => {
    const client = stubClient();
    renderApp(client, "#/alpha/inbox");
    await navIs(["Overview", "Inbox", "Board", "Roadmap", "Flow", "Spec tree", "Graph", "Health", "Questions"]);
    expect(client.getWorkspaces).toHaveBeenCalledTimes(1);
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(client.getWorkspaces).toHaveBeenCalledTimes(2);
    });
  });
});

describe("over the daemon (HttpClient)", () => {
  beforeEach(() => {
    stubEventSource();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /** One project, an empty queue, `/workspaces` answered by `answer`; anything else its 404. */
  function daemon(answer: () => Response) {
    return stubFetch((url) => {
      if (url === "/api/projects") {
        return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
      }
      if (url === "/api/identity") {
        return jsonAnswer(200, { login: null });
      }
      if (url === "/api/projects/alpha/inbox") {
        return jsonAnswer(200, { proposals: [], notes: [] });
      }
      if (url === "/api/projects/alpha/workspaces") {
        return answer();
      }
      return errorAnswer(404, `no route ${url}`);
    });
  }

  it("asks the route once, no query, and draws the served workspace", async () => {
    const fetchStub = daemon(() => jsonAnswer(200, anImplicitWorkspaces()));
    renderApp(new HttpClient(), "#/alpha/inbox");
    await navIs(["Overview", "Inbox", "Board", "Roadmap", "Flow", "Spec tree", "Graph", "Health", "Questions"]);
    expect(urlsOf(fetchStub).filter((url) => url.includes("/workspaces"))).toEqual(["/api/projects/alpha/workspaces"]);
  });

  it("an older daemon's 404: every section, its words in the notes, no alert; never Not built yet", async () => {
    daemon(() => errorAnswer(404, "no route GET /api/projects/alpha/workspaces: try tree, nodes, search"));
    renderApp(new HttpClient(), "#/alpha/inbox");
    await navIs(NINE);
    fireEvent.click(await screen.findByRole("button", { name: enT("workspace.notesButton", { n: "1" }) }));
    expect(screen.getByText("no route GET /api/projects/alpha/workspaces: try tree, nodes, search")).toBeTruthy();
    expect(screen.queryByText(/Not built yet/)).toBeNull();
    expect(screen.queryAllByRole("alert").filter((alert) => alert.textContent.includes("workspaces"))).toEqual([]);
  });
});

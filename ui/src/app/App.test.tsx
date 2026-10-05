import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Inbox, Project } from "../api/types";
import { takeConsoleCalls } from "../test/console";
import { aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { App } from "./App";

// AC-11, AC-16 and AC-17 of docs/features/ui-shell.md.

const UNBUILT = [
  ["tasks", "Tasks", "ui-tasks"],
  ["tree", "Spec tree", "ui-tree-node"],
  ["graph", "Graph", "ui-graph"],
  ["health", "Health", "ui-health-round"],
  ["questions", "Questions", "ui-health-round"],
] as const;

/** The app's header (several `header` elements count as banners to Testing Library). */
function appHeader(): HTMLElement {
  const header = document.querySelector<HTMLElement>("header.app-header");
  if (header === null) {
    throw new Error("no app header");
  }
  return header;
}

function navLinks(): HTMLElement[] {
  return within(screen.getByRole("navigation", { name: "Sections" })).getAllByRole("link");
}

describe("the shell (AC-11)", () => {
  it("lists the six sections in order and marks the current one", async () => {
    renderApp(stubClient([aProposal({ id: "PR-1" })]), "#/alpha/inbox");
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    expect(navLinks().map((link) => link.textContent)).toEqual([
      "Inbox",
      "Tasks",
      "Spec tree",
      "Graph",
      "Health",
      "Questions",
    ]);
    expect(navLinks().map((link) => link.getAttribute("aria-current"))).toEqual(["page", null, null, null, null, null]);
    expect(navLinks()[1]?.getAttribute("href")).toBe("#/alpha/tasks");
  });

  it.each(UNBUILT)("names the slice that builds %s", async (section, title, slice) => {
    renderApp(stubClient(), `#/alpha/${section}`);
    expect(await screen.findByRole("heading", { level: 1, name: title })).toBeTruthy();
    expect(screen.getByText(slice).closest("p")?.textContent).toBe(`Not built yet: arrives in slice ${slice}.`);
    const current = navLinks().find((link) => link.getAttribute("aria-current") === "page");
    expect(current?.textContent).toBe(title);
  });

  it("navigates with the nav, and back returns", async () => {
    renderApp(stubClient([aProposal({ id: "PR-1" })]), "#/alpha/inbox");
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    const tasks = navLinks()[1];
    if (tasks === undefined) {
      throw new Error("no Tasks link");
    }
    fireEvent.click(tasks);
    expect(await screen.findByRole("heading", { level: 1, name: "Tasks" })).toBeTruthy();
    expect(window.location.hash).toBe("#/alpha/tasks");
    // jsdom traverses history in a task of its own; wait for the hashchange it fires, not a clock.
    const traversed = new Promise<void>((resolve) => {
      window.addEventListener(
        "hashchange",
        () => {
          resolve();
        },
        { once: true },
      );
    });
    await act(async () => {
      window.history.back();
      await traversed;
    });
    expect(await screen.findByRole("heading", { level: 1, name: "Inbox" })).toBeTruthy();
    expect(window.location.hash).toBe("#/alpha/inbox");
  });

  it("opens the first project's inbox from #/, leaving focus at the top of the page", async () => {
    renderApp(stubClient([aProposal({ id: "PR-1" })]), "#/");
    expect(await screen.findByRole("heading", { level: 1, name: "Inbox" })).toBeTruthy();
    expect(window.location.hash).toBe("#/alpha/inbox");
    await screen.findByRole("listbox");
    expect(document.activeElement).toBe(document.body);
  });

  it("after a successful Retry on the projects' error panel, focuses the heading of the view it opens", async () => {
    const client = stubClient([aProposal({ id: "PR-1" })]);
    client.getProjects.mockRejectedValueOnce(new ClientError({ status: 503, message: "index locked" }));
    renderApp(client, "#/");
    const main = screen.getByRole("main");
    const alert = await within(main).findByRole("alert");
    const retry = within(main).getByRole("button", { name: "Retry" });
    expect(alert.contains(retry)).toBe(false);
    retry.focus();
    fireEvent.click(retry);
    const heading = await screen.findByRole("heading", { level: 1, name: "Inbox" });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
    expect(window.location.hash).toBe("#/alpha/inbox");
  });

  it("after a successful Retry that finds no projects, focuses the No projects heading", async () => {
    const client = stubClient();
    client.getProjects
      .mockRejectedValueOnce(new ClientError({ status: 503, message: "index locked" }))
      .mockResolvedValueOnce([]);
    renderApp(client, "#/");
    const retry = await within(screen.getByRole("main")).findByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    const heading = await screen.findByRole("heading", { level: 1, name: "No projects" });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
  });

  it("says Not found for an unknown hash or project and links the Inbox", async () => {
    renderApp(stubClient(), "#/alpha/nowhere");
    expect(await screen.findByRole("heading", { level: 1, name: "Not found" })).toBeTruthy();
    await waitFor(() => {
      expect(screen.getByRole("link", { name: "Go to the Inbox" }).getAttribute("href")).toBe("#/alpha/inbox");
    });
  });

  it("says Not found for a project the daemon does not serve", async () => {
    renderApp(stubClient(), "#/zeta/inbox");
    expect(await screen.findByRole("heading", { level: 1, name: "Not found" })).toBeTruthy();
    expect(screen.getByText("There is no project zeta.")).toBeTruthy();
  });

  it("says why the projects could not be read, with the daemon's message and a Retry that reads them again", async () => {
    const client = stubClient([aProposal({ id: "PR-1" })]);
    const message = "spec index unavailable: the database is locked (code 5)";
    client.getProjects.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/inbox");
    const header = appHeader();
    expect(await within(header).findByText(message)).toBeTruthy();
    expect(within(header).queryByText("Loading projects")).toBeNull();
    expect(await screen.findByRole("listbox")).toBeTruthy();
    const retry = within(header).getByRole("button", { name: "Retry loading the projects" });
    // The name comes from the button's text, which starts with the visible label (WCAG 2.5.3).
    expect(retry.hasAttribute("aria-label")).toBe(false);
    expect(retry.textContent).toBe("Retry loading the projects");
    expect(retry.querySelector(".sr-only")?.textContent).toBe("loading the projects");
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(client.getProjects).toHaveBeenCalledTimes(2);
    });
    const select = await within(header).findByLabelText("Project");
    await waitFor(() => {
      expect((select as HTMLSelectElement).disabled).toBe(false);
    });
    expect(within(header).queryByText(message)).toBeNull();
    expect((select as HTMLSelectElement).value).toBe("alpha");
    expect(document.activeElement).toBe(select);
  });

  it("switches project with the labelled select, keeping the section", async () => {
    renderApp(stubClient(), "#/alpha/tasks");
    const select = await screen.findByLabelText("Project");
    await waitFor(() => {
      expect((select as HTMLSelectElement).disabled).toBe(false);
    });
    fireEvent.change(select, { target: { value: "beta" } });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/beta/tasks");
    });
  });
});

describe("error boundaries (AC-16)", () => {
  it("shows a failing view's fallback while the nav still works", async () => {
    const client = stubClient();
    client.getInbox.mockResolvedValue({ proposals: null, notes: [] } as unknown as Inbox);
    window.history.replaceState(null, "", "/#/alpha/inbox");
    const caught: unknown[] = [];
    render(<App client={client} scenario={null} />, {
      onCaughtError: (error: unknown) => {
        caught.push(error);
      },
    });
    expect(await screen.findByRole("heading", { level: 1, name: "This view failed" })).toBeTruthy();
    expect(caught.length).toBeGreaterThan(0);
    const tasks = navLinks()[1];
    if (tasks === undefined) {
      throw new Error("no Tasks link");
    }
    fireEvent.click(tasks);
    expect(await screen.findByRole("heading", { level: 1, name: "Tasks" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "This view failed" })).toBeNull();
  });

  it("falls back at the root when the shell itself fails, still saying Mock data", async () => {
    const client = stubClient();
    client.getProjects.mockResolvedValue(null as unknown as Project[]);
    window.history.replaceState(null, "", "/#/alpha/tasks");
    render(<App client={client} scenario="slow" />, { onCaughtError: () => undefined });
    expect(await screen.findByRole("heading", { level: 1, name: "SpecEngine stopped" })).toBeTruthy();
    expect(screen.getByText("Mock data")).toBeTruthy();
    expect(screen.getByText("scenario: slow")).toBeTruthy();
  });
});

describe("the console guard (AC-16)", () => {
  it("records console.error and console.warn so the setup fails the test", () => {
    console.error("first");
    console.warn("second");
    expect(takeConsoleCalls()).toEqual(["console.error: first", "console.warn: second"]);
  });
});

describe("Mock data (AC-17)", () => {
  const routes = ["#/alpha/inbox", ...UNBUILT.map(([section]) => `#/alpha/${section}`), "#/alpha/nowhere", "#/zeta/inbox"];

  it.each(routes)("is shown on %s", async (hash) => {
    renderApp(stubClient(), hash);
    await screen.findByRole("heading", { level: 1 });
    expect(screen.getByText("Mock data")).toBeTruthy();
    expect(screen.queryByText(/scenario:/)).toBeNull();
  });

  it("names a non-default scenario, also when every read fails", async () => {
    const client = stubClient();
    const failure = new ClientError({ status: 503, message: "index locked" });
    client.getProjects.mockRejectedValue(failure);
    client.getInbox.mockRejectedValue(failure);
    renderApp(client, "#/", "error");
    expect(await within(screen.getByRole("main")).findByText("index locked")).toBeTruthy();
    expect(within(appHeader()).getByText("index locked")).toBeTruthy();
    expect(screen.getByText("Mock data")).toBeTruthy();
    expect(screen.getByText("scenario: error")).toBeTruthy();
  });
});

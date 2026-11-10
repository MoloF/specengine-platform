import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// DM-8 of docs/features/doc-modal.md over the mock (harbor-sim), as `pnpm dev` shows it: from each
// screen a document link's plain click opens the document modal on the document it names, its page
// the link's href, and the screen stays mounted beneath. The mock is imported here alone (policy AC-05).

function renderMock(hash: string, scenario: Scenario = "normal") {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
}

/** The REF a document link names: its page's `<REF>`. */
function refOf(link: HTMLElement): string {
  const match = /^#\/harbor-sim\/tree\/([^/?]+)\/page$/.exec(link.getAttribute("href") ?? "");
  if (match === null) {
    throw new Error(`no document page: ${link.getAttribute("href") ?? ""}`);
  }
  return decodeURIComponent(match[1] ?? "");
}

/** A plain click opens the modal on the link's document over the screen named by its `h1`, which stays. */
async function opensOver(link: HTMLElement, view: string) {
  const ref = refOf(link);
  const heading = screen.getByRole("heading", { level: 1, name: view, hidden: true });
  fireEvent.click(link);
  expect(window.location.hash).toMatch(new RegExp(`[?&]doc=${encodeURIComponent(ref).replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`));
  const dialog = await waitFor(() => {
    const found = document.querySelector<HTMLElement>(".doc-modal");
    expect(found).not.toBeNull();
    return found as HTMLElement;
  });
  // The document read, or the daemon's reason when the mock's corpus holds no such document: the way to the tree stands either way.
  const tree = await within(dialog).findByRole("link", { name: "Open in Spec tree" });
  expect(tree.getAttribute("href")).toBe(`#/harbor-sim/tree/${encodeURIComponent(ref)}`);
  expect(within(dialog).getByRole("heading", { level: 2 }).id).toBe(dialog.getAttribute("aria-labelledby"));
  expect(heading.isConnected).toBe(true);
  expect(screen.getByRole("heading", { level: 1, name: view, hidden: true })).toBe(heading);
}

/** The first document link inside `scope`, once one stands there. */
async function firstDocLink(scope: () => HTMLElement | null, selector = "a"): Promise<HTMLElement> {
  return waitFor(() => {
    const found = Array.from(scope()?.querySelectorAll<HTMLElement>(selector) ?? []).find((anchor) => /\/page$/.test(anchor.getAttribute("href") ?? ""));
    expect(found).toBeDefined();
    return found as HTMLElement;
  });
}

describe("one link component over the mock (DM-8 of doc-modal)", () => {
  it("the Board: a tile's source", async () => {
    renderMock("#/harbor-sim/board");
    await screen.findByRole("heading", { level: 1, name: "Board" });
    await opensOver(await firstDocLink(() => document.querySelector(".board-region"), ".board-card-source a"), "Board");
  });

  it("the card's modal: a target, the document over the card", async () => {
    renderMock("#/harbor-sim/board/C-0003");
    const card = await screen.findByRole("dialog", { name: "Night passage rules, plain words" });
    await opensOver(await firstDocLink(() => card, ".board-panel-target a"), "Board");
    expect(card.isConnected).toBe(true);
  });

  it("Tasks: the overview's target", async () => {
    renderMock("#/harbor-sim/tasks/T-0108");
    await screen.findByRole("heading", { level: 1, name: /^T-0108/ });
    await opensOver(await firstDocLink(() => document.querySelector("article"), ".task-target-links a"), "T-0108: Speed limit on night passages");
  });

  it("the Inbox: a proposal's target", async () => {
    renderMock("#/harbor-sim/inbox/PR-0041");
    const article = await screen.findByRole("article");
    await opensOver(await firstDocLink(() => article, ".target-open a"), "Inbox");
  });

  it("Health: an unbound row's node", async () => {
    renderMock("#/harbor-sim/health");
    await screen.findByRole("heading", { level: 1, name: "Health" });
    await opensOver(await firstDocLink(() => document.querySelector("main"), "[data-unbound] a.health-code-id"), "Health");
  });

  it("Graph: the List's node", async () => {
    renderMock("#/harbor-sim/graph/MEC-TIDES");
    const heading = await screen.findByRole("heading", { level: 1, name: /MEC-TIDES/ });
    fireEvent.click(await screen.findByRole("tab", { name: "List" }));
    await opensOver(await firstDocLink(() => document.querySelector("main"), ".graph-list-name"), heading.textContent);
  });

  it("the Questions round: an item's target", async () => {
    renderMock("#/harbor-sim/questions");
    await screen.findByRole("heading", { level: 1, name: "Questions" });
    await opensOver(await firstDocLink(() => document.querySelector("main")), "Questions");
  });

  it("the Roadmap: a lane's item", async () => {
    renderMock("#/harbor-sim/roadmap");
    await screen.findByRole("heading", { level: 1, name: "Roadmap" });
    await opensOver(await firstDocLink(() => document.querySelector("main"), ".roadmap-card-id"), "Roadmap");
  });

  it("Flow: a Records entry, the document over the dialog", async () => {
    renderMock("#/harbor-sim/flow?scope=WBAY-KILN", "aside");
    await screen.findByRole("heading", { level: 1, name: "Flow" });
    const button = await screen.findByRole("button", { name: /^Records for the AI/ });
    fireEvent.click(button);
    const records = await screen.findByRole("dialog", { name: /^Records for the AI/ });
    await opensOver(await firstDocLink(() => records), "Flow");
    expect(records.isConnected).toBe(true);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  });
});

import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Card, CardList } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";

// BH-6 of board-handoff: a card's page, `#/<p>/board/<C>/page`: no columns, one h1 (the title), the
// modal's body and acts, and "Open on the board"; it reads the list and the card as the board does, a
// reload keeps it, the nav marks Board; no such card shows the daemon's words under lang=en.

const LIST: CardList = aCardList(
  [
    aCardRow({ id: "C-0001", title: "The page's card", assignee: "Mira Holt" }),
    aCardRow({ id: "C-0002", title: "Its child", parent: "C-0001", column: "doing" }),
  ],
  { flows: [aCardFlow({ return: "doing" })] },
);

const CARDS: Record<string, Card> = {
  "C-0001": aCard({ id: "C-0001", title: "The page's card", goal: "What it is for", assignee: "Mira Holt", allowed: [{ to: "doing", by: [] }] }),
  "C-0002": aCard({ id: "C-0002", title: "Its child", parent: "C-0001", column: "doing" }),
};

function pageClient(): StubClient {
  const client = stubClient();
  client.getCards.mockImplementation(() => Promise.resolve(structuredClone(LIST)));
  client.getCard.mockImplementation((_project, id) => {
    const card = CARDS[id];
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  return client;
}

async function page(client: StubClient = pageClient(), hash = "#/alpha/board/C-0001/page") {
  renderApp(client, hash);
  return screen.findByRole("heading", { level: 1, name: "The page's card" });
}

describe("a card's page (BH-6)", () => {
  it("no column, one h1, the modal's body and acts, and Open on the board; the nav marks Board", async () => {
    const client = pageClient();
    await page(client);
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(document.querySelector(".board-grid, .board-cell, [data-card]")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("link", { name: "Open on the board" }).getAttribute("href")).toBe("#/alpha/board/C-0001");
    // The work item's order (DF-6 of design-foundation): the head's acts, Move in the side column's Status.
    expect(within(screen.getByRole("group", { name: "Actions" })).getAllByRole("button").map((button) => button.textContent)).toEqual(["Hand off", "Return", "Edit", "Assign", "Cancel card"]);
    expect(within(screen.getByRole("region", { name: "Status" })).getAllByRole("button").map((button) => button.textContent)).toEqual(["Move"]);
    expect(within(await screen.findByRole("region", { name: "Goal" })).getByText("What it is for")).toBeTruthy();
    const child = within(screen.getByRole("region", { name: "Hand-offs" })).getByRole("link", { name: "C-0002" });
    expect(child.getAttribute("href")).toBe("#/alpha/board/C-0002/page");
    expect(within(screen.getByRole("navigation", { name: "Sections" })).getByRole("link", { name: "Board" }).getAttribute("aria-current")).toBe("page");
    expect(client.getCards).toHaveBeenCalled();
    expect(client.getCard.mock.calls.map((call) => call[1])).toContain("C-0001");
  });

  it("a remount on that hash draws it again, as a reload does", async () => {
    await page();
    cleanup();
    await page();
    expect(screen.getByRole("link", { name: "Open on the board" })).toBeTruthy();
  });

  it("a child's link goes to its page, replacing this one", async () => {
    await page();
    const length = window.history.length;
    fireEvent.click(within(await screen.findByRole("region", { name: "Hand-offs" })).getByRole("link", { name: "C-0002" }));
    await screen.findByRole("heading", { level: 1, name: "Its child" });
    expect([window.location.hash, window.history.length]).toEqual(["#/alpha/board/C-0002/page", length]);
  });

  it("its acts open their dialogs over the page: Return asks why and moves with the note", async () => {
    const client = pageClient();
    await page(client);
    fireEvent.click(await screen.findByRole("button", { name: "Return" }));
    const dialog = await screen.findByRole("dialog", { name: "Return C-0001" });
    fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: "Two endings." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Return card" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]).toEqual(["alpha", "C-0001", { to: "doing", note: "Two endings.", revision: "1", as: null }]);
  });

  // Replaced by DF-7 of design-foundation: the part is folded under the side column's Development, expanded first.
  it("shows the card's Tasks part under Development, as the modal does: a card with no link says No task yet and how to make and link one (CT-5 of card-tasks)", async () => {
    await page();
    const development = await screen.findByRole("button", { name: "Development \u00b7 No agent tasks" });
    expect(screen.queryByRole("region", { name: "Tasks" })).toBeNull();
    fireEvent.click(development);
    const part = screen.getByRole("region", { name: "Tasks" });
    expect(within(part).getByRole("heading", { level: 3, name: "No task yet." })).toBeTruthy();
    expect(Array.from(part.querySelectorAll("code"), (code) => code.textContent)).toEqual(['spec task new --nodes REF --title "..."', "spec card edit C-0001 --task T-NNNN"]);
    expect(part.closest(".issue-view-side")).not.toBeNull();
  });

  it("no such card: the daemon's `no card C-9999` verbatim under lang=en", async () => {
    renderApp(pageClient(), "#/alpha/board/C-9999/page");
    await screen.findByRole("heading", { level: 1, name: "C-9999" });
    const reason = await screen.findByText("no card C-9999");
    expect(reason.getAttribute("lang")).toBe("en");
  });
});

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Card, CardList } from "../api/types";
import { aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";

// docs/features/design-sweep-b.md "Acceptance criteria", DB6: the Board's three old empty states take
// EmptyState and nothing else, a notice's wrapper kept for its surface over the grid; no task, a
// filter with no match (Clear filters its primary) and no such task in the card's view.

function boardClient(list: CardList, cards: Record<string, Card> = {}): StubClient {
  const client = stubClient();
  client.getCards.mockImplementation(() => Promise.resolve(structuredClone(list)));
  client.getCard.mockImplementation((_project, id) => {
    const card = cards[id];
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  return client;
}

async function open(client: StubClient, hash = "#/alpha/board") {
  renderApp(client, hash);
  await screen.findByRole("heading", { level: 1, name: "Board" });
  await waitFor(() => {
    expect(client.getCards).toHaveBeenCalled();
  });
}

describe("the Board's empty states (DB6 of design-sweep-b)", () => {
  it("draws no task as an EmptyState in a notice over the grid", async () => {
    await open(boardClient(aCardList([])));
    const title = await screen.findByRole("heading", { level: 2, name: /^No tasks yet\./ });
    const box = title.closest(".empty-state-box");
    expect(box?.parentElement?.className).toBe("board-notice");
    expect(box?.querySelector(".empty-state-acts")).toBeNull();
    expect(document.querySelector(".board-notices .empty-state, .board-notices .empty-filter")).toBeNull();
  });

  it("draws a filter with no match as an EmptyState whose primary is Clear filters", async () => {
    await open(boardClient(aCardList([aCardRow({ id: "C-0001", discipline: "copy", assignee: "Mira Holt" }), aCardRow({ id: "C-0002", discipline: "plates", assignee: "Ivo Brandt" })])));
    await waitFor(() => {
      expect(document.querySelectorAll("[data-card]")).toHaveLength(2);
    });
    fireEvent.change(screen.getByLabelText("Discipline"), { target: { value: "plates" } });
    fireEvent.change(screen.getByLabelText("Assignee"), { target: { value: "mira holt" } });
    const title = await screen.findByRole("heading", { level: 2, name: "No task matches the filters." });
    const box = title.closest(".empty-state-box");
    expect(box?.parentElement?.className).toBe("board-notice");
    expect(Array.from(box?.querySelectorAll(".empty-state-acts button") ?? [], (button) => [button.textContent, button.className])).toEqual([["Clear filters", "button button-primary"]]);
    fireEvent.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => {
      expect(document.querySelectorAll("[data-card]")).toHaveLength(2);
    });
  });

  it("draws no such task as an EmptyState, the daemon's words its title under lang=en, the way back its description", async () => {
    await open(boardClient(aCardList([])), "#/alpha/board/C-0099");
    const title = await screen.findByRole("heading", { level: 3, name: "no card C-0099" });
    expect([title.className, title.getAttribute("lang")]).toEqual(["empty-state-title", "en"]);
    const link = title.closest(".empty-state-box")?.querySelector(".empty-state-description a");
    expect([link?.textContent, link?.getAttribute("href")]).toEqual(["Back to the board", "#/alpha/board"]);
  });
});

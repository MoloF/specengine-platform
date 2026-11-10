import { createEvent, fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aCard, aCardFlow, aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";

// BH-5 of board-handoff: every tile holds one link, its title, to the card's page, stretched over the
// tile. A plain click is prevented and opens the modal; a Meta, Ctrl or Shift click, a middle click
// and the context menu are the browser's, so they open the page, and change neither the hash nor the
// history here. A1 of the same: a tile in its flow's return column says Returned, a parent counts its
// subtasks.

const FLOW = aCardFlow({ return: "doing" });

async function board() {
  const client = stubClient();
  client.getCards.mockResolvedValue(
    aCardList(
      [
        aCardRow({ id: "C-0001", title: "A tile" }),
        aCardRow({ id: "C-0002", column: "doing", parent: "C-0001", title: "Back" }),
        aCardRow({ id: "C-0003", column: "check", parent: "C-0001", title: "Taken" }),
        aCardRow({ id: "C-0004", parent: "C-0001", title: "Gone", cancelled: true, position: 0 }),
      ],
      { flows: [FLOW] },
    ),
  );
  client.getCard.mockImplementation((_project, id) => Promise.resolve(aCard({ id, title: "A tile" })));
  renderApp(client, "#/alpha/board");
  await screen.findByRole("heading", { level: 1, name: "Board" });
  return waitFor(() => {
    const found = document.querySelector<HTMLElement>('[data-card="C-0001"]');
    if (found === null) {
      throw new Error("no tile");
    }
    return found;
  });
}

/**
 * Whether the page prevented a click: read after every handler, on `window`. jsdom has no new tab
 * and would follow the link here, so the reading stops it, once the page has said its word.
 */
function prevented(target: Element, init: MouseEventInit): boolean {
  const said: boolean[] = [];
  const read = (event: Event) => {
    said.push(event.defaultPrevented);
    event.preventDefault();
  };
  window.addEventListener("click", read);
  fireEvent.click(target, init);
  window.removeEventListener("click", read);
  const first = said[0];
  if (first === undefined) {
    throw new Error("the click never reached the window");
  }
  return first;
}

function tile(id: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(`[data-card="${id}"]`);
  if (found === null) {
    throw new Error(`no tile ${id}`);
  }
  return found;
}

describe("a tile's link (BH-5)", () => {
  it("every tile holds one link to its page, the title; the source link aside, none other", async () => {
    await board();
    for (const id of ["C-0001", "C-0002", "C-0003"]) {
      const titles = tile(id).querySelectorAll("a.board-card-link");
      expect(titles).toHaveLength(1);
      expect(titles[0]?.getAttribute("href")).toBe(`#/alpha/board/${id}/page`);
      expect(titles[0]?.getAttribute("draggable")).toBe("false");
    }
    expect(tile("C-0001").draggable).toBe(true);
  });

  it.each([
    ["Meta", { metaKey: true }],
    ["Ctrl", { ctrlKey: true }],
    ["Shift", { shiftKey: true }],
    ["middle", { button: 1 }],
  ])("a %s click anywhere on the tile is the browser's: not prevented, no hash, no entry", async (_name, init) => {
    const element = await board();
    const length = window.history.length;
    for (const target of [element.querySelector("a.board-card-link") as Element, element]) {
      expect(prevented(target, init)).toBe(false);
    }
    expect([window.location.hash, window.history.length]).toEqual(["#/alpha/board", length]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("the context menu is the browser's too", async () => {
    const element = await board();
    const link = element.querySelector("a.board-card-link") as Element;
    const event = createEvent.contextMenu(link);
    fireEvent(link, event);
    expect(event.defaultPrevented).toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("a plain click on the title is prevented and opens the modal; one on the tile's body too", async () => {
    const element = await board();
    const link = element.querySelector("a.board-card-link") as Element;
    expect(prevented(link, { button: 0 })).toBe(true);
    const dialog = await screen.findByRole("dialog", { name: "A tile" });
    expect(window.location.hash).toBe("#/alpha/board/C-0001");
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    // The body: the assignee's line, under the stretched link in a browser, the tile's own in jsdom.
    const body = tile("C-0001").querySelector(".board-card-who") as Element;
    prevented(body, { button: 0 });
    expect(await screen.findByRole("dialog", { name: "A tile" })).toBeTruthy();
    expect(window.location.hash).toBe("#/alpha/board/C-0001");
  });
});

describe("a tile's hand-off words (BH-15, BH-16)", () => {
  it("Returned with its icon on a card standing in its flow's return column, none elsewhere", async () => {
    await board();
    const badge = tile("C-0002").querySelector(".badge");
    expect([badge?.textContent, badge?.querySelector("svg") !== null, badge?.getAttribute("data-tone")]).toEqual(["Returned", true, "card-returned"]);
    expect(tile("C-0002").getAttribute("aria-label")).toBe("C-0002: Back, Returned");
    expect(tile("C-0003").textContent).not.toContain("Returned");
  });

  it("a parent counts its standing subtasks, and its returned ones with the badge", async () => {
    await board();
    const line = tile("C-0001").querySelector(".board-card-handoffs");
    expect(line?.textContent).toBe("2 subtasks1 returned");
    expect(line?.querySelector(".badge")?.getAttribute("data-tone")).toBe("card-returned");
    expect(tile("C-0003").querySelector(".board-card-handoffs")).toBeNull();
  });
});

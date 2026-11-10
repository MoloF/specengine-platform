import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Card, CardList } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow, aCardWrite } from "../test/builders";
import { docView, FERRY } from "../test/docStub";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { statusLook } from "./category";
import { recallLaneMode } from "./swimlanes";

// BH-3, BH-4, BH-7 and BH-16 of board-handoff: a card opens as a modal dialog over a board that holds
// still, one history entry pushed; it closes by Esc, Close, the scrim or Back, replacing when it was
// opened by URL, focus back on its tile; a write dialog stacks over it; a link inside it replaces and
// switches no tab; it lists the card's subtasks.

function boardClient(list: CardList, cards: Record<string, Card> = {}): StubClient {
  const client = stubClient();
  client.getCards.mockImplementation(() => Promise.resolve(structuredClone(list)));
  client.getCard.mockImplementation((_project, id) => {
    const card = cards[id];
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  return client;
}

function tile(id: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(`[data-card="${id}"]`);
  if (found === null) {
    throw new Error(`no card ${id}`);
  }
  return found;
}

/** Whether an element lies in an inert subtree (the property, which a dialog sets on the page under it). */
function inert(element: HTMLElement): boolean {
  for (let node: HTMLElement | null = element; node !== null; node = node.parentElement) {
    if (node.inert) {
      return true;
    }
  }
  return false;
}

function region(): HTMLElement {
  const found = document.querySelector<HTMLElement>(".board-region");
  if (found === null) {
    throw new Error("no board region");
  }
  return found;
}

const TWO_FLOWS = [aCardFlow({ name: "copy", disciplines: ["copy"] }), aCardFlow({ name: "plates", disciplines: ["plates"], return: "doing" })];

const LIST = aCardList(
  [
    aCardRow({ id: "C-0001", flow: "copy", title: "The parent", assignee: "Mira Holt" }),
    aCardRow({ id: "C-0002", flow: "copy", title: "Second", position: 2 }),
    aCardRow({ id: "C-0003", flow: "plates", title: "A plate", parent: "C-0001" }),
    aCardRow({ id: "C-0004", flow: "plates", column: "doing", title: "Sent back", parent: "C-0001", assignee: "Noor Ainsley" }),
    aCardRow({ id: "C-0005", flow: "plates", column: "check", title: "In work", parent: "C-0001" }),
    aCardRow({ id: "C-0006", flow: "plates", column: "closed", title: "Done one", parent: "C-0001", accepted: true }),
    aCardRow({ id: "C-0007", flow: "plates", title: "Dropped", parent: "C-0001", cancelled: true, position: 0 }),
    aCardRow({ id: "C-0008", flow: "gone", column: "x", title: "Elsewhere", parent: "C-0001" }),
  ],
  { flows: TWO_FLOWS },
);

const CARDS: Record<string, Card> = {
  "C-0001": aCard({ id: "C-0001", flow: "copy", title: "The parent", assignee: "Mira Holt" }),
  "C-0002": aCard({ id: "C-0002", flow: "copy", title: "Second", position: 2 }),
  "C-0003": aCard({ id: "C-0003", flow: "plates", title: "A plate", parent: "C-0001" }),
};

async function board(client: StubClient = boardClient(LIST, CARDS), hash = "#/alpha/board") {
  renderApp(client, hash);
  await screen.findByRole("heading", { level: 1, name: "Board" });
  await waitFor(() => tile("C-0001"));
  return client;
}

/** A plain click on a tile's title, where a click anywhere on the tile lands: the stretched link. */
function clickTitle(id: string) {
  const link = tile(id).querySelector<HTMLElement>("a.board-card-link");
  if (link === null) {
    throw new Error(`no title link on ${id}`);
  }
  fireEvent.click(link);
}

describe("a plain click opens the modal (BH-3)", () => {
  it("one dialog, modal, labelled by the card's title, focus on its heading; one history entry; tab, filters, lanes and the region's scroll unchanged; the board inert", async () => {
    await board();
    fireEvent.change(screen.getByLabelText<HTMLSelectElement>("Assignee"), { target: { value: "mira holt" } });
    const scroller = region();
    scroller.scrollTop = 40;
    scroller.scrollLeft = 12;
    const tab = screen.getByRole("tab", { selected: true }).textContent;
    const lanes = screen.getByLabelText<HTMLSelectElement>("Lanes").value;
    const length = window.history.length;
    clickTitle("C-0001");
    const dialog = await screen.findByRole("dialog", { name: "The parent" });
    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    await waitFor(() => {
      expect(document.activeElement).toBe(within(dialog).getByRole("heading", { level: 2, name: "The parent" }));
    });
    expect(window.location.hash).toBe("#/alpha/board/C-0001");
    expect(window.history.length).toBe(length + 1);
    expect(screen.getByRole("tab", { selected: true, hidden: true }).textContent).toBe(tab);
    expect(screen.getByLabelText<HTMLSelectElement>("Assignee").value).toBe("mira holt");
    expect(screen.getByLabelText<HTMLSelectElement>("Lanes").value).toBe(lanes);
    expect([region().scrollTop, region().scrollLeft]).toEqual([40, 12]);
    expect(inert(tile("C-0001"))).toBe(true);
    expect(inert(screen.getByLabelText("Acting as"))).toBe(true);
    expect(inert(dialog)).toBe(false);
    expect(recallLaneMode("alpha")).toBe("none");
  });
});

describe("closing (BH-4)", () => {
  it("Esc and Close, opened from the board, go back: the hash is the board's, no entry added, focus on the tile", async () => {
    await board();
    for (const how of ["Escape", "Close"]) {
      clickTitle("C-0002");
      const dialog = await screen.findByRole("dialog", { name: "Second" });
      const length = window.history.length;
      if (how === "Escape") {
        fireEvent.keyDown(within(dialog).getByRole("heading", { level: 2 }), { key: "Escape" });
      } else {
        fireEvent.click(within(dialog).getByRole("button", { name: /^Close/ }));
      }
      await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
      });
      expect([how, window.location.hash, window.history.length]).toEqual([how, "#/alpha/board", length]);
      await waitFor(() => {
        expect(document.activeElement).toBe(tile("C-0002"));
      });
    }
  });

  it("opened by URL, Esc replaces: the board's hash, no entry added", async () => {
    await board(boardClient(LIST, CARDS), "#/alpha/board/C-0002");
    const dialog = await screen.findByRole("dialog", { name: "Second" });
    const length = window.history.length;
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect([window.location.hash, window.history.length]).toEqual(["#/alpha/board", length]);
    expect(document.activeElement).toBe(tile("C-0002"));
  });

  it("Back closes it, and a click on the scrim does too", async () => {
    await board();
    clickTitle("C-0002");
    await screen.findByRole("dialog", { name: "Second" });
    act(() => {
      window.history.back();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(window.location.hash).toBe("#/alpha/board");
    clickTitle("C-0002");
    await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(document.querySelector("[data-dialog-scrim]") as Element);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("focus goes back to the tile in its new column if it moved meanwhile; a tile not drawn gives it to the board's heading", async () => {
    let rows = LIST.cards.filter((row) => row.flow === "copy");
    const client = boardClient(LIST, CARDS);
    client.getCards.mockImplementation(() => Promise.resolve(aCardList(rows, { flows: TWO_FLOWS })));
    client.moveCard.mockImplementation((_project, id, body) => {
      rows = rows.map((row) => (row.id === id ? { ...row, column: body.to ?? row.column, revision: "2" } : row));
      return Promise.resolve(aCardWrite(id, { flow: "copy", column: body.to ?? null }));
    });
    await board(client);
    clickTitle("C-0002");
    const dialog = await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Move" }));
    fireEvent.click(within(await screen.findByRole("menu", { name: "Move C-0002" })).getByRole("menuitem", { name: /^Doing/ }));
    await waitFor(() => {
      expect(tile("C-0002").dataset.column).toBe("doing");
    });
    // Focus stays in the modal while it is open: on the Move button the menu came from.
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Move" }));
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(document.activeElement).toBe(tile("C-0002"));
    });
    // A filter hides the tile meanwhile: focus on the board's heading.
    clickTitle("C-0001");
    const parent = await screen.findByRole("dialog", { name: "The parent" });
    const assignee = screen.getByLabelText<HTMLSelectElement>("Assignee");
    fireEvent.change(assignee, { target: { value: within(assignee).getByRole<HTMLOptionElement>("option", { name: "Unassigned", hidden: true }).value } });
    expect(document.querySelector('[data-card="C-0001"]')).toBeNull();
    fireEvent.click(within(parent).getByRole("button", { name: /^Close/ }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1, name: "Board" }));
  });

  it("with a write dialog over it, Esc closes only that dialog; while a submit is out Esc, Close and the scrim close nothing", async () => {
    let answer: (write: ReturnType<typeof aCardWrite>) => void = () => undefined;
    const client = boardClient(LIST, CARDS);
    client.assignCard.mockImplementation(
      () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    );
    await board(client, "#/alpha/board/C-0002");
    const dialog = await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Assign" }));
    const assign = await screen.findByRole("dialog", { name: "Assign C-0002" });
    fireEvent.keyDown(within(assign).getByLabelText("Assignee"), { key: "Escape" });
    await waitFor(() => {
      expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Assign" }));
    const again = await screen.findByRole("dialog", { name: "Assign C-0002" });
    fireEvent.change(within(again).getByLabelText("Assignee"), { target: { value: "Ana" } });
    fireEvent.click(within(again).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.assignCard).toHaveBeenCalledTimes(1);
    });
    fireEvent.keyDown(within(again).getByLabelText("Assignee"), { key: "Escape" });
    fireEvent.keyDown(dialog, { key: "Escape" });
    fireEvent.click(within(dialog).getByRole("button", { name: /^Close/ }));
    fireEvent.click(document.querySelectorAll("[data-dialog-scrim]")[0] as Element);
    expect(screen.getAllByRole("dialog")).toEqual([dialog, again]);
    expect(window.location.hash).toBe("#/alpha/board/C-0002");
    await act(async () => {
      answer(aCardWrite("C-0002"));
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    });
  });

  it("Back while a submit is out takes effect once it settles", async () => {
    let answer: (write: ReturnType<typeof aCardWrite>) => void = () => undefined;
    const client = boardClient(LIST, CARDS);
    client.assignCard.mockImplementation(
      () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    );
    await board(client);
    clickTitle("C-0002");
    const dialog = await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Assign" }));
    const assign = await screen.findByRole("dialog", { name: "Assign C-0002" });
    fireEvent.change(within(assign).getByLabelText("Assignee"), { target: { value: "Ana" } });
    fireEvent.click(within(assign).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.assignCard).toHaveBeenCalledTimes(1);
    });
    act(() => {
      window.history.back();
    });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/board");
    });
    expect(screen.getAllByRole("dialog")).toEqual([dialog, assign]);
    await act(async () => {
      answer(aCardWrite("C-0002"));
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });
});

describe("links inside the modal (BH-7)", () => {
  it("a child link replaces the hash and switches no tab; the parent's link back replaces too", async () => {
    const client = boardClient(LIST, CARDS);
    await board(client);
    expect(screen.getByRole("tab", { selected: true }).textContent).toBe("copy");
    clickTitle("C-0001");
    const parent = await screen.findByRole("dialog", { name: "The parent" });
    const length = window.history.length;
    const child = within(await within(parent).findByRole("region", { name: "Subtasks" })).getByRole("link", { name: "C-0003" });
    expect(child.getAttribute("href")).toBe("#/alpha/board/C-0003");
    fireEvent.click(child);
    const plate = await screen.findByRole("dialog", { name: "A plate" });
    expect([window.location.hash, window.history.length]).toEqual(["#/alpha/board/C-0003", length]);
    expect(screen.getByRole("tab", { selected: true, hidden: true }).textContent).toBe("copy");
    fireEvent.click(within(within(plate).getByRole("region", { name: "Details" })).getByRole("link", { name: "C-0001" }));
    await screen.findByRole("dialog", { name: "The parent" });
    expect([window.location.hash, window.history.length]).toEqual(["#/alpha/board/C-0001", length]);
    expect(screen.getByRole("tab", { selected: true, hidden: true }).textContent).toBe("copy");
  });

  it("no such card: the daemon's words verbatim under lang=en", async () => {
    await board(boardClient(LIST, CARDS), "#/alpha/board/C-9999");
    const dialog = await screen.findByRole("dialog", { name: "C-9999" });
    const reason = await within(dialog).findByText("no card C-9999");
    expect(reason.getAttribute("lang")).toBe("en");
  });
});

describe("the parent's subtasks (BH-16)", () => {
  // Replaced by DF-5 of design-foundation: the raw `<flow>/<column>` leaves the row for the flow's name
  // and the child's status lozenge, the look `statusLook` gives; an unserved flow's child is stranded.
  it("lists each child with ID, title, its flow's name and status lozenge, assignee and word; a cancelled one left out, an unserved flow's stranded with no word", async () => {
    await board(boardClient(LIST, CARDS), "#/alpha/board/C-0001");
    const dialog = await screen.findByRole("dialog", { name: "The parent" });
    const items = within(await within(dialog).findByRole("region", { name: "Subtasks" })).getAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "C-0003A plateplatesTo doUnassignedHanded off",
      "C-0004Sent backplatesDoingNoor AinsleyReturned",
      "C-0005In workplatesCheckUnassignedTaken",
      "C-0006Done oneplatesClosedUnassignedAccepted",
      "C-0008ElsewheregonexUnassigned",
    ]);
    expect(items.map((item) => item.textContent).join("")).not.toMatch(/plates\/|gone\//);
    const flows = new Map(TWO_FLOWS.map((flow) => [flow.name, flow]));
    const children = LIST.cards.filter((row) => row.parent === "C-0001" && !row.cancelled);
    expect(items.map((item) => [item.querySelector(".lozenge")?.getAttribute("data-appearance"), item.querySelector(".lozenge-label")?.textContent])).toEqual(
      children.map((row) => {
        const look = statusLook(flows.get(row.flow), row.column);
        return [look.appearance, look.label];
      }),
    );
    expect(items.map((item) => item.querySelector(".lozenge")?.getAttribute("data-appearance"))).toEqual(["neutral", "warning", "information", "success", "neutral"]);
    for (const item of items.slice(0, 4)) {
      expect(item.querySelector(".badge svg")).not.toBeNull();
    }
  });
});

// CT-5 of card-tasks, as DF-7 of design-foundation folds it: the modal shows the card's linked `T-`
// tasks under the side column's Development, collapsed by default, inside the body that scrolls, so
// opening a card with tasks, or its Development, moves no column (BH-1).
describe("the card's tasks (CT-5 of card-tasks)", () => {
  it("holds the Tasks part inside Development, collapsed by default, in the scrolling body; the board's heads hold still and nothing reads a task", async () => {
    const linked: Record<string, Card> = {
      ...CARDS,
      "C-0001": aCard({
        id: "C-0001",
        flow: "copy",
        title: "The parent",
        tasks: [
          { id: "T-0001", status: "review", title: "Light the north lamp", stale: false },
          { id: "T-0002", status: "in_progress", title: "Wire the switch", stale: true },
          { id: "T-0003", status: null, title: null, stale: null },
        ],
        notes: ["T-0003: no such task in this queue; no state shown"],
      }),
    };
    const client = await board(boardClient(LIST, linked));
    const heads = Array.from(document.querySelectorAll(".board-head > *"), (head) => head.textContent);
    expect(heads.length).toBeGreaterThan(0);
    clickTitle("C-0001");
    const dialog = await screen.findByRole("dialog", { name: "The parent" });
    const development = await within(dialog).findByRole("button", { name: "Agent tasks \u00b7 3 linked" });
    expect(development.getAttribute("aria-expanded")).toBe("false");
    expect(within(dialog).queryByRole("region", { name: "Agent tasks" })).toBeNull();
    fireEvent.click(development);
    expect(development.getAttribute("aria-expanded")).toBe("true");
    const part = within(dialog).getByRole("region", { name: "Agent tasks" });
    expect(development.getAttribute("aria-controls")).toBe(part.id);
    expect(part.closest(".issue-view-side")).not.toBeNull();
    expect(part.closest(".issue-view-body")).not.toBeNull();
    expect(Array.from(part.querySelectorAll("li[data-task]"), (row) => row.textContent)).toEqual([
      "T-0001Light the north lampPlan reviewAwaiting youspec task approve T-0001Copy the command: spec task approve T-0001",
      "T-0002Wire the switchIn progressSpec changed",
      "T-0003State not read hereT-0003: no such task in this queue; no state shown",
    ]);
    expect(Array.from(document.querySelectorAll(".board-head > *"), (head) => head.textContent)).toEqual(heads);
    expect(client.getTask).not.toHaveBeenCalled();
    expect(client.getTasks).not.toHaveBeenCalled();
  });
});

// Iteration 2 of board-handoff: what the modal holds belongs to its card. Another card, or none, takes
// its refusal, its lines and notes, its Move menu and a dialog over it away; Back closes them with
// it; a write made in the modal says its outcome in the modal, the board being inert beneath.
describe("the modal's own state", () => {
  it("a refusal of a move made in the modal goes with it: another card's modal shows none", async () => {
    const client = boardClient(LIST, { ...CARDS, "C-0005": aCard({ id: "C-0005", flow: "plates", column: "check", title: "In work" }) });
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 409, message: "C-0002 is in todo: no move to `doing`; nothing changed" }));
    await board(client);
    clickTitle("C-0002");
    const second = await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(within(second).getByRole("button", { name: "Move" }));
    fireEvent.click(within(await screen.findByRole("menu", { name: "Move C-0002" })).getByRole("menuitem", { name: /^Doing/ }));
    expect((await within(second).findByRole("alert")).textContent).toContain("no move to `doing`");
    fireEvent.keyDown(second, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    window.location.hash = "#/alpha/board/C-0005";
    const other = await screen.findByRole("dialog", { name: "In work" });
    expect(within(other).queryByRole("alert")).toBeNull();
    expect(document.querySelector('[role="alert"]')).toBeNull();
  });

  it("Back with a write dialog over the modal closes both, the page no longer inert; n and ? act again", async () => {
    await board();
    clickTitle("C-0002");
    const second = await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(within(second).getByRole("button", { name: "Assign" }));
    await screen.findByRole("dialog", { name: "Assign C-0002" });
    act(() => {
      window.history.back();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(window.location.hash).toBe("#/alpha/board");
    expect(inert(tile("C-0002"))).toBe(false);
    expect(inert(screen.getByLabelText("Acting as"))).toBe(false);
    await waitFor(() => {
      expect(document.activeElement).toBe(tile("C-0002"));
    });
    fireEvent.keyDown(tile("C-0002"), { key: "n" });
    expect(await screen.findByRole("dialog", { name: "New task" })).toBeTruthy();
  });

  it("Back with the modal's Move menu open closes both; the menu comes back with no next modal; ? lists the keys again", async () => {
    await board();
    clickTitle("C-0002");
    const second = await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(within(second).getByRole("button", { name: "Move" }));
    await screen.findByRole("menu", { name: "Move C-0002" });
    act(() => {
      window.history.back();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(screen.queryByRole("menu")).toBeNull();
    clickTitle("C-0001");
    await screen.findByRole("dialog", { name: "The parent" });
    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.keyDown(screen.getByRole("dialog", { name: "The parent" }), { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    fireEvent.keyDown(tile("C-0001"), { key: "?" });
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeTruthy();
  });

  it("a move made in the modal says a `--as` line and its notes there; the toolbar and the board's notices say nothing", async () => {
    const line = "spec: --as: 129 bytes; at most 128";
    const client = boardClient(LIST, CARDS);
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 400, message: line }));
    client.moveCard.mockResolvedValueOnce(aCardWrite("C-0002", { flow: "copy", column: "doing", notes: ["C-0002: a note on the move"] }));
    await board(client);
    clickTitle("C-0002");
    const second = await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(within(second).getByRole("button", { name: "Move" }));
    fireEvent.click(within(await screen.findByRole("menu", { name: "Move C-0002" })).getByRole("menuitem", { name: /^Doing/ }));
    const said = await within(second).findByRole("alert");
    expect([said.textContent, said.getAttribute("lang")]).toEqual([line, "en"]);
    expect(document.querySelector(".board-acting .field-error")).toBeNull();
    fireEvent.click(within(second).getByRole("button", { name: "Move" }));
    fireEvent.click(within(await screen.findByRole("menu", { name: "Move C-0002" })).getByRole("menuitem", { name: /^Doing/ }));
    const notes = await within(second).findByRole("list", { name: "Notes from the daemon on the last write" });
    expect(within(notes).getByText("C-0002: a note on the move").getAttribute("lang")).toBe("en");
    expect(notes.closest('[role="status"]')).not.toBeNull();
    expect(within(second).queryByRole("alert")).toBeNull();
    expect(document.querySelector(".board-notices")?.textContent).not.toContain("a note on the move");
  });

  it("a dialog's write from the modal leaves its notes in the modal", async () => {
    const client = boardClient(LIST, CARDS);
    client.assignCard.mockResolvedValueOnce(aCardWrite("C-0002", { notes: ["C-0002: assigned with a note"] }));
    await board(client, "#/alpha/board/C-0002");
    const second = await screen.findByRole("dialog", { name: "Second" });
    fireEvent.click(within(second).getByRole("button", { name: "Assign" }));
    const assign = await screen.findByRole("dialog", { name: "Assign C-0002" });
    fireEvent.change(within(assign).getByLabelText("Assignee"), { target: { value: "Ana" } });
    fireEvent.click(within(assign).getByRole("button", { name: "Save" }));
    expect(await within(second).findByText("C-0002: assigned with a note")).toBeTruthy();
    expect(document.querySelector(".board-notices")?.textContent).not.toContain("assigned with a note");
  });

  it("a card named from outside (a link, Forward) is closed in place: Esc replaces, never goes back to the card before", async () => {
    await board(boardClient(LIST, { ...CARDS, "C-0005": aCard({ id: "C-0005", flow: "plates", column: "check", title: "In work" }) }));
    clickTitle("C-0002");
    await screen.findByRole("dialog", { name: "Second" });
    window.location.hash = "#/alpha/board/C-0005";
    const other = await screen.findByRole("dialog", { name: "In work" });
    const length = window.history.length;
    fireEvent.keyDown(other, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect([window.location.hash, window.history.length]).toEqual(["#/alpha/board", length]);
  });
});

describe("the heads hold still under Show cancelled (D2)", () => {
  it("a cancelled card a column left behind adds no Not in the flow head", async () => {
    const client = boardClient(LIST, CARDS);
    client.getCards.mockImplementation((_project, options) =>
      Promise.resolve(aCardList([...LIST.cards, ...(options?.with_cancelled === true ? [aCardRow({ id: "C-0009", flow: "copy", column: "parked", cancelled: true, position: 0 })] : [])], { flows: TWO_FLOWS })),
    );
    await board(client);
    const heads = () => Array.from(document.querySelectorAll(".board-head > h2")).map((head) => head.textContent.replace(/,.*$/, ""));
    const before = heads();
    fireEvent.click(screen.getByLabelText("Show cancelled"));
    await waitFor(() => {
      expect(client.getCards.mock.calls.at(-1)?.[1]).toEqual({ with_cancelled: true, marks: true });
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(heads()).toEqual(before);
    expect(before).not.toContain("Not in the flow");
  });
});

// DM-10 of docs/features/doc-modal.md: a card's target opens the document over the card's modal, the
// document always the top dialog: the card's modal inert under it, Esc and Back closing the document
// alone; a reload of both draws the document on top, however late the card's read lands.
describe("the document over the card (DM-10 of doc-modal)", () => {
  const TARGETED: Record<string, Card> = { ...CARDS, "C-0002": aCard({ id: "C-0002", flow: "copy", title: "Second", position: 2, targets: [FERRY] }) };

  function targetedClient(): StubClient {
    const client = boardClient(LIST, TARGETED);
    client.getNode.mockImplementation((_project, ref) => Promise.resolve(docView(ref)));
    return client;
  }

  function hostOf(dialog: HTMLElement): HTMLElement {
    const host = dialog.closest<HTMLElement>("[data-dialog-host]");
    if (host === null) {
      throw new Error("no dialog host");
    }
    return host;
  }

  it("a target link pushes the document's pair; the card's modal inert beneath; Esc closes the document alone, focus back to the link; Back likewise", async () => {
    await board(targetedClient());
    clickTitle("C-0002");
    const card = await screen.findByRole("dialog", { name: "Second" });
    const target = await within(card).findByRole("link", { name: FERRY });
    expect(target.getAttribute("href")).toBe(`#/alpha/tree/${FERRY}/page`);
    for (const how of ["Escape", "Back"] as const) {
      const before = window.history.length;
      fireEvent.click(target);
      expect(window.location.hash).toBe(`#/alpha/board/C-0002?doc=${FERRY}`);
      if (how === "Escape") {
        expect(window.history.length).toBe(before + 1);
      }
      const doc = await screen.findByRole("dialog", { name: "The ferry" });
      await waitFor(() => {
        expect(document.activeElement).toBe(within(doc).getByRole("heading", { level: 2, name: "The ferry" }));
      });
      expect(inert(card)).toBe(true);
      expect(inert(doc)).toBe(false);
      expect(hostOf(card).compareDocumentPosition(hostOf(doc)) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      if (how === "Escape") {
        fireEvent.keyDown(document.activeElement ?? doc, { key: "Escape" });
      } else {
        act(() => {
          window.history.back();
        });
      }
      await waitFor(() => {
        expect(screen.queryByRole("dialog", { name: "The ferry" })).toBeNull();
      });
      expect([how, window.location.hash]).toEqual([how, "#/alpha/board/C-0002"]);
      expect(screen.getByRole("dialog", { name: "Second" })).toBe(card);
      expect(inert(card)).toBe(false);
      await waitFor(() => {
        expect(document.activeElement).toBe(target);
      });
    }
  });

  it("a cold render of the card with the document draws the document on top, though the card's read lands later (M: the document mounted before the view)", async () => {
    const client = targetedClient();
    let land: (card: Card) => void = () => undefined;
    client.getCard.mockImplementation(() => new Promise((resolve) => {
      land = resolve;
    }));
    renderApp(client, `#/alpha/board/C-0002?doc=${FERRY}`);
    const doc = await screen.findByRole("dialog", { name: "The ferry" });
    await within(doc).findByText("A flat boat crosses the river on a cable.");
    const card = screen.getAllByRole("dialog").find((dialog) => dialog !== doc) as HTMLElement;
    expect(card).toBeTruthy();
    const check = () => {
      expect(hostOf(card).compareDocumentPosition(hostOf(doc)) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      expect(inert(card)).toBe(true);
      expect(inert(doc)).toBe(false);
      expect(doc.contains(document.activeElement)).toBe(true);
    };
    await waitFor(check);
    act(() => {
      land(structuredClone(TARGETED["C-0002"] as Card));
    });
    await within(card).findByRole("link", { name: FERRY });
    check();
    // Its close, the modal not pushed here: a replace, focus to the card's title beneath.
    fireEvent.keyDown(doc, { key: "Escape" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/board/C-0002");
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(within(card).getByRole("heading", { level: 2, name: "Second" }));
    });
  });
});

import { createEvent, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { Card, CardRow } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow, aCardWrite } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { ACTING_KEY } from "./acting";

// BH-15 of board-handoff: a return is a move into the flow's `return` column with a reason. The Move
// menu names it "Return\u2026", `r` and a drop from another column open its dialog too; Cancel moves
// nothing; submit is one `POST .../move` with `to` and `note`. A drop or a reorder inside the column
// needs none. A tile standing there says Returned; the modal says the latest reason, who and when.
// The column is looked up in `flows[]`: here `doing`, a word of data like any other.

const FLOW = aCardFlow({ return: "doing" });

function returnClient(start: CardRow[], cards: Record<string, Card> = {}): StubClient {
  let rows = start;
  const client = stubClient();
  client.getCards.mockImplementation(() => Promise.resolve(aCardList(rows, { flows: [FLOW] })));
  client.getCard.mockImplementation((_project, id) => {
    const card = cards[id] ?? aCard({ id, column: rows.find((row) => row.id === id)?.column ?? "todo", allowed: [{ to: "doing", by: [] }] });
    return Promise.resolve(structuredClone(card));
  });
  client.moveCard.mockImplementation((_project, id, body) => {
    rows = rows.map((row) => (row.id === id ? { ...row, column: body.to ?? row.column, revision: String(Number(row.revision) + 1) } : row));
    return Promise.resolve(aCardWrite(id, { column: body.to ?? null }));
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

/** A column's cell in the one lane of None. */
function cell(title: string): HTMLElement {
  const key = screen.getByRole("heading", { level: 2, name: new RegExp(`^${title}\\b`) }).dataset.columnHead ?? "";
  const found = Array.from(document.querySelectorAll<HTMLElement>("section.board-cell")).find((candidate) => candidate.dataset.lane?.endsWith(`|${key}`) === true);
  if (found === undefined) {
    throw new Error(`no cell ${title}`);
  }
  return found;
}

/** jsdom has no DragEvent, so a drag's height is set on the event by hand. */
function dropAt(target: Element, clientY: number) {
  for (const type of ["dragOver", "drop"] as const) {
    const event = createEvent[type](target);
    Object.defineProperty(event, "clientY", { value: clientY });
    fireEvent(target, event);
  }
}

const ROWS = [aCardRow({ id: "C-0001", revision: "3" }), aCardRow({ id: "C-0002", position: 2 }), aCardRow({ id: "C-0003", column: "doing" }), aCardRow({ id: "C-0004", column: "doing", position: 2 })];

async function board(client: StubClient) {
  renderApp(client, "#/alpha/board");
  await waitFor(() => tile("C-0004"));
}

describe("Return (BH-15)", () => {
  it("the Move menu's line for the return column reads Return\u2026 and opens the dialog; submit is one move with `to` and `note`", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = returnClient(ROWS);
    await board(client);
    tile("C-0001").focus();
    fireEvent.keyDown(tile("C-0001"), { key: "m" });
    const menu = await screen.findByRole("menu", { name: "Move C-0001" });
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Return\u2026", "Check", "Down"]);
    fireEvent.click(within(menu).getByRole("menuitem", { name: /^Return/ }));
    const dialog = await screen.findByRole("dialog", { name: "Return C-0001" });
    expect(dialog.textContent).toContain("The task moves to Doing with this reason. The task that asked for it stays where it is.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Say why it goes back.");
    expect(client.moveCard).not.toHaveBeenCalled();
    fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: " Two endings contradict SCN-LAMP-ROOM " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]).toEqual(["alpha", "C-0001", { to: "doing", note: "Two endings contradict SCN-LAMP-ROOM", revision: "3", as: "Ann" }]);
    await waitFor(() => {
      expect(tile("C-0001").dataset.column).toBe("doing");
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(tile("C-0001"));
    });
    // Said without echoing the column's title, which a return column's often is.
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("C-0001 sent back");
  });

  it("is not touched by the hand-off's reviewer rule: it has no reviewer and goes out as the acting name, `agent` too (L2 of departments' looks)", async () => {
    window.localStorage.setItem(ACTING_KEY, "agent");
    const client = returnClient(ROWS);
    await board(client);
    tile("C-0001").focus();
    fireEvent.keyDown(tile("C-0001"), { key: "r" });
    const dialog = await screen.findByRole("dialog", { name: "Return C-0001" });
    expect(within(dialog).queryByLabelText("Reviewer")).toBeNull();
    fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: "Wrong lamp." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2]).toEqual({ to: "doing", note: "Wrong lamp.", revision: "3", as: "agent" });
  });

  it("`r` on a tile opens it, a Russian layout's key too; never in the column itself", async () => {
    await board(returnClient(ROWS));
    tile("C-0002").focus();
    fireEvent.keyDown(tile("C-0002"), { key: "\u043a", code: "KeyR" });
    expect(await screen.findByRole("dialog", { name: "Return C-0002" })).toBeTruthy();
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    tile("C-0003").focus();
    fireEvent.keyDown(tile("C-0003"), { key: "r" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("a drop onto the return column from another column opens it; Cancel moves nothing; submit posts the note (M18)", async () => {
    const client = returnClient(ROWS);
    await board(client);
    fireEvent.dragStart(tile("C-0002"));
    dropAt(cell("Doing"), 0);
    const dialog = await screen.findByRole("dialog", { name: "Return C-0002" });
    expect(client.moveCard).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.moveCard).not.toHaveBeenCalled();
    expect(tile("C-0002").dataset.column).toBe("todo");
    fireEvent.dragStart(tile("C-0002"));
    dropAt(cell("Doing"), 0);
    const again = await screen.findByRole("dialog", { name: "Return C-0002" });
    fireEvent.change(within(again).getByLabelText("Reason"), { target: { value: "Wrong lamp." } });
    fireEvent.click(within(again).getByRole("button", { name: "Return task" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2]).toMatchObject({ to: "doing", note: "Wrong lamp." });
    expect(client.assignCard).not.toHaveBeenCalled();
  });

  it("a drop inside the return column, and Up and Down there, move with no dialog and no note", async () => {
    const client = returnClient(ROWS);
    await board(client);
    vi.spyOn(tile("C-0003"), "getBoundingClientRect").mockReturnValue({ top: 0, height: 40 } as DOMRect);
    vi.spyOn(tile("C-0004"), "getBoundingClientRect").mockReturnValue({ top: 100, height: 40 } as DOMRect);
    fireEvent.dragStart(tile("C-0004"));
    dropAt(cell("Doing"), 10);
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(client.moveCard.mock.calls[0]?.[2]).toEqual({ to: "doing", before: "C-0003", revision: "1", as: null });
    tile("C-0003").focus();
    fireEvent.keyDown(tile("C-0003"), { key: "m" });
    const menu = await screen.findByRole("menu");
    fireEvent.click(within(menu).getByRole("menuitem", { name: /^Up|^Down/ }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(2);
    });
    expect(client.moveCard.mock.calls[1]?.[2]).not.toHaveProperty("note");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("a tile in its return column says Returned with an icon, and the word goes when it leaves", async () => {
    const client = returnClient(ROWS);
    await board(client);
    const badge = within(tile("C-0003")).getByText("Returned").closest(".badge");
    expect(badge?.querySelector("svg")).not.toBeNull();
    fireEvent.click(within(tile("C-0003")).getByRole("button", { name: /^Move/, hidden: true }));
    fireEvent.click(within(await screen.findByRole("menu")).getByRole("menuitem", { name: /^To do/ }));
    await waitFor(() => {
      expect(tile("C-0003").dataset.column).toBe("todo");
    });
    expect(within(tile("C-0003")).queryByText("Returned")).toBeNull();
  });

  it("the modal: Return beside the acts, the latest reason with who and when; a card that came back long ago says its history keeps none", async () => {
    const back = aCard({
      id: "C-0003",
      title: "Came back",
      column: "doing",
      history: [
        { seq: 1, type: "card.moved", at: "2026-11-01T09:00:00Z", payload: { id: "C-0003", from: "todo", to: "doing", as: "Ivo", note: "First reason." } },
        { seq: 2, type: "card.moved", at: "2026-11-02T09:00:00Z", payload: { id: "C-0003", from: "doing", to: "check", as: "Ann", note: null } },
        { seq: 3, type: "card.moved", at: "2026-11-03T09:00:00Z", payload: { id: "C-0003", from: "check", to: "doing", as: "Lena", note: "Second reason." } },
      ],
    });
    const long = aCard({ id: "C-0004", title: "Long ago", column: "doing", history: [{ seq: 9, type: "card.edited", at: "2026-11-03T09:00:00Z", payload: { id: "C-0004", fields: ["goal"], as: "Ann" } }] });
    const open = aCard({ id: "C-0001", title: "Open one", allowed: [{ to: "doing", by: ["lead"] }] });
    renderApp(returnClient(ROWS, { "C-0003": back, "C-0004": long, "C-0001": open }), "#/alpha/board/C-0003");
    const modal = await screen.findByRole("dialog", { name: "Came back" });
    const part = await within(modal).findByRole("region", { name: "Returned" });
    expect(part.textContent).toContain("Returned by Lena, 2026-11-03 09:00 UTC");
    expect(part.textContent).toContain("Second reason.");
    expect(within(within(modal).getByRole("region", { name: "History" })).getAllByRole("listitem").map((line) => line.textContent)).toEqual([
      "2026-11-01 09:00 UTCMoved by Ivo from To do to DoingWhy: First reason.",
      "2026-11-02 09:00 UTCMoved by Ann from Doing to Check",
      "2026-11-03 09:00 UTCMoved by Lena from Check to DoingWhy: Second reason.",
    ]);
    expect(within(modal).queryByRole("button", { name: "Return" })).toBeNull();
    window.location.hash = "#/alpha/board/C-0004";
    const gone = await screen.findByRole("dialog", { name: "Long ago" });
    expect((await within(gone).findByRole("region", { name: "Returned" })).textContent).toContain("is no longer in this task's history");
    window.location.hash = "#/alpha/board/C-0001";
    const ready = await screen.findByRole("dialog", { name: "Open one" });
    fireEvent.click(await within(ready).findByRole("button", { name: "Return" }));
    expect(await screen.findByRole("dialog", { name: "Return C-0001" })).toBeTruthy();
    expect(Array.from(ready.querySelectorAll(".note")).map((note) => note.textContent)).toContain("Return needs lead");
  });

  it("a refusal keeps the dialog and the reason, the daemon's words under lang=en", async () => {
    const client = returnClient(ROWS);
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 409, message: "C-0002 enters `doing`, its flow's return column: say why with `--note`; nothing changed" }));
    await board(client);
    tile("C-0002").focus();
    fireEvent.keyDown(tile("C-0002"), { key: "r" });
    const dialog = await screen.findByRole("dialog", { name: "Return C-0002" });
    fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: "Kept" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    const said = await within(dialog).findByText(/enters `doing`/);
    expect(said.getAttribute("lang")).toBe("en");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Reason").value).toBe("Kept");
  });
});

import { createEvent, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { Card, CardRow, IntakeDocument } from "../api/types";
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
    expect(within(within(modal).getByRole("region", { name: "Activity" })).getAllByRole("listitem").map((line) => line.textContent)).toEqual([
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

// U2 of docs/features/card-questions.md "Acceptance criteria": a return may ask a question with it.
// A card with targets has the part, closed; opened, its three required texts and the targets, each
// checked. Submit sends the move, then, once it is taken, the question: two writes, the move first. A
// refused move asks nothing. A question not stored keeps the dialog, says the card went back and
// lists the hit; Ask anyway sends the question alone, naming it. No answer offers Ask again, the
// question alone again; with every listed hit named and nothing stored, the too-many line and no Ask
// anyway. With the part closed, the move is today's, byte for byte.

const TARGETED = [aCardRow({ id: "C-0001", revision: "3", targets: ["SCN-LAMP", "SCN-STORM"] }), aCardRow({ id: "C-0002", position: 2 }), aCardRow({ id: "C-0004", column: "doing", position: 2 })];

const NOT_STORED: IntakeDocument = {
  id: null,
  created: false,
  hits: [{ id: "PR-0009", source: "queue", status: "open", path: null, answer: null, record: null }],
  related: [],
  linked: null,
  diagnostics: [],
  notes: [],
};

/** The order the two writes went in, each by its name. */
function writesOf(client: StubClient): string[] {
  const order: string[] = [];
  const move = client.moveCard.getMockImplementation();
  client.moveCard.mockImplementation((...args) => {
    order.push("move");
    return move === undefined ? Promise.reject(new Error("no move")) : move(...args);
  });
  client.raiseOnCard.mockImplementation(() => {
    order.push("raise");
    return Promise.resolve({ id: "PR-0012", created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] });
  });
  return order;
}

async function returning(client: StubClient, id = "C-0001"): Promise<HTMLElement> {
  await board(client);
  tile(id).focus();
  fireEvent.keyDown(tile(id), { key: "r" });
  return screen.findByRole("dialog", { name: `Return ${id}` });
}

function asking(dialog: HTMLElement) {
  fireEvent.click(within(dialog).getByRole("button", { name: "Ask a question with it" }));
  fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: "Two endings contradict SCN-LAMP." } });
  fireEvent.change(within(dialog).getByLabelText("Question"), { target: { value: "Which ending stands?" } });
  fireEvent.change(within(dialog).getByLabelText("Working answer"), { target: { value: "The storm ending." } });
  fireEvent.change(within(dialog).getByLabelText("What another answer would cost"), { target: { value: "Chapter three rewritten." } });
}

describe("Return with a question (U2 of card-questions)", () => {
  it("a card with targets has the part, closed; opened, the three required texts and the targets, each checked; a card with none has no part", async () => {
    const client = returnClient(TARGETED);
    const dialog = await returning(client);
    const part = within(dialog).getByRole("button", { name: "Ask a question with it" });
    expect(part.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(part);
    expect(part.getAttribute("aria-expanded")).toBe("true");
    for (const label of ["Question", "Working answer", "What another answer would cost"]) {
      expect([label, within(dialog).getByLabelText(label).getAttribute("aria-required")]).toEqual([label, "true"]);
    }
    const about = within(dialog).getByRole("group", { name: "About" });
    expect(within(about).getAllByRole("checkbox").map((box) => [box.parentElement?.textContent.trim(), (box as HTMLInputElement).checked])).toEqual([
      ["SCN-LAMP", true],
      ["SCN-STORM", true],
    ]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    tile("C-0002").focus();
    fireEvent.keyDown(tile("C-0002"), { key: "r" });
    const bare = await screen.findByRole("dialog", { name: "Return C-0002" });
    expect(within(bare).queryByRole("button", { name: "Ask a question with it" })).toBeNull();
  });

  it("sends the move, then the question with its texts, targets and acting name (M: the question sent first)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = returnClient(TARGETED);
    const order = writesOf(client);
    const dialog = await returning(client);
    asking(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Return C-0001" })).toBeNull();
    });
    expect(order).toEqual(["move", "raise"]);
    expect(client.moveCard.mock.calls[0]?.[2]).toEqual({ to: "doing", note: "Two endings contradict SCN-LAMP.", revision: "3", as: "Ann" });
    expect(client.raiseOnCard.mock.calls).toEqual([
      ["alpha", "C-0001", { kind: "question", node_ids: ["SCN-LAMP", "SCN-STORM"], text: "Which ending stands?", working_answer: "The storm ending.", price_of_other: "Chapter three rewritten.", as: "Ann" }],
    ]);
    // The move said, then the question stored with it (U2 of card-questions, minor 6 of its review).
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("C-0001 sent back. Asked PR-0012");
  });

  it("a refused move asks nothing and shows the refusal", async () => {
    const client = returnClient(TARGETED);
    const order = writesOf(client);
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 409, message: "C-0001 is in todo: the move to `doing` is `lead`'s, not `local`'s; nothing changed" }));
    const dialog = await returning(client);
    asking(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    expect((await within(dialog).findByText(/the move to `doing` is `lead`'s/)).getAttribute("lang")).toBe("en");
    expect(client.raiseOnCard).not.toHaveBeenCalled();
    expect(order).toEqual([]);
  });

  it("not stored: the card is sent back, the dialog says so and lists the hit; Ask anyway sends the question alone, naming it (M: Ask anyway resending the move)", async () => {
    const client = returnClient(TARGETED);
    client.raiseOnCard.mockResolvedValueOnce(NOT_STORED).mockResolvedValueOnce({ id: "PR-0012", created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] });
    const dialog = await returning(client);
    asking(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    const anyway = await within(dialog).findByRole("button", { name: "Ask anyway" });
    const notice = within(dialog).getByRole("status");
    expect(notice.textContent).toContain("C-0001 sent back; the question was not asked");
    expect(notice.textContent).toContain("Already answered or asked:");
    expect(notice.textContent).toContain("PR-0009");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Reason").readOnly).toBe(true);
    expect(within(dialog).queryByText("This task changed since you opened it")).toBeNull();
    fireEvent.click(anyway);
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Return C-0001" })).toBeNull();
    });
    expect(client.moveCard).toHaveBeenCalledTimes(1);
    expect(client.raiseOnCard.mock.calls.map((call) => call[2].distinct_from)).toEqual([undefined, ["PR-0009"]]);
  });

  it("no answer: Ask again sends the question alone; every listed hit named and nothing stored: the too-many line, no Ask anyway; closing reports the move", async () => {
    const client = returnClient(TARGETED);
    client.raiseOnCard
      .mockRejectedValueOnce(new ClientError({ status: 0, message: "POST /api/projects/alpha/cards/C-0001/items: no response from the daemon (Failed to fetch)" }))
      .mockResolvedValueOnce(NOT_STORED)
      .mockResolvedValueOnce(NOT_STORED);
    const dialog = await returning(client);
    asking(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    fireEvent.click(await within(dialog).findByRole("button", { name: "Ask again" }));
    fireEvent.click(await within(dialog).findByRole("button", { name: "Ask anyway" }));
    await within(dialog).findByText("More earlier answers match than this dialog can name: ask on a terminal, naming each in --distinct-from.");
    expect(within(dialog).queryByRole("button", { name: "Ask anyway" })).toBeNull();
    expect(within(dialog).queryByRole("button", { name: "Ask again" })).toBeNull();
    expect(client.moveCard).toHaveBeenCalledTimes(1);
    expect(client.raiseOnCard.mock.calls.map((call) => call[2].distinct_from)).toEqual([undefined, undefined, ["PR-0009"]]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Return C-0001" })).toBeNull();
    });
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("C-0001 sent back");
  });

  // Minor 2 of card-questions' review: focus after a stop or a refusal goes to a field that takes
  // text, never to the reason held read-only once the card went back.
  it("a blank question stops before the move, focus on the question; after the move a refused question puts focus there too, never on the read-only reason", async () => {
    const client = returnClient(TARGETED);
    client.raiseOnCard.mockRejectedValueOnce(new ClientError({ status: 409, message: "text: 1025 bytes; at most 1024" }));
    const dialog = await returning(client);
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask a question with it" }));
    fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: "Two endings." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Fill in all three: the question, a working answer and what another answer would cost.");
    expect(client.moveCard).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(within(dialog).getByLabelText("Question"));
    asking(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask a question with it" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await within(dialog).findByText("text: 1025 bytes; at most 1024");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Reason").readOnly).toBe(true);
    await waitFor(() => {
      expect(document.activeElement).toBe(within(dialog).getByLabelText("Question"));
    });
  });

  // Minor 3 of card-questions' review: with no submit left, focus goes to the dismiss, never the body.
  it("every listed hit named and nothing stored: the submit goes, and focus with it to Close", async () => {
    const client = returnClient(TARGETED);
    client.raiseOnCard.mockResolvedValue(NOT_STORED);
    const dialog = await returning(client);
    asking(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    const anyway = await within(dialog).findByRole("button", { name: "Ask anyway" });
    anyway.focus();
    fireEvent.click(anyway);
    await within(dialog).findByText("More earlier answers match than this dialog can name: ask on a terminal, naming each in --distinct-from.");
    await waitFor(() => {
      expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Close" }));
    });
  });

  it("a move refused as changed meanwhile, the part open: the fields kept, and the next submit goes on the new revision", async () => {
    let revision = "3";
    const client = stubClient();
    client.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: "C-0001", revision, targets: ["SCN-LAMP"] }), aCardRow({ id: "C-0004", column: "doing" })], { flows: [FLOW] })));
    client.getCard.mockImplementation((_project, id) => Promise.resolve(aCard({ id, allowed: [{ to: "doing", by: [] }] })));
    client.moveCard
      .mockImplementationOnce(() => {
        revision = "4";
        return Promise.reject(new ClientError({ status: 409, message: "`C-0001` changed since this run read it: it is in todo since 2026-11-11T09:00:00Z; nothing changed" }));
      })
      .mockImplementation((_project, id) => Promise.resolve(aCardWrite(id, { column: "doing" })));
    const dialog = await returning(client);
    asking(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await within(dialog).findByText(/changed since this run read it/);
    await within(dialog).findByText("This task changed since you opened it");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Reason").value).toBe("Two endings contradict SCN-LAMP.");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Question").value).toBe("Which ending stands?");
    expect(client.raiseOnCard).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await waitFor(() => {
      expect(client.raiseOnCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls.map((call) => call[2].revision)).toEqual(["3", "4"]);
  });

  it("with the part closed, the one move is today's, byte for byte, and no question goes", async () => {
    const client = returnClient(TARGETED);
    const dialog = await returning(client);
    fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: "Wrong lamp." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(JSON.stringify(client.moveCard.mock.calls[0]?.[2])).toBe('{"to":"doing","note":"Wrong lamp.","revision":"3","as":null}');
    expect(client.raiseOnCard).not.toHaveBeenCalled();
  });
});

// V2 of docs/features/task-mandate.md "Acceptance criteria": a binding task's departure goes back as a
// return with a question, so on a binding task with a target "Ask a question with it" starts expanded;
// the person may still close it, and then the move alone goes. On a task with no mandate, or one its
// department decides, it starts closed as ever. The page sends what is typed; the daemon judges.
describe("Return on a binding task (V2 of task-mandate)", () => {
  const BINDING = [aCardRow({ id: "C-0001", revision: "3", targets: ["SCN-LAMP"], mandate: "binding" }), aCardRow({ id: "C-0002", position: 2, targets: ["SCN-LAMP"], mandate: "open" }), aCardRow({ id: "C-0003", position: 3, targets: ["SCN-LAMP"] }), aCardRow({ id: "C-0004", column: "doing" })];

  it("starts with the question part open on a binding task, its three texts required; closed on an open one and on one with none (M: askOpen ignored)", async () => {
    const client = returnClient(BINDING);
    const dialog = await returning(client);
    const part = within(dialog).getByRole("button", { name: "Ask a question with it" });
    expect(part.getAttribute("aria-expanded")).toBe("true");
    expect(within(dialog).getByLabelText("Question").getAttribute("aria-required")).toBe("true");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    for (const id of ["C-0002", "C-0003"]) {
      tile(id).focus();
      fireEvent.keyDown(tile(id), { key: "r" });
      const other = await screen.findByRole("dialog", { name: `Return ${id}` });
      expect([id, within(other).getByRole("button", { name: "Ask a question with it" }).getAttribute("aria-expanded")]).toEqual([id, "false"]);
      fireEvent.click(within(other).getByRole("button", { name: "Cancel" }));
      await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    }
  });

  it("open, a blank question stops before the move; the person may close it, and the move alone goes", async () => {
    const client = returnClient(BINDING);
    const dialog = await returning(client);
    fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: "Two endings." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Fill in all three: the question, a working answer and what another answer would cost.");
    expect(client.moveCard).not.toHaveBeenCalled();
    const part = within(dialog).getByRole("button", { name: "Ask a question with it" });
    fireEvent.click(part);
    expect(part.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2]).toEqual({ to: "doing", note: "Two endings.", revision: "3", as: null });
    expect(client.raiseOnCard).not.toHaveBeenCalled();
  });

  it("filled in, the move then the question go, as any return with a question", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = returnClient(BINDING);
    const order = writesOf(client);
    const dialog = await returning(client);
    fireEvent.change(within(dialog).getByLabelText("Reason"), { target: { value: "Departs from SCN-LAMP." } });
    fireEvent.change(within(dialog).getByLabelText("Question"), { target: { value: "May the lamp stay lit?" } });
    fireEvent.change(within(dialog).getByLabelText("Working answer"), { target: { value: "Yes, all night." } });
    fireEvent.change(within(dialog).getByLabelText("What another answer would cost"), { target: { value: "The night scene is redrawn." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Return task" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Return C-0001" })).toBeNull();
    });
    expect(order).toEqual(["move", "raise"]);
    expect(client.raiseOnCard.mock.calls[0]?.[1]).toBe("C-0001");
  });

  it("from the work item's Return as from the tile: open on a binding task", async () => {
    const card = aCard({ id: "C-0001", revision: "3", targets: ["SCN-LAMP"], mandate: "binding", allowed: [{ to: "doing", by: [] }] });
    const client = returnClient(BINDING, { "C-0001": card });
    renderApp(client, "#/alpha/board/C-0001");
    const modal = await screen.findByRole("dialog", { name: card.title });
    fireEvent.click(await within(modal).findByRole("button", { name: "Return" }));
    const dialog = await screen.findByRole("dialog", { name: "Return C-0001" });
    expect(within(dialog).getByRole("button", { name: "Ask a question with it" }).getAttribute("aria-expanded")).toBe("true");
  });
});

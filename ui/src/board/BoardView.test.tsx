import { act, cleanup, createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { sentence } from "../test/text";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { Card, CardList, CardRow, QueueEvent } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow, aCardWrite, aTrace, aTraceCard, aTraceNode, aWorkspace, aWorkspaces } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { Icon, type IconName } from "../ui/Icon";
import { ACTING_KEY } from "./acting";
import { COLUMN_SHOWN } from "./filter";

// The Board (docs/features/board-ui.md, board-handoff's modal and lanes) over a stub: what is read,
// what is posted, what is shown. A card opens as a modal dialog over a board that holds still; its
// write dialogs open over it. The whole cycle over the mock's rules is src/mocks/board.smoke.test.tsx.

function boardClient(list: CardList, cards: Record<string, Card> = {}): StubClient {
  const client = stubClient();
  client.getCards.mockImplementation(() => Promise.resolve(structuredClone(list)));
  client.getCard.mockImplementation((_project, id) => {
    const card = cards[id];
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  return client;
}

/** A column's head in the sticky header row. */
function head(title: string): HTMLElement {
  return screen.getByRole("heading", { level: 2, name: new RegExp(`^${title}\\b`) });
}

/** A column's cell in the one lane of None (or in the lane `lane` names), where its tiles stand and a drop lands. */
function column(title: string, lane = 0): HTMLElement {
  const key = head(title).dataset.columnHead ?? "";
  const found = Array.from(document.querySelectorAll<HTMLElement>("section.board-cell")).filter((cell) => cell.dataset.lane?.endsWith(`|${key}`) === true)[lane];
  if (found === undefined) {
    throw new Error(`no column ${title}`);
  }
  return found;
}

/** The card's modal, by its heading, the card's title. */
function modal(title: string): Promise<HTMLElement> {
  return screen.findByRole("dialog", { name: title });
}

/** The top dialog, once one more than the card's modal is open: a write dialog over it. */
async function topDialog(): Promise<HTMLElement> {
  return waitFor(() => {
    const all = screen.getAllByRole("dialog");
    expect(all.length).toBeGreaterThan(1);
    return all.at(-1) as HTMLElement;
  });
}

/** Only the card's modal left open. */
async function onlyModal() {
  await waitFor(() => {
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
  });
}

function card(id: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(`[data-card="${id}"]`);
  if (found === null) {
    throw new Error(`no card ${id}`);
  }
  return found;
}

function shown(): string[] {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-card]")).map((element) => element.dataset.card ?? "");
}

function spoken(): string {
  return document.querySelector('[aria-live="polite"]')?.textContent ?? "";
}

/** jsdom has no DragEvent, so a drag's height is set on the event by hand. */
function dragAt(type: "dragOver" | "drop", target: Element, clientY: number) {
  const event = createEvent[type](target);
  Object.defineProperty(event, "clientY", { value: clientY });
  fireEvent(target, event);
}

function press(key: string, target: Element | null = document.activeElement) {
  fireEvent.keyDown(target ?? document.body, { key });
}

async function open(client: StubClient, hash = "#/alpha/board") {
  renderApp(client, hash);
  await screen.findByRole("heading", { level: 1, name: "Board" });
  await waitFor(() => {
    expect(client.getCards).toHaveBeenCalled();
  });
}

async function openMenuOf(id: string): Promise<HTMLElement> {
  const element = await waitFor(() => card(id));
  element.focus();
  press("m");
  return screen.findByRole("menu", { name: `Move ${id}` });
}

const TWO_FLOWS = [
  aCardFlow({ name: "copy", disciplines: ["copy"] }),
  aCardFlow({
    name: "plates",
    disciplines: ["plates"],
    columns: ["queued", "blocked", "checked", "struck"].map((id) => ({ id, title: id })),
    moves: [{ from: "queued", to: "blocked", by: [] }],
    accept: { from: "checked", to: "struck" },
  }),
];

// DF-5 of docs/features/design-foundation.md ("The Board", "Status categories"): each column head's
// title is its category's lozenge, the count after it; the stranded head neutral with the unknown icon.
describe("the column heads' lozenges (DF-5)", () => {
  /** The path an icon draws, for telling one icon from another. */
  function iconPath(name: IconName): string {
    const { container, unmount } = render(<Icon name={name} />);
    const d = container.querySelector("path")?.getAttribute("d") ?? "";
    unmount();
    return d;
  }

  it("draws each head as its column's category lozenge, label per the table, the count after it; Not in the flow neutral with the unknown icon", async () => {
    const plates = aCardFlow({
      name: "plates",
      columns: ["queued", "blocked", "checked", "struck"].map((id) => ({ id, title: id })),
      moves: [],
      accept: { from: "checked", to: "struck" },
      return: "blocked",
    });
    const list = aCardList([aCardRow({ id: "C-0001", flow: "plates", column: "queued" }), aCardRow({ id: "C-0002", flow: "plates", column: "inked" })], { flows: [plates] });
    await open(boardClient(list));
    await waitFor(() => card("C-0002"));
    const heads = Array.from(document.querySelectorAll<HTMLElement>(".board-head > h2"));
    const looks = heads.map((one) => {
      const lozenge = one.querySelector(".lozenge");
      return [lozenge?.getAttribute("data-appearance"), lozenge?.querySelector(".lozenge-label")?.textContent, lozenge?.querySelector("path")?.getAttribute("d")];
    });
    expect(looks).toEqual([
      ["neutral", "queued", iconPath("open")],
      ["warning", "blocked", iconPath("cardReturned")],
      ["information", "checked", iconPath("taskInProgress")],
      ["success", "struck", iconPath("taskAccepted")],
      ["neutral", "Not in the flow", iconPath("unknown")],
    ]);
    // The lozenge comes first, the count after it; the head's name reads both.
    expect(heads.map((one) => one.firstElementChild?.classList.contains("lozenge"))).toEqual([true, true, true, true, true]);
    expect(head("queued").textContent).toBe("queued, 1 task");
    expect(head("Not in the flow").classList.contains("board-column-title-outside")).toBe(true);
  });
});

// docs/features/workspaces.md "The UI": the workspace's flows, judged against the read list alone.
describe("the workspace's flows", () => {
  it("a routed card of a shown flow, its card read answering before the list: the workspace's tab alone, nothing shown beyond it", async () => {
    const list = aCardList([aCardRow({ id: "C-0001", flow: "copy" }), aCardRow({ id: "C-0002", flow: "plates" })], { flows: TWO_FLOWS });
    const client = boardClient(list, { "C-0001": aCard({ id: "C-0001", flow: "copy" }) });
    client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "w-copy", flows: ["copy"] }), aWorkspace({ name: "w-plates", flows: ["plates"] })]));
    let release: () => void = () => undefined;
    client.getCards.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = () => {
            resolve(structuredClone(list));
          };
        }),
    );
    renderApp(client, "#/alpha/board/C-0001");
    await screen.findByRole("heading", { level: 1, name: "Board" });
    await waitFor(() => {
      expect(client.getCard).toHaveBeenCalled();
    });
    // The card's read has answered; the list's has not.
    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      release();
      await Promise.resolve();
    });
    expect(await screen.findByText("Showing 1 of 2 flows.")).toBeTruthy();
    expect(screen.queryAllByRole("tab")).toEqual([]);
    expect(screen.getByRole("button", { name: "Show all flows" })).toBeTruthy();
    await waitFor(() => {
      expect(card("C-0001")).toBeTruthy();
    });
  });

  it("a routed card of a hidden flow, its card read answering first: every flow, its own selected", async () => {
    const list = aCardList([aCardRow({ id: "C-0001", flow: "copy" }), aCardRow({ id: "C-0002", flow: "plates" })], { flows: TWO_FLOWS });
    const client = boardClient(list, { "C-0002": aCard({ id: "C-0002", flow: "plates" }) });
    client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "w-copy", flows: ["copy"] }), aWorkspace({ name: "w-plates", flows: ["plates"] })]));
    let release: () => void = () => undefined;
    client.getCards.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = () => {
            resolve(structuredClone(list));
          };
        }),
    );
    renderApp(client, "#/alpha/board/C-0002");
    await waitFor(() => {
      expect(client.getCard).toHaveBeenCalled();
    });
    await act(async () => {
      release();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getAllByRole("tab").map((tab) => [tab.textContent, tab.getAttribute("aria-selected")])).toEqual([
        ["copy", "false"],
        ["plates", "true"],
      ]);
    });
    expect(screen.getByRole("button", { name: "Show this department's flows" })).toBeTruthy();
  });
});

describe("the board", () => {
  it("a flow gives no tabs; its columns with titles as stored and counts (one read)", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001" }), aCardRow({ id: "C-0002" }), aCardRow({ id: "C-0003", column: "doing" })]));
    await open(client);
    await screen.findByRole("heading", { level: 2, name: /^To do/ });
    expect(screen.queryByRole("tablist")).toBeNull();
    expect(column("To do").textContent).toContain("2");
    expect(column("Doing").querySelector(".board-column-count")?.textContent).toBe(", 1 task");
    expect(column("Closed").querySelector(".board-column-count")?.textContent).toBe(", 0 tasks");
    expect(client.getCards).toHaveBeenCalledTimes(1);
  });

  it("two flows give two tabs in served order; `blocked` looks as any column (AC-08)", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", flow: "copy" }), aCardRow({ id: "C-0002", flow: "plates", column: "blocked" })], { flows: TWO_FLOWS }));
    await open(client);
    const tabs = await screen.findAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent)).toEqual(["copy", "plates"]);
    expect(shown()).toEqual(["C-0001"]);
    fireEvent.click(tabs[1] as HTMLElement);
    await screen.findByRole("heading", { level: 2, name: /^blocked/ });
    expect(shown()).toEqual(["C-0002"]);
    expect(column("blocked").className).toBe(column("queued").className);
    expect(card("C-0002").className).toBe("board-card");
    expect(card("C-0002").querySelector(".badge")).toBeNull();
    expect(client.getCards).toHaveBeenCalledTimes(1);
  });

  it("a card ID in the route opens its flow and its modal (BH-7)", async () => {
    const list = aCardList([aCardRow({ id: "C-0001", flow: "copy" }), aCardRow({ id: "C-0002", flow: "plates" })], { flows: TWO_FLOWS });
    await open(boardClient(list, { "C-0002": aCard({ id: "C-0002", flow: "plates", title: "A plate" }) }), "#/alpha/board/C-0002");
    await screen.findByRole("heading", { level: 2, name: "A plate" });
    expect(screen.getByRole("tab", { name: "plates" }).getAttribute("aria-selected")).toBe("true");
    expect(card("C-0002").getAttribute("aria-current")).toBe("true");
  });

  it("filters AND over the one answer, the count following; Unassigned; Clear filters", async () => {
    const client = boardClient(
      aCardList([
        aCardRow({ id: "C-0001", discipline: "copy", assignee: "Mira Holt" }),
        aCardRow({ id: "C-0002", discipline: "copy", assignee: "Ivo Brandt" }),
        aCardRow({ id: "C-0003", discipline: "plates", assignee: "Mira Holt" }),
        aCardRow({ id: "C-0004", discipline: "copy" }),
      ]),
    );
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(4);
    });
    const discipline = screen.getByLabelText("Discipline");
    const assignee = screen.getByLabelText("Assignee");
    fireEvent.change(discipline, { target: { value: "copy" } });
    expect(shown()).toEqual(["C-0001", "C-0002", "C-0004"]);
    fireEvent.change(assignee, { target: { value: "mira holt" } });
    expect(shown()).toEqual(["C-0001"]);
    expect(column("To do").querySelector(".board-column-count")?.textContent).toBe(", 1 task");
    fireEvent.change(assignee, { target: { value: within(assignee).getByRole<HTMLOptionElement>("option", { name: "Unassigned" }).value } });
    expect(shown()).toEqual(["C-0004"]);
    fireEvent.change(discipline, { target: { value: "plates" } });
    expect(shown()).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(shown()).toHaveLength(4);
    expect(client.getCards).toHaveBeenCalledTimes(1);
  });

  it("the filters and the tab stay for the project when the screen is left and come back", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", discipline: "copy" }), aCardRow({ id: "C-0002", discipline: "plates" })]));
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(2);
    });
    fireEvent.change(screen.getByLabelText("Discipline"), { target: { value: "plates" } });
    fireEvent.click(await within(screen.getByRole("navigation", { name: "Sections" })).findByRole("link", { name: "Inbox" }));
    window.location.hash = "#/alpha/tasks";
    await screen.findByRole("listbox", { hidden: true }).catch(() => null);
    window.location.hash = "#/alpha/board";
    await waitFor(() => {
      expect(shown()).toEqual(["C-0002"]);
    });
  });

  it("Show cancelled reads with_cancelled=true; a cancelled card shows so and leaves the count", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001" })]));
    client.getCards.mockImplementation((_project, options) =>
      Promise.resolve(aCardList([aCardRow({ id: "C-0001" }), ...(options?.with_cancelled === true ? [aCardRow({ id: "C-0002", cancelled: true, position: 0 })] : [])])),
    );
    await open(client);
    await waitFor(() => {
      expect(shown()).toEqual(["C-0001"]);
    });
    fireEvent.click(screen.getByLabelText("Show cancelled"));
    await waitFor(() => {
      expect(shown()).toEqual(["C-0001", "C-0002"]);
    });
    expect(client.getCards.mock.calls.at(-1)?.[1]).toEqual({ with_cancelled: true, marks: true });
    expect(within(card("C-0002")).getByText("Cancelled")).toBeTruthy();
    expect(within(card("C-0002")).queryByRole("button", { hidden: true })).toBeNull();
    expect(card("C-0002").draggable).toBe(false);
    expect(column("To do").querySelector(".board-column-count")?.textContent).toBe(", 1 task");
  });

  it("a card shows title, assignee or Unassigned, due, estimate, its source: the first target and +n, else By hand", async () => {
    const client = boardClient(
      aCardList([
        aCardRow({ id: "C-0001", title: "First", assignee: "Mira Holt", due: "2999-01-01", estimate: "2-3", targets: ["R-1", "R-2", "R-3"] }),
        aCardRow({ id: "C-0002", title: "Second" }),
      ]),
    );
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(2);
    });
    const first = within(card("C-0001"));
    expect(first.getByText("Mira Holt")).toBeTruthy();
    expect(first.getByText("Due 2999-01-01")).toBeTruthy();
    expect(first.getByText("Estimate 2-3")).toBeTruthy();
    expect(first.getByRole("link", { name: "R-1", hidden: true }).getAttribute("href")).toBe("#/alpha/tree/R-1/page");
    expect(first.getByText("+2")).toBeTruthy();
    const second = within(card("C-0002"));
    expect(second.getByText("Unassigned")).toBeTruthy();
    expect(second.getByText("By hand")).toBeTruthy();
  });

  it("says Overdue with an icon by the local day, never on a closed card", async () => {
    vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 2, 1, 23, 30) });
    const client = boardClient(
      aCardList([
        aCardRow({ id: "C-0001", due: "2026-02-28" }),
        aCardRow({ id: "C-0002", due: "2026-03-01" }),
        aCardRow({ id: "C-0003", due: "2026-02-28", column: "closed", accepted: true }),
      ]),
    );
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(3);
    });
    const badge = within(card("C-0001")).getByText("Overdue").closest(".badge");
    expect(badge?.querySelector("svg")).not.toBeNull();
    expect(within(card("C-0002")).queryByText("Overdue")).toBeNull();
    expect(within(card("C-0003")).queryByText("Overdue")).toBeNull();
    expect(within(card("C-0003")).getByText("Accepted")).toBeTruthy();
  });

  it("past the cap a column ends Show all <n>; pressing it shows them and keeps focus on a card", async () => {
    const rows = Array.from({ length: COLUMN_SHOWN + 5 }, (_, index) => aCardRow({ id: `C-${String(index + 1).padStart(4, "0")}` }));
    await open(boardClient(aCardList(rows)));
    const more = await screen.findByRole("button", { name: `Show all ${String(COLUMN_SHOWN + 5)}` });
    expect(shown()).toHaveLength(COLUMN_SHOWN);
    fireEvent.click(more);
    await waitFor(() => {
      expect(shown()).toHaveLength(COLUMN_SHOWN + 5);
    });
    expect(screen.queryByRole("button", { name: /^Show all/ })).toBeNull();
    await waitFor(() => {
      expect((document.activeElement as HTMLElement).dataset.card).toBe(rows[COLUMN_SHOWN]?.id);
    });
  });

  it("a row outside columns[] stands in a last lane with the daemon's note; a gone flow keeps its rows on a last tab", async () => {
    const note = "1 card(s) in work/parked, from C-0002: the column `parked` is no longer in the flow `work`: served last";
    const client = boardClient(aCardList([aCardRow({ id: "C-0001" }), aCardRow({ id: "C-0002", column: "parked" }), aCardRow({ id: "C-0003", flow: "old" })], { notes: [note, "another note"] }));
    await open(client);
    await screen.findByRole("heading", { level: 2, name: /^Not in the flow/ });
    const lane = column("Not in the flow");
    expect(lane.textContent).toContain(note);
    expect(lane.textContent).toContain("C-0002");
    expect(within(screen.getByRole("list", { name: "Notes from the daemon" })).getByText("another note")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "old" }));
    await waitFor(() => {
      expect(shown()).toEqual(["C-0003"]);
    });
  });

  it("empty: the columns and the line naming how to add a card (AC-09)", async () => {
    await open(boardClient(aCardList([])));
    expect(await screen.findByText("No tasks yet. Next: press New task, press New task on a node in the Spec tree, or have your AI call create_card.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: /^To do/ })).toBeTruthy();
  });

  it("failed: the daemon's words verbatim and Retry, a bad [card] that 503", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001" })]));
    client.getCards.mockRejectedValueOnce(new ClientError({ status: 503, message: "specengine.toml:3: card: a flow needs two columns" }));
    await open(client);
    expect(await screen.findByText("specengine.toml:3: card: a flow needs two columns")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(shown()).toEqual(["C-0001"]);
    });
  });

  it("shows text of any script as stored, and escapes what the queue escapes", async () => {
    const title = "\u0417\u0430\u043f\u0438\u0441\u0430\u0442\u044c \u043f\u0440\u0438\u043b\u0438\u0432\u044b\u202e";
    await open(boardClient(aCardList([aCardRow({ id: "C-0001", title, assignee: "\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430" })])));
    expect(await screen.findByText("\u0417\u0430\u043f\u0438\u0441\u0430\u0442\u044c \u043f\u0440\u0438\u043b\u0438\u0432\u044b\\u{202e}")).toBeTruthy();
    expect(within(card("C-0001")).getByText("\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430")).toBeTruthy();
  });
});

describe("moving", () => {
  const FLOW = aCardFlow({
    moves: [
      { from: "todo", to: "doing", by: ["desk", "lead"] },
      { from: "todo", to: "check", by: [] },
      { from: "doing", to: "check", by: [] },
    ],
  });
  const LIST = aCardList(
    [
      aCardRow({ id: "C-0001", revision: "7" }),
      aCardRow({ id: "C-0002" }),
      aCardRow({ id: "C-0003" }),
      aCardRow({ id: "C-0004", column: "check", reviewer: "Ivo Brandt", assignee: "Mira Holt" }),
    ],
    { flows: [FLOW] },
  );

  function items(menu: HTMLElement): string[] {
    return within(menu)
      .getAllByRole("menuitem")
      .map((item) => item.textContent);
  }

  it("lists the column's moves in served order, each by as needs, then Up, Down; a move's post carries the row's revision (a look-up, no judging)", async () => {
    const client = boardClient(LIST);
    await open(client);
    const menu = await openMenuOf("C-0002");
    expect(items(menu)).toEqual(["Doingneeds desk or lead", "Check", "Up", "Down"]);
    fireEvent.click(within(menu).getByRole("menuitem", { name: /^Check/ }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]).toEqual(["alpha", "C-0002", { to: "check", revision: "1", as: null }]);
  });

  it("the first card has no Up, the last no Down; Accept shows in accept.from alone", async () => {
    await open(boardClient(LIST));
    expect(items(await openMenuOf("C-0001"))).toEqual(["Doingneeds desk or lead", "Check", "Down"]);
    press("Escape");
    expect(items(await openMenuOf("C-0003"))).toEqual(["Doingneeds desk or lead", "Check", "Up"]);
    press("Escape");
    expect(items(await openMenuOf("C-0004"))).toEqual(["Accept"]);
  });

  it("Up and Down post `before`; Down to the end posts its own column", async () => {
    const client = boardClient(LIST);
    await open(client);
    fireEvent.click(within(await openMenuOf("C-0002")).getByRole("menuitem", { name: "Up" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.queryByRole("menu")).toBeNull();
    });
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: "Down" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(2);
    });
    fireEvent.click(within(await openMenuOf("C-0002")).getByRole("menuitem", { name: "Down" }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(3);
    });
    expect(client.moveCard.mock.calls.map((call) => call[2])).toEqual([
      { before: "C-0001", revision: "1", as: null },
      { before: "C-0003", revision: "7", as: null },
      { to: "todo", revision: "1", as: null },
    ]);
  });

  it("sends the typed name as `as`, trimmed; empty sends null", async () => {
    const client = boardClient(LIST);
    await open(client);
    const field = await screen.findByLabelText("Acting as");
    fireEvent.change(field, { target: { value: "  \u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430 " } });
    expect(window.localStorage.getItem(ACTING_KEY)).toBe("  \u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430 ");
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: /^Check/ }));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2].as).toBe("\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430");
  });

  it("keys alone: m, Down, Enter move the card; focus follows it, the live region speaks (AC-07)", async () => {
    let row = aCardRow({ id: "C-0001" });
    const client = stubClient();
    client.getCards.mockImplementation(() => Promise.resolve(aCardList([row], { flows: [FLOW] })));
    client.moveCard.mockImplementation((_project, id, body) => {
      row = { ...row, column: body.to ?? row.column, revision: "2" };
      return Promise.resolve(aCardWrite(id, { column: row.column }));
    });
    await open(client);
    const menu = await openMenuOf("C-0001");
    expect(document.activeElement).toBe(within(menu).getAllByRole("menuitem")[0]);
    press("ArrowDown");
    expect(document.activeElement?.textContent).toBe("Check");
    press("Enter");
    await waitFor(() => {
      expect(spoken()).toBe("C-0001 moved to Check");
    });
    expect(client.moveCard.mock.calls[0]?.[2]).toEqual({ to: "check", revision: "1", as: null });
    await waitFor(() => {
      expect(card("C-0001").dataset.column).toBe("check");
      expect(document.activeElement).toBe(card("C-0001"));
    });
    expect(column("Check").contains(card("C-0001"))).toBe(true);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("the Move button opens the same menu; Esc closes it, nothing posted, focus on the card", async () => {
    const client = boardClient(LIST);
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(4);
    });
    fireEvent.click(within(card("C-0001")).getByRole("button", { name: /^Move/, hidden: true }));
    expect(await screen.findByRole("menu", { name: "Move C-0001" })).toBeTruthy();
    press("Escape");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(card("C-0001"));
    expect(client.moveCard).not.toHaveBeenCalled();
  });

  it("a click off the menu closes it", async () => {
    await open(boardClient(LIST));
    await openMenuOf("C-0001");
    const scrim = document.querySelector("[data-menu-scrim]");
    fireEvent.click(scrim as Element);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("a card a column left behind takes its moves from its read; Accept and a closed card's `m` offer nothing to judge", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", column: "parked" }), aCardRow({ id: "C-0002", column: "closed", accepted: true })], { flows: [FLOW] }), {
      "C-0001": aCard({ id: "C-0001", column: "parked", allowed: [{ to: "todo", by: [] }] }),
    });
    await open(client);
    const menu = await openMenuOf("C-0001");
    await waitFor(() => {
      expect(items(menu)).toEqual(["To do"]);
    });
    press("Escape");
    card("C-0002").focus();
    press("m");
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("a refusal: the daemon's reason in an alert, the board read again, nothing moved", async () => {
    const client = boardClient(LIST);
    const reason = "C-0001 is in todo: the move to `doing` is `desk` or `lead`'s, not `local`'s; nothing changed";
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 409, message: reason }));
    await open(client);
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: /^Doing/ }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain(reason);
    await waitFor(() => {
      expect(client.getCards).toHaveBeenCalledTimes(2);
    });
    expect(column("To do").contains(card("C-0001"))).toBe(true);
    expect(document.activeElement).toBe(card("C-0001"));
  });

  it("a name off its grammar: the daemon's line next to the Acting as field, gone once the name changes", async () => {
    const client = boardClient(LIST);
    const line = "spec: --as: 129 bytes; at most 128";
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 400, message: line }));
    await open(client);
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: /^Check/ }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(line);
    const field = screen.getByLabelText("Acting as");
    expect(field.closest(".board-acting")?.contains(alert)).toBe(true);
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(field.getAttribute("aria-describedby")).toBe(alert.id);
    fireEvent.change(field, { target: { value: "Mira Holt" } });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("a drag is the same post: between two cards with `before`, to a column's end without", async () => {
    const client = boardClient(LIST);
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(4);
    });
    vi.spyOn(card("C-0002"), "getBoundingClientRect").mockReturnValue({ top: 100, height: 40 } as DOMRect);
    vi.spyOn(card("C-0001"), "getBoundingClientRect").mockReturnValue({ top: 0, height: 40 } as DOMRect);
    vi.spyOn(card("C-0003"), "getBoundingClientRect").mockReturnValue({ top: 200, height: 40 } as DOMRect);
    const todo = column("To do");
    const dragged = card("C-0004");
    fireEvent.dragStart(dragged);
    dragAt("dragOver", todo, 110);
    expect(card("C-0002").closest("li")?.className).toContain("board-slot-drop");
    dragAt("drop", todo, 110);
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]).toEqual(["alpha", "C-0004", { to: "todo", before: "C-0002", revision: "1", as: null }]);
    await waitFor(() => {
      expect(spoken()).toBe("C-0004 moved to To do");
    });
    fireEvent.dragStart(card("C-0001"));
    fireEvent.drop(column("Doing"));
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(2);
    });
    expect(client.moveCard.mock.calls[1]?.[2]).toEqual({ to: "doing", revision: "7", as: null });
  });

  it("a drop where the card already stands posts nothing, and a closed card does not drag", async () => {
    const client = boardClient(LIST);
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(4);
    });
    fireEvent.dragStart(card("C-0003"));
    fireEvent.drop(column("To do"));
    expect(client.moveCard).not.toHaveBeenCalled();
  });
});

describe("the keyboard", () => {
  const LIST = aCardList([aCardRow({ id: "C-0001" }), aCardRow({ id: "C-0002" }), aCardRow({ id: "C-0003" }), aCardRow({ id: "C-0004", column: "check" })]);

  async function ready(client: StubClient = boardClient(LIST, { "C-0002": aCard({ id: "C-0002" }) })) {
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(4);
    });
    return client;
  }

  it("holds one tab stop, and arrows move it: Up and Down in a cell, Left and Right across, skipping an empty one; Home and End the lane's first and last (BH-10)", async () => {
    await ready();
    const stops = () => shown().filter((id) => card(id).tabIndex === 0);
    expect(stops()).toEqual(["C-0001"]);
    expect(document.querySelectorAll('.board-grid a[tabindex="0"], .board-grid button[tabindex="0"]')).toHaveLength(0);
    card("C-0001").focus();
    const steps: [string, string][] = [
      ["ArrowDown", "C-0002"],
      ["ArrowRight", "C-0004"],
      ["ArrowLeft", "C-0001"],
      ["End", "C-0004"],
      ["Home", "C-0001"],
      ["ArrowDown", "C-0002"],
      ["ArrowDown", "C-0003"],
      ["ArrowUp", "C-0002"],
    ];
    for (const [key, id] of steps) {
      press(key);
      await waitFor(() => {
        expect(document.activeElement).toBe(card(id));
      });
    }
    expect(stops()).toEqual(["C-0002"]);
  });

  it("Enter opens the modal, focus on its heading; Esc closes it, the hash back, focus back on the card (BH-10)", async () => {
    await ready();
    card("C-0002").focus();
    press("Enter");
    const dialog = await modal("Card C-0002");
    const heading = within(dialog).getByRole("heading", { level: 2, name: "Card C-0002" });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
    expect(window.location.hash).toBe("#/alpha/board/C-0002");
    press("Escape", heading);
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/board");
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(document.activeElement).toBe(card("C-0002"));
  });

  it("n opens New card and m a card's Move menu under a Russian layout; ? still lists the keys (LG-7 of ui-languages)", async () => {
    await ready();
    card("C-0001").focus();
    fireEvent.keyDown(card("C-0001"), { key: "\u0442", code: "KeyN" });
    const dialog = await screen.findByRole("dialog", { name: "New task" });
    press("Escape", dialog.querySelector("input"));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(document.activeElement).toBe(card("C-0001"));
    fireEvent.keyDown(card("C-0001"), { key: "\u044c", code: "KeyM" });
    expect(await screen.findByRole("menu", { name: "Move C-0001" })).toBeTruthy();
    press("Escape");
    await waitFor(() => {
      expect(screen.queryByRole("menu")).toBeNull();
    });
    fireEvent.keyDown(card("C-0001"), { key: "?", code: "Digit7", shiftKey: true });
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeTruthy();
  });

  it("n opens New card, Esc closes it, focus back where it was; a letter in a text field types", async () => {
    await ready();
    card("C-0001").focus();
    press("n");
    const dialog = await screen.findByRole("dialog", { name: "New task" });
    press("Escape", dialog.querySelector("input"));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(document.activeElement).toBe(card("C-0001"));
    press("n", screen.getByLabelText("Acting as"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("? lists the keys: m, n, h, r and the lanes' Page Down and Page Up among them (BH-10)", async () => {
    await ready();
    card("C-0001").focus();
    press("?");
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText("Tasks: open the Move menu")).toBeTruthy();
    expect(within(dialog).getByText("New task")).toBeTruthy();
    expect(within(dialog).getByText("Tasks: hand the task off")).toBeTruthy();
    expect(within(dialog).getByText("Tasks: return the task")).toBeTruthy();
    const lanes = within(dialog).getByText("Tasks: the same column in the next or previous lane").closest(".shortcut");
    expect(lanes?.textContent).toContain("Page Down");
    expect(lanes?.textContent).toContain("Page Up");
  });

  it("a key with Cmd, Ctrl or Alt does nothing here", async () => {
    await ready();
    card("C-0001").focus();
    fireEvent.keyDown(card("C-0001"), { key: "m", metaKey: true });
    fireEvent.keyDown(card("C-0001"), { key: "n", ctrlKey: true });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("the card's modal", () => {
  const FULL = aCard({
    id: "C-0001",
    column: "check",
    title: "The card",
    goal: "A goal\nof two lines",
    discipline: "copy",
    targets: ["R-1", "R-2"],
    criteria: ["First criterion"],
    assignee: "Mira Holt",
    reviewer: "Ivo Brandt",
    estimate: "3",
    due: "2999-01-01",
    parent: "C-0007",
    corrects: "C-0008",
    created_by: "Lena Fisk",
    evidence: [{ kind: "url", value: "https://example.org/x", by: "Mira Holt", at: "2026-10-30T09:10:00Z" }],
    history: [
      { seq: 12, type: "card.moved", at: "2026-10-30T09:00:00Z", payload: { id: "C-0001", from: "todo", to: "check", as: "Mira Holt" } },
      { seq: 9, type: "card.created", at: "2026-10-29T09:00:00Z", payload: { id: "C-0001", column: "todo", as: "Lena Fisk" } },
      { seq: 13, type: "card.future", at: "2026-10-30T09:30:00Z", payload: null },
    ],
  });

  async function panelOf(document: Card) {
    await open(boardClient(aCardList([aCardRow({ id: document.id, column: document.column })]), { [document.id]: document }), `#/alpha/board/${document.id}`);
    return modal(document.title);
  }

  it("lists the fields as stored; targets link to the tree, parent and corrects to their cards (AC-09)", async () => {
    const panel = within(await panelOf(FULL));
    expect(panel.getByText(/A goal/).textContent).toBe("A goal\nof two lines");
    expect(panel.getByText("First criterion")).toBeTruthy();
    for (const value of ["copy", "3", "2999-01-01", "Mira Holt", "Ivo Brandt", "Lena Fisk"]) {
      expect(panel.getAllByText(value).length).toBeGreaterThan(0);
    }
    expect(panel.getByRole("link", { name: "R-2" }).getAttribute("href")).toBe("#/alpha/tree/R-2/page");
    // The parent is in the breadcrumbs too (DF-8 of design-foundation): its field's link, in Details.
    const details = within(panel.getByRole("region", { name: "Details" }));
    expect(details.getByRole("link", { name: "C-0007" }).getAttribute("href")).toBe("#/alpha/board/C-0007");
    expect(details.getByRole("link", { name: "C-0008" }).getAttribute("href")).toBe("#/alpha/board/C-0008");
  });

  it("evidence: kind, value, by, at; a url is text, never a link", async () => {
    const panel = within(await panelOf(FULL));
    const part = within(panel.getByRole("region", { name: "Evidence" }));
    expect(part.getByText("url")).toBeTruthy();
    expect(part.getByText("https://example.org/x").closest("a")).toBeNull();
    expect(part.queryByRole("link")).toBeNull();
    expect(part.getByText("2026-10-30 09:10 UTC")).toBeTruthy();
  });

  // History is Activity since card-questions (docs/features/card-questions.md "Acceptance criteria", U1): a card's own events as before.
  it("history by seq: time, the event's words, who, from and to in titles; an unknown type raw", async () => {
    const panel = within(await panelOf(FULL));
    const lines = within(panel.getByRole("region", { name: "Activity" })).getAllByRole("listitem");
    expect(lines.map((line) => line.textContent)).toEqual([
      "2026-10-29 09:00 UTCCreated by Lena Fisk in To do",
      "2026-10-30 09:00 UTCMoved by Mira Holt from To do to Check",
      "2026-10-30 09:30 UTCcard.future",
    ]);
  });

  // The work item's order (DF-6 of design-foundation): Accept first and primary in accept.from, Cancel
  // card last; Move stands in the side column's Status. A card with a target asks and records after
  // Return's place (docs/features/card-questions.md "Acceptance criteria", U3).
  it("offers Accept first and primary, in accept.from alone, then Hand off, Split to departments… (U1 of department-split), Ask a question…, Record a decision…, Edit, Assign and Cancel card, with Move in the side column's Status; no Return in a flow that names none", async () => {
    const panel = within(await panelOf(FULL));
    expect(within(panel.getByRole("group", { name: "Actions" })).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Accept",
      "Hand off",
      "Split to departments…",
      "Ask a question…",
      "Record a decision…",
      "Edit",
      "Assign",
      "Cancel task",
    ]);
    expect(within(panel.getByRole("region", { name: "Status" })).getAllByRole("button").map((button) => button.textContent)).toEqual(["Move"]);
    // Each act opens a dialog and says so, Split to departments… among them; Move opens a menu.
    expect(within(panel.getByRole("group", { name: "Actions" })).getAllByRole("button").map((button) => button.getAttribute("aria-haspopup"))).toEqual(Array<string>(8).fill("dialog"));
    expect(panel.getByRole("button", { name: "Move" }).getAttribute("aria-haspopup")).toBe("menu");
  });

  it("has no Accept outside accept.from", async () => {
    const panel = within(await panelOf({ ...FULL, column: "todo" }));
    expect(within(panel.getByRole("group", { name: "Actions" })).getAllByRole("button").map((button) => button.textContent)).toEqual(["Hand off", "Split to departments…", "Ask a question…", "Record a decision…", "Edit", "Assign", "Cancel task"]);
    expect(within(panel.getByRole("region", { name: "Status" })).getAllByRole("button").map((button) => button.textContent)).toEqual(["Move"]);
  });

  // A closed card with a target still asks and records (U3 of docs/features/card-questions.md "Acceptance criteria"): no other act.
  it("an accepted card has no actions but the two that raise an item, and shows who accepted it", async () => {
    const panel = within(await panelOf({ ...FULL, column: "closed", acceptance: { by: "Ivo Brandt", at: "2026-10-30T10:00:00Z", note: "Good.", verdict: "briefed", departs: [], discrepancy: null } }));
    expect(within(panel.getByRole("group", { name: "Actions" })).getAllByRole("button").map((button) => button.textContent)).toEqual(["Ask a question…", "Record a decision…"]);
    const part = panel.getByRole("region", { name: "Acceptance" });
    expect(part.textContent).toContain("Ivo Brandt");
    expect(part.textContent).toContain("Good.");
  });

  it("a cancelled card has no actions but the two that raise an item, and shows why; with no target, none", async () => {
    const panel = within(await panelOf({ ...FULL, cancellation: { by: "Ivo Brandt", at: "2026-10-30T10:00:00Z", reason: "Dropped." } }));
    expect(within(panel.getByRole("group", { name: "Actions" })).getAllByRole("button").map((button) => button.textContent)).toEqual(["Ask a question…", "Record a decision…"]);
    expect(panel.getByRole("region", { name: "Cancellation" }).textContent).toContain("Dropped.");
  });

  it("no such card: its reason as the daemon gives it", async () => {
    await open(boardClient(aCardList([])), "#/alpha/board/C-0099");
    expect(await screen.findByText("no card C-0099")).toBeTruthy();
  });

  it("a failed read: the daemon's words and Retry", async () => {
    const client = boardClient(aCardList([]), { "C-0001": FULL });
    client.getCard.mockRejectedValueOnce(new ClientError({ status: 503, message: "the row is corrupt: evidence" }));
    await open(client, "#/alpha/board/C-0001");
    expect(await screen.findByText("the row is corrupt: evidence")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await modal("The card")).toBeTruthy();
  });
});

describe("the dialogs", () => {
  const CARD = aCard({
    id: "C-0001",
    title: "The card",
    goal: "The goal",
    targets: ["R-1"],
    criteria: ["One"],
    estimate: "3",
    due: "2999-01-01",
    assignee: "Mira Holt",
    reviewer: "Ivo Brandt",
    revision: "4",
  });

  async function action(name: string, document: Card = CARD) {
    const client = boardClient(aCardList([aCardRow({ id: document.id, column: document.column, revision: document.revision })]), { [document.id]: document });
    await open(client, `#/alpha/board/${document.id}`);
    fireEvent.click(await within(await modal(document.title)).findByRole("button", { name }));
    return { client, dialog: await topDialog() };
  }

  function type(dialog: HTMLElement, label: string, value: string) {
    fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
  }

  it("New card sends what was typed, lists one per line, empty fields as no key, the acting name", async () => {
    window.localStorage.setItem(ACTING_KEY, "Lena Fisk");
    const client = boardClient(aCardList([]));
    await open(client);
    fireEvent.click(await screen.findByRole("button", { name: "New task" }));
    const dialog = await screen.findByRole("dialog", { name: "New task" });
    type(dialog, "Title", "  A title ");
    type(dialog, "Source", "R-1\n\n R-2 ");
    type(dialog, "Criteria", "One\nTwo");
    type(dialog, "Due", "2026-12-01");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]).toEqual(["alpha", { title: "A title", target: ["R-1", "R-2"], criterion: ["One", "Two"], due: "2026-12-01", as: "Lena Fisk" }]);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(spoken()).toBe("C-0001 created in To do");
  });

  it("New card needs a title: nothing sent, said in an alert", async () => {
    const client = boardClient(aCardList([]));
    await open(client);
    fireEvent.click(await screen.findByRole("button", { name: "New task" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("A task needs a title.");
    expect(client.createCard).not.toHaveBeenCalled();
  });

  it("a refused New card keeps the dialog and the text, the daemon's words in an alert", async () => {
    const client = boardClient(aCardList([]));
    client.createCard.mockRejectedValueOnce(new ClientError({ status: 409, message: "no flow takes the discipline `x`" }));
    await open(client);
    fireEvent.click(await screen.findByRole("button", { name: "New task" }));
    const dialog = await screen.findByRole("dialog");
    type(dialog, "Title", "Kept");
    type(dialog, "Discipline", "x");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain("no flow takes the discipline `x`");
    expect(within(dialog).getByLabelText<HTMLInputElement>("Title").value).toBe("Kept");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(2);
    });
  });

  it("while a submit is out it is one call, and Esc and Cancel do nothing", async () => {
    let answer: (write: ReturnType<typeof aCardWrite>) => void = () => undefined;
    const client = boardClient(aCardList([]));
    client.createCard.mockImplementation(
      () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    );
    await open(client);
    fireEvent.click(await screen.findByRole("button", { name: "New task" }));
    const dialog = await screen.findByRole("dialog");
    type(dialog, "Title", "T");
    const submit = within(dialog).getByRole("button", { name: "Create task" });
    fireEvent.click(submit);
    fireEvent.click(submit);
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalled();
    });
    fireEvent.click(submit);
    press("Escape", within(dialog).getByLabelText("Title"));
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(client.createCard).toHaveBeenCalledTimes(1);
    await act(async () => {
      answer(aCardWrite("C-0001"));
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("Edit sends the changed fields alone, `-` for a cleared one, a list whole", async () => {
    const { client, dialog } = await action("Edit");
    expect(within(dialog).queryByLabelText("Discipline")).toBeNull();
    expect(within(dialog).queryByLabelText("Assignee")).toBeNull();
    type(dialog, "Goal", "");
    type(dialog, "Estimate", "5");
    type(dialog, "Source", "");
    type(dialog, "Criteria", "One\nTwo");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.editCard).toHaveBeenCalledTimes(1);
    });
    expect(client.editCard.mock.calls[0]).toEqual(["alpha", "C-0001", { goal: "-", target: ["-"], criterion: ["One", "Two"], estimate: "5", revision: "4", as: null }]);
  });

  it("Edit with nothing changed sends nothing", async () => {
    const { client, dialog } = await action("Edit");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Nothing to save: no field changed.");
    expect(client.editCard).not.toHaveBeenCalled();
  });

  it("Add evidence sends a kind and a value", async () => {
    const { client, dialog } = await action("Edit");
    fireEvent.change(within(dialog).getByLabelText("Kind"), { target: { value: "note" } });
    type(dialog, "Value", "Read by the desk");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.editCard).toHaveBeenCalledTimes(1);
    });
    expect(client.editCard.mock.calls[0]?.[2]).toEqual({ evidence: ["note=Read by the desk"], revision: "4", as: null });
  });

  it("Assign sends the names changed, `-` for one cleared", async () => {
    const { client, dialog } = await action("Assign");
    type(dialog, "Assignee", "");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.assignCard).toHaveBeenCalledTimes(1);
    });
    expect(client.assignCard.mock.calls[0]).toEqual(["alpha", "C-0001", { assignee: "-", revision: "4", as: null }]);
  });

  it("Cancel card needs a reason", async () => {
    const { client, dialog } = await action("Cancel task");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel task" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Say why this task is cancelled.");
    expect(client.cancelCard).not.toHaveBeenCalled();
    type(dialog, "Reason", "Dropped.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel task" }));
    await waitFor(() => {
      expect(client.cancelCard).toHaveBeenCalledTimes(1);
    });
    expect(client.cancelCard.mock.calls[0]).toEqual(["alpha", "C-0001", { reason: "Dropped.", revision: "4", as: null }]);
  });

  it("Accept is disabled while the name is empty; the name typed there is the Board's; sends as, note, revision", async () => {
    const { client, dialog } = await action("Accept", { ...CARD, column: "check" });
    const submit = within(dialog).getByRole<HTMLButtonElement>("button", { name: "Accept task" });
    expect(submit.disabled).toBe(true);
    type(dialog, "Accepting as", " Ivo Brandt ");
    expect(submit.disabled).toBe(false);
    type(dialog, "Note", "Good.");
    fireEvent.click(submit);
    await waitFor(() => {
      expect(client.acceptCard).toHaveBeenCalledTimes(1);
    });
    expect(client.acceptCard.mock.calls[0]).toEqual(["alpha", "C-0001", { as: "Ivo Brandt", note: "Good.", revision: "4", briefed: true }]);
    expect(window.localStorage.getItem(ACTING_KEY)).toBe(" Ivo Brandt ");
    await waitFor(() => {
      expect(spoken()).toBe("C-0001 is accepted");
    });
  });

  it("Accept says the name is typed and nothing verifies it", async () => {
    const { dialog } = await action("Accept", { ...CARD, column: "check" });
    expect(within(dialog).getByText("This is a convention until a named login: the name is typed and nothing verifies it.")).toBeTruthy();
  });

  it("a refused Accept stays open with the daemon's words; the next submit goes on the new revision, the text kept", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", column: "check", revision: "4" })]), { "C-0001": { ...CARD, column: "check" } });
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    client.acceptCard.mockImplementationOnce(() => {
      client.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: "C-0001", column: "check", revision: "5" })])));
      client.getCard.mockImplementation(() => Promise.resolve({ ...CARD, column: "check", revision: "5" }));
      return Promise.reject(new ClientError({ status: 409, message: "`C-0001` changed since this run read it: it is in check since 2026-10-30T09:00:00Z; nothing changed" }));
    });
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Accept" }));
    const dialog = await topDialog();
    type(dialog, "Note", "Good.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept task" }));
    expect((await within(dialog).findByText(/changed since this run read it/)).closest('[role="alert"]')).not.toBeNull();
    expect(await within(dialog).findByText("This task changed since you opened it")).toBeTruthy();
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Note").value).toBe("Good.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept task" }));
    await waitFor(() => {
      expect(client.acceptCard).toHaveBeenCalledTimes(2);
    });
    expect(client.acceptCard.mock.calls[1]?.[2]).toEqual({ as: "Ivo Brandt", note: "Good.", revision: "5", briefed: true });
  });

  it("a name off its grammar in a dialog over the modal: the line there, then in the modal, the board's field being inert beneath", async () => {
    const line = "spec: --as: 129 bytes; at most 128";
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": CARD });
    client.editCard.mockRejectedValueOnce(new ClientError({ status: 400, message: line }));
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const dialog = await topDialog();
    type(dialog, "Estimate", "5");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(line);
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await onlyModal();
    const said = within(await modal("The card")).getByRole("alert");
    expect([said.textContent, said.getAttribute("lang")]).toEqual([line, "en"]);
    expect(document.querySelector(".board-acting .field-error")).toBeNull();
  });

  it("another person's line stays in the dialog, away from the Acting as field", async () => {
    const line = "spec: reviewer: 129 bytes; at most 128";
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": CARD });
    client.assignCard.mockRejectedValueOnce(new ClientError({ status: 400, message: line }));
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Assign" }));
    const dialog = await topDialog();
    type(dialog, "Reviewer", "Somebody");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(line);
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await onlyModal();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("closes back to the trigger; one taken away by the write gives focus to the panel's heading", async () => {
    const { dialog } = await action("Assign");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Assign" }));
    });
  });
});

/** A live tail the test feeds: another writer's card event, one per call. */
function liveTail(client: StubClient) {
  let tail: Parameters<StubClient["subscribe"]>[1] = () => undefined;
  let seq = 0;
  client.subscribe = (_project, onEvent) => {
    tail = onEvent;
    return () => undefined;
  };
  return (id: string) => {
    seq += 1;
    act(() => {
      tail({ seq, type: "card.edited", payload: { id, as: "Ben" } });
    });
  };
}

function typeIn(dialog: HTMLElement, label: string, value: string) {
  fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
}

describe("a revision while a dialog is open", () => {
  it("Assign: sends only the name changed, on the new revision; the reviewer another writer set stands", async () => {
    const first = aCard({ id: "C-0001", assignee: "Mira Holt", reviewer: null, revision: "4" });
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": first });
    const revise = liveTail(client);
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Assign" }));
    const dialog = await topDialog();
    client.getCard.mockImplementation(() => Promise.resolve({ ...first, reviewer: "ben", revision: "5" }));
    revise("C-0001");
    await within(dialog).findByText("This task changed since you opened it");
    expect(within(dialog).getByText("Fields you left alone keep the new value; what you changed is sent.")).toBeTruthy();
    typeIn(dialog, "Assignee", "Omar Lind");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.assignCard).toHaveBeenCalledTimes(1);
    });
    expect(client.assignCard.mock.calls[0]).toEqual(["alpha", "C-0001", { assignee: "Omar Lind", revision: "5", as: null }]);
  });

  it("Assign with nothing typed sends nothing, whatever arrived", async () => {
    const first = aCard({ id: "C-0001", reviewer: null, revision: "4" });
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": first });
    const revise = liveTail(client);
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Assign" }));
    const dialog = await topDialog();
    client.getCard.mockImplementation(() => Promise.resolve({ ...first, reviewer: "ben", revision: "5" }));
    revise("C-0001");
    await within(dialog).findByText("This task changed since you opened it");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect((await within(dialog).findByText("Nothing to save: no name changed.")).getAttribute("role")).toBe("alert");
    expect(client.assignCard).not.toHaveBeenCalled();
  });

  it("Edit: what another writer changed stays out of the body; a field the person cleared goes as `-`", async () => {
    const first = aCard({ id: "C-0001", title: "Old title", goal: "A goal", estimate: "3", due: null, revision: "4" });
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": first });
    const revise = liveTail(client);
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const dialog = await topDialog();
    client.getCard.mockImplementation(() => Promise.resolve({ ...first, title: "New title", due: "2999-01-01", targets: ["R-9"], revision: "5" }));
    revise("C-0001");
    await within(dialog).findByText("This task changed since you opened it");
    typeIn(dialog, "Goal", "");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.editCard).toHaveBeenCalledTimes(1);
    });
    expect(client.editCard.mock.calls[0]?.[2]).toEqual({ goal: "-", revision: "5", as: null });
  });

  it("Edit: text no form holds as stored is not rewritten untouched: a goal ending in a line feed, a criterion holding one", async () => {
    const first = aCard({ id: "C-0001", goal: "A goal\n", criteria: ["one\ntwo", "three"], estimate: "3", revision: "4" });
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": first });
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const dialog = await topDialog();
    typeIn(dialog, "Estimate", "5");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.editCard).toHaveBeenCalledTimes(1);
    });
    expect(client.editCard.mock.calls[0]?.[2]).toEqual({ estimate: "5", revision: "4", as: null });
  });
});

describe("a drop with Show cancelled on", () => {
  const LIST = aCardList([
    aCardRow({ id: "C-0001" }),
    aCardRow({ id: "C-0002" }),
    aCardRow({ id: "C-0003", cancelled: true, position: 0 }),
    aCardRow({ id: "C-0004", column: "doing" }),
  ]);

  async function shownWithCancelled() {
    const client = boardClient(LIST);
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(4);
    });
    fireEvent.click(screen.getByLabelText("Show cancelled"));
    await waitFor(() => {
      expect(client.getCards.mock.calls.at(-1)?.[1]).toEqual({ with_cancelled: true, marks: true });
    });
    vi.spyOn(card("C-0003"), "getBoundingClientRect").mockReturnValue({ top: 200, height: 40 } as DOMRect);
    return client;
  }

  it("over a cancelled card lands past the last that stands: no `before`, the mark at the column's end", async () => {
    const client = await shownWithCancelled();
    const todo = column("To do");
    fireEvent.dragStart(card("C-0004"));
    dragAt("dragOver", todo, 210);
    expect(card("C-0003").closest("li")?.className).not.toContain("board-slot-drop");
    expect(todo.querySelector(".board-drop-end")).not.toBeNull();
    dragAt("drop", todo, 210);
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2]).toEqual({ to: "todo", revision: "1", as: null });
  });

  it("the last card standing, dropped there, stays: nothing posted", async () => {
    const client = await shownWithCancelled();
    const todo = column("To do");
    fireEvent.dragStart(card("C-0002"));
    dragAt("dragOver", todo, 210);
    dragAt("drop", todo, 210);
    await act(async () => {
      await Promise.resolve();
    });
    expect(client.moveCard).not.toHaveBeenCalled();
  });
});

describe("focus after a write", () => {
  function pastTheCap(extra: number) {
    let rows = [aCardRow({ id: "C-9000" }), ...Array.from({ length: COLUMN_SHOWN + extra }, (_, index) => aCardRow({ id: `C-${String(index + 1).padStart(4, "0")}`, column: "doing" }))];
    const client = stubClient();
    client.getCards.mockImplementation(() => Promise.resolve(aCardList(rows)));
    client.moveCard.mockImplementation((_project, id, body) => {
      const moved = rows.filter((row) => row.id === id).map((row) => ({ ...row, column: body.to ?? row.column, revision: "2" }));
      rows = [...rows.filter((row) => row.id !== id), ...moved];
      return Promise.resolve(aCardWrite(id, { column: body.to ?? null }));
    });
    return client;
  }

  it("a card landing past the cap reveals its column and takes focus", async () => {
    const client = pastTheCap(3);
    await open(client);
    fireEvent.click(within(await openMenuOf("C-9000")).getByRole("menuitem", { name: /^Doing/ }));
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-9000"));
    });
    expect(card("C-9000").dataset.column).toBe("doing");
    expect(shown()).toHaveLength(COLUMN_SHOWN + 4);
    expect(screen.queryByRole("button", { name: /^Show all/ })).toBeNull();
  });

  it("a card gone from the re-read: focus on a shown card, never the body", async () => {
    let rows = [aCardRow({ id: "C-0001" }), aCardRow({ id: "C-0002" })];
    const client = stubClient();
    client.getCards.mockImplementation(() => Promise.resolve(aCardList(rows)));
    client.moveCard.mockImplementation((_project, id) => {
      rows = rows.filter((row) => row.id !== id);
      return Promise.resolve(aCardWrite(id, { column: "doing" }));
    });
    await open(client);
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: /^Doing/ }));
    await waitFor(() => {
      expect(shown()).toEqual(["C-0002"]);
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-0002"));
    });
  });

  it("a new card a filter hides never takes focus later, when the filter lifts", async () => {
    const seen = aCardRow({ id: "C-0001", discipline: "copy" });
    let rows = [seen];
    const client = stubClient();
    client.getCards.mockImplementation(() => Promise.resolve(aCardList(rows)));
    client.createCard.mockImplementation(() => {
      rows = [seen, aCardRow({ id: "C-0002", discipline: "plates" })];
      return Promise.resolve(aCardWrite("C-0002"));
    });
    await open(client);
    await waitFor(() => {
      expect(shown()).toEqual(["C-0001"]);
    });
    fireEvent.change(screen.getByLabelText("Discipline"), { target: { value: "copy" } });
    fireEvent.click(screen.getByRole("button", { name: "New task" }));
    const dialog = await screen.findByRole("dialog");
    typeIn(dialog, "Title", "Hidden");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    const button = screen.getByRole("button", { name: "New task" });
    await waitFor(() => {
      expect(document.activeElement).toBe(button);
    });
    fireEvent.change(screen.getByLabelText("Discipline"), { target: { value: "" } });
    await waitFor(() => {
      expect(shown()).toEqual(["C-0001", "C-0002"]);
    });
    expect(document.activeElement).toBe(button);
  });

  it.each([
    ["Edit", "is saved", "Estimate", "5"],
    ["Assign", "is assigned", "Reviewer", "Ana Weiss"],
  ])("a real Save in the panel's %s puts focus back on its trigger, not on the board's card", async (action, words, label, value) => {
    const document_ = aCard({ id: "C-0001", estimate: "3", revision: "4" });
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": document_ });
    await open(client, "#/alpha/board/C-0001");
    const trigger = await screen.findByRole("button", { name: action });
    trigger.focus();
    fireEvent.click(trigger);
    const dialog = await topDialog();
    typeIn(dialog, label, value);
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await onlyModal();
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("button", { name: action }));
    });
    expect(spoken()).toBe(`C-0001 ${words}`);
  });

  it("a Save that takes its trigger away gives focus to the panel's heading", async () => {
    const first = aCard({ id: "C-0001", title: "The card", revision: "4" });
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": first });
    client.cancelCard.mockImplementation((_project, id) => {
      client.getCard.mockImplementation(() => Promise.resolve({ ...first, cancellation: { by: "local", at: "2026-10-30T10:00:00Z", reason: "Dropped." } }));
      return Promise.resolve(aCardWrite(id));
    });
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Cancel task" }));
    const dialog = await topDialog();
    typeIn(dialog, "Reason", "Dropped.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel task" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("heading", { level: 2, name: "The card" }));
    });
  });

  it("Accept from the panel leaves focus on the panel's heading, its button gone with the write", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const first = aCard({ id: "C-0001", title: "The card", column: "check", reviewer: "Ivo Brandt", revision: "4" });
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", column: "check", reviewer: "Ivo Brandt", revision: "4" })]), { "C-0001": first });
    client.acceptCard.mockImplementation((_project, id) => {
      client.getCard.mockImplementation(() => Promise.resolve({ ...first, column: "closed", acceptance: { by: "Ivo Brandt", at: "2026-10-30T10:00:00Z", note: null, verdict: "briefed", departs: [], discrepancy: null } }));
      return Promise.resolve(aCardWrite(id, { column: "closed" }));
    });
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Accept" }));
    const dialog = await topDialog();
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept task" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("heading", { level: 2, name: "The card" }));
    });
  });

  it("Accept from the Move menu follows the card to its new column", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    let row = aCardRow({ id: "C-0001", column: "check", reviewer: "Ivo Brandt", revision: "4" });
    const client = stubClient();
    client.getCards.mockImplementation(() => Promise.resolve(aCardList([row])));
    client.acceptCard.mockImplementation((_project, id) => {
      row = { ...row, column: "closed", accepted: true };
      return Promise.resolve(aCardWrite(id, { column: "closed" }));
    });
    await open(client);
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept task" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-0001"));
    });
    expect(card("C-0001").dataset.column).toBe("closed");
  });
});

describe("the acting field", () => {
  it("keeps what is typed when the browser refuses to store it", async () => {
    await open(boardClient(aCardList([])));
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("refused");
    });
    const field = await screen.findByLabelText<HTMLInputElement>("Acting as");
    fireEvent.change(field, { target: { value: "I" } });
    expect(field.value).toBe("I");
    fireEvent.change(field, { target: { value: "Iv" } });
    expect(field.value).toBe("Iv");
    vi.restoreAllMocks();
    fireEvent.change(field, { target: { value: "" } });
  });
});

describe("the small edges", () => {
  it("a menu whose card leaves the list closes, focus on a card, and the board's keys work again", async () => {
    let rows = [aCardRow({ id: "C-0001" }), aCardRow({ id: "C-0002" })];
    const client = stubClient();
    client.getCards.mockImplementation(() => Promise.resolve(aCardList(rows)));
    const revise = liveTail(client);
    await open(client);
    await openMenuOf("C-0001");
    rows = [aCardRow({ id: "C-0002" })];
    revise("C-0001");
    await waitFor(() => {
      expect(screen.queryByRole("menu")).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-0002"));
    });
    press("n");
    expect(await screen.findByRole("dialog", { name: "New task" })).toBeTruthy();
  });

  it("a lane takes the note naming its own place, not one whose name merely starts the same", async () => {
    const own = "1 card(s) in work/parked, from C-0001: the column `parked` is no longer in the flow `work`: served last";
    const other = "1 card(s) in work/parked-old, from C-0002: a card elsewhere";
    await open(boardClient(aCardList([aCardRow({ id: "C-0001", column: "parked" })], { notes: [own, other] })));
    await screen.findByRole("heading", { level: 2, name: /^Not in the flow/ });
    const lane = column("Not in the flow");
    expect(lane.textContent).toContain(own);
    expect(lane.textContent).not.toContain(other);
    expect(within(screen.getByRole("list", { name: "Notes from the daemon" })).getByText(other)).toBeTruthy();
  });

  it("Show all in one flow leaves a column of the same name in another cut", async () => {
    const rows = (flow: string, from: number) => Array.from({ length: COLUMN_SHOWN + 1 }, (_, index) => aCardRow({ id: `C-${String(from + index)}`, flow }));
    const client = boardClient(aCardList([...rows("a", 1000), ...rows("b", 2000)], { flows: [aCardFlow({ name: "a" }), aCardFlow({ name: "b" })] }));
    await open(client);
    fireEvent.click(await screen.findByRole("button", { name: `Show all ${String(COLUMN_SHOWN + 1)}` }));
    await waitFor(() => {
      expect(shown()).toHaveLength(COLUMN_SHOWN + 1);
    });
    fireEvent.click(screen.getByRole("tab", { name: "b" }));
    await waitFor(() => {
      expect(shown()[0]).toBe("C-2000");
    });
    expect(shown()).toHaveLength(COLUMN_SHOWN);
  });
});

describe("the card against its brief", () => {
  const SUSPECT = aCard({ id: "C-0001", title: "The card", targets: ["R-1"], revision: "4" });

  function suspectRow(fields: Partial<CardRow> = {}): CardRow {
    return aCardRow({ id: "C-0001", title: "The card", column: "todo", revision: "4", mark: "suspect", on: "R-1", ...fields });
  }

  function suspectClient(document: Card = SUSPECT, row: Partial<CardRow> = {}) {
    const client = boardClient(aCardList([suspectRow({ column: document.column, ...row })]), { "C-0001": document });
    client.getTrace.mockResolvedValue(aTrace([aTraceNode("R-1", [aTraceCard({ id: "C-0001", mark: "suspect", on: "R-1" })], { rollup: "suspect" })]));
    return client;
  }

  it("the mark is the list's: a failed trace read leaves it and its Clear, and says what it could not read", async () => {
    const client = suspectClient();
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "the index is locked" }));
    await open(client, "#/alpha/board/C-0001");
    const panel = await modal("The card");
    expect((await within(panel).findByText(sentence(/could not be read: the index is locked/))).textContent).toContain("The state of this task's targets");
    expect(within(panel).getByRole("button", { name: "Clear" }).getAttribute("aria-haspopup")).toBe("dialog");
    expect(within(card("C-0001")).getByText("Suspect")).toBeTruthy();
  });

  it("a good read, then a failed one: no tile stands on the last read, the daemon's words stand where the columns stood, Retry brings them back (AC-09)", async () => {
    const client = suspectClient(SUSPECT, { mark: "open", on: null });
    let tail: (event: QueueEvent) => void = () => undefined;
    renderApp(
      {
        ...client,
        subscribe: (_project, onEvent) => {
          tail = onEvent;
          return () => undefined;
        },
      },
      "#/alpha/board",
    );
    const tile = await waitFor(() => card("C-0001"));
    tile.focus();
    client.getCards.mockRejectedValue(new ClientError({ status: 503, message: "the index could not be read for the marks" }));
    act(() => {
      tail({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The board could not be read");
    expect(alert.textContent).toContain("the index could not be read for the marks");
    expect(document.querySelector("[data-card]")).toBeNull();
    expect(screen.queryByRole("group", { name: "Board" })).toBeNull();
    await waitFor(() => {
      expect(document.activeElement).toBe(document.querySelector(".board-results"));
    });
    client.getCards.mockImplementation(() => Promise.resolve(aCardList([suspectRow()])));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(within(card("C-0001")).getByText("Suspect")).toBeTruthy();
    });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("a failed re-read takes the panel's mark and its Clear too, and says why: a mark that may be stale is never shown", async () => {
    const client = suspectClient();
    let tail: (event: QueueEvent) => void = () => undefined;
    renderApp(
      {
        ...client,
        subscribe: (_project, onEvent) => {
          tail = onEvent;
          return () => undefined;
        },
      },
      "#/alpha/board/C-0001",
    );
    const panel = await modal("The card");
    await within(panel).findByRole("button", { name: "Clear" });
    client.getCards.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    act(() => {
      tail({ seq: 9, type: "card.moved", payload: { id: "C-0002" } });
    });
    expect((await within(panel).findByText(sentence(/^Whether the brief changed.*could not be read: the queue is locked/))).textContent).toContain("Whether the brief changed");
    // The children read failed with it, said in Subtasks; its last parts stand (docs/features/handoff-progress.md "Acceptance criteria", HP-1).
    expect(await within(within(panel).getByRole("region", { name: "Subtasks" })).findByText(sentence(/^The subtasks could not be read: the queue is locked$/))).toBeTruthy();
    expect(within(panel).queryByRole("button", { name: "Clear" })).toBeNull();
    expect(within(panel).queryByRole("region", { name: "Against the brief" })).toBeNull();
  });

  it("a failed re-read takes the Move menu", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001" })]));
    let tail: (event: QueueEvent) => void = () => undefined;
    renderApp(
      {
        ...client,
        subscribe: (_project, onEvent) => {
          tail = onEvent;
          return () => undefined;
        },
      },
      "#/alpha/board",
    );
    await openMenuOf("C-0001");
    client.getCards.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    act(() => {
      tail({ seq: 9, type: "card.moved", payload: { id: "C-0002" } });
    });
    await screen.findByRole("alert");
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("a failed re-read ends a drag under way: its tile went, and a drop on the board that comes back posts nothing", async () => {
    const rows = [aCardRow({ id: "C-0001" }), aCardRow({ id: "C-0002", position: 2 })];
    const client = boardClient(aCardList(rows));
    let tail: (event: QueueEvent) => void = () => undefined;
    renderApp(
      {
        ...client,
        subscribe: (_project, onEvent) => {
          tail = onEvent;
          return () => undefined;
        },
      },
      "#/alpha/board",
    );
    fireEvent.dragStart(await waitFor(() => card("C-0002")));
    dragAt("dragOver", column("To do"), 0);
    expect(document.querySelector(".board-slot-drop, .board-drop-end")).not.toBeNull();
    client.getCards.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    act(() => {
      tail({ seq: 9, type: "card.moved", payload: { id: "C-0003" } });
    });
    await screen.findByRole("alert");
    client.getCards.mockImplementation(() => Promise.resolve(aCardList(rows)));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => card("C-0001"));
    expect(document.querySelector(".board-slot-drop, .board-drop-end")).toBeNull();
    dragAt("dragOver", column("To do"), 0);
    expect(document.querySelector(".board-slot-drop, .board-drop-end")).toBeNull();
    dragAt("drop", column("To do"), 0);
    expect(client.moveCard).not.toHaveBeenCalled();
  });

  it("a word this build does not know, on no target, is the neutral badge on the tile and in the panel, never a clean tile", async () => {
    const client = suspectClient(SUSPECT, { mark: "pondering", on: null });
    await open(client, "#/alpha/board/C-0001");
    const panel = await modal("The card");
    const tile = await waitFor(() => card("C-0001"));
    expect(within(tile).getByText("pondering").closest(".badge")?.getAttribute("data-tone")).toBe("card-mark-unknown");
    expect(tile.getAttribute("aria-label")).toContain("pondering");
    expect(within(panel).getByRole("region", { name: "Against the brief" }).textContent).toContain("pondering");
    expect(within(panel).queryByRole("button", { name: "Clear" })).toBeNull();
  });

  it("window focus reads an open panel's trace again, for a hand edit, with the list", async () => {
    const client = suspectClient();
    await open(client, "#/alpha/board/C-0001");
    const source = await screen.findByRole("region", { name: "Source" });
    await waitFor(() => {
      expect(within(source).getByRole("link", { name: "R-1" }).closest("li")?.textContent).toContain("Suspect");
    });
    client.getTrace.mockResolvedValue(aTrace([aTraceNode("R-1", [], { rollup: "confirmed" })]));
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => {
      expect(within(source).getByRole("link", { name: "R-1" }).closest("li")?.textContent).toContain("Confirmed");
    });
  });

  it("a trace re-read that failed takes each target's word away: the panel says none it cannot vouch for", async () => {
    const client = suspectClient();
    await open(client, "#/alpha/board/C-0001");
    const source = await screen.findByRole("region", { name: "Source" });
    await waitFor(() => {
      expect(within(source).getByRole("link", { name: "R-1" }).closest("li")?.textContent).toContain("Suspect");
    });
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "the index is locked" }));
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => {
      expect(within(source).getByRole("link", { name: "R-1" }).closest("li")?.querySelector(".badge")).toBeNull();
    });
  });

  it("reads the whole trace only for an open panel: a tile needs none (trace-views)", async () => {
    const client = suspectClient();
    await open(client);
    await screen.findByText("The card");
    expect(within(card("C-0001")).getByText("Suspect")).toBeTruthy();
    expect(client.getTrace).not.toHaveBeenCalled();
  });

  it("gives each target on the panel its rollup word; a target the read does not hold has none", async () => {
    const document = aCard({ ...SUSPECT, targets: ["R-1", "GONE"] });
    await open(suspectClient(document), "#/alpha/board/C-0001");
    const source = await screen.findByRole("region", { name: "Source" });
    await waitFor(() => {
      expect(within(source).getByRole("link", { name: "R-1" }).closest("li")?.textContent).toContain("Suspect");
    });
    expect(within(source).getByRole("link", { name: "GONE" }).closest("li")?.querySelector(".badge")).toBeNull();
  });

  it("a card on a gone target: no node of the trace holds it, and the tile, the panel and Clear still stand (AC-09)", async () => {
    const client = suspectClient(aCard({ ...SUSPECT, targets: ["GONE"] }), { on: "GONE" });
    client.getTrace.mockResolvedValue(aTrace([], { notes: ["C-0001: `GONE` no longer resolves"] }));
    await open(client, "#/alpha/board/C-0001");
    const panel = await modal("The card");
    await screen.findAllByText("The card");
    const tile = await waitFor(() => card("C-0001"));
    expect(within(tile).getByText("Suspect").closest(".badge")?.getAttribute("data-tone")).toBe("card-suspect");
    expect(within(tile).getByText("GONE")).toBeTruthy();
    expect(within(panel).getByRole("region", { name: "Against the brief" }).textContent).toContain("The brief changed at GONE");
    fireEvent.click(within(panel).getByRole("button", { name: "Clear" }));
    expect(await screen.findByRole("dialog", { name: "Clear C-0001" })).toBeTruthy();
  });

  it.each([
    ["suspect", "Suspect", "card-suspect", { on: "R-1" }, "R-1"],
    ["departed", "Departed", "card-departed", { on: "R-1", discrepancy: "PR-7" }, "PR-7"],
    ["correcting", "Correcting", "card-correcting", { on: "R-1", correction: "C-0009" }, "C-0009"],
  ])("%s stands first on the tile, named, with its icon, and an accepted card's word yields to it (AC-09)", async (mark, word, tone, keys, named) => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", accepted: true, mark, ...keys })]));
    await open(client);
    const tile = await waitFor(() => card("C-0001"));
    const badge = within(tile).getByText(word).closest(".badge");
    expect([badge?.getAttribute("data-tone"), badge?.querySelector("svg") !== null]).toEqual([tone, true]);
    expect(tile.firstElementChild?.textContent).toBe(`${word}${named}`);
    expect(within(tile).queryByText("Accepted")).toBeNull();
  });

  it("an accepted card with nothing against it says so; a plain row says nothing", async () => {
    await open(boardClient(aCardList([aCardRow({ id: "C-0001", accepted: true, mark: "accepted" }), aCardRow({ id: "C-0002", position: 2 })])));
    expect(within(await waitFor(() => card("C-0001"))).getByText("Accepted")).toBeTruthy();
    expect(card("C-0002").querySelector(".board-card-mark")).toBeNull();
  });

  it("a mark moved by a live event shows on the tile with no reload (AC-09)", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", targets: ["R-1"] })]));
    let tail: (event: QueueEvent) => void = () => undefined;
    renderApp(
      {
        ...client,
        subscribe: (_project, onEvent) => {
          tail = onEvent;
          return () => undefined;
        },
      },
      "#/alpha/board",
    );
    await screen.findByText("Card C-0001");
    expect(card("C-0001").querySelector(".board-card-mark")).toBeNull();
    client.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: "C-0001", targets: ["R-1"], mark: "suspect", on: "R-1" })])));
    act(() => {
      tail({ seq: 9, type: "proposal.applied", payload: { id: "PR-1" } });
    });
    await waitFor(() => {
      expect(within(card("C-0001")).getByText("Suspect")).toBeTruthy();
    });
  });

  it("a failed mark read draws no tile: the daemon's words stand where a clean board would (AC-09)", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001" })]));
    client.getCards.mockRejectedValue(new ClientError({ status: 503, message: "the index could not be read for the marks" }));
    renderApp(client, "#/alpha/board");
    expect((await screen.findByRole("alert")).textContent).toContain("the index could not be read for the marks");
    expect(document.querySelector("[data-card]")).toBeNull();
  });

  it("Clear sends the outcome, the reason, the revision and the typed name; focus lands on the panel's heading", async () => {
    window.localStorage.setItem(ACTING_KEY, "\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430");
    const client = suspectClient();
    client.clearCard.mockImplementation((_project, id) => {
      client.getCards.mockImplementation(() => Promise.resolve(aCardList([suspectRow({ mark: "open", on: null })])));
      return Promise.resolve(aCardWrite(id));
    });
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    const dialog = await screen.findByRole("dialog", { name: "Clear C-0001" });
    typeIn(dialog, "Reason", " Wording only ");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear task" }));
    await waitFor(() => {
      expect(client.clearCard).toHaveBeenCalledTimes(1);
    });
    expect(client.clearCard.mock.calls[0]).toEqual(["alpha", "C-0001", { outcome: "no_effect", reason: "Wording only", revision: "4", as: "\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430" }]);
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("heading", { level: 2, name: "The card" }));
    });
    expect(spoken()).toBe("C-0001 is cleared");
  });

  it("Clear is disabled while the name is empty", async () => {
    await open(suspectClient(), "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    const dialog = await topDialog();
    const submit = within(dialog).getByRole<HTMLButtonElement>("button", { name: "Clear task" });
    expect(submit.disabled).toBe(true);
    typeIn(dialog, "Clearing as", "Ivo Brandt");
    expect(submit.disabled).toBe(false);
  });

  it("Needs correction on an accepted card sends the estimate, a title only when typed, and shows on an accepted card too", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const client = suspectClient({ ...SUSPECT, column: "closed", acceptance: { by: "Ivo Brandt", at: "2026-10-30T10:00:00Z", note: null, verdict: "briefed", departs: [], discrepancy: null } });
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    const dialog = await topDialog();
    fireEvent.click(within(dialog).getByRole("radio", { name: "Needs correction" }));
    typeIn(dialog, "Reason", "The rates moved.");
    typeIn(dialog, "Estimate", "2");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear task" }));
    await waitFor(() => {
      expect(client.clearCard).toHaveBeenCalledTimes(1);
    });
    expect(client.clearCard.mock.calls[0]?.[2]).toEqual({ outcome: "needs_correction", reason: "The rates moved.", estimate: "2", revision: "4", as: "Ivo Brandt" });
  });

  it("a refused clear keeps the dialog and the text, the daemon's words in an alert", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const client = suspectClient();
    client.clearCard.mockRejectedValueOnce(new ClientError({ status: 409, message: "C-0001 is not suspect: there is no mark to clear; nothing changed" }));
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    const dialog = await topDialog();
    typeIn(dialog, "Reason", "Kept");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear task" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain("C-0001 is not suspect");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Reason").value).toBe("Kept");
  });

  it("a mark this build does not know: raw, neutral, no Clear", async () => {
    const client = suspectClient(SUSPECT, { mark: "pondering" });
    await open(client, "#/alpha/board/C-0001");
    const panel = await modal("The card");
    await waitFor(() => {
      expect(within(card("C-0001")).getByText("pondering").closest(".badge")?.getAttribute("data-tone")).toBe("card-mark-unknown");
    });
    expect(within(panel).queryByRole("button", { name: "Clear" })).toBeNull();
  });

  it("Accept as departing sends the targets and the three texts, no `briefed`; a card by hand has none to depart from", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", column: "check", targets: ["R-1", "R-2"], revision: "4" }), aCardRow({ id: "C-0002", column: "check" })]));
    await open(client);
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: "Accept C-0001" });
    fireEvent.click(within(dialog).getByRole("radio", { name: "Departs from it" }));
    const submit = within(dialog).getByRole<HTMLButtonElement>("button", { name: "Accept task" });
    expect(submit.disabled).toBe(true);
    typeIn(dialog, "Departs from", "R-2");
    typeIn(dialog, "Summary", "S");
    typeIn(dialog, "What the result does", "O");
    typeIn(dialog, "What the brief says", "D");
    fireEvent.click(submit);
    await waitFor(() => {
      expect(client.acceptCard).toHaveBeenCalledTimes(1);
    });
    expect(client.acceptCard.mock.calls[0]?.[2]).toEqual({ as: "Ivo Brandt", note: null, revision: "4", departs: ["R-2"], summary: "S", observed: "O", documented: "D", gap_type: "contradicts" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    fireEvent.click(within(await openMenuOf("C-0002")).getByRole("menuitem", { name: "Accept" }));
    const manual = await screen.findByRole("dialog", { name: "Accept C-0002" });
    expect(within(manual).getByRole<HTMLInputElement>("radio", { name: "Departs from it" }).disabled).toBe(true);
  });

  it("a new estimate on a card holding one asks why and sends the reason; a first estimate does not", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", revision: "4" })]), { "C-0001": aCard({ id: "C-0001", estimate: "3", revision: "4" }) });
    await open(client, "#/alpha/board/C-0001");
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const dialog = await topDialog();
    expect(within(dialog).queryByLabelText("Why the estimate changes")).toBeNull();
    typeIn(dialog, "Estimate", "5");
    typeIn(dialog, "Why the estimate changes", "The survey grew.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(client.editCard).toHaveBeenCalledTimes(1);
    });
    expect(client.editCard.mock.calls[0]?.[2]).toEqual({ estimate: "5", reason: "The survey grew.", revision: "4", as: null });
  });

  it("history words the three new events, a clear's outcome among its details", async () => {
    const document_ = aCard({
      id: "C-0001",
      title: "The card",
      history: [
        { seq: 1, type: "card.cleared", at: "2026-10-30T09:00:00Z", payload: { id: "C-0001", outcome: "no_effect", as: "Ivo Brandt" } },
        { seq: 2, type: "card.reestimated", at: "2026-10-30T09:10:00Z", payload: { id: "C-0001", as: "Ivo Brandt" } },
        { seq: 3, type: "card.refreshed", at: "2026-10-30T09:20:00Z", payload: { id: "C-0001" } },
      ],
    });
    await open(boardClient(aCardList([aCardRow({ id: "C-0001" })]), { "C-0001": document_ }), "#/alpha/board/C-0001");
    const panel = await modal("The card");
    expect(within(within(panel).getByRole("region", { name: "Activity" })).getAllByRole("listitem").map((line) => line.querySelector("strong")?.textContent)).toEqual(["Cleared", "Re-estimated", "Refreshed"]);
  });
});

// board-handoff (RM-0085): a board that holds still, swimlanes, a drop that only moves, the lanes by
// keyboard, and a cap per cell.

describe("a board that holds still (BH-1)", () => {
  const ROWS = [aCardRow({ id: "C-0001", revision: "3" }), aCardRow({ id: "C-0002", position: 2 }), aCardRow({ id: "C-0003", column: "doing" })];

  function grid(): HTMLElement {
    const found = document.querySelector<HTMLElement>(".board-grid");
    if (found === null) {
      throw new Error("no grid");
    }
    return found;
  }

  function columnsNow(): Element[] {
    return Array.from(document.querySelectorAll(".board-head > h2, section.board-cell"));
  }

  it("opening and closing the modal leaves every column head and cell, and their order, as they were", async () => {
    await open(boardClient(aCardList(ROWS), { "C-0001": aCard({ id: "C-0001", title: "First" }) }));
    await waitFor(() => {
      expect(shown()).toHaveLength(3);
    });
    const before = columnsNow();
    expect(before.map((element) => element.textContent.replace(/,.*$/, ""))).toEqual(["To do", "Doing", "Check", "Closed", "To do", "Doing", "Check", "Closed"]);
    fireEvent.click(card("C-0001").querySelector("a.board-card-link") as Element);
    const dialog = await modal("First");
    expect(columnsNow()).toEqual(before);
    expect(grid().contains(dialog)).toBe(false);
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(columnsNow()).toEqual(before);
  });

  it("a 409 and a write's notes stand in .board-notices, after the grid and never before it (M2)", async () => {
    const client = boardClient(aCardList(ROWS));
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 409, message: "C-0001 is in todo: no move to `closed`; nothing changed" }));
    client.moveCard.mockResolvedValueOnce(aCardWrite("C-0002", { column: "doing", notes: ["C-0002: a note on the write"] }));
    await open(client);
    await waitFor(() => {
      expect(shown()).toHaveLength(3);
    });
    const before = columnsNow();
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: /^Doing/ }));
    const alert = await screen.findByRole("alert");
    const notices = document.querySelector(".board-notices");
    expect(notices?.contains(alert)).toBe(true);
    expect(grid().compareDocumentPosition(alert) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(grid().contains(alert)).toBe(false);
    expect(columnsNow()).toEqual(before);
    fireEvent.click(within(alert).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.click(within(await openMenuOf("C-0002")).getByRole("menuitem", { name: /^Doing/ }));
    const notes = await screen.findByRole("list", { name: "Notes from the daemon on the last write" });
    expect(notes.closest('[role="status"]')?.closest(".board-notices")).not.toBeNull();
    expect(grid().contains(notes)).toBe(false);
    expect(within(notes).getByText("C-0002: a note on the write").getAttribute("lang")).toBe("en");
  });
});

describe("the acting name's line holds the board still (BH-1)", () => {
  it("lies in .board-acting, laid over the board (styles.test.ts pins its rule): no column head or cell is drawn again or reordered", async () => {
    const client = boardClient(aCardList([aCardRow({ id: "C-0001" })]));
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 400, message: "spec: --as: 129 bytes; at most 128" }));
    await open(client);
    await waitFor(() => card("C-0001"));
    const before = Array.from(document.querySelectorAll(".board-head > h2, section.board-cell"));
    const tools = document.querySelector(".board-tools")?.children.length;
    fireEvent.click(within(await openMenuOf("C-0001")).getByRole("menuitem", { name: /^Doing/ }));
    const line = await screen.findByRole("alert");
    expect(line.parentElement?.classList.contains("board-acting")).toBe(true);
    expect(line.classList.contains("field-error")).toBe(true);
    expect(document.querySelector(".board-tools")?.children.length).toBe(tools);
    expect(Array.from(document.querySelectorAll(".board-head > h2, section.board-cell"))).toEqual(before);
  });
});

describe("swimlanes (BH-8)", () => {
  const ROWS = [
    aCardRow({ id: "C-0001", assignee: "Zoe" }),
    aCardRow({ id: "C-0002", assignee: "anna", column: "doing" }),
    aCardRow({ id: "C-0003", position: 2 }),
    aCardRow({ id: "C-0004", assignee: "Anna", parent: "C-0090", column: "check" }),
    aCardRow({ id: "C-0090", flow: "hidden", column: "draft", title: "A hidden flow's card" }),
  ];

  function laneHeads(): string[] {
    return Array.from(document.querySelectorAll(".board-lane-head h2")).map((head) => head.textContent);
  }

  function lanesSelect(): HTMLSelectElement {
    return screen.getByLabelText<HTMLSelectElement>("Lanes");
  }

  const LIST = aCardList(ROWS, { flows: [aCardFlow(), aCardFlow({ name: "hidden", columns: [{ id: "draft", title: "Draft" }, { id: "done", title: "Done" }], moves: [], accept: { from: "draft", to: "done" } })] });

  it("None draws one lane with no head; Assignee one per name, folded, then Unassigned; the mode stored and kept through a remount", async () => {
    const first = await (async () => {
      renderApp(boardClient(LIST), "#/alpha/board");
      await waitFor(() => card("C-0001"));
      return true;
    })();
    expect(first).toBe(true);
    expect(Array.from(lanesSelect().options).map((option) => [option.value, option.textContent])).toEqual([
      ["none", "None"],
      ["assignee", "Assignee"],
      ["parent", "Parent"],
    ]);
    expect(laneHeads()).toEqual([]);
    fireEvent.change(lanesSelect(), { target: { value: "assignee" } });
    expect(laneHeads()).toEqual(["Anna", "anna", "Zoe", "Unassigned"]);
    expect(window.localStorage.getItem("specengine.board.lanes.alpha")).toBe("assignee");
    cleanup();
    renderApp(boardClient(LIST), "#/alpha/board");
    await waitFor(() => card("C-0001"));
    expect(lanesSelect().value).toBe("assignee");
    expect(laneHeads()).toEqual(["Anna", "anna", "Zoe", "Unassigned"]);
  });

  it("an unknown stored mode reads as None", async () => {
    window.localStorage.setItem("specengine.board.lanes.alpha", "rivers");
    await open(boardClient(LIST));
    await waitFor(() => card("C-0001"));
    expect(lanesSelect().value).toBe("none");
    expect(laneHeads()).toEqual([]);
  });

  it("Parent lanes: each head names its parent, a link opening its modal, and where it stands, a hidden flow's too; then Everything else", async () => {
    window.localStorage.setItem("specengine.board.lanes.alpha", "parent");
    const client = boardClient(LIST, { "C-0090": aCard({ id: "C-0090", flow: "hidden", column: "draft", title: "A hidden flow's card" }) });
    await open(client);
    await waitFor(() => card("C-0004"));
    expect(laneHeads()).toEqual(["A hidden flow's card", "Everything else"]);
    const head = document.querySelector(".board-lane-head") as HTMLElement;
    // Where it stands (DF-5 of design-foundation): its flow's name and its column's lozenge, the raw pair gone.
    const status = head.querySelector(".board-lane-status") as HTMLElement;
    expect(status.querySelector("bdi")?.textContent).toBe("hidden");
    const lozenge = status.querySelector(".lozenge");
    expect([lozenge?.getAttribute("data-appearance"), lozenge?.querySelector(".lozenge-label")?.textContent, lozenge?.querySelector("svg")?.getAttribute("aria-hidden")]).toEqual(["neutral", "Draft", "true"]);
    expect(head.textContent).not.toContain("hidden/draft");
    const link = within(head).getByRole("link", { name: "C-0090" });
    expect(link.getAttribute("href")).toBe("#/alpha/board/C-0090");
    fireEvent.click(link);
    expect(await modal("A hidden flow's card")).toBeTruthy();
    expect(Array.from(document.querySelectorAll('[role="tab"]')).map((tab) => [tab.textContent, tab.getAttribute("aria-selected")])).toEqual([
      ["work", "true"],
      ["hidden", "false"],
    ]);
  });

  it("a collapsed lane draws no tile and each column's count, and stays collapsed through a re-read; the heads never move", async () => {
    window.localStorage.setItem("specengine.board.lanes.alpha", "assignee");
    const client = boardClient(LIST);
    const revise = liveTail(client);
    await open(client);
    await waitFor(() => card("C-0001"));
    const heads = Array.from(document.querySelectorAll(".board-head > h2"));
    const toggle = screen.getByRole("button", { name: "Zoe" });
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(document.querySelector('[data-card="C-0001"]')).toBeNull();
    const zoe = Array.from(document.querySelectorAll<HTMLElement>("section.board-cell")).filter((cell) => cell.dataset.lane?.includes("assignee:Zoe") === true);
    expect(zoe.map((cell) => cell.querySelector(".board-cell-count")?.textContent)).toEqual(["1 task", "0 tasks", "0 tasks", "0 tasks"]);
    expect(zoe.every((cell) => cell.querySelector("[data-card]") === null)).toBe(true);
    revise("C-0001");
    await waitFor(() => {
      expect(client.getCards).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByRole("button", { name: "Zoe" }).getAttribute("aria-expanded")).toBe("false");
    expect(document.querySelector('[data-card="C-0001"]')).toBeNull();
    expect(Array.from(document.querySelectorAll(".board-head > h2"))).toEqual(heads);
  });

  it("every cell is labelled by its column's head and its lane, and keyed to its head", async () => {
    window.localStorage.setItem("specengine.board.lanes.alpha", "assignee");
    await open(boardClient(LIST));
    await waitFor(() => card("C-0001"));
    const heads = Array.from(document.querySelectorAll<HTMLElement>(".board-head > h2"));
    for (const cell of Array.from(document.querySelectorAll<HTMLElement>("section.board-cell"))) {
      const [headId, laneId] = (cell.getAttribute("aria-labelledby") ?? "").split(" ");
      const head = heads.find((candidate) => candidate.id === headId);
      expect(cell.dataset.lane?.endsWith(`|${head?.dataset.columnHead ?? "?"}`)).toBe(true);
      expect(document.getElementById(laneId ?? "")?.closest(".board-lane-head")).not.toBeNull();
    }
  });
});

describe("a drop is a move (BH-9)", () => {
  it("a drop into another lane's cell of another column sends exactly one move with that column, and no assign (M11)", async () => {
    window.localStorage.setItem("specengine.board.lanes.alpha", "assignee");
    const client = boardClient(aCardList([aCardRow({ id: "C-0001", assignee: "Ann" }), aCardRow({ id: "C-0002", assignee: "Bo", column: "check" })]));
    await open(client);
    await waitFor(() => card("C-0002"));
    fireEvent.dragStart(card("C-0001"));
    // Bo's lane, the Doing column: an empty cell of another lane and another column.
    const target = Array.from(document.querySelectorAll<HTMLElement>("section.board-cell")).find((cell) => cell.dataset.lane?.includes("assignee:Bo") === true && cell.dataset.cellColumn === "doing");
    expect(target).toBeDefined();
    dragAt("dragOver", target as Element, 0);
    dragAt("drop", target as Element, 0);
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]).toEqual(["alpha", "C-0001", { to: "doing", revision: "1", as: null }]);
    expect(client.assignCard).not.toHaveBeenCalled();
    expect(client.editCard).not.toHaveBeenCalled();
  });
});

describe("lanes by keyboard (BH-10)", () => {
  it("Page Down and Page Up go to the same column in the next or previous lane holding a card there; none, focus stays; a collapsed lane is passed", async () => {
    window.localStorage.setItem("specengine.board.lanes.alpha", "assignee");
    const client = boardClient(
      aCardList([
        aCardRow({ id: "C-0001", assignee: "Ann", column: "doing" }),
        aCardRow({ id: "C-0002", assignee: "Bo" }),
        aCardRow({ id: "C-0003", assignee: "Cy", column: "doing" }),
        aCardRow({ id: "C-0004", assignee: "Di", column: "doing" }),
      ]),
    );
    await open(client);
    await waitFor(() => card("C-0004"));
    card("C-0001").focus();
    press("PageDown");
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-0003"));
    });
    press("PageUp");
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-0001"));
    });
    press("PageUp");
    expect(document.activeElement).toBe(card("C-0001"));
    fireEvent.click(screen.getByRole("button", { name: "Cy" }));
    card("C-0001").focus();
    press("PageDown");
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-0004"));
    });
    expect(card("C-0004").tabIndex).toBe(0);
  });

  it("Enter opens the modal; m, n, h and r act through letterOf, a Russian layout's keys too (M12)", async () => {
    await open(boardClient(aCardList([aCardRow({ id: "C-0001" })], { flows: [aCardFlow({ return: "doing" })] }), { "C-0001": aCard({ id: "C-0001" }) }));
    const tile = await waitFor(() => card("C-0001"));
    for (const [key, code, name] of [
      ["\u0440", "KeyH", "Hand off C-0001"],
      ["\u043a", "KeyR", "Return C-0001"],
      ["\u0442", "KeyN", "New task"],
    ] as const) {
      tile.focus();
      fireEvent.keyDown(tile, { key, code });
      const dialog = await screen.findByRole("dialog", { name });
      fireEvent.keyDown(dialog, { key: "Escape" });
      await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    }
    tile.focus();
    fireEvent.keyDown(tile, { key: "\u044c", code: "KeyM" });
    expect(await screen.findByRole("menu", { name: "Move C-0001" })).toBeTruthy();
    press("Escape");
    await waitFor(() => {
      expect(screen.queryByRole("menu")).toBeNull();
    });
    tile.focus();
    press("Enter", tile);
    expect(await modal("Card C-0001")).toBeTruthy();
  });
});

describe("scale (BH-22)", () => {
  it("5 000 cards in one flow: every cell, Assignee lanes included, draws at most 200 and Show all; a collapsed lane draws none (M25)", async () => {
    const rows = Array.from({ length: 5000 }, (_, index) => aCardRow({ id: `C-${String(index + 1).padStart(5, "0")}`, assignee: index % 2 === 0 ? "Ann" : "Bo", column: index < 4000 ? "todo" : "doing", position: index + 1 }));
    await open(boardClient(aCardList(rows)));
    await waitFor(() => {
      expect(shown()).toHaveLength(COLUMN_SHOWN + 200);
    });
    expect(screen.getByRole("button", { name: "Show all 4000" })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Lanes"), { target: { value: "assignee" } });
    // Each lane holds 2 000 in To do and 500 in Doing: four cells past the cap.
    await waitFor(() => {
      expect(screen.getAllByRole("button", { name: /^Show all/ }).map((button) => button.textContent)).toEqual(["Show all 2000", "Show all 500", "Show all 2000", "Show all 500"]);
    });
    for (const cell of Array.from(document.querySelectorAll<HTMLElement>("section.board-cell"))) {
      expect(cell.querySelectorAll("[data-card]").length).toBeLessThanOrEqual(COLUMN_SHOWN);
    }
    expect(shown()).toHaveLength(4 * COLUMN_SHOWN);
    fireEvent.click(screen.getByRole("button", { name: "Bo" }));
    expect(shown()).toHaveLength(2 * COLUMN_SHOWN);
  }, 30_000);
});

// DP-10 of docs/features/departments.md: a flow is shown by its title, a raw pair stays raw.
describe("flow titles (DP-10 of departments)", () => {
  const TITLED = aCardList(
    [aCardRow({ id: "C-0001", flow: "copy", title: "Foreword" }), aCardRow({ id: "C-0002", flow: "plates", parent: "C-0001", title: "Plate" }), aCardRow({ id: "C-0003", flow: "inks", title: "Ink" })],
    { flows: [aCardFlow({ name: "copy", title: "Copy desk" }), aCardFlow({ name: "plates" }), aCardFlow({ name: "inks", title: "Copy desk" })] },
  );

  it("labels each tab and the board's name by the flow's title, the name when untitled; equal titles stay two tabs (M: tabs labelled by name)", async () => {
    await open(boardClient(TITLED));
    await waitFor(() => {
      expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Copy desk", "plates", "Copy desk"]);
    });
    expect(screen.getByRole("group", { name: "Board Copy desk" })).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "plates" }));
    expect(await screen.findByRole("group", { name: "Board plates" })).toBeTruthy();
  });

  it("a Parent lane head and the side Flow row show the parent's flow title; the head keeps no raw pair", async () => {
    window.localStorage.setItem("specengine.board.lanes.alpha", "parent");
    await open(boardClient(TITLED, { "C-0001": aCard({ id: "C-0001", flow: "copy", title: "Foreword" }) }), "#/alpha/board");
    fireEvent.click(await screen.findByRole("tab", { name: "plates" }));
    await waitFor(() => card("C-0002"));
    const head = document.querySelector<HTMLElement>(".board-lane-head");
    expect(head?.querySelector(".board-lane-status bdi")?.textContent).toBe("Copy desk");
    expect(head?.textContent).not.toContain("copy/");
    fireEvent.click(within(head as HTMLElement).getByRole("link", { name: "C-0001" }));
    const panel = await modal("Foreword");
    const flow = Array.from(panel.querySelectorAll("dt")).find((term) => term.textContent === "Flow");
    expect(flow?.nextElementSibling?.textContent).toBe("Copy desk");
  });
});

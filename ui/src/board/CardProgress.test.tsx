import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError, type CardsOptions } from "../api/client";
import type { Card, CardFlow, CardItem, CardRow, QueueEvent } from "../api/types";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { aCard, aCardFlow, aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { enT, ruT, sentence } from "../test/text";
import { Icon, type IconName } from "../ui/Icon";
import { progressWords } from "./labels";

// docs/features/handoff-progress.md "Acceptance criteria", HP-1 and HP-3 to HP-6 on stub clients: the
// work item's own children read, the bar and its words line, Ready to accept as a status line, Relies
// on, the cancelled subtasks and their reasons. harbor-sim's cases run on the mock, in
// src/mocks/progress.smoke.test.tsx, the one place a test may import it (policy AC-05).

const FLOW = aCardFlow({ return: "back", columns: [...aCardFlow().columns, { id: "back", title: "Back" }] });

/** A child of P as the list serves it. */
function child(id: string, fields: Partial<CardRow> = {}): CardRow {
  return aCardRow({ id, parent: "P", title: `Child ${id}`, ...fields });
}

interface Reads {
  /** Every row the project holds, the cancelled among them; each read takes its own. */
  rows: CardRow[];
  /** The cards a `getCard` answers; any other, `{id, reason}`. */
  cards?: Record<string, Card>;
  flows?: CardFlow[];
}

/**
 * A stub answering the Board's list and the children read by their options, as the daemon does: a
 * cancelled row only with `with_cancelled`, the children of `parent` alone with it.
 */
function progressClient({ rows, cards = {}, flows = [FLOW] }: Reads): StubClient {
  const client = stubClient();
  client.getCards.mockImplementation((_project: string, options?: CardsOptions) =>
    Promise.resolve(
      aCardList(
        structuredClone(rows.filter((row) => (options?.with_cancelled === true || !row.cancelled) && (options?.parent === undefined || row.parent === options.parent))),
        { flows: structuredClone(flows) },
      ),
    ),
  );
  client.getCard.mockImplementation((_project: string, id: string) => {
    const card = cards[id];
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  return client;
}

/** The parent P: open in `check`, its flow's `accept.from`, unless told. */
function parent(fields: Partial<Card> = {}): Card {
  return aCard({ id: "P", title: "The parent", column: "check", goal: "What it is for", ...fields });
}

/** A cancelled child's card, its reason with it. */
function cancelledCard(id: string, reason: string): Card {
  return aCard({ id, parent: "P", title: `Child ${id}`, cancellation: { by: "Ann Lee", at: "2026-11-01T10:00:00Z", reason } });
}

/** The children read's calls. */
function childrenCalls(client: StubClient): unknown[] {
  return client.getCards.mock.calls.map((call) => call[1]).filter((options) => options?.parent !== undefined);
}

async function open(client: StubClient, hash = "#/alpha/board/P"): Promise<HTMLElement> {
  renderApp(client, hash);
  return screen.findByRole("dialog", { name: "The parent" });
}

async function subtasks(dialog: HTMLElement): Promise<HTMLElement> {
  return within(dialog).findByRole("region", { name: "Subtasks" });
}

/** The words line, once read. */
async function wordsLine(part: HTMLElement): Promise<HTMLElement> {
  return waitFor(() => {
    const line = part.querySelector<HTMLElement>(".progress-words");
    if (line === null) {
      throw new Error("no words line yet");
    }
    return line;
  });
}

/** The words line as a screen reader gets it: each count's words, the decorative separators left out. */
function spokenWords(line: HTMLElement): string[] {
  const shown = line.cloneNode(true) as HTMLElement;
  for (const hidden of Array.from(shown.querySelectorAll("[aria-hidden='true']"))) {
    hidden.remove();
  }
  return Array.from(shown.querySelectorAll(".progress-word"), (word) => word.textContent);
}

/** The separators between the words: decorative middle dots, hidden from assistive technology (L3 of the looks). */
function separatorsOf(line: HTMLElement): [string | null, string | null][] {
  return Array.from(line.querySelectorAll(".progress-separator"), (separator) => [separator.textContent, separator.getAttribute("aria-hidden")]);
}

/** The path an icon draws, for telling one icon from another. */
function iconPath(name: IconName): string {
  const { container, unmount } = render(<Icon name={name} />);
  const d = container.querySelector("path")?.getAttribute("d") ?? "";
  unmount();
  return d;
}

/** The acts' names in order, the head's group. */
function acts(dialog: HTMLElement): string[] {
  return within(within(dialog).getByRole("group", { name: "Actions" }))
    .getAllByRole("button")
    .map((button) => `${button.textContent}|${button.getAttribute("aria-disabled") ?? ""}`);
}

describe("the children read (HP-1)", () => {
  it("reads the card's children with parent and with_cancelled, never marks; the Board's read stays its own", async () => {
    const client = progressClient({ rows: [aCardRow({ id: "P", title: "The parent" }), child("A"), child("Z", { cancelled: true })], cards: { P: parent(), Z: cancelledCard("Z", "Gone") } });
    const dialog = await open(client);
    await wordsLine(await subtasks(dialog));
    // M: the read sent without with_cancelled.
    expect(childrenCalls(client)).toEqual([{ parent: "P", with_cancelled: true }]);
    expect(client.getCards.mock.calls.map((call) => call[1]).filter((options) => options?.parent === undefined)).toEqual(expect.arrayContaining([{ marks: true }]));
    expect(client.getCards.mock.calls.every((call) => call[1]?.parent === undefined || call[1].marks === undefined)).toBe(true);
    // M: the children taken from the Board's list leave the cancelled child out.
    expect(within(await subtasks(dialog)).getByRole("list", { name: "Cancelled subtasks" }).textContent).toContain("Child Z");
  });

  it("holds a skeleton labelled Reading the subtasks while the read is out; the other parts stand", async () => {
    const client = progressClient({ rows: [aCardRow({ id: "P", title: "The parent" })], cards: { P: parent() } });
    const board = client.getCards.getMockImplementation();
    client.getCards.mockImplementation((project, options, signal) => (options?.parent === undefined ? (board?.(project, options, signal) ?? new Promise(() => undefined)) : new Promise(() => undefined)));
    const dialog = await open(client);
    const part = await subtasks(dialog);
    expect(within(part).getByRole("status", { name: "Reading the subtasks" })).toBeTruthy();
    expect(within(dialog).getByRole("region", { name: "Goal" })).toBeTruthy();
  });

  it("says a failed read in the daemon's words under lang=en, with Retry; Goal, Source, Evidence and Activity stand", async () => {
    const client = progressClient({ rows: [aCardRow({ id: "P", title: "The parent" }), child("A")], cards: { P: parent() } });
    const board = client.getCards.getMockImplementation();
    client.getCards.mockImplementation((project, options, signal) =>
      options?.parent === undefined ? (board?.(project, options, signal) ?? Promise.reject(new Error("no board"))) : Promise.reject(new ClientError({ status: 503, message: "the index is busy" })),
    );
    const dialog = await open(client);
    const part = await subtasks(dialog);
    const line = await within(part).findByText(sentence(/^The subtasks could not be read: the index is busy$/));
    expect(line.querySelector("[lang='en']")?.textContent).toBe("the index is busy");
    for (const name of ["Goal", "Source", "Evidence", "Activity"]) {
      expect([name, within(dialog).queryByRole("region", { name }) !== null]).toEqual([name, true]);
    }
    client.getCards.mockImplementation((project, options, signal) => board?.(project, options, signal) ?? Promise.reject(new Error("no board")));
    fireEvent.click(within(part).getByRole("button", { name: "Retry" }));
    await wordsLine(part);
    expect(part.textContent).not.toContain("could not be read");
  });

  it("keeps the last parts when a re-read fails, saying so", async () => {
    const client = progressClient({ rows: [aCardRow({ id: "P", title: "The parent" }), child("A", { column: "closed", accepted: true })], cards: { P: parent() } });
    let tail: (event: QueueEvent) => void = () => undefined;
    renderApp(
      {
        ...client,
        subscribe: (_project, onEvent) => {
          tail = onEvent;
          return () => undefined;
        },
      },
      "#/alpha/board/P",
    );
    const dialog = await screen.findByRole("dialog", { name: "The parent" });
    const part = await subtasks(dialog);
    expect((await wordsLine(part)).textContent).toBe("1 accepted");
    client.getCards.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    act(() => {
      tail({ seq: 9, type: "card.moved", payload: { id: "A" } });
    });
    await within(part).findByText(sentence(/^The subtasks could not be read: the queue is locked$/));
    expect((await wordsLine(part)).textContent).toBe("1 accepted");
    expect(within(part).getByText("Ready to accept")).toBeTruthy();
  });
});

describe("the bar (HP-3)", () => {
  it("weighs each segment by its count, the bar hidden from assistive technology, the words line in the bar's order", async () => {
    const rows = [
      aCardRow({ id: "P", title: "The parent" }),
      child("D1", { column: "doing" }),
      child("A1", { column: "closed", accepted: true }),
      child("A2", { column: "closed", accepted: true }),
      child("D2", { column: "doing" }),
      child("A3", { column: "closed", accepted: true }),
    ];
    const dialog = await open(progressClient({ rows, cards: { P: parent() } }));
    const part = await subtasks(dialog);
    expect(spokenWords(await wordsLine(part))).toEqual(["3 accepted", "2 in progress"]);
    expect(separatorsOf(await wordsLine(part))).toEqual([["\u00b7", "true"]]);
    const bar = part.querySelector<HTMLElement>(".progress-bar");
    expect(bar?.getAttribute("aria-hidden")).toBe("true");
    expect(Array.from(bar?.querySelectorAll<HTMLElement>(".progress-segment") ?? [], (segment) => [segment.dataset.appearance, segment.style.flexGrow])).toEqual([
      ["success", "3"],
      ["information", "2"],
    ]);
    const words = Array.from((await wordsLine(part)).querySelectorAll(".progress-word"));
    expect(words.map((word) => word.querySelector("path")?.getAttribute("d"))).toEqual([iconPath("taskAccepted"), iconPath("taskInProgress")]);
  });

  it("says each count with its word: never colour alone", async () => {
    const dialog = await open(progressClient({ rows: [aCardRow({ id: "P", title: "The parent" }), child("R1", { column: "back" })], cards: { P: parent() } }));
    const line = await wordsLine(await subtasks(dialog));
    // M: a segment's words dropped, leaving colour alone.
    expect(line.textContent).toBe("1 returned");
    expect(line.querySelector("path")?.getAttribute("d")).toBe(iconPath("cardReturned"));
  });

  it("takes ru's one, few and many at 1, 3 and 5", () => {
    expect([1, 3, 5].map((count) => progressWords("accepted", count, ruT))).toEqual([
      ru.board.progress.accepted_one.replace("{{count}}", "1"),
      ru.board.progress.accepted_few.replace("{{count}}", "3"),
      ru.board.progress.accepted_many.replace("{{count}}", "5"),
    ]);
    expect(new Set([ru.board.progress.cancelled_one, ru.board.progress.cancelled_few, ru.board.progress.cancelled_many]).size).toBe(3);
    expect([1, 3, 5].map((count) => progressWords("todo", count, enT))).toEqual(["1 to do", "3 to do", "5 to do"]);
  });
});

describe("Ready to accept (HP-4)", () => {
  it("shows on an open parent whose two children are cancelled; a status line holding no control, the acts byte-equal with and without it", async () => {
    const without = await open(progressClient({ rows: [aCardRow({ id: "P", title: "The parent" }), child("T1")], cards: { P: parent() } }));
    await wordsLine(await subtasks(without));
    const before = acts(without);
    expect(within(await subtasks(without)).queryByText("Ready to accept")).toBeNull();
    cleanup();

    const rows = [aCardRow({ id: "P", title: "The parent" }), child("C1", { cancelled: true }), child("C2", { cancelled: true })];
    const dialog = await open(progressClient({ rows, cards: { P: parent(), C1: cancelledCard("C1", "One"), C2: cancelledCard("C2", "Two") } }));
    const part = await subtasks(dialog);
    const hint = await within(part).findByText("Ready to accept");
    const line = hint.closest("p") as HTMLElement;
    expect(line.textContent).toBe("Ready to acceptEvery subtask is accepted or cancelled.");
    expect(line.querySelector("button, a, [aria-disabled]")).toBeNull();
    expect(line.querySelector(".lozenge")?.getAttribute("data-appearance")).toBe("success");
    // Accept stands where the acts put it: present, enabled, first; the acts as without the hint.
    expect(acts(dialog)).toEqual(before);
    expect(acts(dialog)[0]).toBe("Accept|false");
  });

  it("shows on a parent in its flow's first column, every child accepted, and no Accept", async () => {
    const rows = [aCardRow({ id: "P", title: "The parent" }), child("A1", { column: "closed", accepted: true })];
    const dialog = await open(progressClient({ rows, cards: { P: parent({ column: "todo" }) } }));
    expect(await within(await subtasks(dialog)).findByText("Ready to accept")).toBeTruthy();
    expect(within(dialog).queryByRole("button", { name: "Accept" })).toBeNull();
  });

  const closedParents: [string, Partial<Card>][] = [
    ["an accepted parent", { acceptance: { by: "Ann Lee", at: "2026-11-01T10:00:00Z", note: null, verdict: "briefed", departs: [], discrepancy: null } }],
    ["a cancelled parent", { cancellation: { by: "Ann Lee", at: "2026-11-01T10:00:00Z", reason: "Not needed" } }],
  ];
  it.each(closedParents)("shows none on %s; the bar and the lists stand, Relies on does not", async (_case, fields) => {
    const rows = [aCardRow({ id: "P", title: "The parent" }), child("A1", { column: "closed", accepted: true }), child("T1")];
    const dialog = await open(progressClient({ rows, cards: { P: parent(fields) } }));
    const part = await subtasks(dialog);
    // M: the hint on a closed parent.
    expect(spokenWords(await wordsLine(part))).toEqual(["1 accepted", "1 to do"]);
    expect(within(part).queryByText("Ready to accept")).toBeNull();
    expect(within(part).queryByRole("list", { name: "Relies on" })).toBeNull();
  });
});

describe("Relies on (HP-5)", () => {
  it("lists the open children by ID, title and flow title, as Subtasks' rows; never an accepted or cancelled one", async () => {
    const rows = [aCardRow({ id: "P", title: "The parent" }), child("T1"), child("A1", { column: "closed", accepted: true }), child("C1", { cancelled: true }), child("S1", { flow: "gone" })];
    const dialog = await open(progressClient({ rows, cards: { P: parent(), C1: cancelledCard("C1", "One") } }));
    const list = await within(await subtasks(dialog)).findByRole("list", { name: "Relies on" });
    // M: an accepted child in it; the child's title dropped (L2 of the looks).
    expect(within(list).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["T1Child T1work", "S1Child S1gone"]);
    expect(within(list).getAllByRole("listitem").map((item) => item.querySelector(".board-handoff-title")?.textContent)).toEqual(["Child T1", "Child S1"]);
    expect(within(list).getAllByRole("link").map((link) => link.getAttribute("href"))).toEqual(["#/alpha/board/T1", "#/alpha/board/S1"]);
  });

  it("names no agent task and no item: a card with a linked task and an open item but no child has none", async () => {
    const item: CardItem = {
      id: "PR-0001",
      kind: "question",
      status: "open",
      summary: "Which tide?",
      working_answer: null,
      why: null,
      author: { type: "human", role: null, model: null, run: null },
      created_at: "2026-11-01T10:00:00Z",
      updated_at: "2026-11-01T10:00:00Z",
      staged_at: null,
      decided_by: null,
      decided_at: null,
      decision_note: null,
      record_id: null,
      record_path: null,
      record_title: null,
    };
    const card = parent({ items: [item], tasks: [{ id: "T-0001", status: "ready", title: "Plan the tide", stale: false }] });
    const dialog = await open(progressClient({ rows: [aCardRow({ id: "P", title: "The parent" })], cards: { P: card } }));
    const part = await subtasks(dialog);
    // M: an item's ID in the list.
    expect(await within(part).findByText("None yet.")).toBeTruthy();
    expect(within(part).queryByRole("list", { name: "Relies on" })).toBeNull();
    expect(part.textContent).not.toMatch(/PR-0001|T-0001/);
  });
});

describe("cancelled subtasks (HP-6)", () => {
  it("reads one card per cancelled child, none for an open or accepted one", async () => {
    const rows = [aCardRow({ id: "P", title: "The parent" }), child("T1"), child("A1", { column: "closed", accepted: true }), child("C1", { cancelled: true }), child("C2", { cancelled: true })];
    const client = progressClient({ rows, cards: { P: parent(), C1: cancelledCard("C1", "One plate holds both."), C2: cancelledCard("C2", "Folded into C1.") } });
    const dialog = await open(client);
    const list = await within(await subtasks(dialog)).findByRole("list", { name: "Cancelled subtasks" });
    await within(list).findByText("Folded into C1.");
    // M: getCard for every child.
    expect(client.getCard.mock.calls.map((call) => call[1]).sort()).toEqual(["C1", "C2", "P"]);
    expect(within(list).getAllByRole("listitem").map((item) => item.querySelector(".board-text")?.textContent)).toEqual(["One plate holds both.", "Folded into C1."]);
  });

  it("says a reason pending, then failed in the daemon's words; the bar, the rows and Ready stand either way", async () => {
    const rows = [aCardRow({ id: "P", title: "The parent" }), child("C1", { cancelled: true }), child("C2", { cancelled: true })];
    const client = progressClient({ rows, cards: { P: parent() } });
    let fail: (error: Error) => void = () => undefined;
    client.getCard.mockImplementation((_project: string, id: string) =>
      id === "P"
        ? Promise.resolve(structuredClone(parent()))
        : new Promise((_resolve, reject) => {
            fail = reject;
          }),
    );
    const dialog = await open(client);
    const part = await subtasks(dialog);
    // M: Ready waiting on the reasons.
    expect(await within(part).findByText("Ready to accept")).toBeTruthy();
    expect(within(part).getAllByRole("status", { name: "Reading why it was cancelled" })).toHaveLength(2);
    act(() => {
      fail(new ClientError({ status: 503, message: "the store is busy" }));
    });
    const reasons = await within(part).findAllByText(sentence(/^Why it was cancelled could not be read: the store is busy$/));
    // No live region: two failures make no two assertive announcements (the review's nit).
    expect(reasons.map((reason) => reason.closest("[role='alert'], [aria-live]"))).toEqual(reasons.map(() => null));
    expect(within(part).getByText("Ready to accept")).toBeTruthy();
    expect((await wordsLine(part)).textContent).toBe("2 cancelled");
  });
});

describe("the words' keys (HP-3, D8)", () => {
  it("holds every new word in both catalogs, English as the spec gives it", () => {
    expect(en.board.progress.doing_other).toBe("{{count}} in progress");
    expect(en.board.progress.stranded_one).toBe("{{count}} not in the flow");
    expect(Object.keys(ru.board.progress).filter((key) => !key.endsWith("_few") && !key.endsWith("_many")).sort()).toEqual(Object.keys(en.board.progress).sort());
  });
});

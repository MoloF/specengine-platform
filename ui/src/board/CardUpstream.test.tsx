import { act, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Card, CardRow, CardUpstream, CardUpstreamChange } from "../api/types";
import { switchLanguage } from "../i18n";
import { ru } from "../i18n/ru";
import { aCard, aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";

// docs/features/upstream-signal.md "Acceptance criteria", US-13, over the stub board (the mock's look
// runs in src/mocks/upstream.smoke.test.tsx): a row flagged `true` gains Changed above on its tile,
// after its mark, first with none; the card read's `upstream` draws a part right after the brief's
// mark: the parent's mark line, each change oldest first with its time and who, the older ones
// counted; `null` draws none. A word this build does not know stands raw under lang="en".

function anEdit(seq: number, fields: string[], by: string | null = "Ann"): CardUpstreamChange {
  return { seq, at: `2026-11-12T0${String(seq % 10)}:00:00Z`, source: "edit", by, fields, item: null, kind: null, record: null, on: null };
}

function anItem(seq: number, kind: string | null, item: string, record: string | null, on: string | null, by: string | null = "Ann Owner"): CardUpstreamChange {
  return { seq, at: `2026-11-12T1${String(seq % 10)}:00:00Z`, source: "item", by, fields: [], item, kind, record, on };
}

function anUpstream(fields: Partial<CardUpstream> = {}): CardUpstream {
  return { parent: "C-0001", mark: null, on: null, correction: null, changes: [], more: 0, ...fields };
}

/** The scenario's C-0002: its parent suspect on TIDE-BOOK, its goal edited, a decision applied on TIDE-NEAP. */
const SCENARIO = anUpstream({
  mark: "suspect",
  on: "TIDE-BOOK",
  changes: [anEdit(4, ["goal"]), anItem(9, "decision", "PR-0040", "DEC-0031", "TIDE-NEAP")],
});

function client(card: Card, rows: CardRow[] = [aCardRow({ id: card.id, title: card.title, parent: card.parent })]): StubClient {
  const stub = stubClient();
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList(rows)));
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === card.id ? structuredClone(card) : aCard({ id })));
  return stub;
}

/** The card's modal, its main column read. */
async function modal(card: Card, stub: StubClient = client(card)): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${card.id}`);
  const dialog = await screen.findByRole("dialog", { name: card.title });
  await within(dialog).findByRole("region", { name: "Details" });
  return dialog;
}

function mainSections(scope: HTMLElement): string[] {
  const main = scope.querySelector<HTMLElement>(".issue-view-main");
  if (main === null) {
    throw new Error("no main column");
  }
  return within(main)
    .getAllByRole("region")
    .map((region) => region.getAttribute("aria-label") ?? "");
}

/** The part's lines, each its text. */
function lines(part: HTMLElement): string[] {
  return within(part)
    .getAllByRole("listitem")
    .map((item) => item.textContent);
}

/** A tile on the board, once drawn. */
async function tile(id: string): Promise<HTMLElement> {
  return waitFor(() => {
    const found = document.querySelector<HTMLElement>(`[data-card="${id}"]`);
    if (found === null) {
      throw new Error(`no tile ${id}`);
    }
    return found;
  });
}

describe("the work item's Changed above (US-13)", () => {
  it("stands right after the brief's mark: the parent's mark, the edit, the decision with its record a document link, each with its time and who", async () => {
    const card = aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001", upstream: SCENARIO });
    const rows = [aCardRow({ id: "C-0002", title: card.title, parent: "C-0001", mark: "suspect", on: "TIDE-SPRING", upstream: true })];
    const dialog = await modal(card, client(card, rows));
    expect(mainSections(dialog).slice(0, 3)).toEqual(["Against the brief", "Changed above", "Source"]);
    const part = within(dialog).getByRole("region", { name: "Changed above" });
    expect(lines(part)).toEqual([
      "The parent's document changed at TIDE-BOOK.",
      "The parent's goal changed2026-11-12 04:00 UTC by Ann",
      "Decision DEC-0031 applied on TIDE-NEAP2026-11-12 19:00 UTC by Ann Owner",
    ]);
    const record = within(part).getByRole("link", { name: "DEC-0031" });
    expect(record.getAttribute("href")).toBe("#/alpha/tree/DEC-0031/page");
    expect(Array.from(part.querySelectorAll("time"), (time) => time.getAttribute("dateTime"))).toEqual(["2026-11-12T04:00:00Z", "2026-11-12T19:00:00Z"]);
    expect(Array.from(part.querySelectorAll("li > span > bdi:not(.mono)"), (name) => name.textContent)).toEqual(["Ann", "Ann Owner"]);
  });

  it("draws no part for `upstream: null`", async () => {
    const dialog = await modal(aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001" }));
    expect(mainSections(dialog)).not.toContain("Changed above");
    expect(within(dialog).queryByRole("region", { name: "Changed above" })).toBeNull();
  });

  it("names a correcting parent's correction; a question with no record links its item in the Inbox; one with no `by` names no one; the fields joined in the viewer's language", async () => {
    const upstream = anUpstream({
      mark: "correcting",
      on: "TIDE-BOOK",
      correction: "C-0009",
      changes: [anEdit(3, ["goal", "targets", "criteria"]), anItem(5, "question", "PR-0041", null, "TIDE-SPRING", null), anItem(6, "discrepancy", "PR-0042", "DISC-1", "TIDE-NEAP")],
    });
    const dialog = await modal(aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001", upstream }));
    const part = within(dialog).getByRole("region", { name: "Changed above" });
    expect(lines(part)).toEqual([
      "The parent's document changed at TIDE-BOOK; C-0009 is putting it right.",
      "The parent's goal, source, and criteria changed2026-11-12 03:00 UTC by Ann",
      "Question PR-0041 answered on TIDE-SPRING2026-11-12 15:00 UTC",
      "Discrepancy DISC-1 settled on TIDE-NEAP2026-11-12 16:00 UTC by Ann Owner",
    ]);
    expect(within(part).getByRole("link", { name: "PR-0041" }).getAttribute("href")).toBe("#/alpha/inbox/PR-0041");
  });

  it("says \"-\" for an item's ID or node the read does not serve, never the slot's name", async () => {
    const upstream = anUpstream({ changes: [anItem(7, "decision", "PR-0040", null, null)] });
    const dialog = await modal(aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001", upstream }));
    expect(lines(within(dialog).getByRole("region", { name: "Changed above" }))).toEqual(["Decision PR-0040 applied on -2026-11-12 17:00 UTC by Ann Owner"]);
  });

  it("counts one older change in the singular", async () => {
    const one = await modal(aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001", upstream: anUpstream({ changes: [anEdit(30, ["goal"])], more: 1 }) }));
    expect(lines(within(one).getByRole("region", { name: "Changed above" })).at(-1)).toBe("1 earlier change not listed");
  });

  it("ends with the count of four older changes after sixteen listed", async () => {
    const changes = Array.from({ length: 16 }, (_, index) => anEdit(40 + index, ["criteria"]));
    const dialog = await modal(aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001", upstream: anUpstream({ changes, more: 4 }) }));
    const shown = lines(within(dialog).getByRole("region", { name: "Changed above" }));
    expect([shown.length, shown.at(-1)]).toEqual([17, "4 earlier changes not listed"]);
  });

  it("shows a source, kind, field or mark word this build does not know raw, under lang=\"en\"", async () => {
    const upstream = anUpstream({
      mark: "wobbly",
      on: "TIDE-BOOK",
      changes: [anEdit(2, ["goal", "tone"]), { ...anEdit(3, []), source: "rename" }, anItem(4, "amendment", "PR-0050", null, "TIDE-NEAP")],
    });
    const dialog = await modal(aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001", upstream }));
    const part = within(dialog).getByRole("region", { name: "Changed above" });
    const raw = Array.from(part.querySelectorAll('[lang="en"]'), (element) => element.textContent);
    expect(raw).toEqual(["wobbly", "tone", "rename", "amendment"]);
    expect(lines(part).slice(0, 2)).toEqual(["wobbly TIDE-BOOK", "The parent's goal and tone changed2026-11-12 02:00 UTC by Ann"]);
    expect(lines(part)[3]).toBe("amendment PR-0050 TIDE-NEAP2026-11-12 14:00 UTC by Ann Owner");
  });

  it("speaks Russian in a Russian page: the words, the fields' list and the plural", async () => {
    switchLanguage("ru");
    const upstream = anUpstream({ mark: "suspect", on: "TIDE-BOOK", changes: [anEdit(4, ["goal", "criteria"])], more: 3 });
    renderApp(client(aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001", upstream })), "#/alpha/board/C-0002");
    const dialog = await screen.findByRole("dialog", { name: "Tide plate" });
    const part = await within(dialog).findByRole("region", { name: ru.board.upstream.label });
    // The two fields as the browser joins them in Russian; the stamp in the Russian catalog's order.
    const fields = new Intl.ListFormat("ru", { type: "conjunction" }).format([ru.board.upstream.field.goal, ru.board.upstream.field.criteria]);
    const stamp = Object.entries({ year: "2026", month: "11", day: "12", hour: "04", minute: "00" }).reduce((text, [key, value]) => text.replace(`{{${key}}}`, value), ru.ui.time.stamp);
    expect(lines(part)).toEqual([
      ru.board.upstream.mark.replace("{{on}}", "TIDE-BOOK"),
      `${ru.board.upstream.edited.replace("{{fields}}", fields)}${stamp} ${ru.board.upstream.by.replace("{{name}}", "Ann")}`,
      ru.board.upstream.more_few.replace("{{count}}", "3"),
    ]);
  });
});

describe("the tile's Changed above (US-13)", () => {
  it("follows the mark on a flagged row, first with no mark; `false` and null draw none; the tile's name says it", async () => {
    const rows = [
      aCardRow({ id: "C-0002", title: "Marked", parent: "C-0001", mark: "suspect", on: "TIDE-SPRING", upstream: true }),
      aCardRow({ id: "C-0003", title: "Plain", parent: "C-0001", upstream: true }),
      aCardRow({ id: "C-0004", title: "Quiet", parent: "C-0001", upstream: false }),
      aCardRow({ id: "C-0005", title: "Unread", parent: "C-0001", upstream: null }),
    ];
    renderApp(client(aCard({ id: "C-0002" }), rows), "#/alpha/board");
    await screen.findByRole("heading", { level: 1, name: "Board" });
    const marked = await tile("C-0002");
    const said = (scope: HTMLElement) => Array.from(scope.querySelectorAll(".board-card-mark > *"), (element) => element.getAttribute("data-tone") ?? element.textContent);
    expect(said(marked)).toEqual(["card-suspect", "TIDE-SPRING", "card-upstream"]);
    expect(marked.querySelector('.board-card-mark [data-tone="card-upstream"]')?.textContent).toBe("Changed above");
    expect(marked.querySelectorAll('[data-tone="card-upstream"] svg.icon')).toHaveLength(1);
    expect(marked.getAttribute("aria-label")).toBe("C-0002: Marked, Suspect, Changed above");
    expect(said(await tile("C-0003"))).toEqual(["card-upstream"]);
    for (const id of ["C-0004", "C-0005"]) {
      const quiet = await tile(id);
      expect([id, quiet.querySelector('[data-tone="card-upstream"]'), quiet.querySelector(".board-card-mark")]).toEqual([id, null, null]);
    }
  });
});

describe("live: a parent's write re-reads its open subtask (US-13)", () => {
  it("a `card.*` event on the parent reads the open subtask again; one on another card does not", async () => {
    const card = aCard({ id: "C-0002", title: "Tide plate", parent: "C-0001", upstream: SCENARIO });
    const stub = client(card);
    let tail: Parameters<StubClient["subscribe"]>[1] = () => undefined;
    stub.subscribe = (_project, onEvent) => {
      tail = onEvent;
      return () => undefined;
    };
    await modal(card, stub);
    const reads = () => stub.getCard.mock.calls.filter((call) => call[1] === "C-0002").length;
    const before = reads();
    act(() => {
      tail({ seq: 50, type: "card.edited", payload: { id: "C-0009", as: "Ben" } });
    });
    await waitFor(() => {
      expect(stub.getCards.mock.calls.length).toBeGreaterThan(1);
    });
    expect(reads()).toBe(before);
    act(() => {
      tail({ seq: 51, type: "card.edited", payload: { id: "C-0001", as: "Ann" } });
    });
    await waitFor(() => {
      expect(reads()).toBe(before + 1);
    });
  });
});

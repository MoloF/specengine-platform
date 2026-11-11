import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import type { Card, CardList, CardRow } from "../api/types";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";
import { aCard, aCardFlow, aCardList, aCardRow, aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { focusables } from "../ui/Dialog";

// The items' chunk and the Inbox's Confirm dialog loaded once for the file, so a lazy block or dialog never waits on its first transform.
beforeAll(async () => {
  await Promise.all([import("./CardItems"), import("../inbox/ConfirmDialog")]);
});

// DF-4, DF-6, DF-7 and DF-8 of docs/features/design-foundation.md ("The work item: the task modal and
// page"), over the stub board (the policy keeps the mock's client in src/mocks/): the card modal and
// the card page are one IssueView. At most one primary per dialog, per head's acts and per side column,
// Accept the primary in `accept.from` alone; the side column's labels and the main column's sections
// in the work item's order, empty fields folded under More fields; Development folded, its summary
// counting every link; the breadcrumbs Project, Board, the parent and the card.

const FLOW = aCardFlow({ return: "doing" });

/** A card with every field filled, in `accept.from`, a suspect mark on its row, two linked tasks (one not read here). */
const FULL: Card = aCard({
  id: "C-0001",
  column: "check",
  title: "The whole card",
  goal: "What it is for",
  discipline: "copy",
  targets: ["R-1"],
  criteria: ["Reads aloud"],
  assignee: "Mira Holt",
  reviewer: "Ivo Brandt",
  estimate: "3",
  due: "2999-01-01",
  parent: "C-0007",
  corrects: "C-0008",
  created_by: "Lena Fisk",
  evidence: [{ kind: "note", value: "Read twice", by: "Mira Holt", at: "2026-10-30T09:10:00Z" }],
  history: [{ seq: 1, type: "card.created", at: "2026-10-29T09:00:00Z", payload: { id: "C-0001", column: "todo", as: "Lena Fisk" } }],
  allowed: [
    { to: "doing", by: [] },
    { to: "todo", by: [] },
  ],
  tasks: [
    { id: "T-0001", status: "review", title: "Light the north lamp", stale: false },
    { id: "T-0002", status: null, title: null, stale: null },
  ],
  notes: ["T-0002: no such task in this queue; no state shown"],
});

/** A card with no parent, nothing optional filled, no task linked, in the flow's first column. */
const BARE: Card = aCard({ id: "C-0002", title: "A bare card" });

/** A card standing in its flow's return column, sent back with a note, with its mark. */
const RETURNED: Card = aCard({
  id: "C-0003",
  column: "doing",
  title: "Sent back",
  goal: "Again",
  history: [{ seq: 4, type: "card.moved", at: "2026-10-30T09:00:00Z", payload: { id: "C-0003", from: "check", to: "doing", note: "Two endings.", as: "Ivo Brandt" } }],
});

const CARDS: Record<string, Card> = { [FULL.id]: FULL, [BARE.id]: BARE, [RETURNED.id]: RETURNED, "C-0007": aCard({ id: "C-0007", title: "The parent" }) };

const ROWS: CardRow[] = [
  aCardRow({ id: "C-0001", column: "check", title: FULL.title, parent: "C-0007", mark: "suspect", on: "R-1" }),
  aCardRow({ id: "C-0002", title: BARE.title }),
  aCardRow({ id: "C-0003", column: "doing", title: RETURNED.title, mark: "suspect", on: "R-1" }),
  aCardRow({ id: "C-0007", column: "todo", title: "The parent" }),
];

const LIST: CardList = aCardList(ROWS, { flows: [FLOW] });

function client(cards: Record<string, Card> = CARDS): StubClient {
  const stub = stubClient();
  stub.getCards.mockImplementation(() => Promise.resolve(structuredClone(LIST)));
  stub.getCard.mockImplementation((_project, id) => {
    const card = cards[id];
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  return stub;
}

/** The card's modal over the board, read whole. */
async function modal(card: Card, stub: StubClient = client()): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${card.id}`);
  const dialog = await screen.findByRole("dialog", { name: card.title });
  await within(dialog).findByRole("region", { name: "Details" });
  return dialog;
}

/** The card's page, read whole. */
async function page(card: Card, stub: StubClient = client()): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${card.id}/page`);
  await screen.findByRole("heading", { level: 1, name: card.title });
  await screen.findByRole("region", { name: "Details" });
  return document.querySelector<HTMLElement>(".card-page") as HTMLElement;
}

function part(scope: HTMLElement, selector: string): HTMLElement {
  const found = scope.querySelector<HTMLElement>(selector);
  if (found === null) {
    throw new Error(`no ${selector}`);
  }
  return found;
}

function primaries(scope: Element): number {
  return scope.querySelectorAll(".button-primary").length;
}

/** The side column's labels in DOM order, the status's first; only those shown. */
function sideLabels(scope: HTMLElement): string[] {
  return Array.from(part(scope, ".issue-view-side").querySelectorAll<HTMLElement>(".property-label"))
    .filter((label) => label.closest("[hidden]") === null)
    .map((label) => label.textContent);
}

/** The main column's sections, by their names, in DOM order. */
function mainSections(scope: HTMLElement): string[] {
  return within(part(scope, ".issue-view-main"))
    .getAllByRole("region")
    .map((region) => region.getAttribute("aria-label") ?? "");
}

function crumbs(scope: HTMLElement): [string, string | null, string | null][] {
  const nav = within(scope).getByRole("navigation", { name: "Breadcrumbs" });
  return within(nav)
    .getAllByRole("listitem")
    .map((item) => {
      const link = item.querySelector("a");
      return [item.querySelector("bdi")?.textContent ?? "", link?.getAttribute("href") ?? null, item.querySelector('[aria-current="page"]') === null ? null : "page"];
    });
}

const SIDE_ORDER = ["Status", "Assignee", "Reviewer", "Flow", "Due", "Estimate", "Discipline", "Parent", "Corrects", "Created by", "Created at"];

describe("one primary per area (DF-4)", () => {
  it("in the modal and each dialog it opens: at most one primary per dialog, in the head's acts and in the side column; Accept the one, in accept.from", async () => {
    const dialog = await modal(FULL);
    const acts = part(dialog, ".issue-view-actions");
    expect(within(acts).getAllByRole("button").filter((button) => button.classList.contains("button-primary")).map((button) => button.textContent)).toEqual(["Accept"]);
    expect([primaries(dialog), primaries(acts), primaries(part(dialog, ".issue-view-side"))]).toEqual([1, 1, 0]);
    for (const [act, title] of [
      ["Hand off", "Hand off C-0001"],
      ["Return", "Return C-0001"],
      // card-questions (U3; minor 1 of its review): the two raise dialogs, one primary each.
      ["Ask a question…", "Ask a question on C-0001"],
      ["Record a decision…", "Record a decision on C-0001"],
      ["Edit", "Edit C-0001"],
      ["Assign", "Assign C-0001"],
      ["Cancel task", "Cancel C-0001"],
      ["Accept", "Accept C-0001"],
      ["Clear", "Clear C-0001"],
    ] as const) {
      fireEvent.click(within(dialog).getByRole("button", { name: act }));
      const over = await screen.findByRole("dialog", { name: new RegExp(`^${title}`) });
      expect([act, primaries(over) <= 1]).toEqual([act, true]);
      fireEvent.keyDown(over, { key: "Escape" });
      await waitFor(() => {
        expect(screen.getAllByRole("dialog")).toEqual([dialog]);
      });
    }
  });

  // Minor 1 of card-questions' review: Answer… staged on a hub that lets the login confirm shows
  // Confirm, the one primary; staging again is a plain button there.
  it("in Answer…, staged on a hub that confirms: one primary, Confirm", async () => {
    const asking = aCard({
      id: "C-0009",
      title: "A card that asks",
      targets: ["R-1"],
      items: [
        {
          id: "PR-0001",
          kind: "question",
          status: "open",
          summary: "Which bell?",
          working_answer: "The harbour bell.",
          why: null,
          author: { type: "human", role: null, model: null, run: null },
          created_at: "2026-11-11T08:00:00Z",
          updated_at: "2026-11-11T08:00:00Z",
          staged_at: null,
          decided_by: null,
          decided_at: null,
          decision_note: null,
          record_id: null,
          record_path: null,
          record_title: null,
        },
      ],
    });
    const stub = client({ ...CARDS, [asking.id]: asking });
    stub.state.proposals = [aProposal({ id: "PR-0001", kind: "question", summary: "Which bell?", working_answer: "The harbour bell.", updated_at: "2026-11-11T08:00:00Z" })];
    stub.getIdentity.mockResolvedValue({ login: "mira" });
    stub.getTeam.mockResolvedValue({ login: "mira", name: null, allowed: ["stage", "confirm"] });
    renderApp(stub, `#/alpha/board/${asking.id}`);
    const card = await screen.findByRole("dialog", { name: asking.title });
    const item = await within(card).findByRole("article", { name: "PR-0001" });
    fireEvent.click(within(item).getByRole("button", { name: "Answer…" }));
    const dialog = await screen.findByRole("dialog", { name: "Answer PR-0001" });
    fireEvent.click(await within(dialog).findByRole("radio", { name: "Take the working answer" }));
    expect(primaries(dialog)).toBe(1);
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(dialog).findByRole("region", { name: "Staged decision" });
    expect(within(staged).getByRole("button", { name: /^Confirm/ }).classList.contains("button-primary")).toBe(true);
    expect(primaries(dialog)).toBe(1);
    expect(within(dialog).getByRole("button", { name: "Stage 1 answer" }).classList.contains("button-primary")).toBe(false);
  });

  it("on the page alike; outside accept.from no act is primary", async () => {
    const view = await page(FULL);
    expect([primaries(part(view, ".issue-view-actions")), primaries(part(view, ".issue-view-side"))]).toEqual([1, 0]);
    const bare = await modal(BARE);
    expect(primaries(bare)).toBe(0);
    expect(within(part(bare, ".issue-view-actions")).getAllByRole("button").map((button) => button.textContent)).toEqual(["Hand off", "Edit", "Assign", "Cancel task"]);
  });
});

describe("the work item's order (DF-6)", () => {
  // Activity stands where History stood (docs/features/card-questions.md "Acceptance criteria", U1).
  it("reads the side column Status, then the people's fields in order, every field filled; the main column the card's blocks, Goal, Criteria, Source, Hand-offs, Evidence, Activity", async () => {
    const dialog = await modal(FULL);
    expect(sideLabels(dialog)).toEqual(SIDE_ORDER);
    expect(within(dialog).queryByRole("button", { name: /^More fields/ })).toBeNull();
    expect(mainSections(dialog)).toEqual(["Against the brief", "Goal", "Criteria", "Source", "Subtasks", "Evidence", "Activity"]);
    // The status: the column's lozenge, then Move.
    const status = within(dialog).getByRole("region", { name: "Status" });
    expect([status.querySelector(".lozenge")?.getAttribute("data-appearance"), status.querySelector(".lozenge-label")?.textContent]).toEqual(["information", "Check"]);
    expect(within(status).getByRole("button", { name: "Move" }).classList.contains("button-compact")).toBe(true);
  });

  it("puts the latest return reason among the card's blocks, before Goal", async () => {
    const view = await page(RETURNED);
    expect(mainSections(view)).toEqual(["Against the brief", "Returned", "Goal", "Source", "Subtasks", "Evidence", "Activity"]);
    const status = within(view).getByRole("region", { name: "Status" });
    expect(status.querySelector(".lozenge")?.getAttribute("data-appearance")).toBe("warning");
  });

  it("folds assignee, due and estimate when null: Details lacks them, More fields (3) collapsed lists them in order, each None", async () => {
    const dialog = await modal({ ...FULL, assignee: null, due: null, estimate: null }, client({ ...CARDS, [FULL.id]: { ...FULL, assignee: null, due: null, estimate: null } }));
    const details = within(dialog).getByRole("region", { name: "Details" });
    expect(sideLabels(dialog)).toEqual(SIDE_ORDER.filter((label) => !["Assignee", "Due", "Estimate"].includes(label)));
    const more = within(details).getByRole("button", { name: "More fields (3)" });
    expect(more.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(more);
    const folded = part(document.body, `[id="${more.getAttribute("aria-controls") ?? ""}"]`);
    expect(Array.from(folded.querySelectorAll("dt"), (term) => term.textContent)).toEqual(["Assignee", "Due", "Estimate"]);
    expect(Array.from(folded.querySelectorAll("dd"), (value) => value.textContent)).toEqual(["None", "None", "None"]);
  });

  it("holds Flow, Created by and Created at alone when no optional field is filled", async () => {
    const dialog = await modal(BARE);
    expect(sideLabels(dialog)).toEqual(["Status", "Flow", "Created by", "Created at"]);
    expect(within(dialog).getByRole("button", { name: "More fields (7)" }).getAttribute("aria-expanded")).toBe("false");
  });

  it("keeps the side column at most 330 px beside the main column, one column under 42.5rem of its own width", () => {
    expect(/--issue-side-width:\s*20\.625rem;/.test(tokensCss)).toBe(true);
    expect(/\n\.issue-view-side \{[^}]*max-width: var\(--issue-side-width\);/.test(appCss)).toBe(true);
    expect(/@container issue-view \(min-width: 42\.5rem\) \{\s*\.issue-view-body \{\s*grid-template-columns: minmax\(var\(--issue-main-min\), 1fr\) minmax\(0, var\(--issue-side-width\)\);/.test(appCss)).toBe(true);
    expect(/\n\.issue-view-body \{[^}]*grid-template-columns: minmax\(0, 1fr\);/.test(appCss)).toBe(true);
  });
});

describe("Development, folded (DF-7)", () => {
  it("reads Agent tasks · 2 linked for two links, one not read here; its Tasks region out of the tree until clicked, then the rows", async () => {
    const dialog = await modal(FULL);
    const side = part(dialog, ".issue-view-side");
    const development = within(side).getByRole("button", { name: "Agent tasks · 2 linked" });
    expect(development.getAttribute("aria-expanded")).toBe("false");
    expect(within(dialog).queryByRole("region", { name: "Agent tasks" })).toBeNull();
    expect(within(dialog).queryByRole("list", { name: "Agent tasks linked to C-0001" })).toBeNull();
    development.focus();
    fireEvent.click(development);
    expect(document.activeElement).toBe(development);
    const tasks = within(dialog).getByRole("region", { name: "Agent tasks" });
    expect(Array.from(tasks.querySelectorAll("li[data-task]"), (row) => row.getAttribute("data-task"))).toEqual(["T-0001", "T-0002"]);
    expect(within(tasks).getByText("spec task approve T-0001")).toBeTruthy();
  });

  it("reads No agent tasks with none linked; its empty state holds No task yet and both commands with Copy", async () => {
    const view = await page(BARE);
    const development = within(view).getByRole("button", { name: "Agent tasks · none linked" });
    fireEvent.click(development);
    const tasks = within(view).getByRole("region", { name: "Agent tasks" });
    expect(within(tasks).getByRole("heading", { name: "No agent task yet." }).closest(".empty-state-box")).not.toBeNull();
    expect(Array.from(tasks.querySelectorAll("code"), (code) => code.textContent)).toEqual(['spec task new --nodes REF --title "..."', "spec card edit C-0002 --task T-NNNN"]);
    expect(within(tasks).getAllByRole("button").map((button) => button.textContent)).toEqual([
      'Copy the command: spec task new --nodes REF --title "..."',
      "Copy the command: spec card edit C-0002 --task T-NNNN",
    ]);
  });
});

describe("the breadcrumbs (DF-8)", () => {
  it("in the modal: Project, Board, the parent and the card's ID, the last current", async () => {
    const dialog = await modal(FULL);
    expect(crumbs(dialog)).toEqual([
      ["Alpha", "#/alpha", null],
      ["Board", "#/alpha/board", null],
      ["C-0007", "#/alpha/board/C-0007", null],
      ["C-0001", null, "page"],
    ]);
    // The parent replaces the modal's card, as a card link does.
    const length = window.history.length;
    fireEvent.click(within(within(dialog).getByRole("navigation", { name: "Breadcrumbs" })).getByRole("link", { name: "C-0007" }));
    await screen.findByRole("dialog", { name: "The parent" });
    expect([window.location.hash, window.history.length]).toEqual(["#/alpha/board/C-0007", length]);
  });

  it("on the page alike; a card with no parent has no parent crumb", async () => {
    const view = await page(FULL);
    expect(crumbs(view)).toEqual([
      ["Alpha", "#/alpha", null],
      ["Board", "#/alpha/board", null],
      ["C-0007", "#/alpha/board/C-0007", null],
      ["C-0001", null, "page"],
    ]);
    expect(within(part(view, ".issue-view-bar")).getByRole("link", { name: "Open on the board" }).getAttribute("href")).toBe("#/alpha/board/C-0001");
    const bare = await modal(BARE);
    expect(crumbs(bare)).toEqual([
      ["Alpha", "#/alpha", null],
      ["Board", "#/alpha/board", null],
      ["C-0002", null, "page"],
    ]);
    expect(within(part(bare, ".issue-view-bar")).getAllByRole("link").map((link) => link.textContent)).toEqual(["Alpha", "Board", "Open as a page"]);
  });
});

// The review of design-foundation (iteration 2): a collapsed Development keeps its task links and Copy
// in the DOM, `hidden`; the modal's focus trap skips them, so Shift+Tab from the title comes round to
// the Development button, never to a stop no one can see. From the title Tab goes on to the first stop
// after it in the DOM, the acts, though the breadcrumbs stand above it.
describe("focus in the card's modal (the focus trap)", () => {
  it("skips the stops of a collapsed Development: Shift+Tab from the title lands on its button, the trap's stops none hidden", async () => {
    const dialog = await modal(FULL);
    const title = within(dialog).getByRole("heading", { level: 2, name: FULL.title });
    await waitFor(() => {
      expect(document.activeElement).toBe(title);
    });
    const development = within(dialog).getByRole("button", { name: "Agent tasks · 2 linked" });
    // The collapsed region holds a task link and a Copy button, in the DOM after Development.
    const region = part(document.body, `[id="${development.getAttribute("aria-controls") ?? ""}"]`);
    expect([region.hidden, region.querySelectorAll("a[href], button").length > 0]).toEqual([true, true]);
    const stops = focusables(dialog);
    expect(stops.filter((stop) => stop.closest("[hidden]") !== null)).toEqual([]);
    expect(stops.at(-1)).toBe(development);
    fireEvent.keyDown(title, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(development);
    // Tab past it goes round to the first stop, inside the dialog.
    fireEvent.keyDown(development, { key: "Tab" });
    expect(document.activeElement).toBe(stops[0]);
    // Expanded, its stops count again.
    fireEvent.click(development);
    expect(focusables(dialog).at(-1)?.closest(`[id="${region.id}"]`)).toBe(region);
  });

  it("sends Tab from the title to the first stop after it, the first act, and Shift+Tab to the last", async () => {
    const dialog = await modal(FULL);
    const title = within(dialog).getByRole("heading", { level: 2, name: FULL.title });
    await waitFor(() => {
      expect(document.activeElement).toBe(title);
    });
    fireEvent.keyDown(title, { key: "Tab" });
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Accept" }));
    title.focus();
    fireEvent.keyDown(title, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(focusables(dialog).at(-1));
  });
});

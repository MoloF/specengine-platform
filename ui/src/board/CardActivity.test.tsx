import { screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import type { Card, CardEvent, CardItem } from "../api/types";
import { docPageHash } from "../app/routes";
import { aCard, aCardFlow, aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";

// U1 of docs/features/card-questions.md "Acceptance criteria": Activity stands where History stood,
// one list in served `seq` order: the card's events as History drew them, its items' events by their
// words (labels.ts), and each item's block at the event that raised it. The card is harbor-sim's
// C-0008 as the mock serves it (src/mocks/cards.ts, src/mocks/harbor-sim/fixtures.ts), written here
// on the stub (the policy keeps the mock's client in src/mocks/): sent back by Noor Ainsley with a
// question, PR-0055, open; a decision, PR-0056, recorded by Tomas Oakes with its why and applied as
// DEC-0013. The question was raised a moment after the return, its `at` thirty seconds before it,
// as two clocks' writes may be: served order stands, never time order. One item raised before the
// history's last events stands under Earlier items; an event type this build does not know stays raw.

const FLOW = aCardFlow({ name: "plates", return: "doing" });

// The items' chunk loaded once for the file, so a lazy block or dialog never waits on its first transform.
beforeAll(async () => {
  await import("./CardItems");
});


const PERSON = { type: "human", role: null, model: null, run: null };

/** An item as the card's read serves it; omitted keys null. */
function anItem(fields: Partial<CardItem> & Pick<CardItem, "id" | "kind" | "status">): CardItem {
  return {
    summary: null,
    working_answer: null,
    why: null,
    author: PERSON,
    created_at: "2026-11-11T08:00:00Z",
    updated_at: "2026-11-11T08:00:00Z",
    staged_at: null,
    decided_by: null,
    decided_at: null,
    decision_note: null,
    record_id: null,
    record_path: null,
    record_title: null,
    ...fields,
  };
}

const ITEMS: CardItem[] = [
  anItem({ id: "PR-0054", kind: "question", status: "rejected", summary: "Print the old reach too?", decided_by: "Mira Holt <mira@harbor.example.invalid>", decision_note: "No: one reach per plate." }),
  anItem({
    id: "PR-0055",
    kind: "question",
    status: "open",
    summary: "Which reach does the crane diagram show: the survey's new numbers or the maker's sheet?",
    working_answer: "The maker's sheet until the survey is filed; the plate is redrawn then.",
  }),
  anItem({
    id: "PR-0056",
    kind: "decision",
    status: "applied",
    summary: "Draw the reach at full boom only?",
    working_answer: "Yes: the diagram shows the reach at full boom; partial booms are left out.",
    why: "Pilots read the diagram for the worst case, and full boom is it.",
    decided_by: "Mira Holt <mira@harbor.example.invalid>",
    decided_at: "2026-11-11T09:00:00Z",
    decision_note: "Checked with the harbour office.",
    record_id: "DEC-0013",
    record_path: "docs/decisions/DEC-0013.md",
    record_title: "Crane reach at full boom",
  }),
];

/** The card's history as the daemon merges it by `seq`: the return's `at` after the question's. */
const HISTORY: CardEvent[] = [
  { seq: 8, type: "card.created", at: "2026-11-08T09:00:00Z", payload: { id: "C-0008", column: "todo", as: "Mira Holt" } },
  {
    seq: 30,
    type: "card.moved",
    at: "2026-11-11T08:00:00Z",
    payload: { id: "C-0008", from: "todo", to: "doing", as: "Noor Ainsley", note: "The survey's crane reach numbers are not in MEC-CARGO-CRANES yet." },
  },
  { seq: 31, type: "proposal.created", at: "2026-11-11T07:59:30Z", payload: { id: "PR-0055", as: "Noor Ainsley" } },
  { seq: 32, type: "proposal.created", at: "2026-11-11T08:30:00Z", payload: { id: "PR-0056", as: "Tomas Oakes" } },
  { seq: 33, type: "proposal.approved", at: "2026-11-11T09:00:00Z", payload: { id: "PR-0056", record: "DEC-0013" } },
  { seq: 34, type: "proposal.applied", at: "2026-11-11T09:00:00Z", payload: { id: "PR-0056" } },
  { seq: 35, type: "proposal.reopened", at: "2026-11-11T09:10:00Z", payload: { id: "PR-0055" } },
];

const C_0008: Card = aCard({
  id: "C-0008",
  flow: "plates",
  column: "doing",
  title: "Crane reach diagram",
  targets: ["MEC-CARGO-CRANES", "MEC-OLD-SLIPWAY"],
  assignee: "Noor Ainsley",
  items: ITEMS,
  history: HISTORY,
});

function client(card: Card = C_0008): StubClient {
  const stub = stubClient();
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: card.id, flow: "plates", column: card.column, title: card.title, targets: card.targets })], { flows: [FLOW] })));
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === card.id ? structuredClone(card) : { id, reason: `no card ${id}` }));
  return stub;
}

/** The card's modal over the board, its Activity read. */
async function activity(card: Card = C_0008): Promise<HTMLElement> {
  renderApp(client(card), `#/alpha/board/${card.id}`);
  const dialog = await screen.findByRole("dialog", { name: card.title });
  return within(dialog).findByRole("region", { name: "Activity" });
}

/** An item's block, once its chunk arrived. */
async function block(scope: HTMLElement, id: string): Promise<HTMLElement> {
  return within(scope).findByRole("article", { name: id });
}

/** The thread's lines: each one's words, the bold part. */
function words(region: HTMLElement): string[] {
  const thread = region.querySelector("ol");
  return Array.from(thread?.children ?? [], (line) => line.querySelector("strong")?.textContent ?? "");
}

describe("Activity (U1 of card-questions)", () => {
  it("stands where History stood, the last part of the main column; no History part is left", async () => {
    renderApp(client(), "#/alpha/board/C-0008");
    const dialog = await screen.findByRole("dialog", { name: "Crane reach diagram" });
    const main = dialog.querySelector<HTMLElement>(".issue-view-main");
    await within(dialog).findByRole("region", { name: "Activity" });
    const parts = within(main as HTMLElement)
      .getAllByRole("region")
      .map((region) => region.getAttribute("aria-label"));
    expect(parts.at(-1)).toBe("Activity");
    expect(parts).not.toContain("History");
  });

  it("keeps the served order, never the time: the return, then the question raised with it though its time is earlier (M: sorted by `at`)", async () => {
    const region = await activity();
    await block(region, "PR-0055");
    expect(words(region)).toEqual(["Created", "Moved", "Raised", "Raised", "Approved", "Recorded", "proposal.reopened"]);
    const lines = Array.from(region.querySelector("ol")?.children ?? []);
    // The return move shows with its note, as History drew it.
    expect(lines[1]?.textContent).toContain("Moved by Noor Ainsley from To do to Doing");
    expect(lines[1]?.textContent).toContain("Why: The survey's crane reach numbers are not in MEC-CARGO-CRANES yet.");
    // An item's line: its word, its ID and who, from the event's `as`.
    expect(lines[2]?.querySelector(".board-history-line > span")?.textContent).toBe("Raised PR-0055 by Noor Ainsley");
    expect(lines[3]?.querySelector(".board-history-line > span")?.textContent).toBe("Raised PR-0056 by Tomas Oakes");
    // An event naming no `as`: who is the item's author's login, else its role; a person raised from the page has neither.
    expect(lines[4]?.querySelector(".board-history-line > span")?.textContent).toBe("Approved PR-0056");
  });

  it("opens PR-0055's block at the event that raised it: `question` raw under lang=en, the open lozenge, the summary and the working answer; Answer… on it", async () => {
    const region = await activity();
    const question = await block(region, "PR-0055");
    const raised = Array.from(region.querySelector("ol")?.children ?? [])[2];
    expect(raised?.contains(question)).toBe(true);
    const kind = within(question).getByText("question");
    expect([kind.getAttribute("lang"), kind.classList.contains("kind-tag")]).toEqual(["en", true]);
    const lozenge = question.querySelector(".lozenge");
    expect([lozenge?.getAttribute("data-appearance"), lozenge?.textContent]).toEqual(["neutral", "Open"]);
    expect(question.textContent).toContain("Which reach does the crane diagram show: the survey's new numbers or the maker's sheet?");
    expect(Array.from(question.querySelectorAll("dt"), (term) => [term.textContent, term.nextElementSibling?.textContent])).toEqual([
      ["Working answer", "The maker's sheet until the survey is filed; the plate is redrawn then."],
    ]);
    expect(within(question).getByRole("button", { name: "Answer…" })).toBeTruthy();
  });

  it("shows PR-0056's why, its record as a document link by its title, the approver's note as a note, never its answer, and who decided; no Answer… on an applied item (M: the block chosen by kind; minor 5 of the review: a note labelled Answer whatever the state)", async () => {
    const region = await activity();
    const decision = await block(region, "PR-0056");
    const raised = Array.from(region.querySelector("ol")?.children ?? [])[3];
    expect(raised?.contains(decision)).toBe(true);
    expect(within(decision).getByText("decision").getAttribute("lang")).toBe("en");
    expect(decision.querySelector(".lozenge")?.getAttribute("data-appearance")).toBe("success");
    expect(Array.from(decision.querySelectorAll("dt"), (term) => [term.textContent, term.nextElementSibling?.textContent])).toEqual([
      ["Working answer", "Yes: the diagram shows the reach at full boom; partial booms are left out."],
      ["Why", "Pilots read the diagram for the worst case, and full boom is it."],
      ["Record", "Crane reach at full boom"],
      ["Last decision note", "Checked with the harbour office."],
      ["Decided by", "Mira Holt <mira@harbor.example.invalid>"],
    ]);
    const record = within(decision).getByRole("link", { name: "Crane reach at full boom" });
    expect(record.getAttribute("href")).toBe(docPageHash("alpha", "DEC-0013"));
    expect(within(decision).queryByRole("button", { name: "Answer…" })).toBeNull();
  });

  it("stands an item whose raising is not in the history under Earlier items, first, its answer the decision's note; an unknown type stays raw", async () => {
    const region = await activity();
    const earlier = within(region).getByRole("list", { name: "Earlier items" });
    const old = await block(earlier, "PR-0054");
    expect(Array.from(old.querySelectorAll("dt"), (term) => [term.textContent, term.nextElementSibling?.textContent])).toEqual([
      ["Answer", "No: one reach per plate."],
      ["Decided by", "Mira Holt <mira@harbor.example.invalid>"],
    ]);
    expect(region.firstElementChild?.nextElementSibling?.textContent).toBe("Earlier items");
    // The two items the history raised are not under it.
    expect(within(earlier).queryByRole("article", { name: "PR-0055" })).toBeNull();
    expect(words(region).at(-1)).toBe("proposal.reopened");
    // The daemon's word this build has none for, under lang="en"; a known one in the page's language.
    const strong = Array.from(region.querySelector("ol")?.children ?? [], (line) => line.querySelector("strong"));
    expect(strong.map((word) => word?.getAttribute("lang") ?? null)).toEqual([null, null, null, null, null, null, "en"]);
  });

  // Minor 7 of card-questions' review: who an event names with no `as` is the item's author's login,
  // else its role; a card's unknown event stays raw under lang="en" too.
  it("names who without `as`: a hub author's login, else an agent's role; a card's unknown type raw under lang=en", async () => {
    const hubAuthor = { type: "agent", role: "drafter", model: "m-1", run: "r-1", login: "mira" };
    const agentAuthor = { type: "agent", role: "rust-developer", model: "m-1", run: "r-2" };
    const card = aCard({
      id: "C-0009",
      flow: "plates",
      title: "Who asked",
      targets: ["MEC-FAIRWAY"],
      items: [anItem({ id: "PR-0060", kind: "question", status: "open", author: hubAuthor }), anItem({ id: "PR-0061", kind: "discrepancy", status: "open", author: agentAuthor })],
      history: [
        { seq: 1, type: "proposal.created", at: "2026-11-11T08:00:00Z", payload: { id: "PR-0060" } },
        { seq: 2, type: "proposal.created", at: "2026-11-11T08:01:00Z", payload: { id: "PR-0061" } },
        { seq: 3, type: "card.future", at: "2026-11-11T08:02:00Z", payload: { id: "C-0009" } },
      ],
    });
    const region = await activity(card);
    await block(region, "PR-0061");
    const lines = Array.from(region.querySelector("ol")?.children ?? []);
    expect(lines.slice(0, 2).map((line) => line.querySelector(".board-history-line > span")?.textContent)).toEqual(["Raised PR-0060 by mira", "Raised PR-0061 by rust-developer"]);
    const unknown = lines[2]?.querySelector("strong");
    expect([unknown?.textContent, unknown?.getAttribute("lang")]).toEqual(["card.future", "en"]);
  });

  it("says none yet with no event and no item; a card's history alone draws as History drew it", async () => {
    const bare = aCard({ id: "C-0002", flow: "plates", title: "Bare", items: [], history: [] });
    const region = await activity(bare);
    expect(region.textContent).toBe("ActivityNone yet.");
    const plain = aCard({ id: "C-0003", flow: "plates", title: "Plain", history: [HISTORY[0] as CardEvent] });
    renderApp(client(plain), "#/alpha/board/C-0003");
    const dialog = await screen.findByRole("dialog", { name: "Plain" });
    const lines = await waitFor(() => within(within(dialog).getByRole("region", { name: "Activity" })).getAllByRole("listitem"));
    expect(lines.map((line) => line.textContent)).toEqual(["2026-11-08 09:00 UTCCreated by Mira Holt in To do"]);
  });
});

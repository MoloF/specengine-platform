import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import type { Author, Card, CardEvent, CardItem, Proposal, ProposalOption, RoundItem } from "../api/types";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { answersOf } from "../round/drafts";
import { aCard, aCardFlow, aCardList, aCardRow, aProposal, aRoundItem, aRoundSheet } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { shownSlot, shownText } from "../ui/text";
import { recommendedBy } from "./labels";

// PQ12 and PQ13 of docs/features/plain-questions.md "Acceptance criteria": a question holding a gist
// leads with it on one line, then why it matters, then its options or controls, the recommendation
// marked with who made it ("Recommended by the AI" for an agent, "Recommended by the asker" for anyone
// else) and never picked; what it said before (its text, working answer, price of another answer,
// why) sits in a collapsed Disclosure, the text verbatim in `<bdi>`. A question with options offers
// its options, its working answer and an own answer; a discrepancy, its options alone; the Inbox's
// dialog offers no own answer and keeps Accept off until a pick. On the Inbox card and dialog, the
// Questions screen, a task's Activity and its Answer…. An item without a gist renders as before:
// its own tests stand unedited.

const RLO = String.fromCodePoint(0x202e);
const AGENT: Author = { type: "agent", role: "rust-developer", model: "claude-opus-5-5", run: "R-2290" };
const PERSON: Author = { type: "human", role: null, model: null, run: null };

/** The agent's own words: IDs, a path, a code name, a bidirectional override and two lines. */
const ASKED = `Does RULE-TIDE-WINDOW apply when tide::entry_window_system() runs before the table loads (docs/spec/tides.md)?${RLO}\nAt 00:00 it lets every ship in.`;
const GIST = "Should a deep ship wait at the anchorage until the day's tide table is read?";
const MATTERS = "Two agent tasks build on the answer.\r\nA wrong guess grounds a deep ship.";
const WORKING = "Deep ships wait until the table is read.";
const PRICE = "The tide window rewritten.";
const OPTIONS: ProposalOption[] = [
  { label: "Wait at the anchorage", effect: "Deep ships hold.", price: "None: as built." },
  { label: "Enter on yesterday's table", effect: "Ships keep moving.", price: "A grounding risk." },
];

/** An agent's question raised with all four fields, open on C-0001. */
function aLeadQuestion(fields: Partial<Proposal> & { id: string }): Proposal {
  return aProposal({
    kind: "question",
    severity: "high",
    target_ids: ["RULE-TIDE-WINDOW"],
    summary: ASKED,
    gist: GIST,
    matters: MATTERS,
    options: OPTIONS,
    recommendation: 0,
    working_answer: WORKING,
    price_of_other: PRICE,
    author: AGENT,
    card_id: "C-0001",
    ...fields,
  });
}

/** A discrepancy: options, a recommendation and no price of another answer. */
const DISCREPANCY = aProposal({
  id: "PR-0061",
  kind: "discrepancy",
  severity: "normal",
  gap_type: "contradicts",
  target_ids: ["RULE-TIDE-WINDOW"],
  summary: "Entry is not limited to the tide window",
  options: OPTIONS,
  recommendation: 1,
  working_answer: "Entry stays open.",
  author: AGENT,
});

/** `a` comes before `b` in the document. */
function before(a: Node, b: Node): boolean {
  return (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
}

/** The fold under a lead: its button and its region, by the button's `aria-controls`. */
function fold(scope: HTMLElement): { button: HTMLElement; region: HTMLElement } {
  const button = within(scope).getByRole("button", { name: "The question as asked" });
  const region = document.getElementById(button.getAttribute("aria-controls") ?? "");
  if (region === null) {
    throw new Error("the fold controls no region");
  }
  return { button, region };
}

/** Each radio's label as shown, white space collapsed, and whether it is checked. */
function radios(scope: HTMLElement): [string, boolean][] {
  return within(scope)
    .getAllByRole<HTMLInputElement>("radio")
    .map((radio) => [(radio.closest("label")?.textContent ?? "").replace(/\s+/g, " ").trim(), radio.checked]);
}

describe("who recommends, labels.ts alone (PQ12)", () => {
  it("says the AI's for an agent, the asker's for a person, a type this build does not know and none", () => {
    const t = (key: string) => key;
    expect(recommendedBy(AGENT, t as never)).toBe("inbox.lead.recommendedByAgent");
    expect(recommendedBy(PERSON, t as never)).toBe("inbox.lead.recommendedByAsker");
    expect(recommendedBy({ type: "robot" }, t as never)).toBe("inbox.lead.recommendedByAsker");
    expect(recommendedBy(null, t as never)).toBe("inbox.lead.recommendedByAsker");
  });

  it("holds the five lead keys in both catalogs, the English as the spec words them (PQ13)", () => {
    expect(en.inbox.lead).toEqual({
      matters: "Why it matters",
      asked: "The question as asked",
      recommendedByAgent: "Recommended by the AI",
      recommendedByAsker: "Recommended by the asker",
      pickFirst: "Pick an answer first",
    });
    expect(Object.keys(ru.inbox.lead)).toEqual(Object.keys(en.inbox.lead));
    for (const value of Object.values(ru.inbox.lead)) {
      expect(value.trim()).not.toBe("");
    }
    // The bare word left with its users.
    expect(Object.keys(en.inbox)).not.toContain("recommended");
    expect(Object.keys(en.round)).not.toContain("recommended");
  });
});

describe("controls by structure (PQ12)", () => {
  it("offers a question with options its options, its working answer and an own answer; a discrepancy its options alone; an option-less item as before", () => {
    const question = aLeadQuestion({ id: "PR-0059" });
    expect(answersOf(question).map((offered) => [offered.pick, offered.option])).toEqual([
      ["option", 0],
      ["option", 1],
      ["working", null],
      ["answer", null],
    ]);
    expect(answersOf({ ...question, working_answer: null }).map((offered) => offered.pick)).toEqual(["option", "option", "answer"]);
    expect(answersOf(DISCREPANCY).map((offered) => [offered.pick, offered.option])).toEqual([
      ["option", 0],
      ["option", 1],
    ]);
    expect(answersOf({ options: [], working_answer: "w", price_of_other: null }).map((offered) => offered.pick)).toEqual(["working", "answer"]);
    expect(answersOf({ options: [], working_answer: null, price_of_other: null }).map((offered) => offered.pick)).toEqual(["answer"]);
  });
});

describe("the Inbox: the card, the row and the dialog (PQ12)", () => {
  async function inboxCard(proposals: Proposal[], id: string): Promise<{ client: StubClient; card: HTMLElement }> {
    const client = stubClient(proposals);
    renderApp(client, `#/alpha/inbox/${id}`);
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    return { client, card };
  }

  it("titles the card by the gist and leads with why it matters, then the options, the recommendation the AI's; the asked text folded, collapsed, verbatim", async () => {
    const { card } = await inboxCard([aLeadQuestion({ id: "PR-0059" })], "PR-0059");
    const title = within(card).getByRole("heading", { level: 2 });
    expect(title.textContent).toBe(GIST);
    const matters = within(card).getByText("Why it matters");
    expect(matters.nextElementSibling?.textContent).toBe(shownText(MATTERS));
    expect(matters.nextElementSibling?.querySelector("bdi")).not.toBeNull();
    const options = card.querySelector(".option-list");
    expect(Array.from(options?.querySelectorAll(".option-label") ?? [], (label) => label.textContent)).toEqual(["Wait at the anchorageRecommended by the AI", "Enter on yesterday's table"]);
    const { button, region } = fold(card);
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(region.hidden).toBe(true);
    expect(before(title, matters) && options !== null && before(matters, options) && before(options, button)).toBe(true);
    // The asked text verbatim in its own bdi, its override written out; the working answer and its price with it.
    const asked = Array.from(region.querySelectorAll("bdi")).map((bdi) => bdi.textContent);
    expect(asked).toContain(shownText(ASKED));
    expect(shownText(ASKED)).toContain("\\u{202e}");
    // Collapsed, the region is out of the accessibility tree: its parts are found as hidden.
    expect(within(region).getByRole("heading", { level: 3, name: "Working answer", hidden: true })).toBeTruthy();
    expect(region.textContent).toContain(PRICE);
    fireEvent.click(button);
    expect([button.getAttribute("aria-expanded"), region.hidden]).toEqual(["true", false]);
    // Nothing of the asked text stands outside the fold.
    expect(within(card).getAllByText((_content, element) => element?.tagName === "BDI" && element.textContent === shownText(ASKED))).toHaveLength(1);
  });

  it("marks a person's recommendation the asker's", async () => {
    const { card } = await inboxCard([aLeadQuestion({ id: "PR-0059", author: PERSON })], "PR-0059");
    expect(card.querySelector(".option-recommended .recommended-mark")?.textContent).toBe("Recommended by the asker");
  });

  it("lists the row by the gist, else the summary", async () => {
    const client = stubClient([aLeadQuestion({ id: "PR-0059" }), DISCREPANCY]);
    renderApp(client, "#/alpha/inbox/PR-0059");
    const list = await screen.findByRole("listbox");
    const rows = within(list).getAllByRole("option");
    expect(rows.map((row) => row.querySelector(".queue-item-summary")?.textContent)).toEqual([GIST, "Entry is not limited to the tide window"]);
  });

  it("offers in the dialog each option and the working answer, nothing picked, Accept off until a pick; the working answer staged as an approve with no option", async () => {
    const { client, card } = await inboxCard([aLeadQuestion({ id: "PR-0059" })], "PR-0059");
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-0059" });
    expect(dialog.querySelector(".lead-gist")?.textContent).toBe(GIST);
    expect(within(dialog).getByText("Why it matters")).toBeTruthy();
    expect(radios(dialog)).toEqual([
      ["[0] Wait at the anchorageRecommended by the AIDeep ships hold.Price: None: as built.", false],
      ["[1] Enter on yesterday's tableShips keep moving.Price: A grounding risk.", false],
      [`Take the working answer${WORKING}`, false],
    ]);
    expect(within(dialog).queryByRole("radio", { name: /Your answer/ })).toBeNull();
    const { button, region } = fold(dialog);
    expect([button.getAttribute("aria-expanded"), region.hidden]).toEqual(["false", true]);
    expect(Array.from(region.querySelectorAll("bdi"), (bdi) => bdi.textContent)).toContain(shownText(ASKED));
    const lead = dialog.querySelector(".lead-gist");
    const firstRadio = within(dialog).getAllByRole("radio")[0];
    expect(lead !== null && firstRadio !== undefined && before(lead, within(dialog).getByText("Why it matters")) && before(within(dialog).getByText("Why it matters"), firstRadio)).toBe(true);
    const accept = within(dialog).getByRole("button", { name: "Stage accept" });
    expect(accept.getAttribute("aria-disabled")).toBe("true");
    expect(document.getElementById(accept.getAttribute("aria-describedby") ?? "")?.textContent).toBe("Pick an answer first");
    fireEvent.click(accept);
    expect(client.stageDecision).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("radio", { name: /^Take the working answer/ }));
    expect(accept.getAttribute("aria-disabled")).toBe("false");
    expect(within(dialog).queryByText("Pick an answer first")).toBeNull();
    fireEvent.click(accept);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.stageDecision.mock.calls).toEqual([["alpha", "PR-0059", { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, "2026-10-01T10:00:00Z"]]);
  });

  it("draws a gist holding U+202E verbatim in a bdi through shownSlot, its override written out, as the card's title, the queue row and the not-staged dialog's line", async () => {
    const gist = `Should a deep ship wait ${RLO}at the anchorage?`;
    const shown = shownSlot(gist);
    expect(shown).toBe("Should a deep ship wait \\u{202e}at the anchorage?");
    const { card } = await inboxCard([aLeadQuestion({ id: "PR-0059", gist })], "PR-0059");
    const title = within(card).getByRole("heading", { level: 2 });
    const row = within(screen.getByRole("listbox")).getByRole("option").querySelector(".queue-item-summary");
    fireEvent.click(within(card).getByRole("button", { name: "Defer" }));
    const dialog = await screen.findByRole("dialog", { name: "Defer PR-0059" });
    const line = dialog.querySelector(".dialog-text");
    for (const place of [title, row, line]) {
      expect([place?.querySelector("bdi")?.textContent, place?.textContent]).toEqual([shown, shown]);
    }
  });

  it("shows a gist question's working answer once when Accept records it: out of the fold, which keeps it for Reject", async () => {
    const { card } = await inboxCard([aLeadQuestion({ id: "PR-0059", options: [], recommendation: null })], "PR-0059");
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const accept = await screen.findByRole("dialog", { name: "Accept PR-0059" });
    expect(within(accept).queryAllByRole("radio")).toEqual([]);
    expect(within(accept).getByText("The working answer is recorded:").nextElementSibling?.textContent).toBe(WORKING);
    expect(within(accept).getAllByText(WORKING)).toHaveLength(1);
    const asked = fold(accept).region;
    expect(Array.from(asked.querySelectorAll("bdi"), (bdi) => bdi.textContent)).toEqual([shownText(ASKED), PRICE]);
    fireEvent.click(within(accept).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    fireEvent.click(within(card).getByRole("button", { name: "Reject" }));
    const reject = await screen.findByRole("dialog", { name: "Reject PR-0059" });
    expect(within(reject).queryByText("The working answer is recorded:")).toBeNull();
    expect(Array.from(fold(reject).region.querySelectorAll("bdi"), (bdi) => bdi.textContent)).toEqual([shownText(ASKED), WORKING, PRICE]);
  });

  // docs/features/release-hardening.md "Acceptance criteria", RH-U2: an option-less gist question shows
  // its working answer and price after why it matters, outside the fold, in the options' place; the fold
  // keeps the text, the pair and the why, never the working answer. M: the working answer only in the
  // fold; drawn in both places.
  it("leads an option-less gist question's card with its working answer and price after why it matters, out of the fold; the fold keeps the text, the pair and the why (RH-U2)", async () => {
    const why = "The tide table is the only source of the window.";
    const pair = {
      left: { id: "RULE-TIDE-WINDOW", title: "The tide window", path: "docs/spec/tides.md", evidence: [] },
      right: { id: "RULE-DEEP-SHIPS", title: "Deep ships", path: "docs/spec/ships.md", evidence: [] },
      other: [],
    };
    const { card } = await inboxCard([aLeadQuestion({ id: "PR-0059", options: [], recommendation: null, why, pair })], "PR-0059");
    expect(within(card).queryByText("No options attached.")).toBeNull();
    expect(within(card).queryByRole("heading", { level: 3, name: "Options" })).toBeNull();
    const matters = within(card).getByText("Why it matters");
    const working = within(card).getByRole("heading", { level: 3, name: "Working answer" });
    const { button, region } = fold(card);
    expect(region.contains(working)).toBe(false);
    expect(before(matters, working) && before(working, button)).toBe(true);
    const answer = working.closest("section");
    expect(answer?.textContent).toContain(WORKING);
    expect(answer?.textContent).toContain(PRICE);
    // Drawn once, out of the fold: the fold holds neither the working answer nor its price.
    expect(within(card).getAllByText(WORKING)).toHaveLength(1);
    expect(within(region).queryByRole("heading", { name: "Working answer", hidden: true })).toBeNull();
    expect(region.textContent).not.toContain(WORKING);
    expect(region.textContent).not.toContain(PRICE);
    // The fold keeps the asked text, the pair and the why.
    expect(Array.from(region.querySelectorAll("bdi"), (bdi) => bdi.textContent)).toContain(shownText(ASKED));
    expect(await within(region).findByRole("region", { name: "What conflicts with what", hidden: true })).toBeTruthy();
    expect(within(region).getByRole("heading", { level: 3, name: "Why", hidden: true })).toBeTruthy();
    expect(region.textContent).toContain(why);
  });

  it("keeps a gist question with options as it was: its options lead, its working answer in the fold alone (RH-U2)", async () => {
    const { card } = await inboxCard([aLeadQuestion({ id: "PR-0059" })], "PR-0059");
    const { region } = fold(card);
    expect(card.querySelector(".option-list")).not.toBeNull();
    expect(within(region).getByRole("heading", { level: 3, name: "Working answer", hidden: true })).toBeTruthy();
    expect(within(card).getAllByText(WORKING)).toHaveLength(1);
  });

  it("keeps an option-less item without a gist as before: no fold, its options' note, its working answer (RH-U2)", async () => {
    const { card } = await inboxCard([aLeadQuestion({ id: "PR-0060", gist: null, matters: null, options: [], recommendation: null })], "PR-0060");
    expect(within(card).queryByRole("button", { name: "The question as asked" })).toBeNull();
    expect(within(card).getByRole("heading", { level: 3, name: "Options" }).closest("section")?.textContent).toBe("OptionsNo options attached.");
    expect(within(card).getByRole("heading", { level: 3, name: "Working answer" }).closest("section")?.textContent).toContain(WORKING);
  });

  it("picks nothing for a discrepancy either: its options alone, Accept off until one is picked, then sent by number", async () => {
    const { client, card } = await inboxCard([DISCREPANCY], "PR-0061");
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-0061" });
    expect(radios(dialog)).toEqual([
      ["[0] Wait at the anchorageDeep ships hold.Price: None: as built.", false],
      ["[1] Enter on yesterday's tableRecommended by the AIShips keep moving.Price: A grounding risk.", false],
    ]);
    expect(within(dialog).queryByRole("button", { name: "The question as asked" })).toBeNull();
    const accept = within(dialog).getByRole("button", { name: "Stage accept" });
    expect(accept.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(accept);
    expect(client.stageDecision).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("radio", { name: /^\[1\]/ }));
    fireEvent.click(accept);
    await waitFor(() => {
      expect(client.stageDecision.mock.calls).toEqual([["alpha", "PR-0061", { decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: null }, "2026-10-01T10:00:00Z"]]);
    });
  });
});

describe("the Questions screen (PQ12)", () => {
  /** A round item as the sheet serves a review document. */
  function itemOf(review: Proposal): RoundItem {
    return aRoundItem({
      id: review.id ?? "",
      kind: review.kind ?? "",
      severity: review.severity,
      gap_type: review.gap_type,
      summary: review.summary ?? "",
      gist: review.gist,
      matters: review.matters,
      working_answer: review.working_answer,
      price_of_other: review.price_of_other,
      options: review.options,
      recommendation: review.recommendation,
      author: review.author ?? PERSON,
    });
  }

  async function questions(items: RoundItem[]): Promise<StubClient> {
    const client = stubClient();
    client.state.round = aRoundSheet(items);
    renderApp(client, "#/alpha/questions");
    await screen.findByRole("heading", { level: 1, name: "Questions" });
    return client;
  }

  it("leads with the gist, then why it matters, then the options, the working answer and an own answer, nothing picked; the asked text folded, collapsed, verbatim", async () => {
    await questions([itemOf(aLeadQuestion({ id: "PR-0059" }))]);
    const article = await screen.findByRole("article", { name: "PR-0059" });
    const gist = article.querySelector(".lead-gist");
    expect(gist?.textContent).toBe(GIST);
    expect(gist?.querySelector("bdi")).not.toBeNull();
    const matters = within(article).getByText("Why it matters");
    expect(matters.nextElementSibling?.textContent).toBe(shownText(MATTERS));
    expect(radios(article)).toEqual([
      ["[0] Wait at the anchorageRecommended by the AIDeep ships hold.Price: None: as built.", false],
      ["[1] Enter on yesterday's tableShips keep moving.Price: A grounding risk.", false],
      ["Take the working answer", false],
      ["Your answer", false],
      ["Reject with a reason", false],
      ["Leave open", true],
    ]);
    const first = within(article).getAllByRole("radio")[0];
    const { button, region } = fold(article);
    expect(gist !== null && first !== undefined && before(gist, matters) && before(matters, first) && before(first, button)).toBe(true);
    expect([button.getAttribute("aria-expanded"), region.hidden]).toEqual(["false", true]);
    expect(Array.from(region.querySelectorAll(".round-text"), (text) => text.textContent)).toEqual([shownText(ASKED), WORKING, PRICE]);
    // The sheet's text stands nowhere else.
    expect(Array.from(article.querySelectorAll(".round-text")).every((text) => region.contains(text))).toBe(true);
  });

  it("stages the working answer or an own answer on a question with options", async () => {
    const client = await questions([itemOf(aLeadQuestion({ id: "PR-0059" }))]);
    const article = await screen.findByRole("article", { name: "PR-0059" });
    fireEvent.click(within(article).getByRole("radio", { name: "Your answer" }));
    fireEvent.change(within(article).getByLabelText("Your answer to PR-0059"), { target: { value: "Only at spring tides" } });
    fireEvent.click(screen.getByRole("button", { name: "Stage 1 answer" }));
    await waitFor(() => {
      expect(client.stageRoundAnswers.mock.calls.map((call) => call[1])).toEqual([
        [{ id: "PR-0059", stage: { decision: "approve", option: null, answer: "Only at spring tides", canon: null, editorial: null, note: null, updated_at: "2026-10-01T10:00:00Z" } }],
      ]);
    });
  });

  it("marks a person's recommendation the asker's, and gives a discrepancy its options alone", async () => {
    await questions([itemOf(aLeadQuestion({ id: "PR-0059", author: PERSON })), itemOf(DISCREPANCY)]);
    const question = await screen.findByRole("article", { name: "PR-0059" });
    expect(question.querySelector(".recommended-mark")?.textContent).toBe("Recommended by the asker");
    const discrepancy = screen.getByRole("article", { name: "PR-0061" });
    expect(radios(discrepancy).map(([label]) => label)).toEqual([
      "[0] Wait at the anchorageDeep ships hold.Price: None: as built.",
      "[1] Enter on yesterday's tableRecommended by the AIShips keep moving.Price: A grounding risk.",
      "Reject with a reason",
      "Leave open",
    ]);
    expect(discrepancy.querySelector(".lead-gist")).toBeNull();
    expect(within(discrepancy).queryByRole("button", { name: "The question as asked" })).toBeNull();
  });
});

describe("a task's Activity and Answer… (PQ12)", () => {
  beforeAll(async () => {
    await Promise.all([import("../board/CardItems"), import("./ConfirmDialog")]);
  });

  const LEAD = aLeadQuestion({ id: "PR-0059", updated_at: "2026-11-11T08:00:00Z" });
  const PLAIN = aProposal({ id: "PR-0060", kind: "question", summary: "Does the foreword name the datum?", working_answer: "Yes.", price_of_other: "One line.", author: PERSON, card_id: "C-0001", updated_at: "2026-11-11T08:00:00Z" });

  /** A card's item as its read serves a review document. */
  function cardItemOf(review: Proposal): CardItem {
    return {
      id: review.id ?? "",
      kind: review.kind ?? "",
      status: review.status ?? "open",
      summary: review.summary,
      gist: review.gist,
      matters: review.matters,
      working_answer: review.working_answer,
      why: review.why,
      author: review.author ?? PERSON,
      created_at: "2026-11-11T08:00:00Z",
      updated_at: "2026-11-11T08:00:00Z",
      staged_at: null,
      decided_by: null,
      decided_at: null,
      decision_note: null,
      record_id: null,
      record_path: null,
      record_title: null,
    };
  }

  const HISTORY: CardEvent[] = [
    { seq: 1, type: "card.created", at: "2026-11-10T08:00:00Z", payload: { id: "C-0001", column: "todo", as: "Mira Holt" } },
    { seq: 2, type: "proposal.created", at: "2026-11-11T08:00:00Z", payload: { id: "PR-0059", as: "agent" } },
    { seq: 3, type: "proposal.created", at: "2026-11-11T08:05:00Z", payload: { id: "PR-0060", as: "Mira Holt" } },
  ];

  const CARD: Card = aCard({ id: "C-0001", title: "The tide foreword", targets: ["RULE-TIDE-WINDOW"], items: [cardItemOf(LEAD), cardItemOf(PLAIN)], history: HISTORY });

  function client(): StubClient {
    const stub = stubClient([LEAD, PLAIN]);
    stub.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: CARD.id, title: CARD.title, targets: CARD.targets })], { flows: [aCardFlow({ return: "doing" })] })));
    stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === CARD.id ? structuredClone(CARD) : { id, reason: `no card ${id}` }));
    return stub;
  }

  async function block(stub: StubClient, id: string): Promise<HTMLElement> {
    renderApp(stub, "#/alpha/board/C-0001");
    const dialog = await screen.findByRole("dialog", { name: "The tide foreword" });
    return within(dialog).findByRole("article", { name: id });
  }

  it("leads an item's block with the gist and why it matters, its text and working answer folded; an item without a gist as before", async () => {
    const stub = client();
    const item = await block(stub, "PR-0059");
    const gist = item.querySelector(".lead-gist");
    expect(gist?.textContent).toBe(GIST);
    const matters = within(item).getByText("Why it matters");
    expect(matters.nextElementSibling?.textContent).toBe(shownText(MATTERS));
    const { button, region } = fold(item);
    expect([button.getAttribute("aria-expanded"), region.hidden]).toEqual(["false", true]);
    expect(gist !== null && before(gist, matters) && before(matters, button)).toBe(true);
    expect(Array.from(region.querySelectorAll("bdi"), (bdi) => bdi.textContent)).toEqual([shownText(ASKED), WORKING]);
    expect(within(region).getByText("Working answer")).toBeTruthy();
    expect(item.querySelector(".board-item-summary")).toBeNull();
    const plain = screen.getByRole("article", { name: "PR-0060" });
    expect(plain.querySelector(".board-item-summary")?.textContent).toBe("Does the foreword name the datum?");
    expect(plain.querySelector(".lead-gist")).toBeNull();
    expect(within(plain).queryByRole("button", { name: "The question as asked" })).toBeNull();
  });

  it("offers in Answer… the options, the working answer, an own answer and Reject, nothing picked, the recommendation the AI's; the asked text folded", async () => {
    const stub = client();
    const item = await block(stub, "PR-0059");
    fireEvent.click(within(item).getByRole("button", { name: "Answer…" }));
    const dialog = await screen.findByRole("dialog", { name: "Answer PR-0059" });
    await within(dialog).findByRole("group", { name: /^Answer PR-0059/ });
    expect(dialog.querySelector(".lead-gist")?.textContent).toBe(GIST);
    expect(radios(dialog)).toEqual([
      ["[0] Wait at the anchorageRecommended by the AIDeep ships hold.Price: None: as built.", false],
      ["[1] Enter on yesterday's tableShips keep moving.Price: A grounding risk.", false],
      ["Take the working answer", false],
      ["Your answer", false],
      ["Reject with a reason", false],
    ]);
    const { button, region } = fold(dialog);
    expect([button.getAttribute("aria-expanded"), region.hidden]).toEqual(["false", true]);
    expect(Array.from(region.querySelectorAll("bdi"), (bdi) => bdi.textContent)).toEqual([shownText(ASKED), WORKING, PRICE]);
    fireEvent.click(within(dialog).getByRole("radio", { name: /^\[1\]/ }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    await within(dialog).findByRole("region", { name: "Staged decision" });
    expect(stub.stageDecision.mock.calls[0]?.[2]).toEqual({ decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: null });
  });
});

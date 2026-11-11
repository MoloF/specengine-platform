import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Card, CardItem, Proposal, TeamAct } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow, aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { STAGED_AT, STAGED_SHOWN, stubClient, type StubClient } from "../test/stubClient";
import { ACTING_KEY } from "./acting";

// U4 of docs/features/card-questions.md "Acceptance criteria": Answer… on an open item reads its
// review document once and offers the Questions screen's controls by its structure, never its kind:
// with no options, Take the working answer, Your answer and Reject with a reason; with options, each
// and Reject. Stage posts the decision route with the stage, `updated_at` as read and `as`; the
// dialog then shows Staged <at> and the terminal command with Copy, `spec reject …` after a reject,
// with no Confirm on a local daemon. On a hub, `allowed` with `confirm` adds Confirm; outside `[team]`
// Answer… is off, with its line. An applied item has none; a refusal keeps the dialog.

const FLOW = aCardFlow({ return: "doing" });

// The items' chunk and the Inbox's Confirm dialog loaded once for the file, so a lazy block or dialog never waits on its first transform.
beforeAll(async () => {
  await Promise.all([import("./CardItems"), import("../inbox/ConfirmDialog")]);
});

const PERSON = { type: "human", role: null, model: null, run: null };

function anItem(fields: Partial<CardItem> & Pick<CardItem, "id" | "status">): CardItem {
  return {
    kind: "question",
    summary: `Summary of ${fields.id}`,
    working_answer: "The harbour bell.",
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

/** The open question as its review document serves it: no options, a working answer and its price. */
const QUESTION: Proposal = aProposal({
  id: "PR-0055",
  kind: "question",
  target_ids: ["SCN-LAMP"],
  summary: "Which bell rings at dusk?",
  working_answer: "The harbour bell.",
  price_of_other: "Two scenes rewritten.",
  card_id: "C-0001",
  updated_at: "2026-11-11T08:00:00Z",
});

/** A question with options: each is an answer. */
const CHOOSING: Proposal = aProposal({
  id: "PR-0057",
  kind: "question",
  summary: "Which lamp?",
  options: [
    { label: "Oil", effect: "The lamp burns oil.", price: "None." },
    { label: "Gas", effect: "The lamp burns gas.", price: "A new scene." },
  ],
  recommendation: 0,
  updated_at: "2026-11-11T08:00:00Z",
});

/** An item whose ID is off the queue's form: no command is offered for it. */
const ODD: Proposal = aProposal({ id: "PR-55", kind: "question", summary: "Odd ID", working_answer: "Yes.", updated_at: "2026-11-11T08:00:00Z" });

function cardOf(items: CardItem[]): Card {
  return aCard({ id: "C-0001", title: "The lamp room", targets: ["SCN-LAMP"], items, history: [] });
}

const CARD = cardOf([
  anItem({ id: "PR-0055", status: "open" }),
  anItem({ id: "PR-0056", kind: "decision", status: "applied", record_id: "DEC-0013", record_title: "Full boom" }),
  anItem({ id: "PR-0057", status: "open" }),
  anItem({ id: "PR-55", status: "open" }),
]);

function client(card: Card = CARD, proposals: Proposal[] = [QUESTION, CHOOSING, ODD]): StubClient {
  const stub = stubClient(proposals);
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: card.id, title: card.title, targets: card.targets })], { flows: [FLOW] })));
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === card.id ? structuredClone(card) : { id, reason: `no card ${id}` }));
  return stub;
}

/** A hub's stub, its login `mira`, `[team]` giving `allowed`. */
function hub(allowed: TeamAct[]): StubClient {
  const stub = client();
  stub.getIdentity.mockResolvedValue({ login: "mira" });
  stub.getTeam.mockResolvedValue({ login: "mira", name: null, allowed });
  return stub;
}

/** The item's block in the card's Activity, once its chunk arrived. */
async function block(stub: StubClient, id: string): Promise<HTMLElement> {
  renderApp(stub, "#/alpha/board/C-0001");
  const dialog = await screen.findByRole("dialog", { name: "The lamp room" });
  return within(dialog).findByRole("article", { name: id });
}

async function answer(stub: StubClient, id: string): Promise<HTMLElement> {
  const item = await block(stub, id);
  fireEvent.click(within(item).getByRole("button", { name: "Answer…" }));
  const dialog = await screen.findByRole("dialog", { name: `Answer ${id}` });
  await within(dialog).findByRole("group", { name: new RegExp(`^Answer ${id}`) });
  return dialog;
}

function picks(dialog: HTMLElement): string[] {
  return within(dialog)
    .getAllByRole("radio")
    .map((radio) => radio.closest("label")?.textContent ?? "");
}

describe("Answer… (U4 of card-questions)", () => {
  it("locally: reads the review once, offers the three by structure; Stage posts the stage, updated_at and as; then the staged line and spec approve with Copy, no Confirm", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const stub = client();
    const dialog = await answer(stub, "PR-0055");
    expect(stub.getProposal.mock.calls.filter((call) => call[1] === "PR-0055")).toHaveLength(1);
    expect(picks(dialog)).toEqual(["Take the working answer", "Your answer", "Reject with a reason"]);
    fireEvent.click(within(dialog).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(dialog).findByRole("region", { name: "Staged decision" });
    expect(stub.stageDecision.mock.calls).toEqual([
      ["alpha", "PR-0055", { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, "2026-11-11T08:00:00Z", "Ann"],
    ]);
    expect(staged.textContent).toContain(`Staged ${STAGED_SHOWN}. Confirm on a terminal: spec approve PR-0055`);
    expect(within(staged).getByText("spec approve PR-0055").tagName).toBe("CODE");
    expect(within(staged).getByRole("button", { name: /^Copy/ })).toBeTruthy();
    expect(within(staged).queryByRole("button", { name: /^Confirm/ })).toBeNull();
    expect(stub.getProposal.mock.calls.filter((call) => call[1] === "PR-0055").length).toBeLessThanOrEqual(2);
  });

  it("a reject with its reason stages a reject and copies spec reject; an own answer is sent as typed, trimmed", async () => {
    const stub = client();
    const dialog = await answer(stub, "PR-0055");
    fireEvent.click(within(dialog).getByRole("radio", { name: "Reject with a reason" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Write a reason to include this item.");
    fireEvent.change(within(dialog).getByLabelText("Reason for rejecting PR-0055"), { target: { value: " The bell is not in this book. " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(dialog).findByRole("region", { name: "Staged decision" });
    expect(stub.stageDecision.mock.calls[0]?.slice(2)).toEqual([{ decision: "reject", reason: "The bell is not in this book." }, "2026-11-11T08:00:00Z"]);
    expect(within(staged).getByText("spec reject PR-0055").tagName).toBe("CODE");
  });

  it("an item with options offers each, by number, with its effect and price as the Questions screen does, and Reject; the option is sent by number", async () => {
    const stub = client();
    const dialog = await answer(stub, "PR-0057");
    expect(picks(dialog).map((label) => label.replace(/\s+/g, " ").trim())).toEqual(["[0] OilRecommendedThe lamp burns oil.Price: None.", "[1] GasThe lamp burns gas.Price: A new scene.", "Reject with a reason"]);
    fireEvent.click(within(dialog).getAllByRole("radio")[1] as HTMLElement);
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    await within(dialog).findByRole("region", { name: "Staged decision" });
    expect(stub.stageDecision.mock.calls[0]?.[2]).toEqual({ decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: null });
  });

  it("copies no command for an ID off ^PR-[0-9]{4,}$ (M: Copy for any ID)", async () => {
    const stub = client();
    const dialog = await answer(stub, "PR-55");
    fireEvent.click(within(dialog).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(dialog).findByRole("region", { name: "Staged decision" });
    expect(staged.textContent).toContain("no command is offered for this ID.");
    expect(within(staged).queryByRole("button", { name: /^Copy/ })).toBeNull();
    expect(staged.querySelector("code")).toBeNull();
  });

  it("a refusal keeps the dialog and the pick, in the daemon's words", async () => {
    const stub = client();
    stub.stageDecision.mockRejectedValueOnce(new ClientError({ status: 409, message: "`PR-0055` changed since it was read; read it again; nothing staged" }));
    const dialog = await answer(stub, "PR-0055");
    fireEvent.click(within(dialog).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    const said = await within(dialog).findByText(/changed since it was read/);
    expect(said.getAttribute("lang")).toBe("en");
    expect(screen.getByRole("dialog", { name: "Answer PR-0055" })).toBe(dialog);
    expect(within(dialog).getByRole<HTMLInputElement>("radio", { name: "Take the working answer" }).checked).toBe(true);
  });

  it("an applied item has no Answer…", async () => {
    const item = await block(client(), "PR-0056");
    expect(within(item).queryByRole("button", { name: "Answer…" })).toBeNull();
  });
});

describe("Answer… on a hub (U4 of card-questions)", () => {
  it("allowed with confirm: Confirm beside the staged choice, the Inbox's dialog; the stage goes as the login", async () => {
    const stub = hub(["stage", "confirm"]);
    const dialog = await answer(stub, "PR-0055");
    fireEvent.click(within(dialog).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(dialog).findByRole("region", { name: "Staged decision" });
    expect(stub.stageDecision.mock.calls[0]?.[4]).toBe("mira");
    fireEvent.click(within(staged).getByRole("button", { name: /^Confirm/ }));
    expect(await screen.findByRole("dialog", { name: "Confirm PR-0055" })).toBeTruthy();
  });

  it("stage alone: no Confirm", async () => {
    const stub = hub(["stage"]);
    const dialog = await answer(stub, "PR-0055");
    fireEvent.click(within(dialog).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(dialog).findByRole("region", { name: "Staged decision" });
    expect(within(staged).queryByRole("button", { name: /^Confirm/ })).toBeNull();
  });

  it("outside [team]: Answer… stays, off, with its line, and opens nothing (M: Answer… on outside [team])", async () => {
    const stub = hub([]);
    const item = await block(stub, "PR-0055");
    await waitFor(() => {
      expect(within(item).getByRole("button", { name: "Answer…" }).getAttribute("aria-disabled")).toBe("true");
    });
    expect(item.textContent).toContain("Your login does not stage on this project; each item stays open for someone who does.");
    fireEvent.click(within(item).getByRole("button", { name: "Answer…" }));
    expect(screen.queryByRole("dialog", { name: "Answer PR-0055" })).toBeNull();
    expect(stub.getProposal).not.toHaveBeenCalled();
    // Raising is a plain write any login makes (docs/canon/card-questions.md "The rule"): both acts stay on.
    const card = screen.getByRole("dialog", { name: "The lamp room" });
    const acts = within(card).getByRole("group", { name: "Actions" });
    for (const act of ["Ask a question…", "Record a decision…"]) {
      expect([act, within(acts).getByRole("button", { name: act }).getAttribute("aria-disabled")]).toEqual([act, null]);
    }
    fireEvent.click(within(acts).getByRole("button", { name: "Ask a question…" }));
    expect(await screen.findByRole("dialog", { name: "Ask a question on C-0001" })).toBeTruthy();
  });

  // M1 of card-questions' review: an item decided anywhere but by this tab's own stage reads back as
  // decided. Here a reject staged and confirmed from the card: the card is read again, its block Rejected.
  it("confirming a reject from the card: the block reads Rejected, its reason the answer, with no Answer…", async () => {
    const stub = hub(["stage", "confirm"]);
    // The card serves its items from the queue as it stands now.
    stub.getCard.mockImplementation((_project, id) => {
      const items = CARD.items.map((item) => {
        const stored = stub.state.proposals.find((proposal) => proposal.id === item.id);
        return stored === undefined ? item : { ...item, status: stored.status ?? item.status, staged_at: stored.staged_at, decision_note: stored.decision_note, decided_by: stored.decided_by };
      });
      return Promise.resolve(id === CARD.id ? structuredClone({ ...CARD, items }) : { id, reason: `no card ${id}` });
    });
    stub.getConfirm.mockImplementation((_project, id) => {
      const stored = stub.state.proposals.find((proposal) => proposal.id === id);
      return Promise.resolve({ id, question: `staged ${STAGED_AT}: spec reject ${id}\nreject ${id}? [y/N]`, updated_at: stored?.updated_at ?? STAGED_AT });
    });
    stub.confirmProposal.mockImplementation((_project, id) => {
      const stored = stub.state.proposals.find((proposal) => proposal.id === id) ?? aProposal({ id });
      const reason = stored.staged?.decision === "reject" ? stored.staged.reason : null;
      const decided: Proposal = { ...stored, status: "rejected", decision_note: reason, decided_by: "mira <mira@example.invalid>", staged: null, staged_at: null, updated_at: "2026-11-11T10:00:00Z" };
      stub.state.proposals = stub.state.proposals.map((proposal) => (proposal.id === id ? decided : proposal));
      return Promise.resolve(structuredClone(decided));
    });
    const dialog = await answer(stub, "PR-0055");
    fireEvent.click(within(dialog).getByRole("radio", { name: "Reject with a reason" }));
    fireEvent.change(within(dialog).getByLabelText("Reason for rejecting PR-0055"), { target: { value: "The bell is not in this book." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage 1 answer" }));
    const staged = await within(dialog).findByRole("region", { name: "Staged decision" });
    fireEvent.click(within(staged).getByRole("button", { name: /^Confirm/ }));
    const confirming = await screen.findByRole("dialog", { name: "Confirm PR-0055" });
    const press = await within(confirming).findByRole("button", { name: "Confirm PR-0055" });
    await waitFor(() => {
      expect(press.getAttribute("aria-disabled")).toBe("false");
    });
    fireEvent.click(press);
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Answer PR-0055" })).toBeNull();
    });
    const card = screen.getByRole("dialog", { name: "The lamp room" });
    await waitFor(() => {
      expect(within(card).getByRole("article", { name: "PR-0055" }).querySelector(".lozenge-label")?.textContent).toBe("Rejected");
    });
    const item = within(card).getByRole("article", { name: "PR-0055" });
    expect(within(item).queryByRole("button", { name: "Answer…" })).toBeNull();
    expect(item.textContent).not.toContain("Staged");
    expect(Array.from(item.querySelectorAll("dt"), (term) => [term.textContent, term.nextElementSibling?.textContent])).toContainEqual(["Answer", "The bell is not in this book."]);
  });
});

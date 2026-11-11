import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Card, IntakeDocument } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { ACTING_KEY } from "./acting";

// U3 of docs/features/card-questions.md "Acceptance criteria": Ask a question… and Record a
// decision… stand after Return… on a card with a target, open or closed, and on none without one.
// Ask posts `kind` `question`; Record `kind` `decision`, with `why` when given, else null, never an
// empty string. A hit keeps the dialog with the hit listed and Ask anyway, which names it; a refusal
// keeps the dialog, its fields and the daemon's words. The stub answers each raise as the test says.

const FLOW = aCardFlow({ return: "doing" });

// The items' chunk loaded once for the file, so a lazy block or dialog never waits on its first transform.
beforeAll(async () => {
  await import("./CardItems");
});


const OPEN: Card = aCard({ id: "C-0001", title: "The lamp room", targets: ["SCN-LAMP", "SCN-STORM"], allowed: [{ to: "doing", by: [] }] });

const CLOSED: Card = aCard({
  id: "C-0002",
  column: "closed",
  title: "Accepted already",
  targets: ["SCN-LAMP"],
  acceptance: { by: "Ivo Brandt", at: "2026-11-10T09:00:00Z", note: null, verdict: "briefed", departs: [], discrepancy: null },
});

const NO_TARGET: Card = aCard({ id: "C-0003", title: "By hand", targets: [] });

const STORED: IntakeDocument = { id: "PR-0012", created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] };

function client(): StubClient {
  const stub = stubClient();
  const cards = [OPEN, CLOSED, NO_TARGET];
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList(cards.map((card) => aCardRow({ id: card.id, column: card.column, title: card.title, targets: card.targets })), { flows: [FLOW] })));
  stub.getCard.mockImplementation((_project, id) => {
    const card = cards.find((candidate) => candidate.id === id);
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  stub.raiseOnCard.mockResolvedValue(STORED);
  return stub;
}

async function modal(card: Card, stub: StubClient = client()): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${card.id}`);
  const dialog = await screen.findByRole("dialog", { name: card.title });
  await within(dialog).findByRole("region", { name: "Details" });
  return dialog;
}

function acts(dialog: HTMLElement): string[] {
  const group = within(dialog).queryByRole("group", { name: "Actions" });
  return group === null ? [] : within(group).getAllByRole("button").map((button) => button.textContent);
}

function type(dialog: HTMLElement, label: string, value: string) {
  fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
}

async function opened(card: Card, act: string, stub: StubClient): Promise<HTMLElement> {
  const dialog = await modal(card, stub);
  fireEvent.click(within(dialog).getByRole("button", { name: act }));
  return screen.findByRole("dialog", { name: act === "Ask a question…" ? `Ask a question on ${card.id}` : `Record a decision on ${card.id}` });
}

function fillQuestion(dialog: HTMLElement) {
  type(dialog, "Question", " Which bell rings at dusk? ");
  type(dialog, "Working answer", "The harbour bell, as in chapter one.");
  type(dialog, "What another answer would cost", "Two scenes rewritten.");
}

function fillDecision(dialog: HTMLElement, why: string) {
  type(dialog, "What had to be decided", "Does the lamp burn oil?");
  type(dialog, "The decision", "Yes: whale oil, as the chapter says.");
  type(dialog, "What another answer would cost", "The lamp scenes are rewritten.");
  type(dialog, "Why (optional)", why);
}

describe("Ask a question… and Record a decision… (U3 of card-questions)", () => {
  it("stand after Return… on an open card with a target, alone on a closed one, and on none without a target", async () => {
    expect(acts(await modal(OPEN))).toEqual(["Hand off", "Return", "Ask a question…", "Record a decision…", "Edit", "Assign", "Cancel task"]);
    expect(acts(await modal(CLOSED))).toEqual(["Ask a question…", "Record a decision…"]);
    const bare = await modal(NO_TARGET);
    expect(acts(bare)).toEqual(["Hand off", "Edit", "Assign", "Cancel task"]);
  });

  it("Ask posts one raise of kind question with its texts, the targets and the acting name; stored, it closes and says so", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const stub = client();
    const dialog = await opened(OPEN, "Ask a question…", stub);
    // About: the card's targets, each checked.
    const about = within(dialog).getByRole("group", { name: "About" });
    expect(within(about).getAllByRole("checkbox").map((box) => [box.parentElement?.textContent.trim(), (box as HTMLInputElement).checked])).toEqual([
      ["SCN-LAMP", true],
      ["SCN-STORM", true],
    ]);
    fireEvent.click(within(about).getByRole("checkbox", { name: "SCN-STORM" }));
    fillQuestion(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Ask a question on C-0001" })).toBeNull();
    });
    expect(stub.raiseOnCard.mock.calls).toEqual([
      ["alpha", "C-0001", { kind: "question", node_ids: ["SCN-LAMP"], text: "Which bell rings at dusk?", working_answer: "The harbour bell, as in chapter one.", price_of_other: "Two scenes rewritten.", as: "Ann" }],
    ]);
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("Asked PR-0012");
  });

  it("asks nothing while a text is blank or no target is chosen, saying which", async () => {
    const stub = client();
    const dialog = await opened(OPEN, "Ask a question…", stub);
    type(dialog, "Question", "Which bell?");
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Fill in all three: the question, a working answer and what another answer would cost.");
    fillQuestion(dialog);
    const about = within(dialog).getByRole("group", { name: "About" });
    for (const box of within(about).getAllByRole("checkbox")) {
      fireEvent.click(box);
    }
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Choose at least one target it is about.");
    expect(stub.raiseOnCard).not.toHaveBeenCalled();
  });

  it("Record posts kind decision with its why when given, else null, never an empty string (M: why sent as \"\")", async () => {
    const stub = client();
    const dialog = await opened(CLOSED, "Record a decision…", stub);
    fillDecision(dialog, "  ");
    fireEvent.click(within(dialog).getByRole("button", { name: "Record" }));
    await waitFor(() => {
      expect(stub.raiseOnCard).toHaveBeenCalledTimes(1);
    });
    expect(stub.raiseOnCard.mock.calls[0]?.[2]).toEqual({
      kind: "decision",
      node_ids: ["SCN-LAMP"],
      text: "Does the lamp burn oil?",
      working_answer: "Yes: whale oil, as the chapter says.",
      price_of_other: "The lamp scenes are rewritten.",
      why: null,
      as: null,
    });
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("Raised PR-0012 for the owner to confirm");
    });
    const again = await opened(OPEN, "Record a decision…", stub);
    fillDecision(again, " The chapter names the oil. ");
    fireEvent.click(within(again).getByRole("button", { name: "Record" }));
    await waitFor(() => {
      expect(stub.raiseOnCard).toHaveBeenCalledTimes(2);
    });
    expect(stub.raiseOnCard.mock.calls[1]?.[2].why).toBe("The chapter names the oil.");
  });

  it("a hit keeps the dialog, lists it and offers Ask anyway, which names it; stored then, it closes", async () => {
    const stub = client();
    stub.raiseOnCard.mockResolvedValueOnce({
      id: null,
      created: false,
      hits: [{ id: "DEC-0023", source: "corpus", status: "accepted", path: "docs/records/DEC/DEC-0023.md", answer: "The harbour bell rings at dusk", record: "DEC-0023" }],
      related: [],
      linked: null,
      diagnostics: [],
      notes: [],
    });
    const dialog = await opened(OPEN, "Ask a question…", stub);
    fillQuestion(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask" }));
    const anyway = await within(dialog).findByRole("button", { name: "Ask anyway" });
    const notice = within(dialog).getByRole("status");
    expect(notice.textContent).toContain("Already answered or asked:");
    expect(notice.textContent).toContain("DEC-0023");
    expect(notice.textContent).toContain("The harbour bell rings at dusk");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Question").value).toBe(" Which bell rings at dusk? ");
    fireEvent.click(anyway);
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Ask a question on C-0001" })).toBeNull();
    });
    expect(stub.raiseOnCard.mock.calls.map((call) => call[2].distinct_from)).toEqual([undefined, ["DEC-0023"]]);
  });

  // Minor 3 of card-questions' review: with every listed hit named and nothing stored, no submit is
  // left; focus goes to Cancel, never the body. Record a decision… takes the same path.
  it.each([
    ["Ask a question…", "Ask", "Ask anyway"],
    ["Record a decision…", "Record", "Record anyway"],
  ])("%s: every listed hit named, nothing stored: the too-many line, no submit, focus on Cancel", async (act, submit, anyway) => {
    const stub = client();
    stub.raiseOnCard.mockResolvedValue({
      id: null,
      created: false,
      hits: [{ id: "PR-0009", source: "queue", status: "open", path: null, answer: null, record: null }],
      related: [],
      linked: null,
      diagnostics: [],
      notes: [],
    });
    const dialog = await opened(OPEN, act, stub);
    if (act === "Ask a question…") {
      fillQuestion(dialog);
    } else {
      fillDecision(dialog, "");
    }
    fireEvent.click(within(dialog).getByRole("button", { name: submit }));
    const again = await within(dialog).findByRole("button", { name: anyway });
    again.focus();
    fireEvent.click(again);
    await within(dialog).findByText("More earlier answers match than this dialog can name: ask on a terminal, naming each in --distinct-from.");
    expect(within(dialog).queryByRole("button", { name: anyway })).toBeNull();
    await waitFor(() => {
      expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Cancel" }));
    });
  });

  it.each([
    [400, "spec: --as: U+202E in a name: one line, no control character or bidirectional mark"],
    [409, "node_ids[0]: `SCN-LAMP` is an alias of `SCN-LAMP-ROOM`; give the canonical ID"],
  ])("a %d shows in the dialog's alert, the daemon's words under lang=en, the fields kept; Ask again resends", async (status, message) => {
    const stub = client();
    stub.raiseOnCard.mockRejectedValueOnce(new ClientError({ status, message }));
    const dialog = await opened(OPEN, "Ask a question…", stub);
    fillQuestion(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask" }));
    const said = await within(dialog).findByText(message);
    expect(said.getAttribute("lang")).toBe("en");
    expect(said.closest('[role="alert"]')).not.toBeNull();
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Working answer").value).toBe("The harbour bell, as in chapter one.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask again" }));
    await waitFor(() => {
      expect(stub.raiseOnCard).toHaveBeenCalledTimes(2);
    });
  });
});

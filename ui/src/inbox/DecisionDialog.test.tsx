import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Proposal, StageChoice } from "../api/types";
import { aProposal } from "../test/builders";
import { Announcer } from "../ui/announcer";
import { DecisionDialog } from "./DecisionDialog";

// QA1 to QA8 of docs/features/question-accept-note.md "Acceptance criteria": the Inbox's Accept on an
// item that takes an own answer (by structure: `answersOf` offers one and the item holds a working
// answer or a price of another) offers Own answer beside the working answer, which an item without
// options checks on open; Your answer is counted and required once picked; the note is the decision's
// note, said so; a note differing from the working answer it is accepted with is warned of, the first
// Accept held, and one button makes it the answer. `onSubmit` is mocked; the view: InboxView.test.tsx.

/** Q1: a question with no options, its working answer and the price of another. */
const Q1 = aProposal({ id: "PR-0001", kind: "question", summary: "How many slots?", working_answer: "Use 2 slots", price_of_other: "a rebalance" });

const OPTIONS = [
  { label: "One slot", effect: "Less room", price: "Fewer items" },
  { label: "Three slots", effect: "More room", price: "A rebalance" },
];

/** Q2: a question with two options, a working answer and a price. */
const Q2 = aProposal({ id: "PR-0002", kind: "question", summary: "Which size?", working_answer: "Use 2 slots", price_of_other: "a rebalance", options: OPTIONS });

/** D1: a discrepancy with options and no price. */
const D1 = aProposal({ id: "PR-0003", kind: "discrepancy", summary: "Code and spec differ", options: OPTIONS, recommendation: 0 });

/** U1: an update. */
const U1 = aProposal({ id: "PR-0004", kind: "update", summary: "A new rule", diff: "@@ -1,1 +1,1 @@\n-old\n+new" });

const WARNING = "Your note is not an answer. Accept records the working answer, with this note beside it.";

const HINT = "Kept in the decision's record beside the answer. It is not the answer: to answer in your own words, choose Own answer.";

type Submit = (stage: StageChoice, updatedAt: string) => Promise<string | null>;

/** The Accept dialog alone on `proposal`, its `onSubmit` counted; `revise` hands it a new version. */
function accept(proposal: Proposal) {
  const id = proposal.id ?? "";
  const onSubmit = vi.fn<Submit>(() => Promise.resolve(null));
  const page = (shown: Proposal) => (
    <Announcer>
      <DecisionDialog id={id} proposal={shown} kind="accept" onCancel={() => undefined} onSubmit={onSubmit} />
    </Announcer>
  );
  const view = render(page(proposal));
  const dialog = screen.getByRole("dialog", { name: `Accept ${id}` });
  return {
    dialog,
    onSubmit,
    revise: (next: Proposal) => {
      view.rerender(page(next));
    },
  };
}

function radioValues(dialog: HTMLElement): string[] {
  return within(dialog)
    .queryAllByRole<HTMLInputElement>("radio")
    .map((radio) => radio.value);
}

function working(dialog: HTMLElement): HTMLInputElement {
  return within(dialog).getByRole("radio", { name: /^Take the working answer/ });
}

function own(dialog: HTMLElement): HTMLInputElement {
  return within(dialog).getByRole("radio", { name: "Own answer" });
}

function note(dialog: HTMLElement): HTMLTextAreaElement {
  return within(dialog).getByLabelText("Decision note (optional)");
}

function yourAnswer(dialog: HTMLElement): HTMLTextAreaElement {
  return within(dialog).getByLabelText("Your answer");
}

function acceptButton(dialog: HTMLElement): HTMLElement {
  return within(dialog).getByRole("button", { name: "Stage accept" });
}

/** The text an element's `aria-describedby` points at. */
function description(element: HTMLElement): string {
  return (element.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .filter((id) => id !== "")
    .map((id) => document.getElementById(id)?.textContent ?? "")
    .join(" ");
}

function assertive(): string {
  return document.querySelector('[aria-live="assertive"]')?.textContent ?? "";
}

async function tick() {
  await act(() => new Promise((settle) => setTimeout(settle, 0)));
}

/** Accept clicked, the call awaited. */
async function staged(dialog: HTMLElement, onSubmit: ReturnType<typeof vi.fn<Submit>>): Promise<StageChoice | undefined> {
  fireEvent.click(acceptButton(dialog));
  await waitFor(() => {
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });
  return onSubmit.mock.calls[0]?.[0];
}

const WORKING_BODY: StageChoice = { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null };

describe("Accept on an item that takes an own answer (question-accept-note)", () => {
  it("QA1: Q2 lists both options, the working answer and Own answer, none checked, Accept off; D1 has no Own answer; U1 no answer radio and today's note", () => {
    const q2 = accept(Q2);
    expect(radioValues(q2.dialog)).toEqual(["0", "1", "working", "answer"]);
    expect(within(q2.dialog).getByRole("radio", { name: /^\[0\] One slot/ })).toBeTruthy();
    expect(within(q2.dialog).getByRole("radio", { name: /^\[1\] Three slots/ })).toBeTruthy();
    expect(working(q2.dialog)).toBeTruthy();
    expect(own(q2.dialog)).toBeTruthy();
    expect(within(q2.dialog).getAllByRole<HTMLInputElement>("radio").map((radio) => radio.checked)).toEqual([false, false, false, false]);
    expect(acceptButton(q2.dialog).getAttribute("aria-disabled")).toBe("true");
    cleanup();

    const d1 = accept(D1);
    expect(radioValues(d1.dialog)).toEqual(["0", "1"]);
    expect(within(d1.dialog).queryByRole("radio", { name: "Own answer" })).toBeNull();
    cleanup();

    const u1 = accept(U1);
    expect(radioValues(u1.dialog)).toEqual([]);
    expect(within(u1.dialog).getByLabelText("Note for the record (optional)")).toBeTruthy();
    expect(within(u1.dialog).queryByLabelText("Decision note (optional)")).toBeNull();
  });

  it("QA2: Q1 opens on the working answer, checked, Own answer beside it; an immediate Accept sends today's body", async () => {
    const { dialog, onSubmit } = accept(Q1);
    expect(radioValues(dialog)).toEqual(["working", "answer"]);
    expect([working(dialog).checked, own(dialog).checked]).toEqual([true, false]);
    expect(within(working(dialog).closest("label") as HTMLElement).getByText("Use 2 slots")).toBeTruthy();
    expect(await staged(dialog, onSubmit)).toEqual(WORKING_BODY);
    expect(onSubmit.mock.calls[0]?.[1]).toBe(Q1.updated_at);
  });

  it("QA3: Own answer on Q1 stages the trimmed answer beside the note, no option, no canon", async () => {
    const { dialog, onSubmit } = accept(Q1);
    fireEvent.click(own(dialog));
    fireEvent.change(yourAnswer(dialog), { target: { value: "  Use 3 slots  " } });
    fireEvent.change(note(dialog), { target: { value: "per the playtest" } });
    expect(await staged(dialog, onSubmit)).toEqual({ decision: "approve", option: null, answer: "Use 3 slots", canon: null, editorial: null, note: "per the playtest" });
  });

  it("QA3: Your answer shows the Questions screen's byte count beside it", () => {
    const { dialog } = accept(Q1);
    fireEvent.click(own(dialog));
    fireEvent.change(yourAnswer(dialog), { target: { value: "  Use 3 slots  " } });
    expect(description(yourAnswer(dialog))).toBe("11/2048 bytes");
  });

  it("QA4: Own answer left blank sends nothing; Write your answer first. stands under Your answer as an alert, the field required and invalid, focus there", async () => {
    const { dialog, onSubmit } = accept(Q1);
    fireEvent.click(own(dialog));
    expect([yourAnswer(dialog).getAttribute("aria-required"), yourAnswer(dialog).getAttribute("aria-invalid")]).toEqual(["true", "false"]);
    fireEvent.change(yourAnswer(dialog), { target: { value: "   " } });
    fireEvent.click(acceptButton(dialog));
    await tick();
    expect(onSubmit).not.toHaveBeenCalled();
    const field = yourAnswer(dialog);
    expect(document.activeElement).toBe(field);
    // Said as the dialog's other stops are: an alert, spoken whatever focus did first.
    const missing = within(dialog).getByRole("alert");
    expect(missing.textContent).toBe("Write your answer first.");
    expect(missing.closest(".round-field")).toBe(field.closest(".round-field"));
    expect([field.getAttribute("aria-required"), field.getAttribute("aria-invalid")]).toEqual(["true", "true"]);
    expect(description(field)).toContain("Write your answer first.");
    // Written, the stop goes.
    fireEvent.change(field, { target: { value: "Use 3 slots" } });
    expect(within(dialog).queryByRole("alert")).toBeNull();
    expect(field.getAttribute("aria-invalid")).toBe("false");
  });

  it.each([
    ["Q1", Q1],
    ["Q2", Q2],
  ])("QA5: on %s the note reads Decision note (optional), described as not the answer", (_case, item) => {
    const { dialog } = accept(item);
    expect(description(note(dialog))).toContain("It is not the answer");
    expect(description(note(dialog))).toContain(HINT);
  });

  it("QA5: U1 keeps Note for the record (optional), with no such description", () => {
    const { dialog } = accept(U1);
    const field = within(dialog).getByLabelText("Note for the record (optional)");
    expect(description(field)).not.toContain("It is not the answer");
    expect(dialog.textContent).not.toContain("It is not the answer");
  });

  it("QA6: a note other than the working answer is warned of; the first Accept stages nothing and speaks it, the second stages the working answer with the note", async () => {
    const { dialog, onSubmit } = accept(Q1);
    fireEvent.change(note(dialog), { target: { value: "Use 3 slots" } });
    const warning = within(dialog).getByRole("status");
    expect(warning.textContent).toBe(`${WARNING}Use the note as my answer`);
    const button = within(warning).getByRole("button", { name: "Use the note as my answer" });
    fireEvent.click(acceptButton(dialog));
    await tick();
    expect(onSubmit).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(button);
    expect(assertive()).toBe(WARNING);
    expect(await staged(dialog, onSubmit)).toEqual({ ...WORKING_BODY, note: "Use 3 slots" });
  });

  it("QA6: editing the note arms the hold again", async () => {
    const { dialog, onSubmit } = accept(Q1);
    fireEvent.change(note(dialog), { target: { value: "Use 3 slots" } });
    fireEvent.click(acceptButton(dialog));
    await tick();
    fireEvent.change(note(dialog), { target: { value: "Use 4 slots" } });
    fireEvent.click(acceptButton(dialog));
    await tick();
    expect(onSubmit).not.toHaveBeenCalled();
    expect(await staged(dialog, onSubmit)).toEqual({ ...WORKING_BODY, note: "Use 4 slots" });
  });

  it.each([
    ["the working answer's own words, spaced", Q1, "  Use 2 slots ", null],
    ["an empty note", Q1, "", null],
    ["an option picked on Q2", Q2, "Use 3 slots", /^\[1\] Three slots/],
    ["Own answer picked", Q1, "Use 3 slots", "Own answer"],
  ])("QA6: no warning for %s", async (_case, item, typed, pick) => {
    const { dialog, onSubmit } = accept(item);
    if (pick !== null) {
      fireEvent.click(within(dialog).getByRole("radio", { name: pick }));
    }
    if (pick === "Own answer") {
      fireEvent.change(yourAnswer(dialog), { target: { value: "Use 3 slots" } });
    }
    fireEvent.change(note(dialog), { target: { value: typed } });
    // The live region stands, empty; the hidden copy that keeps the warning's place is out of reach.
    expect(within(dialog).getByRole("status").textContent).toBe("");
    expect(within(dialog).queryByRole("button", { name: "Use the note as my answer" })).toBeNull();
    fireEvent.click(acceptButton(dialog));
    await waitFor(() => {
      expect(onSubmit).toHaveBeenCalledTimes(1);
    });
  });

  it("QA6: a revision arms the hold again: the new version's first Accept is held too", async () => {
    const { dialog, onSubmit, revise } = accept(Q1);
    fireEvent.change(note(dialog), { target: { value: "Use 3 slots" } });
    fireEvent.click(acceptButton(dialog));
    await tick();
    expect(onSubmit).not.toHaveBeenCalled();
    revise({ ...Q1, summary: "How many slots, revised?", updated_at: "2026-10-01T11:00:00Z" });
    expect(working(dialog).checked).toBe(true);
    expect(within(dialog).getByRole("status").textContent).toContain(WARNING);
    fireEvent.click(acceptButton(dialog));
    await tick();
    expect(onSubmit).not.toHaveBeenCalled();
    expect(await staged(dialog, onSubmit)).toEqual({ ...WORKING_BODY, note: "Use 3 slots" });
    expect(onSubmit.mock.calls[0]?.[1]).toBe("2026-10-01T11:00:00Z");
  });

  it("QA10's layout, as far as a DOM shows it: the warning comes into the note hint's own cell, beside a hidden copy holding its place, so the form's blocks stay as they were", () => {
    const { dialog } = accept(Q1);
    const form = dialog.querySelector("form") as HTMLFormElement;
    const blocks = () => Array.from(form.children, (child) => `${child.tagName}.${child.className}`);
    const before = blocks();
    const guide = dialog.querySelector(".note-guide") as HTMLElement;
    const copy = guide.querySelector('[aria-hidden="true"]') as HTMLElement;
    expect(copy.hasAttribute("inert")).toBe(true);
    expect(copy.textContent).toBe(`${WARNING}Use the note as my answer`);
    fireEvent.change(note(dialog), { target: { value: "Use 3 slots" } });
    expect(blocks()).toEqual(before);
    expect(within(guide).getByRole("status").textContent).toBe(`${WARNING}Use the note as my answer`);
    expect(guide.querySelector(".card-field-hint")?.classList.contains("note-guide-off")).toBe(true);
    expect(before.at(-1)).toBe("DIV.dialog-actions");
  });

  it("QA7: Use the note as my answer moves the note into Your answer, clears it, picks Own answer and focuses it; Accept stages it", async () => {
    const { dialog, onSubmit } = accept(Q1);
    fireEvent.change(note(dialog), { target: { value: "Use 3 slots" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Use the note as my answer" }));
    await tick();
    expect(yourAnswer(dialog).value).toBe("Use 3 slots");
    expect(note(dialog).value).toBe("");
    expect([working(dialog).checked, own(dialog).checked]).toEqual([false, true]);
    expect(document.activeElement).toBe(yourAnswer(dialog));
    expect(await staged(dialog, onSubmit)).toEqual({ ...WORKING_BODY, answer: "Use 3 slots", note: null });
  });

  it("QA8: a revision still offering an own answer keeps the pick and both texts; one that no longer does clears the pick and keeps the texts", () => {
    const { dialog, revise } = accept(Q1);
    fireEvent.click(own(dialog));
    fireEvent.change(yourAnswer(dialog), { target: { value: "Use 3 slots" } });
    fireEvent.change(note(dialog), { target: { value: "per the playtest" } });

    revise({ ...Q1, summary: "How many slots, revised?", updated_at: "2026-10-01T11:00:00Z" });
    expect(within(dialog).getByText("This proposal changed since you opened it")).toBeTruthy();
    expect([working(dialog).checked, own(dialog).checked]).toEqual([false, true]);
    expect(yourAnswer(dialog).value).toBe("Use 3 slots");
    expect(note(dialog).value).toBe("per the playtest");

    revise({ ...Q1, options: OPTIONS, working_answer: null, price_of_other: null, updated_at: "2026-10-01T12:00:00Z" });
    expect(radioValues(dialog)).toEqual(["0", "1"]);
    expect(within(dialog).getAllByRole<HTMLInputElement>("radio").map((radio) => radio.checked)).toEqual([false, false]);
    expect(within(dialog).queryByLabelText("Your answer")).toBeNull();
    const kept = within(dialog).getByLabelText<HTMLTextAreaElement>("Note for the record (optional)");
    expect(kept.value).toBe("per the playtest");
    expect(acceptButton(dialog).getAttribute("aria-disabled")).toBe("true");

    revise({ ...Q1, updated_at: "2026-10-01T13:00:00Z" });
    fireEvent.click(own(dialog));
    expect(yourAnswer(dialog).value).toBe("Use 3 slots");
  });
});

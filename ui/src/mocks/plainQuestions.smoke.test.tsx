import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { StageChoice } from "../api/types";
import { App } from "../app/App";
import { shownText } from "../ui/text";
import { MockClient } from "./MockClient";
import { PERSON_FIELDS, PERSON_QUESTION, PLAIN_CARD, PLAIN_FIELDS, PLAIN_QUESTION } from "./plainQuestions";

// docs/features/plain-questions.md "Acceptance criteria", PQ12 and PQ14, over the mock as `pnpm dev`
// shows it with `?scenario=plain-questions` (src/mocks/plainQuestions.ts): harbor-sim's queue holds an
// agent's question with a gist, why it matters, two options and a recommendation, PR-0059, and a
// person's without them, PR-0060, both on C-0001; `normal` holds neither. The stub cases run in
// src/inbox/ItemLead.test.tsx; the mock is imported here alone (policy AC-05).

const SLUG = "harbor-sim";

// The items' chunk loaded once for the file, so a lazy block never waits on its first transform.
beforeAll(async () => {
  await import("../board/CardItems");
});

function renderMock(hash: string) {
  window.localStorage.setItem("specengine.module.harbor-sim", "");
  window.history.replaceState(null, "", `/?scenario=plain-questions${hash}`);
  const client = new MockClient("plain-questions", { delayMs: 0 });
  render(<App client={client} scenario={null} />);
  return client;
}

describe("the plain-questions scenario's data (PQ10, PQ14)", () => {
  it("serves PR-0059 with its four fields byte-equal, the inbox entry its gist whole, the round item and C-0001's item the gist and why it matters", async () => {
    const client = new MockClient("plain-questions", { delayMs: 0 });
    const review = await client.getProposal(SLUG, PLAIN_QUESTION);
    expect([review.summary, review.gist, review.matters, review.options, review.recommendation, review.author?.type]).toEqual([
      PLAIN_FIELDS.text,
      PLAIN_FIELDS.gist,
      PLAIN_FIELDS.matters,
      PLAIN_FIELDS.options,
      PLAIN_FIELDS.recommendation,
      "agent",
    ]);
    const keys = Object.keys(review);
    expect(keys.slice(keys.indexOf("summary"), keys.indexOf("summary") + 3)).toEqual(["summary", "gist", "matters"]);
    const person = await client.getProposal(SLUG, PERSON_QUESTION);
    expect([person.gist, person.matters, person.options, person.recommendation, person.author?.type]).toEqual([null, null, [], null, "human"]);
    const entries = (await client.getInbox(SLUG)).proposals;
    expect(entries.find((entry) => entry.id === PLAIN_QUESTION)?.gist).toBe(PLAIN_FIELDS.gist);
    expect(entries.find((entry) => entry.id === PERSON_QUESTION)?.gist).toBeNull();
    const round = await client.getRound(SLUG);
    expect(round.items.slice(0, 2).map((item) => [item.id, item.gist, item.matters])).toEqual([
      [PLAIN_QUESTION, PLAIN_FIELDS.gist, PLAIN_FIELDS.matters],
      [PERSON_QUESTION, null, null],
    ]);
    const card = await client.getCard(SLUG, PLAIN_CARD);
    expect("items" in card ? card.items.map((item) => [item.id, item.gist, item.matters]) : null).toEqual([
      [PLAIN_QUESTION, PLAIN_FIELDS.gist, PLAIN_FIELDS.matters],
      [PERSON_QUESTION, null, null],
    ]);
  });

  it("holds neither in `normal`, whose items serve the two keys null", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const ids = (await client.getInbox(SLUG)).proposals.map((entry) => entry.id);
    expect(ids).not.toContain(PLAIN_QUESTION);
    expect(ids).not.toContain(PERSON_QUESTION);
    const round = await client.getRound(SLUG);
    expect(round.items.every((item) => item.gist === null && item.matters === null)).toBe(true);
  });

  it("takes the working answer or an own answer on PR-0059 (judged by its fields), and refuses neither of them a discrepancy would", async () => {
    const client = new MockClient("plain-questions", { delayMs: 0 });
    const review = await client.getProposal(SLUG, PLAIN_QUESTION);
    const staged = await client.stageDecision(SLUG, PLAIN_QUESTION, { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, review.updated_at ?? "");
    expect(staged.staged).toMatchObject({ decision: "approve", option: null, answer: null });
    const outcome = await client.stageRoundAnswers(SLUG, [
      { id: PLAIN_QUESTION, stage: { decision: "approve", option: null, answer: "Only at spring tides", canon: null, editorial: null, note: null, updated_at: staged.updated_at ?? "" } },
    ]);
    expect(outcome.answers.map((answer) => answer.outcome)).toEqual(["staged"]);
    // A discrepancy's options stay its only answers.
    const discrepancy = await client.getProposal(SLUG, "PR-0041");
    await expect(
      client.stageDecision(SLUG, "PR-0041", { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, discrepancy.updated_at ?? ""),
    ).rejects.toThrow(/takes --option N/);
  });

  it("refuses as the daemon does an option with an own answer on PR-0059, and an option on PR-0060, which holds none: the decision route 400 in the CLI's line, the round's reason without its frame; neither staged, the option alone then taken", async () => {
    const client = new MockClient("plain-questions", { delayMs: 0 });
    const both = (await client.getProposal(SLUG, PLAIN_QUESTION)).updated_at ?? "";
    const none = (await client.getProposal(SLUG, PERSON_QUESTION)).updated_at ?? "";
    const refusal = (id: string, stage: StageChoice, at: string) =>
      client.stageDecision(SLUG, id, stage, at).then(
        () => null,
        (error: unknown) => (error instanceof ClientError ? [error.status, error.message] : error),
      );
    const approve = (option: number | null, answer: string | null): StageChoice => ({ decision: "approve", option, answer, canon: null, editorial: null, note: null });
    expect(await refusal(PLAIN_QUESTION, approve(1, "Only at spring tides"), both)).toEqual([
      400,
      `spec: \`--option\` and \`--answer\` each name the owner's choice, and \`${PLAIN_QUESTION}\` takes one: give either; nothing changed`,
    ]);
    expect(await refusal(PERSON_QUESTION, approve(0, null), none)).toEqual([
      400,
      `spec: \`--option\` names an option, and \`${PERSON_QUESTION}\` is a question with none: its working answer, or \`--answer T\`, answers it; nothing changed`,
    ]);
    const outcome = await client.stageRoundAnswers(SLUG, [
      { id: PLAIN_QUESTION, stage: { ...approve(1, "Only at spring tides"), updated_at: both } },
      { id: PERSON_QUESTION, stage: { ...approve(0, null), updated_at: none } },
    ]);
    expect(outcome.answers.map(({ id, outcome: made, reason }) => [id, made, reason])).toEqual([
      [PLAIN_QUESTION, "refused", `\`--option\` and \`--answer\` each name the owner's choice, and \`${PLAIN_QUESTION}\` takes one: give either`],
      [PERSON_QUESTION, "refused", `\`--option\` names an option, and \`${PERSON_QUESTION}\` is a question with none: its working answer, or \`--answer T\`, answers it`],
    ]);
    expect([(await client.getProposal(SLUG, PLAIN_QUESTION)).staged, (await client.getProposal(SLUG, PERSON_QUESTION)).staged]).toEqual([null, null]);
    const alone = await client.stageRoundAnswers(SLUG, [{ id: PLAIN_QUESTION, stage: { ...approve(1, null), updated_at: both } }]);
    expect(alone.answers.map((answer) => answer.outcome)).toEqual(["staged"]);
  });
});

describe("the plain-questions scenario on screen (PQ14)", () => {
  it("leads the Questions screen with PR-0059's gist, why it matters and its options, nothing picked, the fold closed; PR-0060 as before", async () => {
    renderMock("#/harbor-sim/questions");
    const article = await screen.findByRole("article", { name: PLAIN_QUESTION });
    expect(article.querySelector(".lead-gist")?.textContent).toBe(PLAIN_FIELDS.gist);
    expect(within(article).getByText("Why it matters").nextElementSibling?.textContent).toBe(PLAIN_FIELDS.matters);
    expect(article.querySelector(".recommended-mark")?.textContent).toBe("Recommended by the AI");
    expect(within(article).getAllByRole<HTMLInputElement>("radio").filter((radio) => radio.checked).map((radio) => radio.closest("label")?.textContent)).toEqual(["Leave open"]);
    expect(within(article).getByRole("button", { name: "The question as asked" }).getAttribute("aria-expanded")).toBe("false");
    const person = screen.getByRole("article", { name: PERSON_QUESTION });
    expect(person.querySelector(".lead-gist")).toBeNull();
    expect(person.querySelector(".round-text")?.textContent).toBe(PERSON_FIELDS.text);
  });

  it("titles the Inbox card by the gist, folds the agent's text, and leads C-0001's Activity with it", async () => {
    renderMock(`#/harbor-sim/inbox/${PLAIN_QUESTION}`);
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    expect(within(card).getByRole("heading", { level: 2 }).textContent).toBe(PLAIN_FIELDS.gist);
    const fold = within(card).getByRole("button", { name: "The question as asked" });
    const region = document.getElementById(fold.getAttribute("aria-controls") ?? "");
    expect(Array.from(region?.querySelectorAll("bdi") ?? [], (bdi) => bdi.textContent)).toContain(shownText(PLAIN_FIELDS.text));
    fireEvent.click(within(card).getByRole("link", { name: /C-0001/ }));
    const dialog = await screen.findByRole("dialog", { name: "Write the tide table foreword" });
    const item = await within(dialog).findByRole("article", { name: PLAIN_QUESTION });
    expect(item.querySelector(".lead-gist")?.textContent).toBe(PLAIN_FIELDS.gist);
    const plain = within(dialog).getByRole("article", { name: PERSON_QUESTION });
    expect(plain.querySelector(".board-item-summary")?.textContent).toBe(PERSON_FIELDS.text);
  });
});

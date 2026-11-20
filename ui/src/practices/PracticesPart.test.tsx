import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import type { Card, CardEvent, CardItem } from "../api/types";
import { docPageHash } from "../app/routes";
import { switchLanguage } from "../i18n";
import { ru } from "../i18n/ru";
import { aCard, aCardFlow, aCardList, aCardRow, aCited, aPractice } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";

// docs/features/department-practices.md "Acceptance criteria", DP21, over the stub board (the mock's
// look runs in src/mocks/practices.smoke.test.tsx): the work item's Practices part after its targets,
// each practice a document link by its name with its title and rule count; its none and failure states;
// and the card's texts (title, goal, criteria, an item, a history note) linking each served `cited`
// string to its document, an unserved ID-shaped token and one inside code left as text.

const FLOW = aCardFlow({ name: "art", return: "back" });

beforeAll(async () => {
  await import("./PracticesPart");
  await import("../board/CardItems");
});

const PERSON = { type: "human", role: null, model: null, run: null };

/** A question on the card, its texts citing the served rule and an unserved one. */
const ITEM: CardItem = {
  id: "PR-0070",
  kind: "question",
  status: "open",
  summary: "Does PRA-ART-NORMALS hold for props? PRA-NOPE-Y says nothing.",
  gist: null,
  matters: null,
  working_answer: "Yes, as PRA-ART-NORMALS reads.",
  why: null,
  author: PERSON,
  created_at: "2026-11-20T08:00:00Z",
  updated_at: "2026-11-20T08:00:00Z",
  staged_at: null,
  decided_by: null,
  decided_at: null,
  decision_note: null,
  record_id: null,
  record_path: null,
  record_title: null,
};

const HISTORY: CardEvent[] = [
  { seq: 1, type: "card.created", at: "2026-11-19T09:00:00Z", payload: { id: "C-0001", column: "todo", as: "Ann" } },
  { seq: 2, type: "card.moved", at: "2026-11-19T10:00:00Z", payload: { id: "C-0001", from: "todo", to: "doing", as: "Ann", note: "Back: the LODs break PRA-ART-LOD, and PRA-NOPE-Z is unknown." } },
  { seq: 3, type: "proposal.created", at: "2026-11-20T08:00:00Z", payload: { id: "PR-0070", as: "Ann" } },
];

/** C-0001 of the spec's scene: an art card whose goal cites PRA-ART-LOD and PRA-NOPE-X, two practices served. */
const C_0001: Card = aCard({
  id: "C-0001",
  flow: "art",
  column: "doing",
  title: "Model the crane per PRA-ART-REALISM",
  goal: "A crane model that keeps PRA-ART-LOD; `PRA-ART-LOD` in code and PRA-NOPE-X stay text.",
  criteria: ["Normals as PRA-ART-NORMALS asks", "No rule PRA-ART-LODS"],
  discipline: "art",
  targets: ["SCN-CRANE"],
  items: [ITEM],
  history: HISTORY,
  practices: [aPractice({ name: "PRA-ART-MODELS", title: "3D models: agreed rules", path: "docs/practices/art.md", rules: 3 }), aPractice({ name: "PRA-SHARED-NAMING", title: "Naming", path: "docs/practices/shared.md", disciplines: ["art", "development"], rules: 1 })],
  cited: [
    aCited("PRA-ART-REALISM", { name: "art/PRA-ART-REALISM" }),
    aCited("PRA-ART-LOD", { name: "art/PRA-ART-LOD", title: "Level of detail" }),
    aCited("PRA-ART-NORMALS", { name: "art/PRA-ART-NORMALS" }),
  ],
});

function client(card: Card): StubClient {
  const stub = stubClient();
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: card.id, flow: "art", column: card.column, title: card.title, targets: card.targets })], { flows: [FLOW] })));
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === card.id ? structuredClone(card) : { id, reason: `no card ${id}` }));
  return stub;
}

/** The card's modal, its main column read. */
async function modal(card: Card, stub: StubClient = client(card)): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${card.id}`);
  const dialog = await screen.findByRole("dialog", { name: card.title });
  await within(dialog).findByRole("region", { name: "Details" });
  return dialog;
}

/** The main column's parts, by name, in order. */
function parts(dialog: HTMLElement): string[] {
  const main = dialog.querySelector<HTMLElement>(".issue-view-main");
  if (main === null) {
    throw new Error("no main column");
  }
  return Array.from(main.querySelectorAll("section[aria-label]"), (part) => part.getAttribute("aria-label") ?? "");
}

/** Every document link in a scope: its words and where it goes. */
function docLinks(scope: HTMLElement): [string, string][] {
  return within(scope)
    .queryAllByRole("link")
    .map((link) => [link.textContent, link.getAttribute("href") ?? ""]);
}

describe("the work item's Practices part (DP21)", () => {
  it("stands right after Source: each practice a document link by its name, its title and its rule count", async () => {
    const dialog = await modal(C_0001);
    const part = await within(dialog).findByRole("region", { name: "Practices" });
    const names = parts(dialog);
    expect(names.indexOf("Practices")).toBe(names.indexOf("Source") + 1);
    const rows = within(part).getAllByRole("listitem");
    expect(rows.map((row) => row.textContent)).toEqual(["PRA-ART-MODELS3D models: agreed rules3 rules", "PRA-SHARED-NAMINGNaming1 rule"]);
    expect(docLinks(part)).toEqual([
      ["PRA-ART-MODELS", docPageHash("alpha", "PRA-ART-MODELS")],
      ["PRA-SHARED-NAMING", docPageHash("alpha", "PRA-SHARED-NAMING")],
    ]);
    // A plain click opens the document modal over the card, not its page.
    fireEvent.click(within(part).getByRole("link", { name: "PRA-ART-MODELS" }));
    expect(window.location.hash).toBe("#/alpha/board/C-0001?doc=PRA-ART-MODELS");
  });

  it("says when none matches the discipline, naming it, with the next step; a card with no discipline says it has none", async () => {
    const none = await modal(aCard({ id: "C-0003", flow: "art", title: "Sound pass", discipline: "sound", practices: [], cited: [] }));
    const part = within(none).getByRole("region", { name: "Practices" });
    expect(part.textContent).toContain("No practices for the discipline sound");
    expect(part.textContent).toContain("Next: a document that names this discipline in its front-matter disciplines field shows here on the next read.");
    expect(within(part).queryAllByRole("link")).toEqual([]);
  });

  it("says a card with no discipline has no practices", async () => {
    const bare = await modal(aCard({ id: "C-0004", flow: "art", title: "Loose task", discipline: null, practices: [], cited: [] }));
    const part = within(bare).getByRole("region", { name: "Practices" });
    expect(part.textContent).toContain("This task has no discipline, so no practices apply");
  });

  it("shows a failed read in the daemon's words with Retry, which reads the card again; the note leaves the card's notes", async () => {
    const note = "practices: not read: the index at /tmp/idx could not be opened: database is locked";
    const failed = aCard({ ...C_0001, practices: null, cited: null, notes: [note, "an own note"] });
    const stub = client(failed);
    const dialog = await modal(failed, stub);
    const part = within(dialog).getByRole("region", { name: "Practices" });
    const alert = within(part).getByRole("alert");
    expect(alert.textContent).toContain("The practices could not be read");
    expect(alert.textContent).toContain(note);
    expect(within(alert).getByText(note).getAttribute("lang")).toBe("en");
    // Among the card's own notes, the practices' one is not said again.
    const notes = within(dialog).getByRole("list", { name: "Notes from the daemon on the task" });
    expect(within(notes).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["an own note"]);
    // Nothing is linked: the read served no citation.
    expect(within(dialog).queryByRole("link", { name: "PRA-ART-LOD" })).toBeNull();
    const reads = stub.getCard.mock.calls.length;
    stub.getCard.mockImplementation(() => Promise.resolve(structuredClone(C_0001)));
    fireEvent.click(within(part).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(stub.getCard.mock.calls.length).toBe(reads + 1);
    });
    expect(await within(dialog).findAllByRole("link", { name: "PRA-ART-MODELS" })).toHaveLength(1);
  });

  it("is not drawn when the read did not ask (`null` and no note)", async () => {
    const dialog = await modal(aCard({ id: "C-0005", flow: "art", title: "Old read", discipline: "art" }));
    expect(within(dialog).queryByRole("region", { name: "Practices" })).toBeNull();
  });

  it("says past the first 32 how many more, from which, in the daemon's words under the list", async () => {
    const more = "practices: 8 more practice documents of art, from docs/practices/zz.md";
    const dialog = await modal(aCard({ ...C_0001, notes: [more] }));
    const part = within(dialog).getByRole("region", { name: "Practices" });
    expect(within(part).getByText(more).getAttribute("lang")).toBe("en");
    expect(within(dialog).queryByRole("list", { name: "Notes from the daemon on the task" })).toBeNull();
  });

  it("speaks Russian: the part and the rule counts' plural forms", async () => {
    const dialog = await modal(C_0001);
    act(() => {
      switchLanguage("ru");
    });
    const part = await within(dialog).findByRole("region", { name: ru.practices.title });
    expect(within(part).getAllByRole("listitem").map((row) => row.lastElementChild?.textContent)).toEqual(["3 \u043f\u0440\u0430\u0432\u0438\u043b\u0430", "1 \u043f\u0440\u0430\u0432\u0438\u043b\u043e"]);
  });
});

describe("a card's cited links (DP21)", () => {
  it("links each served string in the title, the goal and the criteria; an unserved ID and one inside code stay text", async () => {
    const dialog = await modal(C_0001);
    const heading = within(dialog).getByRole("heading", { level: 2 });
    expect(docLinks(heading)).toEqual([["PRA-ART-REALISM", docPageHash("alpha", "art/PRA-ART-REALISM")]]);
    const goal = within(dialog).getByRole("region", { name: "Goal" });
    expect(docLinks(goal)).toEqual([["PRA-ART-LOD", docPageHash("alpha", "art/PRA-ART-LOD")]]);
    expect(goal.textContent).toContain("A crane model that keeps PRA-ART-LOD; `PRA-ART-LOD` in code and PRA-NOPE-X stay text.");
    const criteria = within(dialog).getByRole("region", { name: "Criteria" });
    // A longer ID the daemon did not serve is never linked in part.
    expect(docLinks(criteria)).toEqual([["PRA-ART-NORMALS", docPageHash("alpha", "art/PRA-ART-NORMALS")]]);
    expect(criteria.textContent).toContain("No rule PRA-ART-LODS");
  });

  it("links them in an item's texts and a history note, never an unserved one", async () => {
    const dialog = await modal(C_0001);
    const activity = within(dialog).getByRole("region", { name: "Activity" });
    const item = await within(activity).findByRole("article", { name: "PR-0070" });
    expect(docLinks(item).filter(([words]) => words.startsWith("PRA-"))).toEqual([
      ["PRA-ART-NORMALS", docPageHash("alpha", "art/PRA-ART-NORMALS")],
      ["PRA-ART-NORMALS", docPageHash("alpha", "art/PRA-ART-NORMALS")],
    ]);
    expect(item.textContent).toContain("PRA-NOPE-Y says nothing.");
    const line = within(activity)
      .getAllByRole("listitem")
      .find((entry) => entry.textContent.includes("the LODs break"));
    expect(line).toBeDefined();
    expect(docLinks(line as HTMLElement)).toEqual([["PRA-ART-LOD", docPageHash("alpha", "art/PRA-ART-LOD")]]);
    expect(line?.textContent).toContain("PRA-NOPE-Z is unknown.");
    // Every link of the work item's texts names a served string: no ID-shaped token links unasked.
    const served = new Set((C_0001.cited ?? []).map((entry) => entry.written));
    const textLinks = within(dialog)
      .getAllByRole("link")
      .filter((link) => link.classList.contains("cited-link"));
    expect(textLinks.length).toBeGreaterThan(4);
    expect(textLinks.filter((link) => !served.has(link.textContent))).toEqual([]);
  });

  it("links nothing in the texts when the read served no citation", async () => {
    const dialog = await modal(aCard({ ...C_0001, cited: [] }));
    expect(within(dialog).queryAllByRole("link").filter((link) => link.classList.contains("cited-link"))).toEqual([]);
    expect(within(dialog).getByRole("region", { name: "Goal" }).textContent).toContain("A crane model that keeps PRA-ART-LOD;");
  });
});

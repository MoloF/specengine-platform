import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import type { Card, CardList, CardSubtaskItem, CardWrite } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow, aCardWrite } from "../test/builders";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";
import { ACTING_KEY } from "./acting";

// V3 of docs/features/task-mandate.md "Acceptance criteria", on the stub board shaped as the mock's
// C-0001 (the mock end to end: src/mocks/mandate.smoke.test.tsx; a test outside src/mocks/ imports no
// mock): a parent's Decisions from subtasks stands after Subtasks when its card's `subtask_items` holds
// a decision, each in served order. Its head: the subtask's ID a link, its title and its flow's title
// from the children read Subtasks holds, the subtask's mandate as that row has it, a binding one's
// decision marked as a departure; then the item's block; then Ask about it…, a question on the subtask
// naming the decision. Questions on subtasks stand on their own Activity, not here. One card read and
// one children read: never a read per subtask.

// The items' chunk loaded once for the file, so the lazy part never waits on its first transform.
beforeAll(async () => {
  await import("./CardItems");
});

const COPY = aCardFlow({ name: "copy", title: "Copy desk", disciplines: ["copy"] });
const PLATES = aCardFlow({ name: "plates", disciplines: ["plates"], return: "doing" });

/** One item naming a subtask, as `subtask_items` serves it: the subtask first, then a card item's keys. */
function anItem(fields: Partial<CardSubtaskItem> & Pick<CardSubtaskItem, "card_id" | "id">): CardSubtaskItem {
  return {
    kind: "decision",
    status: "open",
    summary: `What ${fields.id} decides`,
    working_answer: `The answer of ${fields.id}`,
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
    ...fields,
  };
}

/** C-0019's decision with its why, a question on C-0013 (served, not shown here), C-0013's decision: by ID. */
const ITEMS: readonly CardSubtaskItem[] = [
  anItem({ card_id: "C-0019", id: "PR-0057", summary: "Which words does the glossary define?", working_answer: "Every word a skipper meets in the tables.", why: "The glossary serves the tables." }),
  anItem({ card_id: "C-0013", id: "PR-0058", summary: "One plate or two for the tide tables?", working_answer: "Two plates." }),
  anItem({ card_id: "C-0013", id: "PR-0059", kind: "question", summary: "Is the datum line printed?" }),
];

const PARENT: Card = aCard({ id: "C-0001", flow: "copy", title: "Write the tide table foreword", targets: ["MEC-TIDE-TABLES"], reviewer: "Ivo Brandt", subtask_items: [...ITEMS] });

const CHILDREN: CardList = aCardList(
  [
    aCardRow({ id: "C-0013", flow: "plates", title: "Tide table plate", parent: "C-0001", reviewer: "Mira Holt", mandate: "binding", targets: ["MEC-TIDE-TABLES"] }),
    aCardRow({ id: "C-0019", flow: "copy", column: "doing", title: "Tide table glossary", parent: "C-0001", reviewer: "Mira Holt", mandate: "open", targets: ["DOM-WATER"] }),
    aCardRow({ id: "C-0016", flow: "plates", title: "Tide table cover", parent: "C-0001" }),
  ],
  { flows: [COPY, PLATES] },
);

function client(parent: Card = PARENT, children: Promise<CardList> | null = null): StubClient {
  const stub = stubClient();
  stub.getCards.mockImplementation((_project, options) =>
    options?.parent === "C-0001" ? (children ?? Promise.resolve(structuredClone(CHILDREN))) : Promise.resolve(aCardList([aCardRow({ id: "C-0001", flow: "copy", title: parent.title })], { flows: [COPY, PLATES] })),
  );
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === parent.id ? structuredClone(parent) : { id, reason: `no card ${id}` }));
  stub.raiseOnCard.mockResolvedValue({ id: "PR-0060", created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] });
  return stub;
}

async function opened(stub: StubClient = client(), title = PARENT.title): Promise<HTMLElement> {
  renderApp(stub, "#/alpha/board/C-0001");
  const dialog = await screen.findByRole("dialog", { name: title });
  await within(dialog).findByRole("region", { name: "Details" });
  return dialog;
}

async function part(dialog: HTMLElement): Promise<HTMLElement> {
  const found = await within(dialog).findByRole("region", { name: "Decisions from subtasks" });
  await within(found).findAllByRole("article");
  return found;
}

/** The main column's sections, by name, in DOM order. */
function sections(dialog: HTMLElement): string[] {
  return within(dialog.querySelector<HTMLElement>(".issue-view-main") ?? dialog)
    .getAllByRole("region")
    .map((region) => region.getAttribute("aria-label") ?? "");
}

describe("Decisions from subtasks (V3 of task-mandate)", () => {
  it("stands right after Subtasks, its decisions in served order, each a block of the item's chunk; the question on a subtask is not listed (M: the part listing questions)", async () => {
    const dialog = await opened();
    const decisions = await part(dialog);
    const names = sections(dialog);
    expect(names.slice(names.indexOf("Subtasks"), names.indexOf("Subtasks") + 2)).toEqual(["Subtasks", "Decisions from subtasks"]);
    expect(within(decisions).getAllByRole("article").map((article) => article.getAttribute("data-item"))).toEqual(["PR-0057", "PR-0058"]);
    expect(within(decisions).queryByText("Is the datum line printed?")).toBeNull();
  });

  it("names each subtask by its ID, a link, its title and its flow's title from the children read; a binding one's decision a departure, an open one's the department's", async () => {
    const dialog = await opened();
    const decisions = await part(dialog);
    const heads = Array.from(decisions.querySelectorAll<HTMLElement>(".board-from-subtask > .board-handoff"));
    expect(heads.map((head) => [head.querySelector("a")?.textContent, head.querySelector("a")?.getAttribute("href"), head.querySelector(".board-handoff-title")?.textContent, head.querySelector(".board-handoff-title + bdi")?.textContent, head.querySelector(".lozenge-label")?.textContent])).toEqual([
      ["C-0019", "#/alpha/board/C-0019", "Tide table glossary", "Copy desk", "Department decides"],
      ["C-0013", "#/alpha/board/C-0013", "Tide table plate", "plates", "Departure from a binding task"],
    ]);
    expect(heads.map((head) => head.querySelector(".lozenge")?.getAttribute("data-appearance"))).toEqual(["neutral", "warning"]);
    // Never colour alone: each lozenge carries its icon.
    expect(heads.every((head) => head.querySelector(".lozenge svg") !== null)).toBe(true);
  });

  it("shows the item's block: its ID, kind, state, summary, working answer and why", async () => {
    const dialog = await opened();
    const decisions = await part(dialog);
    const glossary = within(decisions).getByRole("article", { name: "PR-0057" });
    expect(glossary.querySelector(".kind-tag")?.textContent).toBe("decision");
    expect(glossary.querySelector(".lozenge-label")?.textContent).toBe("Open");
    expect(within(glossary).getByText("Which words does the glossary define?")).toBeTruthy();
    expect(Array.from(glossary.querySelectorAll("dt"), (term) => term.textContent)).toEqual(["Working answer", "Why"]);
    expect(within(glossary).getByText("The glossary serves the tables.")).toBeTruthy();
  });

  it("a subtask with no mandate: no lozenge; a subtask row not read yet: its ID alone, no Ask about it…", async () => {
    let land: (list: CardList) => void = () => undefined;
    const pending = new Promise<CardList>((resolve) => {
      land = resolve;
    });
    const parent: Card = { ...PARENT, subtask_items: [anItem({ card_id: "C-0016", id: "PR-0061" })] };
    const dialog = await opened(client(parent, pending));
    const decisions = await part(dialog);
    const head = decisions.querySelector<HTMLElement>(".board-from-subtask > .board-handoff");
    expect([head?.textContent, within(decisions).queryByRole("button", { name: "Ask about it…" })]).toEqual(["C-0016", null]);
    land(structuredClone(CHILDREN));
    await waitFor(() => {
      expect(head?.querySelector(".board-handoff-title")?.textContent).toBe("Tide table cover");
    });
    expect(head?.querySelector(".lozenge")).toBeNull();
    expect(within(decisions).queryByRole("button", { name: "Ask about it…" })).toBeNull();
  });

  it("Ask about it… on C-0019's decision opens Ask a question on C-0019, its question prefilled about PR-0057, its targets C-0019's; posts to C-0019's items; focus back on the act", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const stub = client();
    const dialog = await opened(stub);
    const decisions = await part(dialog);
    const entry = within(decisions).getByRole("article", { name: "PR-0057" }).closest("li") as HTMLElement;
    const ask = within(entry).getByRole("button", { name: "Ask about it…" });
    expect(ask.getAttribute("aria-haspopup")).toBe("dialog");
    fireEvent.click(ask);
    const asking = await screen.findByRole("dialog", { name: "Ask a question on C-0019" });
    expect(within(asking).getByLabelText<HTMLTextAreaElement>("Question").value).toBe("About PR-0057: ");
    const about = within(asking).getByRole("group", { name: "About" });
    expect(within(about).getAllByRole("checkbox").map((box) => box.parentElement?.textContent.trim())).toEqual(["DOM-WATER"]);
    fireEvent.change(within(asking).getByLabelText("Question"), { target: { value: "About PR-0057: why not the harbour's words too?" } });
    fireEvent.change(within(asking).getByLabelText("Working answer"), { target: { value: "The tables' words alone." } });
    fireEvent.change(within(asking).getByLabelText("What another answer would cost"), { target: { value: "A longer glossary." } });
    fireEvent.click(within(asking).getByRole("button", { name: "Ask" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Ask a question on C-0019" })).toBeNull();
    });
    expect(stub.raiseOnCard.mock.calls).toEqual([
      ["alpha", "C-0019", { kind: "question", node_ids: ["DOM-WATER"], text: "About PR-0057: why not the harbour's words too?", working_answer: "The tables' words alone.", price_of_other: "A longer glossary.", as: "Ivo Brandt" }],
    ]);
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("Asked PR-0060");
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(ask);
    });
    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
  });

  it("while a card write is out, Ask about it… and the decision's Answer… open nothing; once it settles they open (M: either act not waiting on useCardWriting)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const stub = client();
    let answer: () => void = () => undefined;
    stub.moveCard.mockImplementation(
      (_project, id) =>
        new Promise<CardWrite>((resolve) => {
          answer = () => {
            resolve(aCardWrite(id, { flow: "copy", column: "doing" }));
          };
        }),
    );
    const dialog = await opened(stub);
    const decisions = await part(dialog);
    const entry = within(decisions).getByRole("article", { name: "PR-0057" }).closest("li") as HTMLElement;
    const ask = within(entry).getByRole("button", { name: "Ask about it…" });
    const reply = within(entry).getByRole("button", { name: "Answer…" });
    // A move from the task's own Move menu: a card write, with no dialog of its own over the work item.
    fireEvent.click(within(dialog).getByRole("button", { name: "Move" }));
    fireEvent.click(within(await screen.findByRole("menu", { name: "Move C-0001" })).getByRole("menuitem", { name: /^Doing/ }));
    await waitFor(() => {
      expect(stub.moveCard).toHaveBeenCalledTimes(1);
    });
    fireEvent.click(ask);
    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    fireEvent.click(reply);
    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    await act(async () => {
      answer();
      await Promise.resolve();
    });
    await waitFor(() => {
      fireEvent.click(ask);
      expect(screen.getAllByRole("dialog").length).toBe(2);
    });
    expect(await screen.findByRole("dialog", { name: "Ask a question on C-0019" })).toBeTruthy();
  });

  it("no part on a card whose subtask_items holds no decision: none at all, or a question alone", async () => {
    for (const items of [[], [ITEMS[2] as CardSubtaskItem]]) {
      const parent: Card = { ...PARENT, subtask_items: items };
      const dialog = await opened(client(parent));
      await within(dialog).findByRole("region", { name: "Subtasks" });
      expect(within(dialog).queryByRole("region", { name: "Decisions from subtasks" })).toBeNull();
      cleanup();
    }
  });

  it("reads the parent once and its children once, never a card per subtask (M: a getCard per child)", async () => {
    const stub = client();
    const dialog = await opened(stub);
    const decisions = await part(dialog);
    await within(decisions).findAllByRole("button", { name: "Ask about it…" });
    const cardReads = argsOf(stub.getCard).map((args) => args[1]);
    expect(cardReads.filter((id) => id === "C-0001")).toHaveLength(1);
    expect(cardReads.filter((id) => id === "C-0013" || id === "C-0019")).toEqual([]);
    expect(argsOf(stub.getCards).filter((args) => (args[1] as { parent?: string } | undefined)?.parent === "C-0001")).toHaveLength(1);
  });
});

// V3 of docs/features/task-mandate.md "Acceptance criteria": the part is the items' chunk's, reached
// from the work item by a dynamic import alone, so it stays out of first load.
const PANEL = import.meta.glob<string>("/src/board/CardPanel.tsx", { query: "?raw", import: "default", eager: true });

describe("the part's chunk (V3 of task-mandate)", () => {
  it("is CardItems.tsx's, reached from CardPanel.tsx by lazy(() => import(...)) alone, under Suspense", () => {
    const text = PANEL["/src/board/CardPanel.tsx"] ?? "";
    expect(text).toContain('const SubtaskDecisions = lazy(() => import("./CardItems").then((module) => ({ default: module.SubtaskDecisions })));');
    expect(text).toMatch(/<Suspense\s[^>]*>[\s\S]*?<SubtaskDecisions /);
    expect(Array.from(text.matchAll(/^import\s[^;]*;/gm), (match) => match[0]).filter((line) => line.includes("./CardItems"))).toEqual([]);
  });
});

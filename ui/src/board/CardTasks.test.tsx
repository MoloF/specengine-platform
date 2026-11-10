import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Card, CardTask } from "../api/types";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { KNOWN_TASK_STATUSES } from "../tasks/labels";
import { aCard, aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { ruT } from "../test/text";
import { CARD_ID, linkCommand, newTaskCommand, notesOn, ownNotes, SAFE_REF } from "./CardTasks";

// CT-5 of docs/features/card-tasks.md: a card's Tasks part, in the modal and on the page alike. A row
// per link in link order: the ID a link to the task on Tasks, its title, its state's label and icon,
// Spec changed when stale; a task in `review` alone waits on the owner, its approval to copy. A task
// not read here: its ID as text, "State not read here", its note verbatim under lang="en". No link:
// "No task yet" and the commands that make one and link it, or on a hub a sentence and no command.
// The part reads nothing of its own and writes nothing (R8); src/board/ names no task state.

const OWN_NOTE = "flow `copy`: the column `old` is no longer in it";
const ELSEWHERE = "T-0003: a task of another repository of the project `studio-lamp`; its state is read there (`spec task show T-0003`)";
const NO_SNAPSHOT = "T-0004: stale unknown: no snapshot (the owner's `spec task approve` freezes one)";
const HUB_NOTE = "a hub reads no task; its state is on the terminal that holds it";

const LINKS: CardTask[] = [
  { id: "T-0001", status: "review", title: "Light the north lamp", stale: false },
  { id: "T-0002", status: "in_progress", title: "Wire the switch", stale: true },
  { id: "T-0003", status: null, title: null, stale: null },
  { id: "T-0004", status: "done", title: null, stale: null },
];

const LINKED = aCard({ id: "C-0001", title: "Light the harbour", targets: ["QS-0001"], tasks: LINKS, notes: [OWN_NOTE, ELSEWHERE, NO_SNAPSHOT] });

function cardClient(card: Card, login: string | null = null): StubClient {
  const client = stubClient();
  client.getIdentity.mockResolvedValue({ login });
  client.getCards.mockResolvedValue(aCardList([aCardRow({ id: card.id, title: card.title })]));
  client.getCard.mockImplementation((_project, id) => Promise.resolve(id === card.id ? structuredClone(card) : { id, reason: `no card ${id}` }));
  return client;
}

/** The work item's Development, folded until clicked (DF-7 of design-foundation). */
async function expandDevelopment(scope: HTMLElement = document.body): Promise<HTMLElement> {
  const development = await within(scope).findByRole("button", { name: /^Agent tasks \u00b7 / });
  fireEvent.click(development);
  return within(scope).getByRole("region", { name: "Agent tasks" });
}

/** The card's page, its Tasks part once read, under Development expanded. */
async function page(card: Card = LINKED, login: string | null = null): Promise<{ client: StubClient; part: HTMLElement }> {
  const client = cardClient(card, login);
  renderApp(client, `#/alpha/board/${card.id}/page`);
  await screen.findByRole("heading", { level: 1, name: card.title });
  return { client, part: await expandDevelopment() };
}

function rowOf(part: HTMLElement, id: string, nth = 0): HTMLElement {
  const row = part.querySelectorAll<HTMLElement>(`li[data-task="${id}"]`)[nth];
  if (row === undefined) {
    throw new Error(`no row ${id}`);
  }
  return row;
}

/** A badge's label and whether an icon stands beside it: a state never by colour alone. */
function badges(row: HTMLElement): [string, boolean][] {
  return Array.from(row.querySelectorAll(".badge"), (badge) => [badge.querySelector(".badge-label")?.textContent ?? "", badge.querySelector("svg") !== null]);
}

const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");

function stubClipboard() {
  const spy = vi.fn<(text: string) => Promise<void>>(() => Promise.resolve());
  Object.defineProperty(navigator, "clipboard", { value: { writeText: spy }, configurable: true });
  return spy;
}

afterEach(() => {
  if (clipboard === undefined) {
    Reflect.deleteProperty(navigator, "clipboard");
  } else {
    Object.defineProperty(navigator, "clipboard", clipboard);
  }
});

/** Every call the stub client has seen, by method name. */
function called(client: StubClient): string[] {
  return Object.entries(client)
    .filter(([, value]) => vi.isMockFunction(value) && value.mock.calls.length > 0)
    .map(([name]) => name)
    .sort();
}

describe("the Tasks part stands in the modal and on the page (CT-5)", () => {
  // Replaced by DF-7 of design-foundation: the part is folded under the side column's Development.
  it("in the modal, under the side column's Development, a row per link in link order once it is expanded", async () => {
    renderApp(cardClient(LINKED), "#/alpha/board/C-0001");
    const dialog = await screen.findByRole("dialog", { name: "Light the harbour" });
    await within(dialog).findByRole("region", { name: "Subtasks" });
    expect(within(dialog).queryByRole("region", { name: "Agent tasks" })).toBeNull();
    const part = await expandDevelopment(dialog);
    expect(part.closest(".issue-view-side")).not.toBeNull();
    expect(within(part).getByRole("list", { name: "Agent tasks linked to C-0001" })).toBeTruthy();
    expect(Array.from(part.querySelectorAll("li[data-task]"), (row) => row.getAttribute("data-task"))).toEqual(["T-0001", "T-0002", "T-0003", "T-0004"]);
  });

  it("on the page alike", async () => {
    const { part } = await page();
    expect(Array.from(part.querySelectorAll("li[data-task]"), (row) => row.getAttribute("data-task"))).toEqual(["T-0001", "T-0002", "T-0003", "T-0004"]);
  });
});

describe("a row per link (CT-5)", () => {
  it("a served row: its ID links to the task on Tasks, its title, its state's label and icon, and Spec changed when stale", async () => {
    const { part } = await page();
    const first = rowOf(part, "T-0001");
    expect(within(first).getByRole("link", { name: "T-0001" }).getAttribute("href")).toBe("#/alpha/tasks/T-0001");
    expect(within(first).getByText("Light the north lamp")).toBeTruthy();
    expect(badges(first)).toEqual([["Plan review", true]]);
    const second = rowOf(part, "T-0002");
    expect(within(second).getByRole("link", { name: "T-0002" }).getAttribute("href")).toBe("#/alpha/tasks/T-0002");
    expect(badges(second)).toEqual([
      ["In progress", true],
      ["Spec changed", true],
    ]);
    // `stale` null is no mark: its note says why.
    expect(badges(rowOf(part, "T-0004"))).toEqual([["Done", true]]);
  });

  it("a task with no title shows the word for none", async () => {
    const { part } = await page();
    expect(within(rowOf(part, "T-0004")).getByText(en.tasks.untitled)).toBeTruthy();
  });

  it("a review row alone shows Awaiting you and `spec task approve` with Copy, which copies exactly the command", async () => {
    const writeText = stubClipboard();
    const { client, part } = await page();
    const awaiting = within(part).getAllByText("Awaiting you");
    expect(awaiting).toHaveLength(1);
    expect(rowOf(part, "T-0001").contains(awaiting[0] ?? null)).toBe(true);
    expect(Array.from(part.querySelectorAll("code"), (code) => code.textContent)).toEqual(["spec task approve T-0001"]);
    const before = called(client);
    fireEvent.click(within(rowOf(part, "T-0001")).getByRole("button", { name: "Copy the command: spec task approve T-0001" }));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith("spec task approve T-0001");
    });
    // Copying is the browser's alone: nothing goes to the daemon.
    expect(called(client)).toEqual(before);
  });

  it("a row not read here: its ID as text, State not read here with an icon, its note verbatim under lang=en, no title and no state word", async () => {
    const { part } = await page();
    const row = rowOf(part, "T-0003");
    expect(within(row).queryByRole("link")).toBeNull();
    expect(within(row).getByText("T-0003")).toBeTruthy();
    expect(badges(row)).toEqual([["State not read here", true]]);
    const note = within(row).getByText(ELSEWHERE);
    expect(note.getAttribute("lang")).toBe("en");
    expect(note.textContent).toBe(ELSEWHERE);
    expect(within(row).queryByText(en.tasks.untitled)).toBeNull();
    expect(within(row).queryByRole("button")).toBeNull();
  });

  it("every row shows its own notes, verbatim under lang=en; the card's own notes stay above it, each note shown once", async () => {
    const { part } = await page();
    const fourth = within(rowOf(part, "T-0004")).getByRole("list", { name: "Notes from the daemon on T-0004" });
    expect(Array.from(fourth.querySelectorAll("li"), (item) => [item.getAttribute("lang"), item.textContent])).toEqual([["en", NO_SNAPSHOT]]);
    expect(within(rowOf(part, "T-0001")).queryByRole("list", { name: "Notes from the daemon on T-0001" })).toBeNull();
    const own = screen.getByRole("list", { name: "Notes from the daemon on the task" });
    expect(Array.from(own.querySelectorAll("li"), (item) => item.textContent)).toEqual([OWN_NOTE]);
    for (const note of [OWN_NOTE, ELSEWHERE, NO_SNAPSHOT]) {
      expect([note, screen.getAllByText(note).length]).toEqual([note, 1]);
    }
  });

  it("an unknown state shows raw in the neutral badge under lang=en; an ID off the task form is text and offers no command, even in review", async () => {
    const odd = aCard({
      id: "C-0002",
      title: "Odd rows",
      tasks: [
        { id: "T-0005", status: "triage", title: "Sort the buoys", stale: false },
        { id: "T-12a", status: "review", title: "Not an ID", stale: false },
      ],
    });
    const { part } = await page(odd);
    const unknown = rowOf(part, "T-0005").querySelector(".badge");
    expect([unknown?.getAttribute("data-tone"), unknown?.querySelector(".badge-label")?.getAttribute("lang"), unknown?.textContent]).toEqual(["task-unknown", "en", "triage"]);
    const bad = rowOf(part, "T-12a");
    expect(within(bad).queryByRole("link")).toBeNull();
    expect(within(bad).queryByRole("button")).toBeNull();
    expect(bad.querySelector("code")).toBeNull();
  });

  it("a done or cancelled task shows its state; one ID listed twice in a hand-edited row shows twice, each its notes", async () => {
    const twice = aCard({
      id: "C-0003",
      title: "Twice",
      tasks: [
        { id: "T-0007", status: "cancelled", title: "Old quays", stale: false },
        { id: "T-0007", status: "cancelled", title: "Old quays", stale: false },
        { id: "T-0008", status: "done", title: "Crew of four", stale: false },
      ],
    });
    const { part } = await page(twice);
    expect(badges(rowOf(part, "T-0007", 0))).toEqual([["Cancelled", true]]);
    expect(badges(rowOf(part, "T-0007", 1))).toEqual([["Cancelled", true]]);
    expect(badges(rowOf(part, "T-0008"))).toEqual([["Done", true]]);
  });

  it("holds no control but Copy, and reads no task: its state comes with the card", async () => {
    const { client, part } = await page();
    expect(within(part).getAllByRole("button").map((button) => button.textContent)).toEqual(["Copy the command: spec task approve T-0001"]);
    expect(client.getTask).not.toHaveBeenCalled();
    expect(client.getTasks).not.toHaveBeenCalled();
  });
});

describe("no link (CT-5)", () => {
  it("locally: No task yet, then the commands that make a task on the card's target and link it, each with Copy", async () => {
    const { part } = await page(aCard({ id: "C-0001", title: "Bare", targets: ["QS-0001"] }));
    expect(within(part).getByText("No agent task yet.")).toBeTruthy();
    expect(within(part).queryByRole("list", { name: /Tasks linked/ })).toBeNull();
    expect(Array.from(part.querySelectorAll("code"), (code) => code.textContent)).toEqual(['spec task new --nodes QS-0001 --title "..."', "spec card edit C-0001 --task T-NNNN"]);
    expect(within(part).getAllByRole("button").map((button) => button.textContent)).toEqual([
      'Copy the command: spec task new --nodes QS-0001 --title "..."',
      "Copy the command: spec card edit C-0001 --task T-NNNN",
    ]);
  });

  it("a manual card, or one with a target unsafe to paste, names REF; the title is never filled in", async () => {
    const { part } = await page(aCard({ id: "C-0001", title: "By hand; rm -rf ~" }));
    expect(part.querySelector("code")?.textContent).toBe('spec task new --nodes REF --title "..."');
    expect(newTaskCommand(["QS-0001", "docs/spec/a b.md"])).toBe('spec task new --nodes REF --title "..."');
    expect(newTaskCommand(["QS-0001", "lamp/QS-0002", "docs/spec/lamp.md"])).toBe('spec task new --nodes QS-0001 lamp/QS-0002 docs/spec/lamp.md --title "..."');
  });

  it("with a hub login: No task yet and the sentence, no command and no Copy", async () => {
    const { part } = await page(aCard({ id: "C-0001", title: "Bare", targets: ["QS-0001"] }), "mira");
    await within(part).findByText(en.board.tasks.onHub);
    expect(within(part).getByText("No agent task yet.")).toBeTruthy();
    expect(part.querySelector("code")).toBeNull();
    expect(within(part).queryByRole("button")).toBeNull();
    expect(within(part).queryByText(en.board.tasks.howTo)).toBeNull();
  });
});

describe("on a hub (CT-5, CT-7)", () => {
  it("shows the IDs alone, State not read here, the hub's note, and that the state lives on a terminal; no task read, no command", async () => {
    const onHub = aCard({
      id: "C-0001",
      title: "Light the harbour",
      tasks: [
        { id: "T-0001", status: null, title: null, stale: null },
        { id: "T-0042", status: null, title: null, stale: null },
      ],
      notes: [`T-0001: ${HUB_NOTE}`, `T-0042: ${HUB_NOTE}`],
    });
    const { client, part } = await page(onHub, "mira");
    await within(part).findByText(en.board.tasks.onHub);
    for (const id of ["T-0001", "T-0042"]) {
      const row = rowOf(part, id);
      expect(within(row).queryByRole("link")).toBeNull();
      expect(badges(row)).toEqual([["State not read here", true]]);
      expect(within(row).getByText(`${id}: ${HUB_NOTE}`).getAttribute("lang")).toBe("en");
    }
    expect(part.querySelector("code")).toBeNull();
    expect(client.getTask).not.toHaveBeenCalled();
    expect(client.getTasks).not.toHaveBeenCalled();
  });
});

describe("in Russian (CT-5)", () => {
  it("the part's chrome is Russian, the daemon's notes and the commands stay as served", async () => {
    const { part } = await page();
    fireEvent.change(screen.getByRole<HTMLSelectElement>("combobox", { name: en.language.label }), { target: { value: "ru" } });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
    });
    const shown = screen.getByRole("region", { name: ru.board.tasks.title });
    expect(shown).toBe(part);
    expect(within(shown).getByRole("list", { name: ruT("board.tasks.list", { id: "C-0001" }) })).toBeTruthy();
    expect(within(shown).getByText(ru.board.tasks.awaiting)).toBeTruthy();
    expect(within(shown).getByText(ru.board.tasks.notRead)).toBeTruthy();
    expect(within(shown).getByText(ru.tasks.status.review)).toBeTruthy();
    expect(within(shown).getByText(ru.tasks.specChanged)).toBeTruthy();
    expect(within(shown).getByText(ELSEWHERE).getAttribute("lang")).toBe("en");
    expect(within(shown).getByText("spec task approve T-0001")).toBeTruthy();
  });
});

describe("the part's helpers (CT-5, R8)", () => {
  it("offers the link command only for a card ID, and names a target only when it is safe to paste", () => {
    expect(linkCommand("C-0001")).toBe("spec card edit C-0001 --task T-NNNN");
    expect(linkCommand("C-12345")).toBe("spec card edit C-12345 --task T-NNNN");
    for (const id of ["C-1", "c-0001", "C-0001; rm", "C-0001\n", "$(C-0001)", ""]) {
      expect([id, linkCommand(id)]).toEqual([id, null]);
    }
    expect(CARD_ID.test("C-0001")).toBe(true);
    for (const ref of ["QS-0001", "lamp/QS-0001", "docs/spec/lamp.md", "a_b.c-d"]) {
      expect([ref, SAFE_REF.test(ref)]).toEqual([ref, true]);
    }
    for (const ref of ["-rf", "a b", "a;b", "$(x)", "`x`", "a'b", "\u0422-0001", "", "a\nb", ".hidden", "/abs"]) {
      expect([ref, SAFE_REF.test(ref)]).toEqual([ref, false]);
    }
    expect(newTaskCommand([])).toBe('spec task new --nodes REF --title "..."');
  });

  it("gives each linked task the notes that start with its ID and a colon, and the card the rest", () => {
    const notes = ["T-0001: one", "T-00012: not T-0001's", "T-0001 without a colon", "own"];
    expect(notesOn(notes, "T-0001")).toEqual(["T-0001: one"]);
    expect(ownNotes({ notes, tasks: [{ id: "T-0001", status: null, title: null, stale: null }] })).toEqual(["T-00012: not T-0001's", "T-0001 without a colon", "own"]);
    expect(ownNotes({ notes, tasks: [] })).toEqual(notes);
  });
});

describe("the Board names no task state, and the part's words are in both catalogs (CT-5)", () => {
  const sources = import.meta.glob<string>(["./*.ts", "./*.tsx", "!./*.test.ts", "!./*.test.tsx"], { query: "?raw", import: "default", eager: true });

  it("no source of src/board/ spells one of the ten states: the one copy is src/tasks/", () => {
    expect(Object.keys(sources)).toEqual(expect.arrayContaining(["./CardTasks.tsx", "./CardPanel.tsx", "./labels.ts"]));
    const spelled = new RegExp(`["'\`](${KNOWN_TASK_STATUSES.join("|")})["'\`]`);
    expect(Object.entries(sources).filter(([, text]) => spelled.test(text)).map(([path]) => path)).toEqual([]);
    // The detector itself.
    expect(spelled.test('if (task.status === "review") {')).toBe(true);
    expect(spelled.test("awaitsOwner(task.status)")).toBe(false);
  });

  it("every board.tasks key has its Russian, none left English", () => {
    expect(Object.keys(ru.board.tasks)).toEqual(Object.keys(en.board.tasks));
    for (const key of Object.keys(en.board.tasks) as (keyof typeof en.board.tasks)[]) {
      expect([key, ru.board.tasks[key] !== "" && ru.board.tasks[key] !== en.board.tasks[key]]).toEqual([key, true]);
    }
  });
});

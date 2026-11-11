import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { Icon, type IconName } from "../ui/Icon";
import { MockClient } from "./MockClient";

// docs/features/handoff-progress.md "Acceptance criteria", HP-1 and HP-3 to HP-6 over the mock, as
// `pnpm dev` shows them: harbor-sim's C-0001, a parent with a child accepted, one cancelled, one in
// progress and one to do; C-0003's returned child; C-0004 in proof, its one child accepted; C-0009's
// child to do; C-0002 with none; C-0008 with an open question and no child. The stub cases run in
// src/board/CardProgress.test.tsx; the mock is imported here alone (policy AC-05).

function renderMock(hash: string) {
  window.history.replaceState(null, "", `/?scenario=normal${hash}`);
  const client = new MockClient("normal", { delayMs: 0 });
  const spies = { getCards: vi.spyOn(client, "getCards"), getCard: vi.spyOn(client, "getCard") };
  render(<App client={client} scenario={null} />);
  return spies;
}

/** The card's modal, by its title, and its Subtasks part. */
async function subtasksOf(title: string): Promise<{ dialog: HTMLElement; part: HTMLElement }> {
  const dialog = await screen.findByRole("dialog", { name: title });
  return { dialog, part: await within(dialog).findByRole("region", { name: "Subtasks" }) };
}

/** The words line once the children read lands. */
async function wordsLine(part: HTMLElement): Promise<HTMLElement> {
  return waitFor(() => {
    const line = part.querySelector<HTMLElement>(".progress-words");
    if (line === null) {
      throw new Error("no words line yet");
    }
    return line;
  });
}

/** The words line as a screen reader gets it: each count's words, the decorative separators left out. */
function spokenWords(line: HTMLElement): string[] {
  const shown = line.cloneNode(true) as HTMLElement;
  for (const hidden of Array.from(shown.querySelectorAll("[aria-hidden='true']"))) {
    hidden.remove();
  }
  return Array.from(shown.querySelectorAll(".progress-word"), (word) => word.textContent);
}

/** The separators between the words: decorative middle dots, hidden from assistive technology (L3 of the looks). */
function separatorsOf(line: HTMLElement): [string | null, string | null][] {
  return Array.from(line.querySelectorAll(".progress-separator"), (separator) => [separator.textContent, separator.getAttribute("aria-hidden")]);
}

/** The path an icon draws, for telling one icon from another. */
function iconPath(name: IconName): string {
  const { container, unmount } = render(<Icon name={name} />);
  const d = container.querySelector("path")?.getAttribute("d") ?? "";
  unmount();
  return d;
}

/** The children reads a spy saw, by their options. */
function childrenReads(spy: ReturnType<typeof renderMock>["getCards"]): unknown[] {
  return spy.mock.calls.filter((call) => call[1]?.parent !== undefined).map((call) => [call[0], call[1]]);
}

describe("the children read on harbor-sim (HP-1)", () => {
  it("opening C-0001's modal, then its page, each reads its children once with parent and with_cancelled, never marks; the Board's read stays marks alone", async () => {
    const spies = renderMock("#/harbor-sim/board/C-0001");
    const { dialog, part } = await subtasksOf("Write the tide table foreword");
    await wordsLine(part);
    expect(childrenReads(spies.getCards)).toEqual([["harbor-sim", { parent: "C-0001", with_cancelled: true }]]);
    expect(spies.getCards.mock.calls.filter((call) => call[1]?.parent === undefined).map((call) => call[1])).toEqual([{ marks: true }]);
    spies.getCards.mockClear();
    fireEvent.click(within(dialog).getByRole("link", { name: "Open as a page" }));
    await screen.findByRole("heading", { level: 1, name: "Write the tide table foreword" });
    await wordsLine(await screen.findByRole("region", { name: "Subtasks" }));
    await waitFor(() => {
      expect(childrenReads(spies.getCards)).toEqual([["harbor-sim", { parent: "C-0001", with_cancelled: true }]]);
    });
  });

  it("asks the Board's read with Show cancelled as with_cancelled and marks, the children read as before", async () => {
    const spies = renderMock("#/harbor-sim/board");
    await screen.findByRole("heading", { level: 1, name: "Board" });
    fireEvent.click(await screen.findByRole("checkbox", { name: "Show cancelled" }));
    await waitFor(() => {
      expect(spies.getCards.mock.calls.map((call) => call[1])).toContainEqual({ with_cancelled: true, marks: true });
    });
    expect(childrenReads(spies.getCards)).toEqual([]);
  });
});

describe("the bar on harbor-sim (HP-3)", () => {
  it("C-0001 reads 1 accepted \u00b7 1 cancelled \u00b7 1 in progress \u00b7 1 to do, each word with its icon, the bar hidden and weighed 1 each", async () => {
    renderMock("#/harbor-sim/board/C-0001");
    const { part } = await subtasksOf("Write the tide table foreword");
    const line = await wordsLine(part);
    // M: cancelled placed after the categories.
    expect(spokenWords(line)).toEqual(["1 accepted", "1 cancelled", "1 in progress", "1 to do"]);
    // The middle dot between them, as the breadcrumbs' separator: decorative, out of the accessible text.
    expect(separatorsOf(line)).toEqual([
      ["\u00b7", "true"],
      ["\u00b7", "true"],
      ["\u00b7", "true"],
    ]);
    expect(spokenWords(line).join(" ")).not.toContain("\u00b7");
    expect(Array.from(line.querySelectorAll(".progress-word path"), (path) => path.getAttribute("d"))).toEqual(["taskAccepted", "taskCancelled", "taskInProgress", "open"].map((name) => iconPath(name as IconName)));
    const bar = part.querySelector<HTMLElement>(".progress-bar");
    expect(bar?.getAttribute("aria-hidden")).toBe("true");
    expect(Array.from(bar?.querySelectorAll<HTMLElement>(".progress-segment") ?? [], (segment) => segment.style.flexGrow)).toEqual(["1", "1", "1", "1"]);
  });

  it.each([
    ["C-0003", "Night passage rules, plain words", "1 returned", "cardReturned"],
    ["C-0009", "Harbour overview, colour plate", "1 to do", "open"],
  ] as const)("%s reads its one child's word with its icon", async (id, title, words, icon) => {
    renderMock(`#/harbor-sim/board/${id}`);
    const { part } = await subtasksOf(title);
    const line = await wordsLine(part);
    expect(line.textContent).toBe(words);
    expect(line.querySelector("path")?.getAttribute("d")).toBe(iconPath(icon));
  });

  it("C-0002 reads None yet. and draws no bar", async () => {
    renderMock("#/harbor-sim/board/C-0002");
    const { part } = await subtasksOf("Caption the berth plans");
    expect(await within(part).findByText("None yet.")).toBeTruthy();
    expect(part.querySelector(".progress-bar, .progress-words")).toBeNull();
  });
});

describe("Ready to accept on harbor-sim (HP-4)", () => {
  it("C-0004, in proof, its one child accepted, says Ready to accept and why; Accept stands present, enabled, first", async () => {
    renderMock("#/harbor-sim/board/C-0004");
    const { dialog, part } = await subtasksOf("Proof the lock opening hours");
    const hint = await within(part).findByText("Ready to accept");
    expect(hint.closest("p")?.textContent).toBe("Ready to acceptEvery subtask is accepted or cancelled.");
    const acts = within(within(dialog).getByRole("group", { name: "Actions" })).getAllByRole("button");
    expect(acts[0]?.textContent).toBe("Accept");
    expect(acts[0]?.getAttribute("aria-disabled")).toBe("false");
    expect(within(part).queryByRole("list", { name: "Relies on" })).toBeNull();
  });

  it("C-0001 says none", async () => {
    renderMock("#/harbor-sim/board/C-0001");
    const { part } = await subtasksOf("Write the tide table foreword");
    await wordsLine(part);
    expect(within(part).queryByText("Ready to accept")).toBeNull();
  });
});

describe("Relies on on harbor-sim (HP-5)", () => {
  it("C-0001 relies on C-0019 then C-0013, each its ID a link, its title and its flow's title; a plain click replaces the modal's card", async () => {
    renderMock("#/harbor-sim/board/C-0001");
    const { part } = await subtasksOf("Write the tide table foreword");
    const list = await within(part).findByRole("list", { name: "Relies on" });
    expect(within(list).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["C-0019Tide table glossaryCopy desk", "C-0013Tide table plateplates"]);
    expect(list.textContent).not.toMatch(/C-0016|C-0017/);
    fireEvent.click(within(list).getByRole("link", { name: "C-0019" }));
    await screen.findByRole("dialog", { name: "Tide table glossary" });
    expect(window.location.hash).toBe("#/harbor-sim/board/C-0019");
  });

  it("C-0008, with the open question PR-0055 and no child, relies on nothing", async () => {
    renderMock("#/harbor-sim/board/C-0008");
    const { part } = await subtasksOf("Crane reach diagram");
    expect(await within(part).findByText("None yet.")).toBeTruthy();
    expect(within(part).queryByRole("list", { name: "Relies on" })).toBeNull();
    expect(part.textContent).not.toContain("PR-0055");
  });
});

describe("cancelled subtasks on harbor-sim (HP-6)", () => {
  it("C-0001 lists C-0017 with its title, flow, lozenge, Cancelled, its reason and who cancelled it when, one card read for it alone", async () => {
    const spies = renderMock("#/harbor-sim/board/C-0001");
    const { part } = await subtasksOf("Write the tide table foreword");
    const list = await within(part).findByRole("list", { name: "Cancelled subtasks" });
    await within(list).findByText("One plate holds both tables.");
    const [item] = within(list).getAllByRole("listitem");
    expect(item?.textContent).toMatch(/^C-0017Second tide plateplatesqueuedCancelledOne plate holds both tables\.By Mira Holt, \d{4}-\d{2}-\d{2} \d{2}:\d{2} UTC$/);
    expect(item?.querySelector(".lozenge")?.getAttribute("data-appearance")).toBe("neutral");
    const read = spies.getCard.mock.calls.map((call) => call[1]);
    expect(read.filter((id) => id === "C-0017")).toHaveLength(1);
    expect(read.filter((id) => ["C-0013", "C-0016", "C-0019"].includes(id))).toEqual([]);
  });
});

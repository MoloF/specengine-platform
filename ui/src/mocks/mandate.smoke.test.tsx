import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";

// docs/features/task-mandate.md "Acceptance criteria", V1 to V3, over the mock as `pnpm dev` shows
// them (harbor-sim): C-0001's plate C-0013 is binding, its glossary C-0019 is the copy desk's to
// decide, and each recorded one decision. C-0001 reads both in its one card read, a departure marked;
// C-0013 leads with Return and Ask a question, C-0019 with Record a decision; Mira Holt, their
// reviewer, edits the mandate. The stub cases run in src/board/; the mock is imported here alone
// (policy AC-05).

// The items' chunk loaded once for the file, so the lazy part never waits on its first transform.
beforeAll(async () => {
  await import("../board/CardItems");
});

function renderMock(hash: string) {
  window.localStorage.setItem("specengine.module.harbor-sim", "");
  window.history.replaceState(null, "", `/?scenario=normal${hash}`);
  const client = new MockClient("normal", { delayMs: 0 });
  const spies = { getCards: vi.spyOn(client, "getCards"), getCard: vi.spyOn(client, "getCard"), raiseOnCard: vi.spyOn(client, "raiseOnCard"), editCard: vi.spyOn(client, "editCard") };
  render(<App client={client} scenario={null} />);
  return { client, ...spies };
}

/** A card's modal, by its title, its side column read. */
async function modal(title: string): Promise<HTMLElement> {
  const dialog = await screen.findByRole("dialog", { name: title });
  await within(dialog).findByRole("region", { name: "Details" });
  return dialog;
}

function acts(dialog: HTMLElement): string[] {
  return within(within(dialog).getByRole("group", { name: "Actions" }))
    .getAllByRole("button")
    .map((button) => button.textContent);
}

describe("a mandate over the mock (task-mandate)", () => {
  it("C-0001's Decisions from subtasks: C-0019's decision the department's, C-0013's a departure, from one card read and one children read (V3)", async () => {
    const { getCard, getCards } = renderMock("#/harbor-sim/board/C-0001");
    const dialog = await modal("Write the tide table foreword");
    const part = await within(dialog).findByRole("region", { name: "Decisions from subtasks" });
    await within(part).findAllByRole("button", { name: "Ask about it\u2026" });
    const heads = Array.from(part.querySelectorAll<HTMLElement>(".board-from-subtask > .board-handoff"), (head) => [
      head.querySelector("a")?.textContent,
      head.querySelector(".board-handoff-title")?.textContent,
      head.querySelector(".lozenge-label")?.textContent,
    ]);
    expect(heads).toEqual([
      ["C-0019", "Tide table glossary", "Department decides"],
      ["C-0013", "Tide table plate", "Departure from a binding task"],
    ]);
    expect(within(part).getAllByRole("article").map((article) => article.getAttribute("data-item"))).toEqual(["PR-0057", "PR-0058"]);
    const reads = getCard.mock.calls.map((call) => call[1]);
    expect(reads.filter((id) => id === "C-0001")).toHaveLength(1);
    expect(reads.filter((id) => id === "C-0013" || id === "C-0019")).toEqual([]);
    expect(getCards.mock.calls.filter((call) => call[1]?.parent === "C-0001")).toHaveLength(1);
  });

  it("Ask about it\u2026 on C-0019's decision raises a question on C-0019, named after PR-0057; C-0019's Activity shows it", async () => {
    window.localStorage.setItem("specengine.board.as", "Ivo Brandt");
    const { raiseOnCard } = renderMock("#/harbor-sim/board/C-0001");
    const dialog = await modal("Write the tide table foreword");
    const part = await within(dialog).findByRole("region", { name: "Decisions from subtasks" });
    const entry = (await within(part).findByRole("article", { name: "PR-0057" })).closest("li") as HTMLElement;
    fireEvent.click(await within(entry).findByRole("button", { name: "Ask about it\u2026" }));
    const asking = await screen.findByRole("dialog", { name: "Ask a question on C-0019" });
    const question = within(asking).getByLabelText<HTMLTextAreaElement>("Question");
    expect(question.value).toBe("About PR-0057: ");
    fireEvent.change(question, { target: { value: "About PR-0057: the harbour's own words too?" } });
    fireEvent.change(within(asking).getByLabelText("Working answer"), { target: { value: "The tables' words alone." } });
    fireEvent.change(within(asking).getByLabelText("What another answer would cost"), { target: { value: "A longer glossary." } });
    fireEvent.click(within(asking).getByRole("button", { name: "Ask" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Ask a question on C-0019" })).toBeNull();
    });
    expect(raiseOnCard.mock.calls.map((call) => [call[1], call[2].kind, call[2].node_ids])).toEqual([["C-0019", "question", ["DOM-WATER"]]]);
    fireEvent.click(within(part).getByRole("link", { name: "C-0019" }));
    const child = await modal("Tide table glossary");
    const activity = within(child).getByRole("region", { name: "Activity" });
    expect(await within(activity).findByRole("article", { name: "PR-0059" })).toBeTruthy();
  });

  it("C-0013, binding, leads with Return and Ask a question\u2026; the note before Goal; Details names it; Return opens its question part (V2)", async () => {
    renderMock("#/harbor-sim/board/C-0013");
    const dialog = await modal("Tide table plate");
    expect(acts(dialog)).toEqual(["Return", "Ask a question\u2026", "Hand off", "Split to departments\u2026", "Record a decision\u2026", "Edit", "Assign", "Cancel task"]);
    expect(within(dialog).getByText("Binding: the brief stands. To depart from it, return the task with a question.")).toBeTruthy();
    const term = Array.from(dialog.querySelectorAll("dt")).find((one) => one.textContent === "Mandate");
    expect(term?.nextElementSibling?.textContent).toBe("Binding");
    fireEvent.click(within(dialog).getByRole("button", { name: "Return" }));
    const back = await screen.findByRole("dialog", { name: "Return C-0013" });
    expect(within(back).getByRole("button", { name: "Ask a question with it" }).getAttribute("aria-expanded")).toBe("true");
  });

  it("C-0019, its department deciding, leads with Record a decision\u2026, whose Why it asks (V2)", async () => {
    renderMock("#/harbor-sim/board/C-0019");
    const dialog = await modal("Tide table glossary");
    expect(acts(dialog)).toEqual(["Record a decision\u2026", "Hand off", "Split to departments\u2026", "Ask a question\u2026", "Edit", "Assign", "Cancel task"]);
    expect(within(dialog).getByText("Department decides: record each decision and why.")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Record a decision\u2026" }));
    const record = await screen.findByRole("dialog", { name: "Record a decision on C-0019" });
    expect(within(record).getByLabelText("Why (required: the department decides)").getAttribute("aria-required")).toBe("true");
  });

  it("Mira Holt, C-0013's reviewer, finds Edit's select at Binding and changes it: the acts follow, the Activity names it (V1)", async () => {
    window.localStorage.setItem("specengine.board.as", "Mira Holt");
    const { editCard } = renderMock("#/harbor-sim/board/C-0013");
    const dialog = await modal("Tide table plate");
    fireEvent.click(within(dialog).getByRole("button", { name: "Edit" }));
    const edit = await screen.findByRole("dialog", { name: "Edit C-0013" });
    const mandate = within(edit).getByRole<HTMLSelectElement>("combobox", { name: "Mandate" });
    expect(mandate.value).toBe("binding");
    fireEvent.change(mandate, { target: { value: "open" } });
    fireEvent.click(within(edit).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Edit C-0013" })).toBeNull();
    });
    expect(editCard.mock.calls[0]?.[2]).toEqual({ mandate: "open", revision: "1", as: "Mira Holt" });
    await waitFor(() => {
      expect(acts(dialog)[0]).toBe("Record a decision\u2026");
    });
    const activity = within(dialog).getByRole("region", { name: "Activity" });
    await waitFor(() => {
      expect(activity.textContent).toContain("mandate Department decides");
    });
  });
});

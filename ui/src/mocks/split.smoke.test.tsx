import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CardSplitBody } from "../api/types";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// Split to departments over the mock, harbor-sim, as `pnpm dev` shows it (docs/features/department-split.md
// "Acceptance criteria", U1, U2): the act at each opener, the dialog with the mock daemon's plan (a
// mechanic's work suggested to copy and the plates, captions taking the task's targets), the write,
// the conflict scenario's refusal and a hub's login. The mock is imported here alone (policy AC-05).

/** The act's name, its ellipsis escaped: the mocks stay ASCII. */
const ACT = "Split to departments\u2026";

function renderMock(hash: string, scenario: Scenario = "normal") {
  window.localStorage.setItem("specengine.module.harbor-sim", "");
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  const splitCard = vi.spyOn(client, "splitCard");
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, splitCard };
}

/** The bodies sent, in order. */
function bodies(spy: { mock: { calls: readonly (readonly unknown[])[] } }): CardSplitBody[] {
  return spy.mock.calls.map((call) => call[1] as CardSplitBody);
}

/** Each row's label and whether it is checked, in row order. */
function rows(dialog: HTMLElement): [string, boolean][] {
  return within(dialog)
    .getAllByRole<HTMLInputElement>("checkbox")
    .map((box) => [box.closest("label")?.textContent ?? "", box.checked]);
}

/** Every checked row's mandate chosen (docs/features/task-mandate.md "Acceptance criteria", V1). */
function chooseMandates(dialog: HTMLElement, value = "binding") {
  for (const select of within(dialog).queryAllByLabelText(/^Mandate for /)) {
    fireEvent.change(select, { target: { value } });
  }
}

/** Mooring's page, its acts landed, Split to departments pressed: the dialog with its plan. */
async function fromMooring(scenario: Scenario = "normal") {
  window.localStorage.setItem("specengine.board.as", "Ivo Brandt");
  const spies = renderMock("#/harbor-sim/tree/MEC-MOORING/page", scenario);
  fireEvent.click(await screen.findByRole("button", { name: ACT }));
  const dialog = await screen.findByRole("dialog", { name: "Split to departments" });
  await within(dialog).findAllByRole("checkbox");
  return { ...spies, dialog };
}

describe("Split to departments over the mock (U1, U2 of department-split)", () => {
  it("from a document's page: one plan read; Galley's two rows and Foundry's one, copy and the plates suggested by kind, captions by the task's targets; Split makes the parent and two subtasks, the parent's work item lists them", async () => {
    const { dialog, splitCard } = await fromMooring();
    expect(bodies(splitCard)).toEqual([{ target: ["MEC-MOORING"], title: "Mooring", goal: null, discipline: "copy", parent: null, child: null, mandate: null, reviewer: null, dry_run: true, as: "Ivo Brandt" }]);
    expect(rows(dialog)).toEqual([
      ["Copy desk ", true],
      ["Copy desk captions", false],
      ["plates ", true],
    ]);
    expect(Array.from(dialog.querySelectorAll(".split-department > legend"), (legend) => legend.textContent)).toEqual(["Galley", "Foundry"]);
    expect(within(dialog).getByText("Suggested by the project's split templates")).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "By kind: 1 document" })).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "The task's targets" })).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "By kind: 3 documents" })).toBeTruthy();
    // The mooring's accepted fees table, a task on this document already: listed, never judged.
    expect(within(dialog).getByRole("link", { name: "C-0005 Mooring fees table" }).getAttribute("href")).toBe("#/harbor-sim/board/C-0005");
    // Each checked subtask's mandate, required (docs/features/task-mandate.md "Acceptance criteria", V1).
    fireEvent.change(within(dialog).getByLabelText("Mandate for Copy desk"), { target: { value: "binding" } });
    fireEvent.change(within(dialog).getByLabelText("Mandate for plates"), { target: { value: "open" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Split" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Split to departments" })).toBeNull();
    });
    expect(bodies(splitCard)[1]).toMatchObject({ child: ["copy", "plates"], mandate: ["copy=binding", "plates=open"], reviewer: "Ivo Brandt", dry_run: false });
    const polite = await waitFor(() => {
      const said = document.querySelector('[aria-live="polite"]')?.textContent ?? "";
      expect(said).toMatch(/^Split into C-\d{4} and 2 subtasks$/);
      return said;
    });
    const parent = /C-\d{4}/.exec(polite)?.[0] ?? "";
    const link = await screen.findByRole("link", { name: parent });
    expect(link.getAttribute("href")).toBe(`#/harbor-sim/board/${parent}`);
    fireEvent.click(link);
    window.location.hash = `#/harbor-sim/board/${parent}`;
    const modal = await screen.findByRole("dialog", { name: "Mooring" });
    const subtasks = await within(modal).findByRole("region", { name: "Subtasks" });
    await waitFor(() => {
      expect(within(subtasks).getAllByRole("link").map((child) => child.textContent)).toEqual(expect.arrayContaining([expect.stringMatching(/^C-\d{4}$/)]));
    });
  });

  it("from a task: C-0001 the parent, no Title; its subtasks made under it, the polite line counts them; an accepted task has no act", async () => {
    window.localStorage.setItem("specengine.board.as", "Mira Holt");
    const { splitCard } = renderMock("#/harbor-sim/board/C-0001");
    const modal = await screen.findByRole("dialog", { name: "Write the tide table foreword" });
    fireEvent.click(await within(modal).findByRole("button", { name: ACT }));
    const dialog = await screen.findByRole("dialog", { name: "Split C-0001 to departments" });
    await within(dialog).findAllByRole("checkbox");
    expect(within(dialog).queryByLabelText("Title")).toBeNull();
    expect(within(dialog).getByLabelText<HTMLInputElement>("Reviewer").value).toBe("Ivo Brandt");
    expect(bodies(splitCard)[0]).toMatchObject({ parent: "C-0001", target: null, title: null, child: null, mandate: null, dry_run: true });
    chooseMandates(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Split" }));
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toMatch(/^\d+ subtasks? for C-0001$/);
    });
    expect(screen.getAllByRole("dialog")).toEqual([modal]);
    window.location.hash = "#/harbor-sim/board/C-0005";
    const accepted = await screen.findByRole("dialog", { name: "Mooring fees table" });
    await within(accepted).findByRole("region", { name: "Acceptance" });
    expect(within(accepted).queryByRole("button", { name: ACT })).toBeNull();
  });

  it("from Flow's panel: the box and the scope after New task, the node the parent's target and its title the parent's", async () => {
    window.localStorage.setItem("specengine.board.as", "Ivo Brandt");
    const { splitCard } = renderMock("#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS");
    const details = await screen.findByRole("complementary", { name: "Details" });
    await waitFor(() => {
      expect(within(details).getByRole("heading", { level: 2 }).textContent).toBe("Mooring");
    });
    // The panel's shown acts: the last box stays mounted, hidden, while the scope shows.
    const acts = () => Array.from(details.querySelectorAll(".flow-panel-actions > *"), (one) => (one.closest("[hidden]") === null ? one.textContent : null)).filter((one) => one !== null);
    expect(acts()).toEqual(["Open document", "New task", ACT]);
    fireEvent.click(within(details).getByRole("button", { name: ACT }));
    const dialog = await screen.findByRole("dialog", { name: "Split to departments" });
    await within(dialog).findAllByRole("checkbox");
    expect(bodies(splitCard)[0]).toMatchObject({ target: ["MEC-MOORING"], title: "Mooring", dry_run: true, child: null });
    chooseMandates(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Split" }));
    const note = await waitFor(() => {
      const found = Array.from(details.querySelectorAll<HTMLElement>(".note")).find((line) => /^Split into C-\d{4} and 2 subtasks$/.test(line.textContent));
      expect(found).toBeDefined();
      return found as HTMLElement;
    });
    expect(note.querySelector("a")?.getAttribute("href")).toMatch(/^#\/harbor-sim\/board\/C-\d{4}$/);
    fireEvent.keyDown(document.querySelector(".react-flow__node[data-id='n:MEC-MOORING']") ?? details, { key: "Escape" });
    await waitFor(() => {
      expect(within(details).getByRole("heading", { level: 2 }).textContent).not.toBe("Mooring");
    });
    expect(acts()).toEqual(["Open document", "New task", ACT]);
  });

  it("from Flow's panel no task of the document is listed, though its page's trace read stands in the cache", async () => {
    window.localStorage.setItem("specengine.board.as", "Ivo Brandt");
    renderMock("#/harbor-sim/tree/MEC-MOORING/page");
    // The page's Cards part, from its trace read: the fees table, a task on the mooring.
    await waitFor(() => {
      expect(document.querySelector(".cards-block")?.textContent).toContain("C-0005");
    });
    window.location.hash = "#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS";
    const details = await screen.findByRole("complementary", { name: "Details" });
    await waitFor(() => {
      expect(within(details).getByRole("heading", { level: 2 }).textContent).toBe("Mooring");
    });
    fireEvent.click(within(details).getByRole("button", { name: ACT }));
    const dialog = await screen.findByRole("dialog", { name: "Split to departments" });
    await within(dialog).findAllByRole("checkbox");
    expect(within(dialog).queryByText("Tasks on this document:")).toBeNull();
    expect(within(dialog).queryByRole("link", { name: "C-0005 Mooring fees table" })).toBeNull();
  });

  it("the conflict scenario: the first Split is refused in the daemon's words, the dialog and its choice kept, nothing stored; the next one is taken", async () => {
    const { client, dialog, splitCard } = await fromMooring("conflict");
    const before = (await client.getCards("harbor-sim")).cards.length;
    chooseMandates(dialog);
    fireEvent.click(within(dialog).getByRole("button", { name: "Split" }));
    const refusal = await within(dialog).findByText("child: no flow takes the discipline `copy`: none lists it, and every `[[card.flow]]` lists its disciplines");
    expect(refusal.getAttribute("lang")).toBe("en");
    expect(refusal.closest('[role="alert"]')).not.toBeNull();
    expect(rows(dialog).map(([, checked]) => checked)).toEqual([true, false, true]);
    expect((await client.getCards("harbor-sim")).cards.length).toBe(before);
    fireEvent.click(within(dialog).getByRole("button", { name: "Split" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Split to departments" })).toBeNull();
    });
    expect(splitCard).toHaveBeenCalledTimes(3);
    expect((await client.getCards("harbor-sim")).cards.length).toBe(before + 3);
  });

  it("a hub: the Reviewer read-only, the login", async () => {
    renderMock("#/harbor-sim/tree/MEC-MOORING/page", "hub");
    fireEvent.click(await screen.findByRole("button", { name: ACT }));
    const dialog = await screen.findByRole("dialog", { name: "Split to departments" });
    await waitFor(() => {
      const reviewer = within(dialog).getByLabelText<HTMLInputElement>("Reviewer");
      expect([reviewer.value, reviewer.readOnly]).toEqual(["mira", true]);
    });
  });
});

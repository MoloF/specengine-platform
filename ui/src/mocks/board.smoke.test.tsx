import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { ACTING_KEY } from "../board/acting";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The Board over the real mock, as `pnpm dev` shows it (docs/features/board-ui.md): AC-05's whole
// cycle on one screen, AC-07's keys, AC-08's two flows, AC-09's empty board and New card from a node.

function renderMock(scenario: Scenario, hash: string) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  const view = render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, view };
}

function card(id: string): HTMLElement {
  const found = document.querySelector<HTMLElement>(`[data-card="${id}"]`);
  if (found === null) {
    throw new Error(`no card ${id}`);
  }
  return found;
}

async function shows(id: string): Promise<HTMLElement> {
  return waitFor(() => card(id));
}

function columnOf(id: string): string {
  return card(id).dataset.column ?? "";
}

async function choose(id: string, line: RegExp | string) {
  const element = await shows(id);
  element.focus();
  fireEvent.keyDown(element, { key: "m" });
  const menu = await screen.findByRole("menu", { name: `Move ${id}` });
  fireEvent.click(within(menu).getByRole("menuitem", { name: line }));
}

function type(scope: HTMLElement, label: string, value: string) {
  fireEvent.change(within(scope).getByLabelText(label), { target: { value } });
}

/** Every write dialog gone; the card's modal, if one is open, stays. */
async function closed() {
  await waitFor(() => {
    expect(screen.queryAllByRole("dialog").filter((dialog) => !dialog.classList.contains("card-modal"))).toEqual([]);
  });
}

/** The card's modal, by its title. */
function modal(title: string): Promise<HTMLElement> {
  return screen.findByRole("dialog", { name: title });
}

/** The modal closed by its Close button. */
async function closeModal(dialog: HTMLElement) {
  fireEvent.click(within(dialog).getByRole("button", { name: /^Close/ }));
  await waitFor(() => {
    expect(screen.queryByRole("dialog")).toBeNull();
  });
}

/** A column's cell in the one lane of None. */
function cell(title: string): HTMLElement {
  const key = screen.getByRole("heading", { level: 2, name: new RegExp(`^${title}\\b`) }).dataset.columnHead ?? "";
  const found = Array.from(document.querySelectorAll<HTMLElement>("section.board-cell")).find((candidate) => candidate.dataset.lane?.endsWith(`|${key}`) === true);
  if (found === undefined) {
    throw new Error(`no cell ${title}`);
  }
  return found;
}

describe("the Board over the mock", () => {
  it("harbor-sim: copy and plates give two tabs; `blocked` is a column like any other; every card links to its source (AC-08)", async () => {
    renderMock("normal", "#/harbor-sim/board");
    // galley, the first workspace, shows copy alone (docs/features/workspaces.md WS-14): every flow for the visit.
    fireEvent.click(await screen.findByRole("button", { name: "Show all flows" }));
    const tabs = await screen.findAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent)).toEqual(["copy", "plates"]);
    await shows("C-0001");
    expect(within(card("C-0001")).getByRole("link", { name: "MEC-TIDE-TABLES", hidden: true }).getAttribute("href")).toBe("#/harbor-sim/tree/MEC-TIDE-TABLES");
    fireEvent.click(tabs[1] as HTMLElement);
    await shows("C-0008");
    expect(screen.getByRole("heading", { level: 2, name: /^blocked/ })).toBeTruthy();
    expect(columnOf("C-0008")).toBe("blocked");
  });

  it("a whole cycle on one screen: new, assign, move, accept; the name typed is the acceptance's and survives a reload (AC-05)", async () => {
    const first = renderMock("normal", "#/ledger-api/board");
    fireEvent.click(await screen.findByRole("button", { name: "New card" }));
    const created = await screen.findByRole("dialog", { name: "New card" });
    type(created, "Title", "Write the close checklist");
    fireEvent.click(within(created).getByRole("button", { name: "Create card" }));
    await closed();
    await shows("C-0106");
    expect(columnOf("C-0106")).toBe("backlog");
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-0106"));
    });

    fireEvent.click(within(card("C-0106")).getByRole("link", { name: "Write the close checklist", hidden: true }));
    const opened = await modal("Write the close checklist");
    fireEvent.click(await within(opened).findByRole("button", { name: "Assign" }));
    const assign = await screen.findByRole("dialog", { name: "Assign C-0106" });
    type(assign, "Assignee", "Omar Lind");
    type(assign, "Reviewer", "Ana Weiss");
    fireEvent.click(within(assign).getByRole("button", { name: "Save" }));
    await closed();
    await within(await shows("C-0106")).findByText("Omar Lind");
    await closeModal(opened);

    await choose("C-0106", /^Review/);
    await waitFor(() => {
      expect(columnOf("C-0106")).toBe("review");
    });

    type(document.body, "Acting as", "Ana Weiss");
    await choose("C-0106", "Accept");
    const accept = await screen.findByRole("dialog", { name: "Accept C-0106" });
    fireEvent.click(within(accept).getByRole("button", { name: "Accept card" }));
    await closed();
    await waitFor(() => {
      expect(columnOf("C-0106")).toBe("done");
    });
    expect(within(card("C-0106")).getByText("Accepted")).toBeTruthy();
    fireEvent.click(within(card("C-0106")).getByRole("link", { name: "Write the close checklist", hidden: true }));
    const done = await modal("Write the close checklist");
    const part = await within(done).findByRole("region", { name: "Acceptance" });
    expect(part.textContent).toContain("Ana Weiss");
    expect(within(done).queryByRole("group", { name: "Actions" })).toBeNull();

    first.view.unmount();
    expect(window.localStorage.getItem(ACTING_KEY)).toBe("Ana Weiss");
    renderMock("normal", "#/ledger-api/board");
    expect((await screen.findByLabelText<HTMLInputElement>("Acting as")).value).toBe("Ana Weiss");
  });

  it("the daemon's rules in its words: the assignee never accepts; a move needing a name refuses without it", async () => {
    window.localStorage.setItem(ACTING_KEY, "Mira Holt");
    renderMock("normal", "#/harbor-sim/board");
    await choose("C-0004", "Accept");
    const accept = await screen.findByRole("dialog", { name: "Accept C-0004" });
    fireEvent.click(within(accept).getByRole("button", { name: "Accept card" }));
    expect((await within(accept).findByRole("alert")).textContent).toContain("C-0004: `Mira Holt` is its assignee, who never accepts a result; nothing changed");
    fireEvent.click(within(accept).getByRole("button", { name: "Cancel" }));
    await closed();
    await choose("C-0002", /needs desk/);
    expect((await screen.findByRole("alert")).textContent).toContain("C-0002 is in draft: the move to `edit` is `desk`'s, not `Mira Holt`'s; nothing changed");
    expect(columnOf("C-0002")).toBe("draft");
  });

  it("keys alone move a card, focus on it, the live region speaks (AC-07)", async () => {
    renderMock("normal", "#/ledger-api/board");
    const element = await shows("C-0101");
    element.focus();
    fireEvent.keyDown(element, { key: "m" });
    await screen.findByRole("menu");
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Enter" });
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("C-0101 moved to Ready");
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(card("C-0101"));
    });
    expect(columnOf("C-0101")).toBe("ready");
  });

  it("ledger-api: one flow, no tabs; a card a column left behind in the last lane, its move out from its read", async () => {
    renderMock("normal", "#/ledger-api/board");
    await screen.findByRole("heading", { level: 2, name: /^Not in the flow/ });
    expect(cell("Not in the flow").textContent).toContain("the column `parked` is no longer in the flow `plain`: served last");
    expect(screen.queryByRole("tablist")).toBeNull();
    const element = await shows("C-0105");
    element.focus();
    fireEvent.keyDown(element, { key: "m" });
    const menu = await screen.findByRole("menu");
    fireEvent.click(await within(menu).findByRole("menuitem", { name: "Backlog" }));
    await waitFor(() => {
      expect(columnOf("C-0105")).toBe("backlog");
    });
    expect(screen.queryByRole("heading", { level: 2, name: /^Not in the flow/ })).toBeNull();
  });

  it("the modal: fields, evidence, history; a cancelled card is listed only when asked, with why (AC-09)", async () => {
    renderMock("normal", "#/harbor-sim/board/C-0003");
    const panel = await modal("Night passage rules, plain words");
    expect(within(panel).getByRole("link", { name: "MEC-NIGHT-PASSAGE" }).getAttribute("href")).toBe("#/harbor-sim/tree/MEC-NIGHT-PASSAGE");
    expect(within(within(panel).getByRole("region", { name: "Evidence" })).getByText("Second read done with the pilots' office")).toBeTruthy();
    expect(within(within(panel).getByRole("region", { name: "History" })).getByText("Created")).toBeTruthy();
    expect(document.querySelector('[data-card="C-0006"]')).toBeNull();
    await closeModal(panel);
    fireEvent.click(screen.getByLabelText("Show cancelled"));
    await shows("C-0006");
    expect(within(card("C-0006")).getByText("Cancelled")).toBeTruthy();
  });

  it("empty: the columns of each flow and the line naming how to add a card (AC-09)", async () => {
    renderMock("empty", "#/harbor-sim/board");
    await screen.findByText("No cards yet. Next: press New card, press New card on a node in the Spec tree, or have your AI call create_card.");
    fireEvent.click(screen.getByRole("button", { name: "Show all flows" }));
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["copy", "plates"]);
    expect(screen.getByRole("heading", { level: 2, name: /^Draft/ })).toBeTruthy();
  });

  it("New card on a node opens with it as the target; the card links back to it (AC-09)", async () => {
    renderMock("normal", "#/harbor-sim/tree/MEC-TIDES");
    fireEvent.click(await screen.findByRole("button", { name: "New card" }));
    const dialog = await screen.findByRole("dialog", { name: "New card" });
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source").value).toBe("MEC-TIDES");
    type(dialog, "Title", "Explain the tides");
    type(dialog, "Discipline", "copy");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create card" }));
    await closed();
    const link = await screen.findByRole("link", { name: "Open it on the Board" });
    expect(link.getAttribute("href")).toBe("#/harbor-sim/board/C-0020");
    fireEvent.click(link);
    window.location.hash = "#/harbor-sim/board/C-0020";
    const source = await shows("C-0020");
    expect(within(source).getByRole("link", { name: "MEC-TIDES", hidden: true }).getAttribute("href")).toBe("#/harbor-sim/tree/MEC-TIDES");
  });

  it("error: the daemon's message, Retry", async () => {
    renderMock("error", "#/harbor-sim/board");
    expect(await screen.findByText(/database is locked/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  });

  it("conflict: the first write finds the card changed, the daemon's words; the next one, on the new read, stands", async () => {
    renderMock("conflict", "#/ledger-api/board");
    await choose("C-0101", /^Ready/);
    expect((await screen.findByRole("alert")).textContent).toContain("`C-0101` changed since this run read it");
    await waitFor(() => {
      expect(columnOf("C-0101")).toBe("backlog");
    });
    await choose("C-0101", /^Ready/);
    await waitFor(() => {
      expect(columnOf("C-0101")).toBe("ready");
    });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("large: a column past the cap ends Show all", async () => {
    renderMock("large", "#/harbor-sim/board");
    // 230 cards and C-0003 and C-0019 in copy's edit.
    expect(await screen.findByRole("button", { name: "Show all 232" })).toBeTruthy();
  });
});

describe("the acting name over the mock", () => {
  it("takes a full name of any script; one off the grammar: the CLI's line next to the field, nothing moved", async () => {
    renderMock("normal", "#/ledger-api/board");
    const field = await screen.findByLabelText("Acting as");
    fireEvent.change(field, { target: { value: "\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430" } });
    await choose("C-0101", /^Ready/);
    await waitFor(() => {
      expect(columnOf("C-0101")).toBe("ready");
    });
    fireEvent.change(field, { target: { value: "x".repeat(129) } });
    await choose("C-0102", /^In progress/);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("spec: --as: 129 bytes; at most 128");
    expect(alert.closest(".board-acting")).not.toBeNull();
    expect(columnOf("C-0102")).toBe("ready");
  });
});

describe("the card against its brief, over the mock", () => {
  function clearDialog() {
    return screen.findByRole("dialog", { name: /^Clear / });
  }

  it("marks the tile and the modal; No effect with a reason clears the mark, the typed name in the card's history (AC-10)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    renderMock("normal", "#/harbor-sim/board/C-0002");
    const panel = await modal("Caption the berth plans");
    await waitFor(() => {
      expect(within(card("C-0002")).getByText("Suspect")).toBeTruthy();
    });
    expect(within(panel).getByRole("region", { name: "Against the brief" }).textContent).toContain("The brief changed at DOM-BERTHS");
    fireEvent.click(within(panel).getByRole("button", { name: "Clear" }));
    const dialog = await clearDialog();
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear card" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Say why the mark clears.");
    type(dialog, "Reason", "Only the numbering moved.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear card" }));
    await closed();
    await waitFor(() => {
      expect(within(card("C-0002")).queryByText("Suspect")).toBeNull();
    });
    const history = await within(panel).findByRole("region", { name: "History" });
    await within(history).findByText("Cleared");
    expect(history.textContent).toContain("by Ivo Brandt");
    expect(screen.queryByRole("button", { name: "Clear" })).toBeNull();
  });

  it("an open card takes no correction: the choice is off, a correction answering an accepted card", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    renderMock("normal", "#/harbor-sim/board/C-0002");
    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    const dialog = await clearDialog();
    expect(within(dialog).getByRole<HTMLInputElement>("radio", { name: "Needs correction" }).disabled).toBe(true);
  });

  it("Needs correction on an accepted card: a new card, the original accepted and correcting; accepted, the correction ends the mark (AC-06)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    renderMock("normal", "#/harbor-sim/board/C-0005");
    fireEvent.click(await screen.findByRole("button", { name: "Clear" }));
    const dialog = await clearDialog();
    fireEvent.click(within(dialog).getByRole("radio", { name: "Needs correction" }));
    type(dialog, "Reason", "The rates moved.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear card" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("A correction needs an estimate.");
    type(dialog, "Estimate", "2");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear card" }));
    await closed();
    const made = await shows("C-0020");
    expect(made.textContent).toContain("Correct Mooring fees table");
    expect(columnOf("C-0020")).toBe("draft");
    await waitFor(() => {
      expect(within(card("C-0005")).getByText("Correcting")).toBeTruthy();
    });
    expect(within(card("C-0005")).queryByText("Accepted")).toBeNull();
    expect(await screen.findByText("C-0020 corrects C-0005: copy/draft")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Clear" })).toBeNull();
  });

  it("a card departed shows it, its discrepancy named; it takes no Clear", async () => {
    renderMock("normal", "#/harbor-sim/board/C-0010");
    const panel = await modal("Deep-draft entry notice");
    const part = await within(panel).findByRole("region", { name: "Against the brief" });
    expect(part.textContent).toContain("Departed");
    expect(part.textContent).toContain("PR-0041");
    expect(within(part).queryByRole("button")).toBeNull();
    expect(within(panel).getByRole("region", { name: "Acceptance" }).textContent).toContain("Departs from RULE-TIDE-WINDOW: PR-0041");
    expect(within(card("C-0010")).getByText("Departed")).toBeTruthy();
  });

  it("a departing accept raises its discrepancy in the Inbox and marks the card; nothing stops it", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const { client } = renderMock("normal", "#/harbor-sim/board");
    await choose("C-0004", "Accept");
    const dialog = await screen.findByRole("dialog", { name: "Accept C-0004" });
    fireEvent.click(within(dialog).getByRole("radio", { name: "Departs from it" }));
    type(dialog, "Summary", "Hours printed from the old sheet");
    type(dialog, "What the result does", "Lock B closes at six.");
    type(dialog, "What the brief says", "Lock B closes at eight.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept card" }));
    await closed();
    await waitFor(() => {
      expect(columnOf("C-0004")).toBe("filed");
    });
    await waitFor(() => {
      expect(within(card("C-0004")).getByText("Departed")).toBeTruthy();
    });
    const raised = (await client.getInbox("harbor-sim")).proposals.at(-1);
    expect([raised?.kind, raised?.summary]).toEqual(["discrepancy", "Hours printed from the old sheet"]);
  });

  it("the daemon's rules on the client: a clear of a card with no mark is refused; a group clear takes what is under its node, no other", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    await expect(client.clearCard("harbor-sim", "C-0001", { outcome: "no_effect", reason: "x", revision: "1", as: "Ivo Brandt" })).rejects.toThrow("C-0001 is not suspect: there is no mark to clear; nothing changed");
    await expect(client.clearCard("harbor-sim", "C-0002", { outcome: "no_effect", reason: "x", revision: "1", as: "local" })).rejects.toThrow("`local` clears no mark");
    await expect(client.clearCardGroup("harbor-sim", { group: "MEC-TIDES", reason: "x", as: "Ivo Brandt" })).rejects.toThrow("no suspect card under `MEC-TIDES`");
    expect((await client.clearCardGroup("harbor-sim", { group: "DOM-BERTHS", reason: "Numbering only.", as: "Ivo Brandt" })).cleared).toEqual(expect.arrayContaining(["C-0002", "C-0005"]));
    const { nodes } = await client.getTrace("harbor-sim");
    expect([...new Set(nodes.flatMap((node) => node.cards).filter((row) => row.on !== null).map((row) => row.id))]).toEqual(["C-0010"]);
    expect((await client.getTrace("harbor-sim", { ref: "NOPE" })).reason).toBe("no node `NOPE`");
  });

  it("a card on a target gone shows its mark and takes its Clear on the page; cleared, a note stays and the mark goes (RM-0048)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    renderMock("normal", "#/harbor-sim/board/C-0008");
    const panel = await modal("Crane reach diagram");
    const tile = await shows("C-0008");
    expect(tile.querySelector(".board-card-mark")?.textContent).toBe("SuspectMEC-OLD-SLIPWAY");
    fireEvent.click(await within(panel).findByRole("button", { name: "Clear" }));
    const dialog = await clearDialog();
    type(dialog, "Reason", "The slipway is out of the book.");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear card" }));
    await closed();
    await waitFor(() => {
      expect(card("C-0008").querySelector(".board-card-mark")).toBeNull();
    });
  });

  it("marks every tile from the list: a whole trace is read only for an open card, so a board of tiles needs none", async () => {
    const { client } = renderMock("normal", "#/harbor-sim/board");
    const getTrace = vi.spyOn(client, "getTrace");
    await shows("C-0002");
    expect(card("C-0002").querySelector(".board-card-mark")?.textContent).toBe("SuspectDOM-BERTHS");
    expect(card("C-0005").querySelector(".board-card-mark")?.textContent).toBe("SuspectMEC-MOORING");
    expect(card("C-0010").querySelector(".board-card-mark")?.textContent).toBe("DepartedPR-0041");
    expect(within(card("C-0005")).queryByText("Accepted")).toBeNull();
    expect(within(card("C-0011")).getByText("Accepted")).toBeTruthy();
    expect(getTrace).not.toHaveBeenCalled();
  });

  it("a re-estimate of a card holding one says why, and stands alone", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    await expect(client.editCard("harbor-sim", "C-0007", { estimate: "5", revision: "1", as: null })).rejects.toThrow("C-0007 holds an estimate: --reason says why it changes");
    await expect(client.editCard("harbor-sim", "C-0007", { estimate: "5", goal: "x", reason: "y", revision: "1", as: null })).rejects.toThrow("takes --reason and no other field");
    await client.editCard("harbor-sim", "C-0007", { estimate: "5", reason: "The survey grew.", revision: "1", as: null });
    const shown = await client.getCard("harbor-sim", "C-0007");
    expect("history" in shown ? shown.history?.at(-1)?.type : null).toBe("card.reestimated");
  });
});

// board-handoff (RM-0085) over the mock: the plates' `blocked` is its return column, a word of data;
// cards hand work across copy and plates both ways; the daemon's rules in its words.
describe("hand-offs and returns over the mock", () => {
  it("a move into the return column with no note is refused in the daemon's words, nothing changed; with one it stands, the note on its event; a reorder there or a move out needs none", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    await expect(client.moveCard("harbor-sim", "C-0007", { to: "blocked", revision: "1", as: null })).rejects.toThrow("C-0007 enters `blocked`, its flow's return column: say why with `--note`; nothing changed");
    expect((await client.getCards("harbor-sim")).cards.find((row) => row.id === "C-0007")?.column).toBe("queued");
    await expect(client.moveCard("harbor-sim", "C-0007", { to: "blocked", note: "  ", revision: "1", as: null })).rejects.toThrow("note: blank; say why it moves");
    const moved = await client.moveCard("harbor-sim", "C-0007", { to: "blocked", note: " The survey is late. ", revision: "1", as: "Ivo Brandt" });
    expect(moved.column).toBe("blocked");
    const shown = await client.getCard("harbor-sim", "C-0007");
    // Stored as sent, as the core stores it.
    expect("history" in shown ? shown.history?.at(-1)?.payload : null).toMatchObject({ from: "queued", to: "blocked", note: " The survey is late. ", as: "Ivo Brandt" });
    await client.moveCard("harbor-sim", "C-0007", { to: "blocked", before: "C-0008", revision: "2", as: null });
    await client.moveCard("harbor-sim", "C-0007", { to: "queued", revision: "3", as: null });
    const other = await client.moveCard("harbor-sim", "C-0013", { to: "checked", revision: "1", as: null });
    expect(other.column).toBe("checked");
  });

  it("serves `return` on each flow, the plates' `blocked`, null elsewhere; a hand-off is one create with its reviewer; `parent` lists the children", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    expect((await client.getCards("harbor-sim")).flows.map((flow) => [flow.name, flow.return])).toEqual([
      ["copy", null],
      ["plates", "blocked"],
    ]);
    expect((await client.getCards("ledger-api")).flows.map((flow) => flow.return)).toEqual([null]);
    const made = await client.createCard("harbor-sim", { title: "Berth plate", discipline: "plates", parent: "C-0002", reviewer: "Ivo Brandt", as: "Ivo Brandt" });
    expect([made.flow, made.column]).toEqual(["plates", "queued"]);
    expect((await client.getCards("harbor-sim", { parent: "C-0002" })).cards.map((row) => [row.id, row.reviewer, row.parent])).toEqual([[made.id, "Ivo Brandt", "C-0002"]]);
    expect((await client.getCards("harbor-sim", { parent: "C-0001" })).cards.map((row) => row.id)).toEqual(["C-0019", "C-0013", "C-0016"]);
    await expect(client.createCard("harbor-sim", { title: "x", reviewer: "agent", as: null })).rejects.toThrow("reviewer: `agent` can be no reviewer: a person accepts a result");
    await expect(client.createCard("harbor-sim", { title: "x", reviewer: "-", as: null })).rejects.toThrow("reviewer: `-` clears a name: a new card has none");
    const created = await client.getCard("harbor-sim", made.id ?? "");
    expect("history" in created ? created.history?.at(0)?.payload : null).toMatchObject({ reviewer: "Ivo Brandt" });
  });

  it("tiles: Returned on a card in the plates' return column; a parent's hand-offs counted, its returned ones with the badge", async () => {
    renderMock("normal", "#/harbor-sim/board");
    fireEvent.click(await screen.findByRole("button", { name: "Show all flows" }));
    await shows("C-0001");
    expect(card("C-0001").querySelector(".board-card-handoffs")?.textContent).toBe("3 hand-offs");
    expect(card("C-0003").querySelector(".board-card-handoffs")?.textContent).toBe("1 hand-off1 returned");
    expect(card("C-0002").querySelector(".board-card-handoffs")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "plates" }));
    await shows("C-0015");
    expect(within(card("C-0015")).getByText("Returned")).toBeTruthy();
    expect(within(card("C-0008")).getByText("Returned")).toBeTruthy();
    expect(within(card("C-0013")).queryByText("Returned")).toBeNull();
    expect(card("C-0009").querySelector(".board-card-handoffs")?.textContent).toBe("1 hand-off");
  });

  it("the modal: a parent's hand-offs with their words across both flows; a returned card's reason, or that its history keeps none", async () => {
    renderMock("normal", "#/harbor-sim/board/C-0001");
    const parent = await modal("Write the tide table foreword");
    const handOffs = within(await within(parent).findByRole("region", { name: "Hand-offs" })).getAllByRole("listitem");
    expect(handOffs.map((item) => item.textContent)).toEqual([
      "C-0019Tide table glossarycopy/editTomas OakesTaken",
      "C-0013Tide table plateplates/queuedUnassignedHanded off",
      "C-0016Tide table coverplates/struckNoor AinsleyAccepted",
    ]);
    fireEvent.click(within(handOffs[1] as HTMLElement).getByRole("link", { name: "C-0013" }));
    await modal("Tide table plate");
    expect(window.location.hash).toBe("#/harbor-sim/board/C-0013");
    window.location.hash = "#/harbor-sim/board/C-0015";
    const back = await modal("Night passage chart");
    const reason = await within(back).findByRole("region", { name: "Returned" });
    expect(reason.textContent).toContain("Returned by Tomas Oakes");
    expect(reason.textContent).toContain("The buoy numbers on the plate disagree with MEC-NIGHT-PASSAGE");
    window.location.hash = "#/harbor-sim/board/C-0008";
    const gone = await modal("Crane reach diagram");
    expect((await within(gone).findByRole("region", { name: "Returned" })).textContent).toContain("is no longer in this card's history");
  });

  it("serves each card's linked tasks read now, in link order, a task not read null with its note; a card with none, none (CT-5 of card-tasks)", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const shown = async (project: string, id: string) => {
      const read = await client.getCard(project, id);
      if (!("tasks" in read)) {
        throw new Error(`no card ${id}`);
      }
      return read;
    };
    const first = await shown("harbor-sim", "C-0001");
    expect(first.tasks).toEqual([
      { id: "T-0107", status: "review", title: "Queue deep ships at the outer anchorage", stale: null },
      { id: "T-0109", status: "in_progress", title: "One loader for both tide-table formats", stale: true },
      { id: "T-0106", status: null, title: null, stale: null },
    ]);
    expect(first.notes).toEqual([
      "T-0107: stale unknown: no snapshot (the owner's `spec task approve` freezes one)",
      "T-0106: its row does not read (`criteria`: bad JSON); no state shown",
    ]);
    expect((await shown("harbor-sim", "C-0004")).tasks.map((task) => [task.id, task.status])).toEqual([["T-0104", "done"]]);
    const accepted = await shown("harbor-sim", "C-0012");
    expect([accepted.acceptance === null, accepted.tasks, accepted.notes]).toEqual([
      false,
      [{ id: "T-0111", status: "ready", title: "A wider window at spring tides", stale: null }],
      ["T-0111: snapshot place /work/harbor-sim/T-0111 is gone (worktree removed); stale unknown"],
    ]);
    expect((await shown("harbor-sim", "C-0002")).tasks).toEqual([]);
    expect((await shown("ledger-api", "C-0102")).tasks.map((task) => [task.id, task.status])).toEqual([["T-0033", "review"]]);
    // The list's rows keep their 21 keys: no task on a tile.
    const list = await client.getCards("harbor-sim", { marks: true });
    expect(list.cards.every((row) => !("tasks" in row) && Object.keys(row).length === 21)).toBe(true);
  });

  it("the modal of a card with three links: its Tasks part shows each, the approval to copy on the one in review", async () => {
    renderMock("normal", "#/harbor-sim/board/C-0001");
    const opened = await modal("Write the tide table foreword");
    const part = await within(opened).findByRole("region", { name: "Tasks" });
    expect(Array.from(part.querySelectorAll("li[data-task]"), (row) => row.getAttribute("data-task"))).toEqual(["T-0107", "T-0109", "T-0106"]);
    expect(within(part).getByRole("link", { name: "T-0107" }).getAttribute("href")).toBe("#/harbor-sim/tasks/T-0107");
    expect(Array.from(part.querySelectorAll("code"), (code) => code.textContent)).toEqual(["spec task approve T-0107"]);
    expect(within(part).getByText("T-0106: its row does not read (`criteria`: bad JSON); no state shown").getAttribute("lang")).toBe("en");
  });
});

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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

async function closed() {
  await waitFor(() => {
    expect(screen.queryByRole("dialog")).toBeNull();
  });
}

describe("the Board over the mock", () => {
  it("harbor-sim: copy and plates give two tabs; `blocked` is a column like any other; every card links to its source (AC-08)", async () => {
    renderMock("normal", "#/harbor-sim/board");
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
    fireEvent.click(await screen.findByRole("button", { name: "Assign" }));
    const assign = await screen.findByRole("dialog", { name: "Assign C-0106" });
    type(assign, "Assignee", "Omar Lind");
    type(assign, "Reviewer", "Ana Weiss");
    fireEvent.click(within(assign).getByRole("button", { name: "Save" }));
    await closed();
    await within(await shows("C-0106")).findByText("Omar Lind");

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
    const part = await screen.findByRole("region", { name: "Acceptance" });
    expect(part.textContent).toContain("Ana Weiss");
    expect(screen.queryByRole("group", { name: "Actions" })).toBeNull();

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
    renderMock("normal", "#/ledger-api/board/C-0105");
    const lane = (await screen.findByRole("heading", { level: 2, name: /^Not in the flow/ })).closest("section");
    expect(lane?.textContent).toContain("the column `parked` is no longer in the flow `plain`: served last");
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

  it("the panel: fields, evidence, history; a cancelled card is listed only when asked, with why (AC-09)", async () => {
    renderMock("normal", "#/harbor-sim/board/C-0003");
    const panel = await screen.findByRole("complementary", { name: "Night passage rules, plain words" });
    expect(within(panel).getByRole("link", { name: "MEC-NIGHT-PASSAGE" }).getAttribute("href")).toBe("#/harbor-sim/tree/MEC-NIGHT-PASSAGE");
    expect(within(within(panel).getByRole("region", { name: "Evidence" })).getByText("Second read done with the pilots' office")).toBeTruthy();
    expect(within(within(panel).getByRole("region", { name: "History" })).getByText("Created")).toBeTruthy();
    expect(document.querySelector('[data-card="C-0006"]')).toBeNull();
    fireEvent.click(screen.getByLabelText("Show cancelled"));
    await shows("C-0006");
    expect(within(card("C-0006")).getByText("Cancelled")).toBeTruthy();
  });

  it("empty: the columns of each flow and the line naming how to add a card (AC-09)", async () => {
    renderMock("empty", "#/harbor-sim/board");
    await screen.findByText("No cards yet. Next: press New card, press New card on a node in the Spec tree, or have your AI call create_card.");
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
    expect(link.getAttribute("href")).toBe("#/harbor-sim/board/C-0011");
    fireEvent.click(link);
    window.location.hash = "#/harbor-sim/board/C-0011";
    const source = await shows("C-0011");
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
    expect(await screen.findByRole("button", { name: "Show all 231" })).toBeTruthy();
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

  it("marks the tile and the panel; No effect with a reason clears the mark, the typed name in the card's history (AC-10)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    renderMock("normal", "#/harbor-sim/board/C-0002");
    const panel = await screen.findByRole("complementary", { name: "Caption the berth plans" });
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
    const made = await shows("C-0011");
    expect(made.textContent).toContain("Correct Mooring fees table");
    expect(columnOf("C-0011")).toBe("draft");
    await waitFor(() => {
      expect(within(card("C-0005")).getByText("Correcting")).toBeTruthy();
    });
    expect(within(card("C-0005")).getByText("Accepted")).toBeTruthy();
    expect(await screen.findByText("C-0011 corrects C-0005: copy/draft")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Clear" })).toBeNull();
  });

  it("a card departed shows it, its discrepancy named; it takes no Clear", async () => {
    renderMock("normal", "#/harbor-sim/board/C-0010");
    const panel = await screen.findByRole("complementary", { name: "Deep-draft entry notice" });
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
    expect(nodes.flatMap((node) => node.cards).filter((row) => row.on !== null).map((row) => row.id)).toEqual(["C-0010"]);
    expect((await client.getTrace("harbor-sim", { ref: "NOPE" })).reason).toBe("no node `NOPE`");
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

import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Card, CardRow, WorkspaceDocument } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow, aCardWrite, aWorkspace, aWorkspaces } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { ACTING_KEY } from "./acting";

// BH-11 of board-handoff: a hand-off is one `POST .../cards`, a child card in another trade's flow,
// its parent the card, its reviewer the asking side; from the modal, the page and `h` on a tile, on
// open cards only. The dialog offers every served flow's disciplines, a hidden flow's too; title, goal
// and targets prefilled; no criteria, estimate, due date, evidence or assignee.

const FLOWS = [
  aCardFlow({ name: "narrative", disciplines: ["narrative"] }),
  aCardFlow({ name: "development", disciplines: ["development", "tools"] }),
  aCardFlow({ name: "art", disciplines: ["art"] }),
];

const SOURCE = aCard({ id: "C-0001", flow: "narrative", title: "Lamp room dialogue", goal: "Two endings, one lamp.", targets: ["SCN-LAMP", "SCN-ROOM"], criteria: ["Reads aloud"], estimate: "3", due: "2999-01-01", revision: "4" });

function handOffClient(rows: CardRow[] = [aCardRow({ id: "C-0001", flow: "narrative", title: "Lamp room dialogue", targets: ["SCN-LAMP", "SCN-ROOM"], revision: "4" })], cards: Record<string, Card> = { "C-0001": SOURCE }): StubClient {
  let list = rows;
  const client = stubClient();
  client.getCards.mockImplementation(() => Promise.resolve(aCardList(list, { flows: FLOWS })));
  client.getCard.mockImplementation((_project, id) => {
    const card = cards[id];
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "writing", flows: ["narrative"] })]));
  client.createCard.mockImplementation((_project, body) => {
    list = [...list, aCardRow({ id: "C-0002", flow: "development", title: body.title, parent: body.parent ?? null, reviewer: body.reviewer ?? null })];
    return Promise.resolve(aCardWrite("C-0002", { flow: "development", column: "todo" }));
  });
  return client;
}

function type(dialog: HTMLElement, label: string, value: string) {
  fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
}

async function fromModal(client: StubClient = handOffClient()): Promise<HTMLElement> {
  renderApp(client, "#/alpha/board/C-0001");
  const modal = await screen.findByRole("dialog", { name: "Lamp room dialogue" });
  fireEvent.click(await within(modal).findByRole("button", { name: "Hand off" }));
  return screen.findByRole("dialog", { name: "Hand off C-0001" });
}

describe("Hand off (BH-11)", () => {
  it("offers every served flow's disciplines, a hidden flow's too; title, goal and targets prefilled; no criteria, estimate, due date, evidence or assignee; the parent fixed", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const dialog = await fromModal();
    const discipline = within(dialog).getByLabelText<HTMLInputElement>("Discipline");
    const options = Array.from(document.getElementById(discipline.getAttribute("list") ?? "")?.querySelectorAll("option") ?? []).map((option) => option.value);
    expect(options).toEqual(["narrative", "development", "tools", "art"]);
    expect(discipline.getAttribute("aria-required")).toBe("true");
    expect(within(dialog).getByLabelText<HTMLInputElement>("Title").value).toBe("Lamp room dialogue");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Goal").value).toBe("Two endings, one lamp.");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source").value).toBe("SCN-LAMP\nSCN-ROOM");
    for (const absent of ["Criteria", "Estimate", "Due", "Assignee", "Kind", "Value"]) {
      expect([absent, within(dialog).queryByLabelText(absent)]).toEqual([absent, null]);
    }
    expect(within(dialog).getByText("C-0001").closest("dd")).not.toBeNull();
    const reviewer = within(dialog).getByLabelText<HTMLInputElement>("Reviewer");
    expect([reviewer.value, reviewer.readOnly]).toEqual(["Ann", false]);
  });

  it("submit is exactly one POST .../cards with title, goal, discipline, target, parent, reviewer and as, and no other write (M13); the child shows under Subtasks after the re-read", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient();
    const dialog = await fromModal(client);
    type(dialog, "Discipline", "development");
    type(dialog, "Reviewer", "Ann Lee");
    type(dialog, "Mandate", "open");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]).toEqual([
      "alpha",
      { title: "Lamp room dialogue", goal: "Two endings, one lamp.", discipline: "development", target: ["SCN-LAMP", "SCN-ROOM"], parent: "C-0001", reviewer: "Ann Lee", mandate: "open", as: "Ann" },
    ]);
    for (const other of [client.assignCard, client.moveCard, client.editCard, client.cancelCard, client.acceptCard, client.clearCard]) {
      expect(other).not.toHaveBeenCalled();
    }
    const modal = screen.getByRole("dialog", { name: "Lamp room dialogue" });
    await waitFor(() => {
      // Relies on and its row (docs/features/handoff-progress.md "Acceptance criteria", HP-5): the children read, read again after the write.
      expect(within(within(modal).getByRole("region", { name: "Subtasks" })).getAllByRole("link", { name: "C-0002" })).toHaveLength(2);
    });
    expect(screen.getAllByRole("dialog")).toEqual([modal]);
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("C-0002 handed off to development/todo");
    });
  });

  it("an empty acting name leaves the reviewer empty: the submit stops with the field's hint and makes no call", async () => {
    const client = handOffClient();
    const dialog = await fromModal(client);
    type(dialog, "Discipline", "development");
    const reviewer = within(dialog).getByLabelText<HTMLInputElement>("Reviewer");
    expect(reviewer.value).toBe("");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Name who checks the result: a person on the side that asks.");
    expect(reviewer.getAttribute("aria-invalid")).toBe("true");
    expect(within(dialog).getByText("Who checks the result: the side that asks.")).toBeTruthy();
    expect(client.createCard).not.toHaveBeenCalled();
  });

  it("no discipline: stops and says why, no call", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient();
    const dialog = await fromModal(client);
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("A hand-off needs a discipline: it picks the flow it goes to.");
    expect(client.createCard).not.toHaveBeenCalled();
  });

  it("a refusal keeps the dialog and the text, the daemon's words under lang=en (R9)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient();
    const { ClientError } = await import("../api/client");
    client.createCard.mockRejectedValueOnce(new ClientError({ status: 409, message: "target: no node `SCN-ROOM`; nothing changed" }));
    const dialog = await fromModal(client);
    type(dialog, "Discipline", "development");
    type(dialog, "Mandate", "binding");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    const said = await within(dialog).findByText("target: no node `SCN-ROOM`; nothing changed");
    expect(said.getAttribute("lang")).toBe("en");
    expect(within(dialog).getByLabelText<HTMLInputElement>("Discipline").value).toBe("development");
  });

  it("on a hub the reviewer is the login, read-only, and goes out as both reviewer and as", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient();
    client.getIdentity.mockResolvedValue({ login: "mira" });
    renderApp(client, "#/alpha/board/C-0001");
    const box = await screen.findByLabelText<HTMLInputElement>("Acting as");
    await waitFor(() => {
      expect(box.value).toBe("mira");
    });
    const modal = await screen.findByRole("dialog", { name: "Lamp room dialogue" });
    fireEvent.click(await within(modal).findByRole("button", { name: "Hand off" }));
    const dialog = await screen.findByRole("dialog", { name: "Hand off C-0001" });
    const reviewer = within(dialog).getByLabelText<HTMLInputElement>("Reviewer");
    expect([reviewer.value, reviewer.readOnly, reviewer.classList.contains("field-fixed")]).toEqual(["mira", true, true]);
    type(dialog, "Discipline", "art");
    type(dialog, "Mandate", "open");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ reviewer: "mira", mandate: "open", as: "mira" });
  });

  it("from `h` on a tile, a Russian layout's key too: the card's goal read for the form; never on a closed card", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient([
      aCardRow({ id: "C-0001", flow: "narrative", title: "Lamp room dialogue", targets: ["SCN-LAMP", "SCN-ROOM"], revision: "4" }),
      aCardRow({ id: "C-0003", flow: "narrative", column: "closed", accepted: true }),
    ]);
    renderApp(client, "#/alpha/board");
    const tile = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('[data-card="C-0001"]');
      if (found === null) {
        throw new Error("no tile");
      }
      return found;
    });
    tile.focus();
    fireEvent.keyDown(tile, { key: "\u0440", code: "KeyH" });
    const dialog = await screen.findByRole("dialog", { name: "Hand off C-0001" });
    await waitFor(() => {
      expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Goal").value).toBe("Two endings, one lamp.");
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(document.activeElement).toBe(tile);
    const closed = document.querySelector<HTMLElement>('[data-card="C-0003"]');
    closed?.focus();
    fireEvent.keyDown(closed ?? document.body, { key: "h" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("from the page as from the modal; a closed card's acts hold no Hand off", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient(undefined, { "C-0001": SOURCE, "C-0009": aCard({ id: "C-0009", title: "Closed one", acceptance: { by: "Ann", at: "2026-11-01T10:00:00Z", note: null, verdict: "briefed", departs: [], discrepancy: null } }) });
    renderApp(client, "#/alpha/board/C-0001/page");
    await screen.findByRole("heading", { level: 1, name: "Lamp room dialogue" });
    fireEvent.click(await screen.findByRole("button", { name: "Hand off" }));
    const dialog = await screen.findByRole("dialog", { name: "Hand off C-0001" });
    type(dialog, "Discipline", "development");
    type(dialog, "Mandate", "binding");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    window.location.hash = "#/alpha/board/C-0009/page";
    await screen.findByRole("heading", { level: 1, name: "Closed one" });
    expect(screen.queryByRole("button", { name: "Hand off" })).toBeNull();
  });
});

// DP-14 of docs/features/departments.md: with two or more departments serving a discipline, a
// required Department select, then a Discipline select defaulting to the department's first when it
// serves several; the write still sends `discipline` alone. Galley serves copy, Foundry plates and
// proofs, Yard nothing (not offered). The form is chosen once, as the dialog opens.

const DEPARTMENTS = aWorkspaces([
  aWorkspace({ name: "galley", title: "Galley", flows: ["copy"], disciplines: ["copy"] }),
  aWorkspace({ name: "foundry", title: "Foundry", flows: ["plates"], disciplines: ["plates", "proofs"] }),
  aWorkspace({ name: "yard", title: "Yard", flows: [], disciplines: [] }),
]);

function departmentClient(workspaces: Promise<WorkspaceDocument> = Promise.resolve(DEPARTMENTS)): StubClient {
  const client = handOffClient();
  client.getCards.mockImplementation(() =>
    Promise.resolve(
      aCardList([aCardRow({ id: "C-0001", flow: "copy", title: "Lamp room dialogue", targets: ["SCN-LAMP", "SCN-ROOM"], revision: "4" })], {
        flows: [aCardFlow({ name: "copy", title: "Copy desk", disciplines: ["copy"] }), aCardFlow({ name: "plates", disciplines: ["plates", "proofs"] })],
      }),
    ),
  );
  client.getWorkspaces.mockImplementation(() => workspaces);
  return client;
}

/** The modal's Hand off once the Shell holds the workspaces read: the header's Department switcher stands. */
async function fromModalRead(client: StubClient): Promise<HTMLElement> {
  renderApp(client, "#/alpha/board/C-0001");
  await screen.findByRole("combobox", { name: "Department" });
  const modal = await screen.findByRole("dialog", { name: "Lamp room dialogue" });
  fireEvent.click(await within(modal).findByRole("button", { name: "Hand off" }));
  return screen.findByRole("dialog", { name: "Hand off C-0001" });
}

function choose(dialog: HTMLElement, label: string, value: string) {
  fireEvent.change(within(dialog).getByRole("combobox", { name: label }), { target: { value } });
}

describe("Hand off by department (DP-14 of departments)", () => {
  it("one required Department select: Galley, Foundry, in served order, no Yard, no default; Submit without a choice sends nothing and says why", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = departmentClient();
    const dialog = await fromModalRead(client);
    const department = within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Department" });
    expect(Array.from(department.options).map((option) => [option.value, option.textContent])).toEqual([
      ["", "Choose a department"],
      ["galley", "Galley"],
      ["foundry", "Foundry"],
    ]);
    expect([department.value, department.getAttribute("aria-required")]).toEqual(["", "true"]);
    expect(within(dialog).getByText("The department that takes the subtask.")).toBeTruthy();
    expect(within(dialog).queryByLabelText("Discipline")).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("A hand-off needs a department: choose who takes the subtask.");
    expect(department.getAttribute("aria-invalid")).toBe("true");
    expect(client.createCard).not.toHaveBeenCalled();
  });

  it("Galley: no Discipline select; Submit posts its one discipline with parent, reviewer and as, and no new key (M: the department's name sent as discipline)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = departmentClient();
    const dialog = await fromModalRead(client);
    choose(dialog, "Department", "galley");
    expect(within(dialog).queryByLabelText("Discipline")).toBeNull();
    choose(dialog, "Mandate", "binding");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]).toEqual([
      "alpha",
      { title: "Lamp room dialogue", goal: "Two endings, one lamp.", discipline: "copy", target: ["SCN-LAMP", "SCN-ROOM"], parent: "C-0001", reviewer: "Ann", mandate: "binding", as: "Ann" },
    ]);
  });

  it("Foundry: a Discipline select showing plates, its first; choosing proofs posts it (M: the second select starting empty)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = departmentClient();
    const dialog = await fromModalRead(client);
    choose(dialog, "Department", "foundry");
    const discipline = within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Discipline" });
    expect([discipline.value, Array.from(discipline.options).map((option) => option.value)]).toEqual(["plates", ["plates", "proofs"]]);
    choose(dialog, "Discipline", "proofs");
    choose(dialog, "Mandate", "open");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ discipline: "proofs", parent: "C-0001" });
    // task-mandate's one key after the reviewer (docs/features/task-mandate.md "Acceptance criteria", V1).
    expect(Object.keys(client.createCard.mock.calls[0]?.[1] ?? {})).toEqual(["title", "goal", "discipline", "target", "parent", "reviewer", "mandate", "as"]);
  });

  it("Foundry, its first discipline untouched, posts plates", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = departmentClient();
    const dialog = await fromModalRead(client);
    choose(dialog, "Department", "foundry");
    choose(dialog, "Mandate", "binding");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ discipline: "plates" });
  });

  it("with Foundry alone serving disciplines: today's Discipline field and its datalist", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = departmentClient(Promise.resolve(aWorkspaces([aWorkspace({ name: "galley", title: "Galley", disciplines: [] }), aWorkspace({ name: "foundry", title: "Foundry", disciplines: ["plates", "proofs"] })])));
    const dialog = await fromModalRead(client);
    expect(within(dialog).queryByRole("combobox", { name: "Department" })).toBeNull();
    const discipline = within(dialog).getByLabelText<HTMLInputElement>("Discipline");
    expect(discipline.tagName).toBe("INPUT");
    expect(Array.from(document.getElementById(discipline.getAttribute("list") ?? "")?.querySelectorAll("option") ?? []).map((option) => option.value)).toEqual(["copy", "plates", "proofs"]);
  });

  it("with the read pending at open: today's field, kept when the read lands", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    let land: (document: WorkspaceDocument) => void = () => undefined;
    const client = departmentClient(
      new Promise((resolve) => {
        land = resolve;
      }),
    );
    const dialog = await fromModal(client);
    expect(within(dialog).getByLabelText("Discipline").tagName).toBe("INPUT");
    act(() => {
      land(DEPARTMENTS);
    });
    await waitFor(() => {
      expect(screen.getByRole("combobox", { name: "Department" })).toBeTruthy();
    });
    expect(within(dialog).queryByRole("combobox", { name: "Department" })).toBeNull();
    expect(within(dialog).getByLabelText("Discipline").tagName).toBe("INPUT");
    type(dialog, "Discipline", "proofs");
    type(dialog, "Mandate", "binding");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ discipline: "proofs" });
  });
});

// The orchestrator's looks for departments (L2): the daemon refuses `agent` as a reviewer, so the
// form never offers it. The reviewer opens as the parent card's reviewer; else the acting name,
// unless it names no person (`agent`, `local`) or is empty; else empty, for the person to fill.
describe("the hand-off's reviewer as the form opens (L2 of departments' looks)", () => {
  async function reviewerOf(acting: string | null, reviewer: string | null): Promise<{ dialog: HTMLElement; client: StubClient }> {
    if (acting !== null) {
      window.localStorage.setItem(ACTING_KEY, acting);
    }
    const client = handOffClient(undefined, { "C-0001": { ...SOURCE, reviewer } });
    const dialog = await fromModal(client);
    return { dialog, client };
  }

  it("takes the parent card's reviewer first, whatever the acting name; the write goes out as the acting name", async () => {
    const { dialog, client } = await reviewerOf("agent", "Ivo Brandt");
    expect(within(dialog).getByLabelText<HTMLInputElement>("Reviewer").value).toBe("Ivo Brandt");
    type(dialog, "Discipline", "development");
    type(dialog, "Mandate", "binding");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ reviewer: "Ivo Brandt", as: "agent" });
  });

  it("else the acting name when it names a person", async () => {
    const { dialog } = await reviewerOf("Ann", null);
    expect(within(dialog).getByLabelText<HTMLInputElement>("Reviewer").value).toBe("Ann");
  });

  it("never agent: with the acting name agent and no parent reviewer the field is empty and a submit stops before the daemon (M: the acting name taken as it is)", async () => {
    const { dialog, client } = await reviewerOf("agent", null);
    const reviewer = within(dialog).getByLabelText<HTMLInputElement>("Reviewer");
    expect(reviewer.value).toBe("");
    type(dialog, "Discipline", "development");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Name who checks the result: a person on the side that asks.");
    expect(client.createCard).not.toHaveBeenCalled();
  });

  it("never local, nor a blank name: the field is empty", async () => {
    expect(within((await reviewerOf("local", null)).dialog).getByLabelText<HTMLInputElement>("Reviewer").value).toBe("");
    cleanup();
    window.localStorage.clear();
    expect(within((await reviewerOf("   ", "")).dialog).getByLabelText<HTMLInputElement>("Reviewer").value).toBe("");
  });
});

// V1 of docs/features/task-mandate.md "Acceptance criteria": the hand-off names the subtask's mandate,
// required, no default: a select right after Reviewer at Choose…, then Binding and Department
// decides. Submitting with none says why in the dialog's alert and posts nothing; a choice goes out
// as `mandate`. On a hub alike, the Reviewer read-only.
describe("the hand-off's mandate (V1 of task-mandate)", () => {
  function fieldLabels(dialog: HTMLElement): string[] {
    return Array.from(dialog.querySelectorAll<HTMLLabelElement>("label.field-label"), (label) => label.textContent);
  }

  it("stands right after Reviewer at Choose…, required, Binding and Department decides after it", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const dialog = await fromModal();
    const labels = fieldLabels(dialog);
    expect(labels.slice(labels.indexOf("Reviewer"))).toEqual(["Reviewer", "Mandate"]);
    const mandate = within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Mandate" });
    expect([mandate.value, mandate.getAttribute("aria-required")]).toEqual(["", "true"]);
    expect(Array.from(mandate.options, (option) => [option.value, option.textContent])).toEqual([
      ["", "Choose…"],
      ["binding", "Binding"],
      ["open", "Department decides"],
    ]);
  });

  it("submitting with none says why in the alert and posts nothing; Department decides posts mandate open (M: a default of binding)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient();
    const dialog = await fromModal(client);
    type(dialog, "Discipline", "development");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Choose whether the subtask is binding or its department decides");
    const mandate = within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Mandate" });
    expect(mandate.getAttribute("aria-invalid")).toBe("true");
    expect(client.createCard).not.toHaveBeenCalled();
    type(dialog, "Mandate", "open");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ parent: "C-0001", reviewer: "Ann", mandate: "open" });
  });

  it("on a hub alike: the Reviewer the login, read-only; none chosen posts nothing; Binding posts it", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient();
    client.getIdentity.mockResolvedValue({ login: "mira" });
    renderApp(client, "#/alpha/board/C-0001");
    const box = await screen.findByLabelText<HTMLInputElement>("Acting as");
    await waitFor(() => {
      expect(box.value).toBe("mira");
    });
    const modal = await screen.findByRole("dialog", { name: "Lamp room dialogue" });
    fireEvent.click(await within(modal).findByRole("button", { name: "Hand off" }));
    const dialog = await screen.findByRole("dialog", { name: "Hand off C-0001" });
    expect(within(dialog).getByLabelText<HTMLInputElement>("Reviewer").readOnly).toBe(true);
    type(dialog, "Discipline", "art");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Choose whether the subtask is binding or its department decides");
    expect(client.createCard).not.toHaveBeenCalled();
    type(dialog, "Mandate", "binding");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ reviewer: "mira", mandate: "binding", as: "mira" });
  });

  it("New task has no mandate (V1)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    renderApp(handOffClient(), "#/alpha/board");
    fireEvent.click(await screen.findByRole("button", { name: "New task" }));
    const dialog = await screen.findByRole("dialog", { name: "New task" });
    expect(within(dialog).queryByLabelText("Mandate")).toBeNull();
  });
});

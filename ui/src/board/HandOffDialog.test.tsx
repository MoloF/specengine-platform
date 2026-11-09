import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Card, CardRow } from "../api/types";
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

  it("submit is exactly one POST .../cards with title, goal, discipline, target, parent, reviewer and as, and no other write (M13); the child shows under Hand-offs after the re-read", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = handOffClient();
    const dialog = await fromModal(client);
    type(dialog, "Discipline", "development");
    type(dialog, "Reviewer", "Ann Lee");
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]).toEqual([
      "alpha",
      { title: "Lamp room dialogue", goal: "Two endings, one lamp.", discipline: "development", target: ["SCN-LAMP", "SCN-ROOM"], parent: "C-0001", reviewer: "Ann Lee", as: "Ann" },
    ]);
    for (const other of [client.assignCard, client.moveCard, client.editCard, client.cancelCard, client.acceptCard, client.clearCard]) {
      expect(other).not.toHaveBeenCalled();
    }
    const modal = screen.getByRole("dialog", { name: "Lamp room dialogue" });
    await waitFor(() => {
      expect(within(within(modal).getByRole("region", { name: "Hand-offs" })).getByRole("link", { name: "C-0002" })).toBeTruthy();
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
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ reviewer: "mira", as: "mira" });
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
    fireEvent.click(within(dialog).getByRole("button", { name: "Hand off" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    window.location.hash = "#/alpha/board/C-0009/page";
    await screen.findByRole("heading", { level: 1, name: "Closed one" });
    expect(screen.queryByRole("button", { name: "Hand off" })).toBeNull();
  });
});

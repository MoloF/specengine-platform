import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Card } from "../api/types";
import { aCard, aCardFlow, aCardList, aCardRow } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { ACTING_KEY } from "./acting";

// V1 of docs/features/task-mandate.md "Acceptance criteria", Edit: only the card's reviewer changes its
// mandate. To that reviewer, the acting name typed here or a hub's login, the select, at the stored
// word, else at Choose…, with no way to clear one set; a changed word goes out as `mandate`, an
// unchanged one is no key. To another name, a line naming who changes it, when one is set; a card with
// no reviewer shows neither. The daemon judges the name either way, its refusal in its own words.

const PLATE: Card = aCard({ id: "C-0013", flow: "plates", title: "Tide table plate", targets: ["MEC-TIDE-TABLES"], reviewer: "Mira Holt", mandate: "binding", revision: "3" });
const NONE_SET: Card = aCard({ id: "C-0020", flow: "plates", title: "Buoy plate", reviewer: "Mira Holt", revision: "2" });
const NO_REVIEWER: Card = aCard({ id: "C-0021", flow: "plates", title: "Nobody's plate", revision: "1" });

function client(): StubClient {
  const stub = stubClient();
  const cards = [PLATE, NONE_SET, NO_REVIEWER];
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList(cards.map((card) => aCardRow({ id: card.id, flow: card.flow, title: card.title, reviewer: card.reviewer, mandate: card.mandate, revision: card.revision })), { flows: [aCardFlow({ name: "plates" })] })));
  stub.getCard.mockImplementation((_project, id) => {
    const card = cards.find((candidate) => candidate.id === id);
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  return stub;
}

async function edit(card: Card, stub: StubClient = client()): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${card.id}`);
  const modal = await screen.findByRole("dialog", { name: card.title });
  fireEvent.click(await within(modal).findByRole("button", { name: "Edit" }));
  return screen.findByRole("dialog", { name: `Edit ${card.id}` });
}

const REVIEWER_ONLY = "Only the reviewer, Mira Holt, changes the mandate.";

/** The line naming who changes the mandate, its name a slot of its own. */
function reviewerOnly(dialog: HTMLElement): HTMLElement | null {
  return Array.from(dialog.querySelectorAll<HTMLElement>(".card-field-hint")).find((line) => line.textContent === REVIEWER_ONLY) ?? null;
}

/** A live tail the test feeds: another writer's card event, one per call. */
function liveTail(stub: StubClient): (id: string) => void {
  let tail: Parameters<StubClient["subscribe"]>[1] = () => undefined;
  stub.subscribe = (_project, onEvent) => {
    tail = onEvent;
    return () => undefined;
  };
  return (id) => {
    act(() => {
      tail({ seq: 1, type: "card.edited", payload: { id, mandate: "binding", as: "Ben Ode" } });
    });
  };
}

describe("the mandate in Edit (V1 of task-mandate)", () => {
  it("Mira Holt, its reviewer, sees the select at Binding, no Choose… to clear it; a change posts mandate, an unchanged word no key", async () => {
    window.localStorage.setItem(ACTING_KEY, "Mira Holt");
    const stub = client();
    const dialog = await edit(PLATE, stub);
    const mandate = within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Mandate" });
    expect([mandate.value, Array.from(mandate.options, (option) => option.value)]).toEqual(["binding", ["binding", "open"]]);
    expect(reviewerOnly(dialog)).toBeNull();
    // Unchanged, with the title changed: no `mandate` key.
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Tide table plates" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(stub.editCard).toHaveBeenCalledTimes(1);
    });
    expect(stub.editCard.mock.calls[0]?.[2]).toEqual({ title: "Tide table plates", revision: "3", as: "Mira Holt" });
    const again = await edit(PLATE, stub);
    fireEvent.change(within(again).getByRole("combobox", { name: "Mandate" }), { target: { value: "open" } });
    fireEvent.click(within(again).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(stub.editCard).toHaveBeenCalledTimes(2);
    });
    expect(stub.editCard.mock.calls[1]?.[2]).toEqual({ mandate: "open", revision: "3", as: "Mira Holt" });
  });

  it("the reviewer of a card with none: the select at Choose…; Choose… sends nothing, a word sends it", async () => {
    window.localStorage.setItem(ACTING_KEY, "Mira Holt");
    const stub = client();
    const dialog = await edit(NONE_SET, stub);
    const mandate = within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Mandate" });
    expect([mandate.value, Array.from(mandate.options, (option) => option.textContent)]).toEqual(["", ["Choose…", "Binding", "Department decides"]]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Nothing to save: no field changed.");
    expect(stub.editCard).not.toHaveBeenCalled();
    fireEvent.change(mandate, { target: { value: "binding" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(stub.editCard).toHaveBeenCalledTimes(1);
    });
    expect(stub.editCard.mock.calls[0]?.[2]).toEqual({ mandate: "binding", revision: "2", as: "Mira Holt" });
  });

  it("Choose… stands by the value as Edit opened: a mandate another writer sets meanwhile leaves it, the select at it, and Save sends no mandate (M: Choose… by the card read live)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Mira Holt");
    const stub = client();
    const revise = liveTail(stub);
    const dialog = await edit(NONE_SET, stub);
    stub.getCard.mockImplementation((_project, id) => Promise.resolve({ ...NONE_SET, id, mandate: "binding", revision: "3" }));
    revise(NONE_SET.id);
    await within(dialog).findByText("This task changed since you opened it");
    const mandate = within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Mandate" });
    expect([mandate.value, Array.from(mandate.options, (option) => option.textContent)]).toEqual(["", ["Choose…", "Binding", "Department decides"]]);
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Buoy plates" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(stub.editCard).toHaveBeenCalledTimes(1);
    });
    expect(stub.editCard.mock.calls[0]?.[2]).toEqual({ title: "Buoy plates", revision: "3", as: "Mira Holt" });
  });

  it("another name sees the line naming the reviewer, no select, and posts no mandate; a card with no reviewer shows neither", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ben Ode");
    const stub = client();
    const dialog = await edit(PLATE, stub);
    expect(within(dialog).queryByRole("combobox", { name: "Mandate" })).toBeNull();
    expect(reviewerOnly(dialog)).not.toBeNull();
    expect(within(dialog).getByText("Binding")).toBeTruthy();
    // The line stands under the word, a block of its own, not run on after it (M: the line a span).
    expect([reviewerOnly(dialog)?.tagName, reviewerOnly(dialog)?.previousElementSibling?.textContent]).toEqual(["P", "Binding"]);
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Plate" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(stub.editCard).toHaveBeenCalledTimes(1);
    });
    expect(Object.keys(stub.editCard.mock.calls[0]?.[2] ?? {})).not.toContain("mandate");
    const unset = await edit(NONE_SET, stub);
    expect([within(unset).queryByRole("combobox", { name: "Mandate" }), within(unset).queryByText(/changes the mandate/)]).toEqual([null, null]);
    const nobody = await edit(NO_REVIEWER, stub);
    expect([within(nobody).queryByRole("combobox", { name: "Mandate" }), within(nobody).queryByText(/changes the mandate/), within(nobody).queryByText("Mandate")]).toEqual([null, null, null]);
  });

  it("the daemon's refusal stays in the dialog in its words, the choice kept", async () => {
    window.localStorage.setItem(ACTING_KEY, "Mira Holt");
    const stub = client();
    const line = "mandate: only C-0013's reviewer `Ana Ruiz` changes its mandate, not `Mira Holt`; nothing changed";
    stub.editCard.mockRejectedValueOnce(new ClientError({ status: 409, message: line }));
    const dialog = await edit(PLATE, stub);
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Mandate" }), { target: { value: "open" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    const said = await within(dialog).findByText(line);
    expect(said.getAttribute("lang")).toBe("en");
    expect(within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Mandate" }).value).toBe("open");
  });

  it("on a hub the login is the acting name: mira, not the card's reviewer, sees the line; Mira Holt's login the select", async () => {
    const stub = client();
    stub.getIdentity.mockResolvedValue({ login: "mira" });
    renderApp(stub, `#/alpha/board/${PLATE.id}`);
    const box = await screen.findByLabelText<HTMLInputElement>("Acting as");
    await waitFor(() => {
      expect(box.value).toBe("mira");
    });
    const modal = await screen.findByRole("dialog", { name: PLATE.title });
    fireEvent.click(await within(modal).findByRole("button", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit C-0013" });
    expect(within(dialog).queryByRole("combobox", { name: "Mandate" })).toBeNull();
    expect(reviewerOnly(dialog)).not.toBeNull();
  });

  it("on a hub whose login is the card's reviewer, Mira Holt: the select at Binding, and a change posts mandate as Mira Holt", async () => {
    const stub = client();
    stub.getIdentity.mockResolvedValue({ login: "Mira Holt" });
    renderApp(stub, `#/alpha/board/${PLATE.id}`);
    const box = await screen.findByLabelText<HTMLInputElement>("Acting as");
    await waitFor(() => {
      expect(box.value).toBe("Mira Holt");
    });
    const modal = await screen.findByRole("dialog", { name: PLATE.title });
    fireEvent.click(await within(modal).findByRole("button", { name: "Edit" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit C-0013" });
    const mandate = within(dialog).getByRole<HTMLSelectElement>("combobox", { name: "Mandate" });
    expect([mandate.value, Array.from(mandate.options, (option) => option.value)]).toEqual(["binding", ["binding", "open"]]);
    expect(reviewerOnly(dialog)).toBeNull();
    fireEvent.change(mandate, { target: { value: "open" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(stub.editCard).toHaveBeenCalledTimes(1);
    });
    expect(stub.editCard.mock.calls[0]?.[2]).toEqual({ mandate: "open", revision: "3", as: "Mira Holt" });
  });
});

import { act, fireEvent, renderHook, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import { HttpClient } from "../api/http";
import { ApiProvider, createQueryClient } from "../api/provider";
import { queryKeys } from "../api/queries";
import type { Card, CardList, Identity, Team } from "../api/types";
import { aCard, aCardList, aCardRow, aFlowNode, aFlowView, aProposal, aRoundItem, aRoundSheet, aTrace, aTraceCard, aTraceNode } from "../test/builders";
import { errorAnswer, jsonAnswer, stubFetch, urlsOf } from "../test/daemonStub";
import { flowClient } from "../test/flowStub";
import { renderApp } from "../test/render";
import { STAGED_AT, stubClient, type StubClient } from "../test/stubClient";
import { ACTING_KEY, useActing, useHubLogin } from "./acting";
import { ACT_OFF, NAME_IS_LOGIN, NAME_IS_TYPED } from "./labels";

// AC-12 of docs/features/hub-identity.md (docs/canon/hub.md "Screen"): a resolved login is the acting
// name, read-only, sent on every write, and `specengine.board.as` is not written. Null, loading or a
// failed read: the box as docs/features/board-ui.md says, the stored name sent, never an empty one.
// A9 of docs/features/hub-team.md at the end: "Accepting as" and "Clearing as" read-only with the
// login and its `name`, NAME_IS_TYPED gone, the controls following `allowed` and never a role word;
// a control `allowed` lists, or a read that said nothing, ends in the daemon's own answer.

const FORBIDDEN = "refused: mira is not in this project's `[team]`: reads and plain writes";

const CARD = aCard({ id: "C-0001", title: "The card", targets: ["R-1"], assignee: "Ivo Brandt", revision: "4" });

const LIST = aCardList([aCardRow({ id: "C-0001", revision: "4" })]);

/** `allowed` as the daemon sends it: a word apiece, no role. Everything this slice offers. */
const LEAD: Team = { login: "mira", name: "Mira K", allowed: ["stage", "accept", "clear", "group_clear"] };

/** The same login, less the group clear. */
const MEMBER: Team = { login: "mira", name: "Mira K", allowed: ["stage", "accept", "clear"] };

/** A login with everything a member has less the accept: a word apiece, so a control follows its own. */
const MEMBER_NO_ACCEPT: Team = { login: "mira", name: "Mira K", allowed: ["stage", "clear"] };

/** A login `[team]` does not list: 200, `name` null, `allowed` empty. */
const OUTSIDER: Team = { login: "mira", name: null, allowed: [] };

/** A Board's stub whose identity read answers `identity`, or as `identity` says when it is a function. */
function boardClient(identity: Identity | (() => Promise<Identity>), list: CardList = LIST, cards: Record<string, Card> = { "C-0001": CARD }): StubClient {
  const client = stubClient();
  client.getIdentity.mockImplementation(typeof identity === "function" ? identity : () => Promise.resolve(identity));
  client.getCards.mockImplementation(() => Promise.resolve(structuredClone(list)));
  client.getCard.mockImplementation((_project, id) => {
    const card = cards[id];
    return Promise.resolve(card === undefined ? { id, reason: `no card ${id}` } : structuredClone(card));
  });
  return client;
}

/** An identity read a test answers by hand, so the page can be seen with it still out. */
function heldIdentity() {
  let answer: (identity: Identity) => void = () => undefined;
  return {
    read: () =>
      new Promise<Identity>((resolve) => {
        answer = resolve;
      }),
    answer: (identity: Identity) => {
      answer(identity);
    },
  };
}

async function box(): Promise<HTMLInputElement> {
  return screen.findByLabelText<HTMLInputElement>("Acting as");
}

/** The box once the read has resolved to `login`. */
async function shows(login: string): Promise<HTMLInputElement> {
  const field = await box();
  await waitFor(() => {
    expect(field.value).toBe(login);
  });
  return field;
}

async function move(id: string) {
  const element = await waitFor(() => {
    const found = document.querySelector<HTMLElement>(`[data-card="${id}"]`);
    if (found === null) {
      throw new Error(`no card ${id}`);
    }
    return found;
  });
  element.focus();
  fireEvent.keyDown(element, { key: "m" });
  fireEvent.click(within(await screen.findByRole("menu", { name: `Move ${id}` })).getByRole("menuitem", { name: /^Doing/ }));
}

async function dialogOf(button: string, scope: HTMLElement = document.body): Promise<HTMLElement> {
  fireEvent.click(await within(scope).findByRole("button", { name: button }));
  return screen.findByRole("dialog");
}

function type(dialog: HTMLElement, label: string, value: string) {
  fireEvent.change(within(dialog).getByLabelText(label), { target: { value } });
}

function stored(): string | null {
  return window.localStorage.getItem(ACTING_KEY);
}

describe("a resolved login", () => {
  it("fills the box, read-only, over a stored name, which stays as it was", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    renderApp(boardClient({ login: "mira" }), "#/alpha/board");
    const field = await shows("mira");
    expect(field.readOnly).toBe(true);
    expect(field.classList.contains("field-fixed")).toBe(true);
    expect(stored()).toBe("Ana");
  });

  it("takes no name: the box keeps the login and nothing reaches the storage", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    renderApp(boardClient({ login: "mira" }), "#/alpha/board");
    const field = await shows("mira");
    fireEvent.change(field, { target: { value: "Someone Else" } });
    expect(field.value).toBe("mira");
    expect(stored()).toBe("Ana");
  });

  it("creates no key when none was stored", async () => {
    const client = boardClient({ login: "mira" });
    renderApp(client, "#/alpha/board");
    await shows("mira");
    await move("C-0001");
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(stored()).toBeNull();
  });

  it("is shown as sent, any script, inner spaces kept", async () => {
    const login = "\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430";
    const client = boardClient({ login });
    renderApp(client, "#/alpha/board");
    await shows(login);
    await move("C-0001");
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2].as).toBe(login);
  });

  describe("goes out as `as` on each of the five writes, over a stored name", () => {
    it("a move", async () => {
      window.localStorage.setItem(ACTING_KEY, "Ana");
      const client = boardClient({ login: "mira" });
      renderApp(client, "#/alpha/board");
      await shows("mira");
      await move("C-0001");
      await waitFor(() => {
        expect(client.moveCard).toHaveBeenCalledTimes(1);
      });
      expect(client.moveCard.mock.calls[0]).toEqual(["alpha", "C-0001", { to: "doing", revision: "4", as: "mira" }]);
      expect(stored()).toBe("Ana");
    });

    it("New card", async () => {
      window.localStorage.setItem(ACTING_KEY, "Ana");
      const client = boardClient({ login: "mira" });
      renderApp(client, "#/alpha/board");
      await shows("mira");
      const dialog = await dialogOf("New card");
      type(dialog, "Title", "A title");
      fireEvent.click(within(dialog).getByRole("button", { name: "Create card" }));
      await waitFor(() => {
        expect(client.createCard).toHaveBeenCalledTimes(1);
      });
      expect(client.createCard.mock.calls[0]?.[1].as).toBe("mira");
      expect(stored()).toBe("Ana");
      await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });

    it("Edit", async () => {
      window.localStorage.setItem(ACTING_KEY, "Ana");
      const client = boardClient({ login: "mira" });
      renderApp(client, "#/alpha/board/C-0001");
      await shows("mira");
      const dialog = await dialogOf("Edit");
      type(dialog, "Goal", "Another goal");
      fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
      await waitFor(() => {
        expect(client.editCard).toHaveBeenCalledTimes(1);
      });
      expect(client.editCard.mock.calls[0]?.[2].as).toBe("mira");
      expect(stored()).toBe("Ana");
    });

    it("Assign", async () => {
      window.localStorage.setItem(ACTING_KEY, "Ana");
      const client = boardClient({ login: "mira" });
      renderApp(client, "#/alpha/board/C-0001");
      await shows("mira");
      const dialog = await dialogOf("Assign");
      type(dialog, "Reviewer", "Lena Fisk");
      fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
      await waitFor(() => {
        expect(client.assignCard).toHaveBeenCalledTimes(1);
      });
      expect(client.assignCard.mock.calls[0]?.[2].as).toBe("mira");
      expect(stored()).toBe("Ana");
    });

    it("Cancel card", async () => {
      window.localStorage.setItem(ACTING_KEY, "Ana");
      const client = boardClient({ login: "mira" });
      renderApp(client, "#/alpha/board/C-0001");
      await shows("mira");
      const dialog = await dialogOf("Cancel card");
      type(dialog, "Reason", "Not needed");
      fireEvent.click(within(dialog).getByRole("button", { name: "Cancel card" }));
      await waitFor(() => {
        expect(client.cancelCard).toHaveBeenCalledTimes(1);
      });
      expect(client.cancelCard.mock.calls[0]?.[2].as).toBe("mira");
      expect(stored()).toBe("Ana");
    });
  });

  it("goes out from a node's Clear all too, where no Board stands", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const client = flowClient(aFlowView([aFlowNode({ id: "A", distance: 0 })], []));
    client.getIdentity.mockResolvedValue({ login: "mira" });
    client.getTrace.mockImplementation(() =>
      Promise.resolve(aTrace([aTraceNode("A", [aTraceCard({ id: "C-0001", mark: "suspect", on: "A" })], { rollup: "suspect" }), aTraceNode("B", [aTraceCard({ id: "C-0002", mark: "suspect", on: "B" })], { rollup: "suspect" })])),
    );
    client.clearCardGroup.mockRejectedValue(new ClientError({ status: 403, message: FORBIDDEN }));
    renderApp(client, "#/alpha/flow/A");
    const block = await screen.findByRole("region", { name: "Cards" });
    const dialog = await dialogOf("Clear all", block);
    const field = within(dialog).getByLabelText<HTMLInputElement>("Clearing as");
    await waitFor(() => {
      expect(field.value).toBe("mira");
    });
    type(dialog, "Reason", "Numbering only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(FORBIDDEN);
    expect(client.clearCardGroup.mock.calls).toEqual([["alpha", { group: "A", reason: "Numbering only", as: "mira", count: 2 }]]);
    expect(stored()).toBe("Ana");
  });

  it("is read once: a field that opens later reads nothing, whichever field it is", async () => {
    const client = boardClient({ login: "mira" });
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    fireEvent.click(within(await dialogOf("Edit")).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    await dialogOf("Assign");
    expect(client.getIdentity).toHaveBeenCalledTimes(1);
  });
});

describe("on a hub a control `allowed` lists still ends in the daemon's answer", () => {
  it("Accept: the button stands, the login goes out, the refusal keeps the dialog in the daemon's words", async () => {
    const client = boardClient({ login: "mira" }, aCardList([aCardRow({ id: "C-0001", column: "check", revision: "4" })]), { "C-0001": { ...CARD, column: "check" } });
    client.getTeam.mockResolvedValue(LEAD);
    client.acceptCard.mockRejectedValue(new ClientError({ status: 403, message: FORBIDDEN }));
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    const dialog = await dialogOf("Accept");
    expect(within(dialog).getByLabelText<HTMLInputElement>("Accepting as").value).toBe("mira");
    fireEvent.click(within(dialog).getByRole("button", { name: "Accept card" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(FORBIDDEN);
    expect(client.acceptCard.mock.calls[0]?.[2].as).toBe("mira");
    expect(screen.getByRole("dialog")).toBe(dialog);
  });

  it("Clear: the same", async () => {
    const client = boardClient({ login: "mira" }, aCardList([aCardRow({ id: "C-0001", revision: "4", targets: ["R-1"], mark: "suspect", on: "R-1" })]));
    client.getTrace.mockResolvedValue(aTrace([aTraceNode("R-1", [aTraceCard({ id: "C-0001", mark: "suspect", on: "R-1" })], { rollup: "suspect" })]));
    client.getTeam.mockResolvedValue(LEAD);
    client.clearCard.mockRejectedValue(new ClientError({ status: 403, message: FORBIDDEN }));
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    const dialog = await dialogOf("Clear");
    type(dialog, "Reason", "Wording only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear card" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(FORBIDDEN);
    expect(client.clearCard.mock.calls[0]?.[2].as).toBe("mira");
    expect(screen.getByRole("dialog")).toBe(dialog);
  });

  it("a refused write says the daemon's line and leaves the box alone", async () => {
    const line = "refused: `as` names Ana; this request's login is mira";
    const client = boardClient({ login: "mira" });
    client.moveCard.mockRejectedValue(new ClientError({ status: 403, message: line }));
    renderApp(client, "#/alpha/board");
    const field = await shows("mira");
    await move("C-0001");
    expect((await screen.findByRole("alert")).textContent).toContain(line);
    expect(field.value).toBe("mira");
    expect(field.readOnly).toBe(true);
  });
});

describe("a local daemon, login null", () => {
  it("the box as today: the stored name shown, writable, typing kept, the typed name sent", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const client = boardClient({ login: null });
    renderApp(client, "#/alpha/board");
    const field = await box();
    await waitFor(() => {
      expect(client.getIdentity).toHaveBeenCalled();
    });
    await move("C-0001");
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(field.value).toBe("Ana");
    expect(field.readOnly).toBe(false);
    expect(client.moveCard.mock.calls[0]?.[2].as).toBe("Ana");
    fireEvent.change(field, { target: { value: " Ivo Brandt " } });
    expect(field.value).toBe(" Ivo Brandt ");
    expect(stored()).toBe(" Ivo Brandt ");
  });

  it("no name kept: null goes out, which the daemon records `local`", async () => {
    const client = boardClient({ login: null });
    renderApp(client, "#/alpha/board");
    await move("C-0001");
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2].as).toBeNull();
  });
});

describe("while the identity read is out", () => {
  it("the stored name stands, never empty, the box writable, and a write sends it", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const identity = heldIdentity();
    const client = boardClient(identity.read);
    renderApp(client, "#/alpha/board");
    const field = await box();
    await waitFor(() => {
      expect(client.getIdentity).toHaveBeenCalled();
    });
    expect(field.value).toBe("Ana");
    expect(field.readOnly).toBe(false);
    await move("C-0001");
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2].as).toBe("Ana");
    fireEvent.change(field, { target: { value: "Ivo Brandt" } });
    expect(stored()).toBe("Ivo Brandt");
  });

  it("no name kept: as today, null, with the box empty", async () => {
    const identity = heldIdentity();
    const client = boardClient(identity.read);
    renderApp(client, "#/alpha/board");
    expect((await box()).value).toBe("");
    await move("C-0001");
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2].as).toBeNull();
  });

  it("a resolved login then replaces the stored name in the box and in the next write, the key left", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const identity = heldIdentity();
    const client = boardClient(identity.read);
    renderApp(client, "#/alpha/board");
    const field = await shows("Ana");
    await waitFor(() => {
      expect(client.getIdentity).toHaveBeenCalled();
    });
    identity.answer({ login: "mira" });
    await shows("mira");
    expect(field.readOnly).toBe(true);
    await move("C-0001");
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2].as).toBe("mira");
    expect(stored()).toBe("Ana");
  });

  it("a dialog opened meanwhile sends the login once it has resolved, not the name it opened on", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const identity = heldIdentity();
    const client = boardClient(identity.read);
    renderApp(client, "#/alpha/board");
    await waitFor(() => {
      expect(client.getIdentity).toHaveBeenCalled();
    });
    const dialog = await dialogOf("New card");
    type(dialog, "Title", "A title");
    identity.answer({ login: "mira" });
    await shows("mira");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create card" }));
    await waitFor(() => {
      expect(client.createCard).toHaveBeenCalledTimes(1);
    });
    expect(client.createCard.mock.calls[0]?.[1].as).toBe("mira");
  });
});

describe("a failed identity read", () => {
  it.each([
    ["a daemon gone", 0, "GET /api/identity: no response from the daemon (Failed to fetch)"],
    ["an older daemon", 404, "unknown route: the routes are /api/projects and the web UI"],
    ["a refusing door", 403, "refused: this hub needs one Remote-User header from its proxy"],
    ["a daemon failing", 503, "spec index unavailable"],
  ])("%s: the box as today, nothing said about it, the stored name sent", async (_case, status, message) => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const client = boardClient(() => Promise.reject(new ClientError({ status, message })));
    renderApp(client, "#/alpha/board");
    const field = await box();
    await move("C-0001");
    await waitFor(() => {
      expect(client.moveCard).toHaveBeenCalledTimes(1);
    });
    expect(client.moveCard.mock.calls[0]?.[2].as).toBe("Ana");
    expect(field.value).toBe("Ana");
    expect(field.readOnly).toBe(false);
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.change(field, { target: { value: "Ivo Brandt" } });
    expect(stored()).toBe("Ivo Brandt");
  });

  it("is asked again by the next field to open, and a login then takes the box", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const client = boardClient({ login: "mira" });
    client.getIdentity.mockRejectedValueOnce(new ClientError({ status: 503, message: "spec index unavailable" }));
    renderApp(client, "#/alpha/board");
    const field = await box();
    await waitFor(() => {
      expect(client.getIdentity).toHaveBeenCalledTimes(1);
    });
    await dialogOf("New card");
    await shows("mira");
    expect(client.getIdentity).toHaveBeenCalledTimes(2);
    expect(field.readOnly).toBe(true);
    expect(stored()).toBe("Ana");
  });
});

/** The Board's stub with one card in the accept column, as `team` answers. */
function acceptClient(team: Team | ClientError): StubClient {
  const client = boardClient({ login: "mira" }, aCardList([aCardRow({ id: "C-0001", column: "check", revision: "4" })]), { "C-0001": { ...CARD, column: "check" } });
  answerTeam(client, team);
  return client;
}

/** The Board's stub with one suspect card, as `team` answers. */
function clearClient(team: Team | ClientError): StubClient {
  const client = boardClient({ login: "mira" }, aCardList([aCardRow({ id: "C-0001", revision: "4", targets: ["R-1"], mark: "suspect", on: "R-1" })]));
  client.getTrace.mockResolvedValue(aTrace([aTraceNode("R-1", [aTraceCard({ id: "C-0001", mark: "suspect", on: "R-1" })], { rollup: "suspect" })]));
  answerTeam(client, team);
  return client;
}

/** The Flow's stub with a group of two suspect cards, as `team` answers; login null: a local daemon's. */
function groupClient(team: Team | ClientError, login: string | null = "mira"): StubClient {
  const client = flowClient(aFlowView([aFlowNode({ id: "A", distance: 0 })], []));
  client.getIdentity.mockResolvedValue({ login });
  client.getTrace.mockImplementation(() =>
    Promise.resolve(aTrace([aTraceNode("A", [aTraceCard({ id: "C-0001", mark: "suspect", on: "A" })], { rollup: "suspect" }), aTraceNode("B", [aTraceCard({ id: "C-0002", mark: "suspect", on: "B" })], { rollup: "suspect" })])),
  );
  client.getCard.mockImplementation((_project, id) => Promise.resolve(aCard({ id, title: `The card ${id}`, targets: ["A"], revision: "7" })));
  answerTeam(client, team);
  return client;
}

function answerTeam(client: StubClient, team: Team | ClientError) {
  if (team instanceof ClientError) {
    client.getTeam.mockRejectedValue(team);
  } else {
    client.getTeam.mockResolvedValue(team);
  }
}

/** A team read a test answers by hand. */
function heldTeam() {
  let answer: (team: Team) => void = () => undefined;
  return {
    read: () =>
      new Promise<Team>((resolve) => {
        answer = resolve;
      }),
    answer: (team: Team) => {
      answer(team);
    },
  };
}

function idle(button: HTMLElement): boolean {
  return button.getAttribute("aria-disabled") === "true";
}

/** Whether `line` is said on the page. */
function says(line: string): boolean {
  return screen.queryByText(line) !== null;
}

/** The field's words beside it, as the field names them. */
function hintOf(field: HTMLElement): string | null {
  const id = field.getAttribute("aria-describedby");
  return id === null ? null : (document.getElementById(id)?.textContent ?? null);
}

/** A turn of the page, so what a click set off has had its chance to go out. */
async function tick() {
  await act(() => new Promise((settle) => setTimeout(settle, 0)));
}

async function teamRead(client: StubClient) {
  await waitFor(() => {
    expect(client.getTeam).toHaveBeenCalled();
  });
  await tick();
}

async function nodeBlock(client: StubClient): Promise<HTMLElement> {
  renderApp(client, "#/alpha/flow/A");
  const block = await screen.findByRole("region", { name: "Cards" });
  await within(block).findByRole("button", { name: "Clear all" });
  return block;
}

describe("a hub's dialogs (A9 of hub-team)", () => {
  it("Accepting as: read-only with the login and its name, no typed-name sentence, the hub's sentence in its place", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const client = acceptClient(LEAD);
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    await teamRead(client);
    const dialog = await dialogOf("Accept");
    const field = within(dialog).getByLabelText<HTMLInputElement>("Accepting as");
    expect([field.value, field.readOnly, field.getAttribute("aria-required")]).toEqual(["mira", true, "false"]);
    expect(field.classList.contains("field-fixed")).toBe(true);
    expect(hintOf(field)).toBe("Mira K");
    expect(says(NAME_IS_TYPED)).toBe(false);
    expect(says(NAME_IS_LOGIN)).toBe(true);
    fireEvent.change(field, { target: { value: "Someone Else" } });
    expect(field.value).toBe("mira");
    expect(stored()).toBe("Ana");
  });

  it("Clearing as, on a card: the same", async () => {
    const client = clearClient(LEAD);
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    await teamRead(client);
    const dialog = await dialogOf("Clear");
    const field = within(dialog).getByLabelText<HTMLInputElement>("Clearing as");
    expect([field.value, field.readOnly]).toEqual(["mira", true]);
    expect(hintOf(field)).toBe("Mira K");
    expect(says(NAME_IS_TYPED)).toBe(false);
    expect(says(NAME_IS_LOGIN)).toBe(true);
  });

  it("Clearing as, on a group: the same, where no Board stands", async () => {
    const block = await nodeBlock(groupClient(LEAD));
    const dialog = await dialogOf("Clear all", block);
    const field = within(dialog).getByLabelText<HTMLInputElement>("Clearing as");
    await waitFor(() => {
      expect(hintOf(field)).toBe("Mira K");
    });
    expect([field.value, field.readOnly]).toEqual(["mira", true]);
    expect(says(NAME_IS_TYPED)).toBe(false);
    expect(says(NAME_IS_LOGIN)).toBe(true);
  });

  it("from a node's Clear too, whichever dialog holds the name", async () => {
    const block = await nodeBlock(groupClient(LEAD));
    const dialog = await dialogOf("Clear C-0001", block);
    const field = within(dialog).getByLabelText<HTMLInputElement>("Clearing as");
    await waitFor(() => {
      expect(hintOf(field)).toBe("Mira K");
    });
    expect(field.readOnly).toBe(true);
    expect(says(NAME_IS_LOGIN)).toBe(true);
  });

  it("shows a name in any script as stored, isolated, an override mark written out", async () => {
    const name = `\u041c\u0438\u0440\u0430\u202eK`;
    const client = acceptClient({ ...LEAD, name });
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    await teamRead(client);
    const field = within(await dialogOf("Accept")).getByLabelText<HTMLInputElement>("Accepting as");
    expect(hintOf(field)).toBe("\u041c\u0438\u0440\u0430\\u{202e}K");
    expect(document.getElementById(field.getAttribute("aria-describedby") ?? "")?.querySelector("bdi")).not.toBeNull();
  });

  it.each([
    ["a name `[team]` leaves null", { ...LEAD, name: null }],
    ["a read that failed", new ClientError({ status: 503, message: "specengine.toml:9: team.mira: role: `boss` is none of owner, lead, member" })],
  ])("%s: the login alone, nothing beside it, and no line about the read", async (_case, team) => {
    const client = acceptClient(team);
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    await teamRead(client);
    const dialog = await dialogOf("Accept");
    const field = within(dialog).getByLabelText<HTMLInputElement>("Accepting as");
    expect([field.value, field.readOnly, field.getAttribute("aria-describedby")]).toEqual(["mira", true, null]);
    expect(says(NAME_IS_LOGIN)).toBe(true);
    expect(screen.queryByText(/specengine\.toml/)).toBeNull();
  });

  it("goes out with the card's revision on an accept and a clear", async () => {
    const accepting = acceptClient(LEAD);
    const first = renderApp(accepting, "#/alpha/board/C-0001");
    await shows("mira");
    fireEvent.click(within(await dialogOf("Accept")).getByRole("button", { name: "Accept card" }));
    await waitFor(() => {
      expect(accepting.acceptCard).toHaveBeenCalledTimes(1);
    });
    expect(accepting.acceptCard.mock.calls[0]?.[2]).toMatchObject({ as: "mira", revision: "4" });
    first.unmount();

    const clearing = clearClient(LEAD);
    renderApp(clearing, "#/alpha/board/C-0001");
    await shows("mira");
    const dialog = await dialogOf("Clear");
    type(dialog, "Reason", "Wording only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear card" }));
    await waitFor(() => {
      expect(clearing.clearCard).toHaveBeenCalledTimes(1);
    });
    expect(clearing.clearCard.mock.calls[0]?.[2]).toMatchObject({ as: "mira", revision: "4" });
  });

  it("sends the group count the dialog shows, the daemon's 409 kept in its words", async () => {
    const line = "`A` holds 3 suspect cards, not 2; nothing changed";
    const client = groupClient(LEAD);
    client.clearCardGroup.mockRejectedValue(new ClientError({ status: 409, message: line }));
    const block = await nodeBlock(client);
    const dialog = await dialogOf("Clear all", block);
    expect(dialog.textContent).toContain("2 suspect cards of A");
    type(dialog, "Reason", "Numbering only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(line);
    expect(client.clearCardGroup.mock.calls).toEqual([["alpha", { group: "A", reason: "Numbering only", as: "mira", count: 2 }]]);
    expect(screen.getByRole("dialog")).toBe(dialog);
  });

  it("counts as the daemon does and shows what it passed over: a departed row is left out, a card suspect on any row stays in, the 200's note kept in the block", async () => {
    const line = "C-0001 reads departed PR-0001: passed over; clear it alone once that is settled";
    const client = groupClient(LEAD);
    client.getTrace.mockImplementation(() =>
      Promise.resolve(
        aTrace([
          aTraceNode("A", [aTraceCard({ id: "C-0001", mark: "departed", on: "A", discrepancy: "PR-0001" }), aTraceCard({ id: "C-0002", mark: "suspect", on: "A" }), aTraceCard({ id: "C-0003", mark: "departed", on: "A", discrepancy: "PR-0002" })], { rollup: "suspect" }),
          aTraceNode("B", [aTraceCard({ id: "C-0003", mark: "suspect", on: "B" })], { rollup: "suspect" }),
        ]),
      ),
    );
    client.clearCardGroup.mockResolvedValue({ group: "A", cleared: ["C-0002", "C-0003"], notes: [line] });
    const block = await nodeBlock(client);
    const dialog = await dialogOf("Clear all", block);
    expect(dialog.textContent).toContain("2 suspect cards of A");
    expect(dialog.textContent).toContain("A card a correction holds, or one that reads departed, is passed over.");
    type(dialog, "Reason", "Numbering only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.clearCardGroup.mock.calls[0]?.[1].count).toBe(2);
    expect(within(within(block).getByRole("list", { name: "Notes from the daemon on the last write" })).getByText(line)).toBeTruthy();
  });

  it("while the login is out the dialog is the local one, and it turns read-only when the login lands", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const identity = heldIdentity();
    const client = boardClient(identity.read, aCardList([aCardRow({ id: "C-0001", column: "check", revision: "4" })]), { "C-0001": { ...CARD, column: "check" } });
    client.getTeam.mockResolvedValue(LEAD);
    renderApp(client, "#/alpha/board/C-0001");
    const dialog = await dialogOf("Accept");
    const field = within(dialog).getByLabelText<HTMLInputElement>("Accepting as");
    expect([field.value, field.readOnly]).toEqual(["Ana", false]);
    expect(says(NAME_IS_TYPED)).toBe(true);
    expect(client.getTeam).not.toHaveBeenCalled();
    identity.answer({ login: "mira" });
    await waitFor(() => {
      expect(within(dialog).getByLabelText<HTMLInputElement>("Accepting as").value).toBe("mira");
    });
    const landed = within(dialog).getByLabelText<HTMLInputElement>("Accepting as");
    expect(landed.readOnly).toBe(true);
    await waitFor(() => {
      expect(hintOf(landed)).toBe("Mira K");
    });
    expect(says(NAME_IS_TYPED)).toBe(false);
    expect(says(NAME_IS_LOGIN)).toBe(true);
  });
});

describe("a hub's controls follow `allowed` (A9 of hub-team)", () => {
  it("Clear all off when `allowed` omits `group_clear`; a card's Clear stays on, the group's line beside the button", async () => {
    const block = await nodeBlock(groupClient(MEMBER));
    const all = within(block).getByRole("button", { name: "Clear all" });
    await waitFor(() => {
      expect(idle(all)).toBe(true);
    });
    expect(says(ACT_OFF.group_clear)).toBe(true);
    expect(says(ACT_OFF.clear)).toBe(false);
    expect(idle(within(block).getByRole("button", { name: "Clear C-0001" }))).toBe(false);
    fireEvent.click(all);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement === all || document.activeElement === document.body).toBe(true);
  });

  it("Clear and Clear all off for a login `[team]` does not list, nothing opening", async () => {
    const client = groupClient(OUTSIDER);
    const block = await nodeBlock(client);
    const clear = within(block).getByRole("button", { name: "Clear C-0001" });
    await waitFor(() => {
      expect(idle(clear)).toBe(true);
    });
    expect(idle(within(block).getByRole("button", { name: "Clear all" }))).toBe(true);
    expect(says(ACT_OFF.clear)).toBe(true);
    fireEvent.click(clear);
    await tick();
    expect(client.getCard).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("branches on the word alone: `group_clear` with nothing else leaves Clear all on and Clear off", async () => {
    const block = await nodeBlock(groupClient({ ...LEAD, allowed: ["group_clear"] }));
    const clear = within(block).getByRole("button", { name: "Clear C-0001" });
    await waitFor(() => {
      expect(idle(clear)).toBe(true);
    });
    expect(idle(within(block).getByRole("button", { name: "Clear all" }))).toBe(false);
  });

  it("the panel's Accept off when `allowed` omits `accept`, the Move menu without it, Edit and the rest as ever", async () => {
    const client = acceptClient(MEMBER_NO_ACCEPT);
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    const actions = await screen.findByRole("group", { name: "Actions" });
    const accept = within(actions).getByRole("button", { name: "Accept" });
    await waitFor(() => {
      expect(idle(accept)).toBe(true);
    });
    expect(says(ACT_OFF.accept)).toBe(true);
    fireEvent.click(accept);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(idle(within(actions).getByRole("button", { name: "Edit" }))).toBe(false);
    const tile = document.querySelector<HTMLElement>('[data-card="C-0001"]');
    if (tile === null) {
      throw new Error("no tile");
    }
    tile.focus();
    fireEvent.keyDown(tile, { key: "m" });
    const menu = await screen.findByRole("menu", { name: "Move C-0001" });
    expect(within(menu).queryByRole("menuitem", { name: /^Accept/ })).toBeNull();
    expect(within(menu).queryByRole("menuitem", { name: /^To do/ })).not.toBeNull();
  });

  it("the Move menu keeps Accept when `allowed` lists it", async () => {
    const client = acceptClient(MEMBER);
    renderApp(client, "#/alpha/board");
    await shows("mira");
    await teamRead(client);
    const tile = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('[data-card="C-0001"]');
      if (found === null) {
        throw new Error("no tile");
      }
      return found;
    });
    tile.focus();
    fireEvent.keyDown(tile, { key: "m" });
    expect(within(await screen.findByRole("menu", { name: "Move C-0001" })).queryByRole("menuitem", { name: /^Accept/ })).not.toBeNull();
  });

  it("the panel's Clear off when `allowed` omits `clear`", async () => {
    const client = clearClient({ ...LEAD, allowed: ["stage", "accept"] });
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    const clear = await screen.findByRole("button", { name: "Clear" });
    await waitFor(() => {
      expect(idle(clear)).toBe(true);
    });
    expect(says(ACT_OFF.clear)).toBe(true);
    fireEvent.click(clear);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("a word this build does not know changes nothing: RM-0050's `confirm` beside the rest", async () => {
    const block = await nodeBlock(groupClient({ ...LEAD, allowed: ["confirm", ...LEAD.allowed] }));
    expect(idle(within(block).getByRole("button", { name: "Clear all" }))).toBe(false);
    expect(says(ACT_OFF.group_clear)).toBe(false);
  });

  it("reads the project's own row: an answer of one root says nothing of another", async () => {
    const client = groupClient(LEAD);
    client.getTeam.mockImplementation((project) => Promise.resolve(project === "alpha" ? OUTSIDER : LEAD));
    const block = await nodeBlock(client);
    await waitFor(() => {
      expect(idle(within(block).getByRole("button", { name: "Clear all" }))).toBe(true);
    });
    expect(client.getTeam.mock.calls.map((call) => call[0])).toEqual(["alpha"]);
  });

  it("is one read per project, whatever opens: a dialog reads it no more", async () => {
    const client = clearClient(LEAD);
    renderApp(client, "#/alpha/board/C-0001");
    await shows("mira");
    await teamRead(client);
    await dialogOf("Clear");
    await tick();
    expect(client.getTeam).toHaveBeenCalledTimes(1);
  });

  it("is read again on window focus, a commit raising no event: a control follows, either way", async () => {
    const client = groupClient(OUTSIDER);
    const block = await nodeBlock(client);
    const all = within(block).getByRole("button", { name: "Clear all" });
    await waitFor(() => {
      expect(idle(all)).toBe(true);
    });
    client.getTeam.mockResolvedValue(LEAD);
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => {
      expect(idle(all)).toBe(false);
    });
    expect(says(ACT_OFF.group_clear)).toBe(false);
  });

  it("a read failing again says no more than a read that failed: what it last said no longer holds", async () => {
    const client = groupClient(OUTSIDER);
    const block = await nodeBlock(client);
    const all = within(block).getByRole("button", { name: "Clear all" });
    await waitFor(() => {
      expect(idle(all)).toBe(true);
    });
    client.getTeam.mockRejectedValue(new ClientError({ status: 503, message: "spec index unavailable" }));
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => {
      expect(idle(all)).toBe(false);
    });
  });

  describe("nothing known, every control on and the route answering for itself", () => {
    it.each([
      ["a bad `[team]`", 503, "specengine.toml:9: team.mira: role: `boss` is none of owner, lead, member"],
      ["an older hub", 404, "no route /api/projects/alpha/team: the routes are /api/projects and the web UI"],
      ["a daemon gone", 0, "GET /api/projects/alpha/team: no response from the daemon (Failed to fetch)"],
    ])("%s: nothing said of the read, the group clear's refusal in the daemon's words", async (_case, status, message) => {
      const client = groupClient(new ClientError({ status, message }));
      client.clearCardGroup.mockRejectedValue(new ClientError({ status, message }));
      const block = await nodeBlock(client);
      await teamRead(client);
      expect(screen.queryByRole("alert")).toBeNull();
      expect(idle(within(block).getByRole("button", { name: "Clear all" }))).toBe(false);
      const dialog = await dialogOf("Clear all", block);
      type(dialog, "Reason", "Numbering only");
      fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
      expect((await within(dialog).findByRole("alert")).textContent).toContain(message);
    });

    it("while the read is out; then the answer turns it off", async () => {
      const team = heldTeam();
      const client = groupClient(LEAD);
      client.getTeam.mockImplementation(team.read);
      const block = await nodeBlock(client);
      await waitFor(() => {
        expect(client.getTeam).toHaveBeenCalled();
      });
      const all = within(block).getByRole("button", { name: "Clear all" });
      expect(idle(all)).toBe(false);
      team.answer(MEMBER);
      await waitFor(() => {
        expect(idle(all)).toBe(true);
      });
      expect(document.body.contains(all)).toBe(true);
    });
  });

  describe("the stage controls", () => {
    const OPEN = aProposal({ id: "PR-0001", summary: "An open one", updated_at: "2026-10-01T10:00:00Z" });
    const STAGED = aProposal({ id: "PR-0002", summary: "A staged one", updated_at: "2026-10-01T10:00:00Z", staged: { decision: "reject", reason: "No" }, staged_at: STAGED_AT });

    function inboxClient(team: Team | ClientError, proposal = OPEN): StubClient {
      const client = stubClient([proposal]);
      client.getIdentity.mockResolvedValue({ login: "mira" });
      answerTeam(client, team);
      return client;
    }

    async function bar(id: string): Promise<HTMLElement> {
      const decide = await screen.findByRole("group", { name: `Decide ${id}` });
      await waitFor(() => {
        expect(idle(within(decide).getByRole("button", { name: /^Needs clarification/ }))).toBe(false);
      });
      return decide;
    }

    it("Inbox: Accept and Reject off, what sends nothing still open, a click and a key staging nothing", async () => {
      const client = inboxClient(OUTSIDER);
      renderApp(client, "#/alpha/inbox");
      const decide = await bar("PR-0001");
      const accept = within(decide).getByRole("button", { name: /^Accept/ });
      await waitFor(() => {
        expect(idle(accept)).toBe(true);
      });
      expect(idle(within(decide).getByRole("button", { name: /^Reject/ }))).toBe(true);
      expect(says(ACT_OFF.stage)).toBe(true);
      fireEvent.click(accept);
      const list = await screen.findByRole("listbox");
      fireEvent.keyDown(within(list).getByRole("option"), { key: "a" });
      fireEvent.keyDown(within(list).getByRole("option"), { key: "r" });
      expect(screen.queryByRole("dialog")).toBeNull();
      await waitFor(() => {
        expect(document.querySelector('[aria-live="polite"]')?.textContent).toContain(ACT_OFF.stage);
      });
      fireEvent.keyDown(within(list).getByRole("option"), { key: "d" });
      expect(await screen.findByRole("dialog")).toBeTruthy();
      expect(client.stageDecision).not.toHaveBeenCalled();
    });

    it("Inbox: on when `allowed` lists `stage`, the stage going out", async () => {
      const client = inboxClient(MEMBER);
      renderApp(client, "#/alpha/inbox");
      const decide = await bar("PR-0001");
      await teamRead(client);
      expect(says(ACT_OFF.stage)).toBe(false);
      fireEvent.click(within(decide).getByRole("button", { name: /^Reject/ }));
      const dialog = await screen.findByRole("dialog");
      fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "Not this" } });
      fireEvent.click(within(dialog).getByRole("button", { name: "Stage reject" }));
      await waitFor(() => {
        expect(client.stageDecision).toHaveBeenCalledTimes(1);
      });
    });

    it("Inbox: Unstage off, the staged choice still shown with its terminal command", async () => {
      const client = inboxClient(OUTSIDER, STAGED);
      renderApp(client, "#/alpha/inbox");
      const unstage = await screen.findByRole("button", { name: "Unstage PR-0002" });
      await waitFor(() => {
        expect(idle(unstage)).toBe(true);
      });
      fireEvent.click(unstage);
      await tick();
      expect(client.unstageDecision).not.toHaveBeenCalled();
      expect(screen.getByText("spec reject PR-0002")).toBeTruthy();
    });

    it("a read that failed leaves the Inbox's decisions on: the route answers", async () => {
      const client = inboxClient(new ClientError({ status: 503, message: "spec index unavailable" }));
      renderApp(client, "#/alpha/inbox");
      const decide = await bar("PR-0001");
      await teamRead(client);
      expect(idle(within(decide).getByRole("button", { name: /^Accept/ }))).toBe(false);
      expect(says(ACT_OFF.stage)).toBe(false);
    });

    function roundClient(team: Team | ClientError, staged = false): StubClient {
      const client = stubClient();
      client.getIdentity.mockResolvedValue({ login: "mira" });
      client.state.round = aRoundSheet([
        aRoundItem({
          id: "PR-0003",
          options: [{ label: "Code to spec", effect: "Fix the code", price: "One test" }],
          ...(staged ? { staged: { decision: "reject" as const, reason: "No" }, staged_at: STAGED_AT } : {}),
        }),
      ]);
      answerTeam(client, team);
      return client;
    }

    it("Questions: the answers stand off, Stage off, nothing goes out", async () => {
      const client = roundClient(OUTSIDER);
      renderApp(client, "#/alpha/questions");
      const stage = await screen.findByRole("button", { name: "Stage 0 answers" });
      await waitFor(() => {
        expect(idle(stage)).toBe(true);
        expect(says(ACT_OFF.stage)).toBe(true);
      });
      const radio = screen.getByRole<HTMLInputElement>("radio", { name: /Code to spec/ });
      expect(radio.closest("fieldset")?.disabled).toBe(true);
      fireEvent.click(stage);
      await tick();
      expect(client.stageRoundAnswers).not.toHaveBeenCalled();
    });

    it("Questions: a draft made before the answer landed stays unsent, Stage off with it", async () => {
      const team = heldTeam();
      const client = roundClient(LEAD);
      client.getTeam.mockImplementation(team.read);
      renderApp(client, "#/alpha/questions");
      fireEvent.click(await screen.findByRole("radio", { name: /Code to spec/ }));
      await waitFor(() => {
        expect(client.getTeam).toHaveBeenCalled();
      });
      team.answer(OUTSIDER);
      const stage = await screen.findByRole("button", { name: "Stage 1 answer" });
      await waitFor(() => {
        expect(idle(stage)).toBe(true);
      });
      fireEvent.click(stage);
      await tick();
      expect(client.stageRoundAnswers).not.toHaveBeenCalled();
    });

    it("Questions: an answer begun before the answer landed leaves no field to go on writing in", async () => {
      const team = heldTeam();
      const client = roundClient(LEAD);
      client.getTeam.mockImplementation(team.read);
      renderApp(client, "#/alpha/questions");
      fireEvent.click(await screen.findByRole("radio", { name: /Reject with a reason/ }));
      expect(await screen.findByLabelText("Reason for rejecting PR-0003")).toBeTruthy();
      await waitFor(() => {
        expect(client.getTeam).toHaveBeenCalled();
      });
      team.answer(OUTSIDER);
      await waitFor(() => {
        expect(screen.queryByLabelText("Reason for rejecting PR-0003")).toBeNull();
      });
    });

    it("Questions: on when `allowed` lists `stage`, the answers going out", async () => {
      const client = roundClient(MEMBER);
      renderApp(client, "#/alpha/questions");
      fireEvent.click(await screen.findByRole("radio", { name: /Code to spec/ }));
      await teamRead(client);
      fireEvent.click(await screen.findByRole("button", { name: "Stage 1 answer" }));
      await waitFor(() => {
        expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
      });
    });

    it("Questions: Unstage off", async () => {
      const client = roundClient(OUTSIDER, true);
      renderApp(client, "#/alpha/questions");
      const unstage = await screen.findByRole("button", { name: "Unstage PR-0003" });
      await waitFor(() => {
        expect(idle(unstage)).toBe(true);
      });
      fireEvent.click(unstage);
      await tick();
      expect(client.unstageDecision).not.toHaveBeenCalled();
    });
  });
});

describe("a local daemon has no team (A7, A9 of hub-team)", () => {
  /** Every screen with a control `allowed` would drive. */
  const SCREENS = ["#/alpha/board/C-0001", "#/alpha/flow/A", "#/alpha/inbox", "#/alpha/questions"];

  function localClient(): StubClient {
    const client = clearClient(LEAD);
    client.getIdentity.mockResolvedValue({ login: null });
    client.getFlow.mockResolvedValue(aFlowView([aFlowNode({ id: "A", distance: 0 })], []));
    return client;
  }

  async function settled(client: StubClient) {
    await waitFor(() => {
      expect(client.getIdentity).toHaveBeenCalled();
    });
    await tick();
  }

  it.each(SCREENS)("%s asks no team route, a login being unknown", async (hash) => {
    const client = localClient();
    renderApp(client, hash);
    await settled(client);
    expect(client.getTeam).not.toHaveBeenCalled();
  });

  it.each(SCREENS)("%s asks none while the identity read is out", async (hash) => {
    const identity = heldIdentity();
    const client = localClient();
    client.getIdentity.mockImplementation(identity.read);
    renderApp(client, hash);
    await settled(client);
    expect(client.getTeam).not.toHaveBeenCalled();
  });

  it.each(SCREENS)("%s asks none when the identity read failed", async (hash) => {
    const client = localClient();
    client.getIdentity.mockRejectedValue(new ClientError({ status: 503, message: "spec index unavailable" }));
    renderApp(client, hash);
    await settled(client);
    expect(client.getTeam).not.toHaveBeenCalled();
  });

  it("Accept as today: the typed field, required, its hint, the typed-name sentence, no hub sentence", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const client = boardClient({ login: null }, aCardList([aCardRow({ id: "C-0001", column: "check", revision: "4" })]), { "C-0001": { ...CARD, column: "check" } });
    renderApp(client, "#/alpha/board/C-0001");
    const dialog = await dialogOf("Accept");
    const field = within(dialog).getByLabelText<HTMLInputElement>("Accepting as");
    expect([field.value, field.readOnly, field.getAttribute("aria-required")]).toEqual(["Ana", false, "true"]);
    expect(field.classList.contains("field-fixed")).toBe(false);
    expect(hintOf(field)).toBe("Its reviewer's name, never its assignee's.");
    expect(says(NAME_IS_TYPED)).toBe(true);
    expect(says(NAME_IS_LOGIN)).toBe(false);
    fireEvent.change(field, { target: { value: "Ivo Brandt" } });
    expect(stored()).toBe("Ivo Brandt");
  });

  it("Clear all as today: no count in the body, no line of `allowed`", async () => {
    const client = groupClient(LEAD, null);
    const block = await nodeBlock(client);
    const dialog = await dialogOf("Clear all", block);
    type(dialog, "Clearing as", "Ivo Brandt");
    type(dialog, "Reason", "Numbering only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    await waitFor(() => {
      expect(client.clearCardGroup).toHaveBeenCalledTimes(1);
    });
    expect(client.clearCardGroup.mock.calls).toEqual([["alpha", { group: "A", reason: "Numbering only", as: "Ivo Brandt" }]]);
    expect(client.getTeam).not.toHaveBeenCalled();
  });
});

describe("useActing's shape", () => {
  function wrapperOf(client: Parameters<typeof ApiProvider>[0]["client"], queryClient = createQueryClient()) {
    return function Wrapper({ children }: { children: ReactNode }) {
      return (
        <ApiProvider client={client} queryClient={queryClient}>
          {children}
        </ApiProvider>
      );
    };
  }

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("two slots at every stage: the stored name and its setter, then the login and a setter keeping nothing", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const identity = heldIdentity();
    const client = boardClient(identity.read);
    const { result } = renderHook(() => ({ acting: useActing(), login: useHubLogin() }), { wrapper: wrapperOf(client) });
    await waitFor(() => {
      expect(client.getIdentity).toHaveBeenCalled();
    });
    expect(result.current.acting[0]).toBe("Ana");
    expect(result.current.acting).toHaveLength(2);
    expect(result.current.login).toBeNull();
    identity.answer({ login: "mira" });
    await waitFor(() => {
      expect(result.current.login).toBe("mira");
    });
    expect(result.current.acting[0]).toBe("mira");
    expect(result.current.acting).toHaveLength(2);
    result.current.acting[1]("Someone Else");
    expect(stored()).toBe("Ana");
  });

  it.each([
    ["locally", null],
    ["on a hub", "mira"],
  ])("a setter that does not change between renders, %s", async (_case, login) => {
    const client = boardClient({ login });
    const queryClient = createQueryClient();
    const { result, rerender } = renderHook(() => useActing(), { wrapper: wrapperOf(client, queryClient) });
    await waitFor(() => {
      expect(queryClient.getQueryState(queryKeys.identity)?.status).toBe("success");
    });
    await waitFor(() => {
      expect(result.current[0]).toBe(login ?? "");
    });
    const set = result.current[1];
    rerender();
    expect(result.current[1]).toBe(set);
  });

  it.each([
    ["a 404 document naming no login", () => jsonAnswer(404, { reason: "not here" })],
    ["an older daemon's error body", () => errorAnswer(404, "unknown route")],
    ["a local daemon's answer", () => jsonAnswer(200, { login: null })],
  ])("over the daemon, %s: the stored name stands and stays writable", async (_case, answer) => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    stubFetch(answer);
    const queryClient = createQueryClient();
    const { result } = renderHook(() => useActing(), { wrapper: wrapperOf(new HttpClient(), queryClient) });
    await waitFor(() => {
      expect(queryClient.getQueryState(queryKeys.identity)?.fetchStatus).toBe("idle");
      expect(queryClient.getQueryState(queryKeys.identity)?.status).not.toBe("pending");
    });
    await act(() => new Promise((settle) => setTimeout(settle, 0)));
    expect(result.current[0]).toBe("Ana");
    act(() => {
      result.current[1]("Ana Weiss");
    });
    expect(stored()).toBe("Ana Weiss");
    expect(result.current[0]).toBe("Ana Weiss");
  });

  it("over the daemon, a hub's answer: the login, from the one path", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const fetched = stubFetch(() => jsonAnswer(200, { login: "mira" }));
    const { result } = renderHook(() => useActing(), { wrapper: wrapperOf(new HttpClient()) });
    await waitFor(() => {
      expect(result.current[0]).toBe("mira");
    });
    expect(urlsOf(fetched)).toEqual(["/api/identity"]);
  });
});

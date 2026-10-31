import { act, fireEvent, renderHook, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import { HttpClient } from "../api/http";
import { ApiProvider, createQueryClient } from "../api/provider";
import { queryKeys } from "../api/queries";
import type { Card, CardList, Identity } from "../api/types";
import { aCard, aCardList, aCardRow, aFlowNode, aFlowView, aTrace, aTraceCard, aTraceNode } from "../test/builders";
import { errorAnswer, jsonAnswer, stubFetch, urlsOf } from "../test/daemonStub";
import { flowClient } from "../test/flowStub";
import { renderApp } from "../test/render";
import { stubClient, type StubClient } from "../test/stubClient";
import { ACTING_KEY, useActing, useHubLogin } from "./acting";

// AC-12 of docs/features/hub-identity.md (docs/canon/hub.md "Screen"): a resolved login is the acting
// name, read-only, sent on every write, and `specengine.board.as` is not written. Null, loading or a
// failed read: the box as docs/features/board-ui.md says, the stored name sent, never an empty one.
// Accept, Clear and the rest stay on the page and show the daemon's 403: hiding them is step 9's.

const FORBIDDEN = "refused: this hub serves GET and five card writes: this route stays local";

const CARD = aCard({ id: "C-0001", title: "The card", targets: ["R-1"], assignee: "Ivo Brandt", revision: "4" });

const LIST = aCardList([aCardRow({ id: "C-0001", revision: "4" })]);

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

  it("goes out from a node's Clear all too, where no Board stands and the dialog alone asks", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ana");
    const client = flowClient(aFlowView([aFlowNode({ id: "A", distance: 0 })], []));
    client.getIdentity.mockResolvedValue({ login: "mira" });
    client.getTrace.mockImplementation(() =>
      Promise.resolve(aTrace([aTraceNode("A", [aTraceCard({ id: "C-0001", mark: "suspect", on: "A" })], { rollup: "suspect" }), aTraceNode("B", [aTraceCard({ id: "C-0002", mark: "suspect", on: "B" })], { rollup: "suspect" })])),
    );
    client.clearCardGroup.mockRejectedValue(new ClientError({ status: 403, message: FORBIDDEN }));
    renderApp(client, "#/alpha/flow/A");
    const block = await screen.findByRole("region", { name: "Cards" });
    expect(client.getIdentity).not.toHaveBeenCalled();
    const dialog = await dialogOf("Clear all", block);
    const field = within(dialog).getByLabelText<HTMLInputElement>("Clearing as");
    await waitFor(() => {
      expect(field.value).toBe("mira");
    });
    type(dialog, "Reason", "Numbering only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(FORBIDDEN);
    expect(client.clearCardGroup.mock.calls).toEqual([["alpha", { group: "A", reason: "Numbering only", as: "mira" }]]);
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

describe("on a hub the local actions stay on the page and show the daemon's 403", () => {
  it("Accept: the button stands, the login goes out, the refusal keeps the dialog in the daemon's words", async () => {
    const client = boardClient({ login: "mira" }, aCardList([aCardRow({ id: "C-0001", column: "check", revision: "4" })]), { "C-0001": { ...CARD, column: "check" } });
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

import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import { SYNC_EVERY_MS } from "../api/queries";
import type { HubSync } from "../api/types";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";
import { notPushedLine } from "./SyncLine";

// U1 of docs/features/hub-git.md on a stub client ("Status and the screen"): a hub with `--push`
// shows a line beside the project's view while `ahead` > 0, and the last sync's `error` beneath as
// `Sync: <error>`, the daemon's words as text, escaped as the Confirm dialog escapes them, never
// markup; `ahead` 0 with no error, a 404 (a hub without `--push`), a failed read and a local page
// show nothing, and a local page never asks. The read is asked once a login is known, again on
// window focus and every 30 s. Non-Latin and format characters are built from code points.

/** A sync document as the daemon sends it: all five keys, a fresh hub's by default. */
function aSync(fields: Partial<HubSync> = {}): HubSync {
  return { ahead: 0, behind: 0, state: "none", error: null, at: null, ...fields };
}

/** A stub of a hub whose sync read answers `sync`, or rejects with it. */
function hubClient(sync: HubSync | ClientError): StubClient {
  const client = stubClient();
  client.getIdentity.mockResolvedValue({ login: "mira" });
  if (sync instanceof ClientError) {
    client.getSync.mockRejectedValue(sync);
  } else {
    client.getSync.mockResolvedValue(sync);
  }
  return client;
}

function syncLine(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".sync-line");
}

async function tick() {
  await act(() => new Promise((settle) => setTimeout(settle, 0)));
}

/** The view is up and the sync read, when one is asked, has answered. */
async function settled(client: StubClient) {
  await screen.findByRole("heading", { level: 1, name: "Inbox" });
  await waitFor(() => {
    expect(client.getIdentity).toHaveBeenCalled();
  });
  await tick();
  await tick();
}

describe("the line on a hub with `--push`", () => {
  it("counts the commits not pushed: `2 commits not pushed`", async () => {
    const client = hubClient(aSync({ ahead: 2, state: "retrying" }));
    renderApp(client, "#/alpha/inbox");
    expect(await screen.findByText("2 commits not pushed")).toBeTruthy();
    expect(syncLine()?.textContent).toBe("2 commits not pushed");
    expect(argsOf(client.getSync)).toEqual([["alpha"]]);
  });

  it("says one commit in the singular", async () => {
    renderApp(hubClient(aSync({ ahead: 1, state: "ok" })), "#/alpha/inbox");
    expect(await screen.findByText("1 commit not pushed")).toBeTruthy();
    expect([1, 2, 12].map(notPushedLine)).toEqual(["1 commit not pushed", "2 commits not pushed", "12 commits not pushed"]);
  });

  it("shows an error as `Sync: ` and the daemon's words as text, escaped, no element made of them", async () => {
    const error = `<b>x</b>${String.fromCodePoint(0x202e)}`;
    renderApp(hubClient(aSync({ ahead: 0, state: "held", error })), "#/alpha/inbox");
    const line = await waitFor(() => {
      const found = syncLine();
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    const shown = line.querySelector(".sync-error");
    expect(shown?.textContent).toBe("Sync: <b>x</b>\\u{202e}");
    expect(line.querySelector("b")).toBeNull();
    expect(line.textContent).not.toContain(String.fromCodePoint(0x202e));
    // No count with `ahead` 0: the error stands alone.
    expect(line.textContent).not.toMatch(/not pushed/);
  });

  it("puts the error beneath the count, its line feeds kept, every format character written out", async () => {
    const error = `the remote refused \`main\`: pre-receive hook declined\nremote: frozen${String.fromCodePoint(0x200b)}`;
    renderApp(hubClient(aSync({ ahead: 3, state: "held", error })), "#/alpha/inbox");
    const count = await screen.findByText("3 commits not pushed");
    const line = syncLine();
    const shown = line?.querySelector(".sync-error");
    expect(shown?.textContent).toBe("Sync: the remote refused `main`: pre-receive hook declined\nremote: frozen\\u{200b}");
    expect(count.closest(".sync-part")?.nextElementSibling).toBe(shown);
  });

  it("shows a non-Latin error as sent, each line in its own direction", async () => {
    const words = "\u043e\u0442\u043a\u0430\u0437 \u0443\u0434\u0430\u043b\u0451\u043d\u043d\u043e\u0433\u043e \u0441\u0435\u0440\u0432\u0435\u0440\u0430";
    renderApp(hubClient(aSync({ ahead: null, behind: null, state: "retrying", error: `push to \`origin\` failed: ${words}` })), "#/alpha/inbox");
    const shown = await waitFor(() => {
      const found = syncLine()?.querySelector(".sync-error .agent-text");
      expect(found).toBeTruthy();
      return found as HTMLElement;
    });
    expect(shown.textContent).toBe(`Sync: push to \`origin\` failed: ${words}`);
    expect(shown.getAttribute("dir")).toBe("auto");
  });

  it("offers no control", async () => {
    renderApp(hubClient(aSync({ ahead: 2, error: "fetch from `origin` failed: no route" })), "#/alpha/inbox");
    await screen.findByText("2 commits not pushed");
    expect(syncLine()?.querySelectorAll("button, a, input, select, textarea, [tabindex]")).toHaveLength(0);
  });

  it("reads the route's own project", async () => {
    const client = hubClient(aSync({ ahead: 2 }));
    client.getSync.mockImplementation((project) => Promise.resolve(aSync({ ahead: project === "beta" ? 5 : 2 })));
    renderApp(client, "#/alpha/inbox");
    await screen.findByText("2 commits not pushed");
    window.location.hash = "#/beta/inbox";
    expect(await screen.findByText("5 commits not pushed")).toBeTruthy();
    expect(screen.queryByText("2 commits not pushed")).toBeNull();
    expect(argsOf(client.getSync).map(([project]) => project)).toContain("beta");
  });
});

describe("no line", () => {
  it.each([
    ["`ahead` 0 and no error", aSync({ ahead: 0, state: "ok", at: "2026-11-04T09:00:00Z" })],
    ["no tracking ref and no error", aSync({ ahead: null, behind: null, state: "none" })],
    ["an empty error", aSync({ ahead: 0, state: "ok", error: "" })],
    ["a hub without `--push`: the unknown route's 404", new ClientError({ status: 404, message: "no route /api/projects/alpha/sync: the routes are /api/projects and the web UI" })],
    ["a failed read, 503", new ClientError({ status: 503, message: "spec index unavailable" })],
    ["no answer, status 0", new ClientError({ status: 0, message: "GET /api/projects/alpha/sync: no response from the daemon (Failed to fetch)" })],
  ])("for %s", async (_case, sync) => {
    const client = hubClient(sync);
    renderApp(client, "#/alpha/inbox");
    await settled(client);
    await waitFor(() => {
      expect(client.getSync).toHaveBeenCalled();
    });
    await tick();
    expect(syncLine()).toBeNull();
    expect(screen.queryByText(/not pushed|^Sync: /)).toBeNull();
  });

  it("on a local page, which never asks", async () => {
    const client = stubClient();
    client.getSync.mockResolvedValue(aSync({ ahead: 2, error: "never shown" }));
    renderApp(client, "#/alpha/inbox");
    await settled(client);
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await tick();
    expect(client.getSync).not.toHaveBeenCalled();
    expect(syncLine()).toBeNull();
  });

  it("while the login is still unknown: nothing asked before it", async () => {
    let answer: (login: string) => void = () => undefined;
    const client = hubClient(aSync({ ahead: 2 }));
    client.getIdentity.mockImplementation(
      () =>
        new Promise((resolve) => {
          answer = (login) => {
            resolve({ login });
          };
        }),
    );
    renderApp(client, "#/alpha/inbox");
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    await tick();
    expect(client.getSync).not.toHaveBeenCalled();
    answer("mira");
    expect(await screen.findByText("2 commits not pushed")).toBeTruthy();
  });

  it("once a read fails after a line: what it last said no longer holds", async () => {
    const client = hubClient(aSync({ ahead: 2 }));
    renderApp(client, "#/alpha/inbox");
    await screen.findByText("2 commits not pushed");
    client.getSync.mockRejectedValue(new ClientError({ status: 0, message: "GET /api/projects/alpha/sync: no response from the daemon (Failed to fetch)" }));
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => {
      expect(syncLine()).toBeNull();
    });
  });
});

describe("when it is read", () => {
  it("reads again on window focus, the line following the answer", async () => {
    const client = hubClient(aSync({ ahead: 2 }));
    renderApp(client, "#/alpha/inbox");
    await screen.findByText("2 commits not pushed");
    expect(client.getSync).toHaveBeenCalledTimes(1);
    client.getSync.mockResolvedValue(aSync({ ahead: 0, state: "ok" }));
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => {
      expect(client.getSync).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(syncLine()).toBeNull();
    });
    client.getSync.mockResolvedValue(aSync({ ahead: 1, state: "retrying" }));
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(await screen.findByText("1 commit not pushed")).toBeTruthy();
  });

  it("reads again every 30 s, and after a 404 too: a hub restarted with `--push` shows its line", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const client = hubClient(new ClientError({ status: 404, message: "no route /api/projects/alpha/sync" }));
    renderApp(client, "#/alpha/inbox");
    await settled(client);
    await waitFor(() => {
      expect(client.getSync).toHaveBeenCalledTimes(1);
    });
    expect(SYNC_EVERY_MS).toBe(30_000);
    act(() => {
      vi.advanceTimersByTime(SYNC_EVERY_MS - 1);
    });
    await tick();
    expect(client.getSync).toHaveBeenCalledTimes(1);
    client.getSync.mockResolvedValue(aSync({ ahead: 4, state: "retrying" }));
    act(() => {
      vi.advanceTimersByTime(1);
    });
    await waitFor(() => {
      expect(client.getSync).toHaveBeenCalledTimes(2);
    });
    expect(await screen.findByText("4 commits not pushed")).toBeTruthy();
  });

  it("is one read however the view changes: a section's view asks no sync of its own", async () => {
    const client = hubClient(aSync({ ahead: 2 }));
    renderApp(client, "#/alpha/inbox");
    await screen.findByText("2 commits not pushed");
    fireEvent.click(screen.getByRole("link", { name: "Tasks" }));
    await screen.findByRole("heading", { level: 1, name: "Tasks" });
    expect(await screen.findByText("2 commits not pushed")).toBeTruthy();
    await tick();
    expect(client.getSync).toHaveBeenCalledTimes(1);
  });
});

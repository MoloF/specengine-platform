import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HttpClient } from "../api/http";
import type { Proposal } from "../api/types";
import { aProposal, entryOf } from "../test/builders";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch, urlsOf, type Answering } from "../test/daemonStub";
import { renderApp } from "../test/render";

// docs/features/hub-confirm.md over HttpClient, the daemon a stubbed `fetch` and `EventSource`: a
// local daemon's page asks no team and no confirm route; a hub's asks both, the POST's body exactly
// `{question, updated_at}`, the review document's commit shown; a refused review document shows its
// last note, an error body its message; a `fetch` the login door's redirect fails, status 0, says to
// read again (a drop after the commit is status 0 too), the read at dialog open first of all.

const AT = "2026-11-02T09:00:00Z";
const QUESTION = `staged ${AT}: spec approve PR-0007\napply PR-0007 to docs/spec/maps/hold.md on main in /work/alpha (applies, staged)? [y/N]`;
const PROPOSAL = "/api/projects/alpha/proposals/PR-0007";
const CONFIRM = `${PROPOSAL}/confirm`;

const STAGED: Proposal = aProposal({
  id: "PR-0007",
  diff: "@@ -1,1 +1,1 @@\n-The old rule.\n+The new rule.",
  staged: { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null, span_hash: null },
  staged_at: AT,
  updated_at: AT,
});

const TEAM = { login: "mira", name: "Mira K", allowed: ["stage", "accept", "clear", "group_clear", "confirm"] };

/** A hub (or, `login` null, a local daemon) serving `queue`; `confirming` answers the confirm path. */
function daemon(login: string | null, queue: { proposal: Proposal | null }, confirming: Answering) {
  return stubFetch((url, init) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/identity") {
      return jsonAnswer(200, { login });
    }
    if (url === "/api/projects/alpha/team" && login !== null) {
      return jsonAnswer(200, TEAM);
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: queue.proposal === null ? [] : [entryOf(queue.proposal)], notes: [] });
    }
    if (url === PROPOSAL && queue.proposal !== null) {
      return jsonAnswer(200, queue.proposal);
    }
    if (url === CONFIRM && login !== null) {
      return confirming(url, init);
    }
    return errorAnswer(404, `no route ${url}: the routes are /api/projects and the web UI`);
  });
}

function asked(): Response {
  return jsonAnswer(200, { id: "PR-0007", question: QUESTION, updated_at: AT });
}

async function opened(): Promise<HTMLElement> {
  fireEvent.click(await screen.findByRole("button", { name: "Confirm PR-0007" }));
  return screen.findByRole("dialog", { name: "Confirm PR-0007" });
}

function confirmIn(dialog: HTMLElement): HTMLElement {
  return within(dialog).getByRole("button", { name: /^Confirm(ing)? PR-/ });
}

async function ready(dialog: HTMLElement): Promise<HTMLElement> {
  await waitFor(() => {
    expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("false");
  });
  return confirmIn(dialog);
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("over the daemon", () => {
  it("a local daemon: no Confirm, and neither route asked", async () => {
    const fetched = daemon(null, { proposal: STAGED }, asked);
    renderApp(new HttpClient(), "#/alpha/inbox");
    await screen.findByRole("button", { name: "Unstage PR-0007" });
    await waitFor(() => {
      expect(urlsOf(fetched)).toContain("/api/identity");
    });
    expect(screen.queryByRole("button", { name: "Confirm PR-0007" })).toBeNull();
    expect(urlsOf(fetched).filter((url) => url.endsWith("/team") || url.endsWith("/confirm"))).toEqual([]);
  });

  it("a hub: the read, then one POST of exactly its two strings, then the commit", async () => {
    const queue: { proposal: Proposal | null } = { proposal: STAGED };
    const bodies: unknown[] = [];
    const fetched = daemon("mira", queue, (_url, init) => {
      if (init?.method !== "POST") {
        return asked();
      }
      bodies.push(typeof init.body === "string" ? JSON.parse(init.body) : null);
      const applied = { ...STAGED, status: "applied", staged: null, staged_at: null, applied_commit: "1a2b3c4" };
      queue.proposal = null;
      return jsonAnswer(200, applied);
    });
    renderApp(new HttpClient(), "#/alpha/inbox");
    const dialog = await opened();
    expect((await within(dialog).findByRole("figure", { name: "The question a terminal would ask" })).textContent).toBe(QUESTION);
    fireEvent.click(await ready(dialog));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(bodies).toEqual([{ question: QUESTION, updated_at: AT }]);
    expect(Object.keys(bodies[0] as object)).toEqual(["question", "updated_at"]);
    expect(urlsOf(fetched).filter((url) => url === CONFIRM)).toHaveLength(2);
    expect(document.querySelector(".decision-result")?.textContent).toContain("commit 1a2b3c4");
  });

  it("a refused review document shows its last note, the dialog open", async () => {
    const reason = "`PR-0007` changed since the question: its staged choice was replaced or removed; nothing changed";
    daemon("mira", { proposal: STAGED }, (_url, init) => (init?.method === "POST" ? jsonAnswer(409, { ...STAGED, notes: ["a first note", reason] }) : asked()));
    renderApp(new HttpClient(), "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    expect((await within(dialog).findByRole("alert")).querySelector(".verbatim")?.textContent).toBe(reason);
  });

  it("a refused read shows its last note at opening: a stage only a terminal confirms", async () => {
    const reason = "`PR-0007` carries `editorial: true`: `spec approve PR-0007` on a terminal confirms it; nothing changed";
    daemon("mira", { proposal: STAGED }, () => jsonAnswer(409, { ...STAGED, notes: [reason] }));
    renderApp(new HttpClient(), "#/alpha/inbox");
    const dialog = await opened();
    expect((await within(dialog).findByRole("alert")).querySelector(".verbatim")?.textContent).toBe(reason);
    expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("true");
  });

  it("a 403's error body shows as the daemon wrote it", async () => {
    const line = "refused: mira leads maps; docs/spec/brief.md lies outside them";
    daemon("mira", { proposal: STAGED }, () => errorAnswer(403, line));
    renderApp(new HttpClient(), "#/alpha/inbox");
    const dialog = await opened();
    expect((await within(dialog).findByRole("alert")).querySelector(".verbatim")?.textContent).toBe(line);
  });

  it("a one-factor session's read meets the login door: status 0, read again, and the POST never goes", async () => {
    const fetched = daemon("mira", { proposal: STAGED }, () => {
      throw new TypeError("Failed to fetch");
    });
    renderApp(new HttpClient(), "#/alpha/inbox");
    const dialog = await opened();
    const alert = await within(dialog).findByRole("alert");
    expect(alert.querySelector(".refusal-title")?.textContent).toBe("No answer: read again to see whether it was confirmed.");
    expect(alert.querySelector(".verbatim")?.textContent).toBe(`GET ${CONFIRM}: no response from the daemon (Failed to fetch)`);
    fireEvent.click(confirmIn(dialog));
    expect(fetched.mock.calls.filter(([, init]) => init?.method === "POST")).toEqual([]);
  });
});

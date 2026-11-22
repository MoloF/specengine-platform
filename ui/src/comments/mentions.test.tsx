import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import type { CommentDocument, QueueEvent } from "../api/types";
import { docPageHash, mentionsHash, navOf, parseHash } from "../app/routes";
import { ALL_SECTIONS } from "../app/routes";
import { ACTING_KEY } from "../board/acting";
import { forgetWanted, wantComment, wantedIn } from "./wanted";
import { aCard, aCardList, aCardRow, aComment } from "../test/builders";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";
import { enT } from "../test/text";

// docs/features/comments.md "UI", Mentions, and CM18: the nav's third entry, `#/<p>/mentions`, with a
// badge counting the rows of the first page whose `id@version` this browser has not seen; the view
// newest first, each row linking to what it is under, where the thread brings the comment into view;
// the rows shown kept seen in `localStorage` per project and login (`specengine.mentions.seen.<p>[.<login>]`),
// at most 100; locally the acting name's when it is a login, else every mention; a hub's login's,
// asked with no `login`. A `comment.*` event reads them again.

beforeAll(async () => {
  await import("./MentionsView");
  await import("./MentionsBadge");
  await import("./CommentsPart");
});

/** lev's two mentions: one on a task, edited once, one on a node. */
function mentions(): CommentDocument[] {
  return [
    aComment({ id: "CM-0012", on: { kind: "card", id: "C-0007" }, actor: "mira", body: "Rail at 1.2 m? @lev knows.", mentions: ["lev"], version: 2 }),
    aComment({ id: "CM-0020", on: { kind: "node", id: "MEC-RAIL" }, actor: "ivo", body: "@lev, the post too.", mentions: ["lev"], created_at: "2026-11-22T11:00:00Z" }),
  ];
}

function client(comments: CommentDocument[] = mentions()): StubClient {
  const stub = stubClient();
  stub.state.comments.push(...comments);
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: "C-0007", title: "Rail height" })])));
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === "C-0007" ? aCard({ id, title: "Rail height", targets: ["MEC-RAIL"] }) : { id, reason: `no card ${id}` }));
  return stub;
}

/** The nav's Mentions link. */
async function navLink(): Promise<HTMLElement> {
  const nav = await screen.findByRole("navigation", { name: "Sections" });
  return await within(nav).findByRole("link", { name: /^Mentions/ });
}

/** The badge's count as drawn, once it stands; null while none does. */
function badgeOf(link: HTMLElement): string | null {
  return link.querySelector(".nav-badge")?.getAttribute("data-unseen") ?? null;
}

describe("the route and the nav", () => {
  it("reads `#/<p>/mentions`, with the document modal over it, and nothing past it", () => {
    expect(parseHash("#/alpha/mentions")).toEqual({ type: "mentions", project: "alpha" });
    expect(parseHash("#/alpha/mentions/")).toEqual({ type: "mentions", project: "alpha" });
    expect(parseHash("#/alpha/mentions?doc=MEC-RAIL")).toEqual({ type: "mentions", project: "alpha", doc: "MEC-RAIL" });
    expect(parseHash("#/alpha/mentions/CM-0012")).toEqual({ type: "not_found" });
    expect(mentionsHash("a b")).toBe("#/a%20b/mentions");
  });

  it("lists Mentions third, after Activity, in every department", () => {
    expect(navOf("alpha", ALL_SECTIONS, enT).slice(0, 3).map(({ key, label, hash }) => [key, label, hash])).toEqual([
      ["overview", "Overview", "#/alpha"],
      ["feed", "Activity", "#/alpha/feed"],
      ["mentions", "Mentions", "#/alpha/mentions"],
    ]);
    expect(navOf("alpha", [], enT).map(({ key }) => key)).toEqual(["overview", "feed", "mentions"]);
  });
});

describe("the badge (CM18: unseen `id@version` rows)", () => {
  it("counts the first page's rows this browser has not seen, the acting name's mentions when it is a login", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    const stub = client();
    renderApp(stub, "#/alpha/inbox");
    const link = await navLink();
    await waitFor(() => {
      expect(badgeOf(link)).toBe("2");
    });
    expect(link.textContent).toContain("2 unseen");
    expect(argsOf(stub.getMentions)).toEqual([["alpha", { login: "lev" }]]);
  });

  it("goes when the view shows the rows, keeping their id@version; an edit is news again", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    const stub = client();
    let emit: (event: QueueEvent) => void = () => undefined;
    stub.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    // One of the two seen before, at its version.
    window.localStorage.setItem("specengine.mentions.seen.alpha.lev", JSON.stringify(["CM-0012@2"]));
    renderApp(stub, "#/alpha/mentions");
    const link = await navLink();
    const view = await screen.findByRole("region", { name: "Mentions" });
    await within(view).findByText("@lev, the post too.");
    // Newest first; the unseen one marked New for the view's life.
    const rows = Array.from(view.querySelectorAll(".mention"));
    expect(rows.map((row) => [row.querySelector(".comment-id")?.textContent, row.getAttribute("data-new")])).toEqual([
      ["CM-0020", "true"],
      ["CM-0012", null],
    ]);
    await waitFor(() => {
      expect(JSON.parse(window.localStorage.getItem("specengine.mentions.seen.alpha.lev") ?? "[]")).toEqual(["CM-0020@1", "CM-0012@2"]);
    });
    expect(badgeOf(link)).toBeNull();
    // mira edits CM-0012: version 3 is unseen, though its ID was.
    Object.assign(stub.state.comments[0] ?? {}, { version: 3, body: "Rail at 1.1 m? @lev knows." });
    act(() => {
      emit({ seq: 41, type: "comment.edited", payload: { id: "CM-0012", version: 3, as: "mira" } });
    });
    // The view shows it at once and keeps it seen: the badge stays off here; another page would count it.
    await within(view).findByText("Rail at 1.1 m? @lev knows.");
    await waitFor(() => {
      expect(JSON.parse(window.localStorage.getItem("specengine.mentions.seen.alpha.lev") ?? "[]")).toEqual(["CM-0020@1", "CM-0012@3"]);
    });
  });

  it("counts an edited row again on a page off the view", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    window.localStorage.setItem("specengine.mentions.seen.alpha.lev", JSON.stringify(["CM-0012@1", "CM-0020@1"]));
    renderApp(client(), "#/alpha/inbox");
    const link = await navLink();
    // CM-0012 is at version 2: unseen, though its ID was seen at version 1.
    await waitFor(() => {
      expect(badgeOf(link)).toBe("1");
    });
  });

  it("on a hub, asks the login's mentions with no `login` and keeps them seen under the login", async () => {
    const stub = client();
    stub.getIdentity.mockImplementation(() => Promise.resolve({ login: "lev" }));
    renderApp(stub, "#/alpha/mentions");
    const view = await screen.findByRole("region", { name: "Mentions" });
    await within(view).findByText("@lev, the post too.");
    // The badge's first page and the view's, each its own read (labels and keys apart: no shared pages).
    expect(argsOf(stub.getMentions)).toEqual([
      ["alpha", {}],
      ["alpha", {}],
    ]);
    expect(view.textContent).toContain("Comments naming lev.");
    await waitFor(() => {
      expect(window.localStorage.getItem("specengine.mentions.seen.alpha.lev")).not.toBeNull();
    });
  });

  it("locally, with no acting name that is a login, lists every mention, kept under the project alone", async () => {
    window.localStorage.setItem(ACTING_KEY, "Lev K");
    const stub = client();
    renderApp(stub, "#/alpha/mentions");
    const view = await screen.findByRole("region", { name: "Mentions" });
    await within(view).findByText("@lev, the post too.");
    expect(argsOf(stub.getMentions)).toEqual([
      ["alpha", {}],
      ["alpha", {}],
    ]);
    expect(view.textContent).toContain("Every comment naming a login");
    await waitFor(() => {
      expect(window.localStorage.getItem("specengine.mentions.seen.alpha")).not.toBeNull();
    });
  });
});

describe("the view's rows", () => {
  it("link to what each is under: a task's card on the Board, a node as a document", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    renderApp(client(), "#/alpha/mentions");
    const view = await screen.findByRole("region", { name: "Mentions" });
    await within(view).findByText("@lev, the post too.");
    expect(within(view).getByRole("link", { name: "C-0007" }).getAttribute("href")).toBe("#/alpha/board/C-0007");
    expect(within(view).getByRole("link", { name: "MEC-RAIL" }).getAttribute("href")).toBe(docPageHash("alpha", "MEC-RAIL"));
  });

  it("follows a row to its task, where the thread brings the comment into view and focuses it", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    renderApp(client(), "#/alpha/mentions");
    const view = await screen.findByRole("region", { name: "Mentions" });
    const link = await within(view).findByRole("link", { name: "C-0007" });
    fireEvent.click(link);
    // The row's own navigation, one route change, whether jsdom follows the anchor or not.
    if (window.location.hash !== "#/alpha/board/C-0007") {
      window.location.hash = "#/alpha/board/C-0007";
    }
    const dialog = await screen.findByRole("dialog", { name: "Rail height" });
    await waitFor(() => {
      expect(document.activeElement?.getAttribute("data-comment")).toBe("CM-0012");
    });
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it("says when there is none, with what brings one", async () => {
    renderApp(client([]), "#/alpha/mentions");
    const view = await screen.findByRole("region", { name: "Mentions" });
    expect((await within(view).findByText("No mentions")).tagName).toBe("H2");
    expect(view.textContent).toContain("A comment that names you as @login shows here.");
  });
});

describe("the badge's own read (no shared pages)", () => {
  it("reads one page for the badge on a comment.* event, however far the view paged back before it was left", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    const stub = client();
    let emit: (event: QueueEvent) => void = () => undefined;
    stub.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    stub.getMentions.mockImplementation((_project, options) =>
      Promise.resolve({
        login: "lev",
        comments: options.before === undefined ? [mentions()[1] as CommentDocument] : [mentions()[0] as CommentDocument],
        next_before: options.before === undefined ? "CM-0020" : null,
        notes: [],
      }),
    );
    renderApp(stub, "#/alpha/mentions");
    const view = await screen.findByRole("region", { name: "Mentions" });
    fireEvent.click(await within(view).findByRole("button", { name: "Load older" }));
    await within(view).findByText("Rail at 1.2 m? @lev knows.");
    window.location.hash = "#/alpha/inbox";
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    const before = stub.getMentions.mock.calls.length;
    act(() => {
      emit({ seq: 70, type: "comment.created", payload: { id: "CM-0030", on: "C-0007", as: "mira" } });
    });
    await waitFor(() => {
      expect(stub.getMentions.mock.calls.length).toBe(before + 1);
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(argsOf(stub.getMentions).slice(before)).toEqual([["alpha", { login: "lev" }]]);
  });
});

describe("a comment followed from Mentions (wanted.ts)", () => {
  it("outlives the row's own route change alone: the next one forgets it", () => {
    wantComment("alpha", "C-0007", "CM-0012");
    expect(wantedIn("alpha", "C-0007")).toBe("CM-0012");
    expect(wantedIn("alpha", "C-0008")).toBeNull();
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(wantedIn("alpha", "C-0007")).toBe("CM-0012");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(wantedIn("alpha", "C-0007")).toBeNull();
    forgetWanted();
  });
});

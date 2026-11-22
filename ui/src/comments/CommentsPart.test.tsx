import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { Card, CommentDocument, QueueEvent } from "../api/types";
import { ACTING_KEY } from "../board/acting";
import { switchLanguage } from "../i18n";
import { ru } from "../i18n/ru";
import { aCard, aCardList, aCardRow, aComment, aCommentList, aTaskEntry, aTaskPackage } from "../test/builders";
import { renderApp } from "../test/render";
import { sentence } from "../test/text";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";

// docs/features/comments.md "UI" and CM18, over the stub board: the Comments part in a task's modal
// (its page draws the same CardView). The thread oldest first with replies under their comment and a
// tombstone with no act; Load older by `next_before`; a body rendered only through src/markdown/; the
// composer's Preview and its one call under the acting name; Reply on a top-level comment; Edit on
// one's own with its version, a 409 keeping the draft and reading the thread again; Delete through a
// dialog, off with its line (`ActOff`) on a hub whose `allowed` lacks `delete_comment`, on for any
// comment locally; Turn into… prefilled with the task's targets, posted to the comment's door; a
// `comment.*` event reading the thread again; the part in Russian.

beforeAll(async () => {
  // The part's chunk and the raise dialogs' chunk, loaded once, so a test meets them drawn.
  await import("./CommentsPart");
  await import("../board/CardItems");
  await import("../markdown/Markdown");
});

const CARD: Card = aCard({ id: "C-0007", title: "Rail height", targets: ["MEC-RAIL", "MEC-POST"] });

/** mira's question, an agent's reply under it, a deleted remark, and lev's own remark. */
function thread(): CommentDocument[] {
  return [
    aComment({ id: "CM-0012", body: "Keep the rail at **1.2 m**? <img src=x onerror=steal(1)> @lev knows the code.", mentions: ["lev"], version: 2, updated_at: "2026-11-22T10:05:00Z" }),
    aComment({
      id: "CM-0013",
      reply_to: "CM-0012",
      actor: "lev",
      author: { type: "agent", role: "spec-writer", model: null, run: null, login: "lev" },
      body: "MEC-RAIL says 1.1 m.",
      created_at: "2026-11-22T10:07:00Z",
    }),
    aComment({ id: "CM-0014", actor: "mira", body: null, deleted: { by: "mira", at: "2026-11-22T10:09:00Z" } }),
    aComment({ id: "CM-0015", actor: "lev", author: { type: "human", role: null, model: null, run: null, login: "lev" }, body: "Lev's own remark.", created_at: "2026-11-22T10:10:00Z" }),
  ];
}

function client(comments: CommentDocument[] = thread()): StubClient {
  const stub = stubClient();
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: CARD.id, title: CARD.title, targets: CARD.targets })])));
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === CARD.id ? structuredClone(CARD) : { id, reason: `no card ${id}` }));
  stub.state.comments.push(...comments);
  return stub;
}

/** A hub page as `login`, its `[team]` row allowing `allowed`. */
function onHub(stub: StubClient, login: string, allowed: string[]): StubClient {
  stub.getIdentity.mockImplementation(() => Promise.resolve({ login }));
  stub.getTeam.mockImplementation(() => Promise.resolve({ login, name: null, allowed }));
  return stub;
}

/** The task's modal and its Comments part, once read. */
async function part(stub: StubClient): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${CARD.id}`);
  const dialog = await screen.findByRole("dialog", { name: CARD.title });
  await within(dialog).findByText("Lev's own remark.", { exact: false });
  return within(dialog).getByRole("region", { name: "Comments" });
}

/** One comment's article by its ID. */
function entry(scope: HTMLElement, id: string): HTMLElement {
  const found = scope.querySelector<HTMLElement>(`[data-comment="${id}"]`);
  if (found === null) {
    throw new Error(`no ${id}`);
  }
  return found;
}

/** An article's own acts group, not its replies'. */
function actsOf(article: HTMLElement): HTMLElement | undefined {
  return Array.from(article.children).find((child): child is HTMLElement => child instanceof HTMLElement && child.getAttribute("role") === "group");
}

/** An article's own acts, by their words. */
function acts(article: HTMLElement): string[] {
  const group = actsOf(article);
  return group === undefined ? [] : Array.from(group.querySelectorAll("button"), (button) => button.textContent);
}

/** One of an article's own acts. */
function act_(article: HTMLElement, name: string): HTMLElement {
  const group = actsOf(article);
  if (group === undefined) {
    throw new Error("no acts");
  }
  return within(group).getByRole("button", { name });
}

describe("the thread (CM18)", () => {
  it("lists oldest first, each reply under its comment, a deleted one by who and when with no act", async () => {
    const shown = await part(client());
    expect(within(shown).getByRole("heading", { level: 3 }).textContent).toBe("Comments · 4");
    const tops = Array.from(shown.querySelectorAll(".comments-thread > li > .comment"), (article) => article.getAttribute("data-comment"));
    expect(tops).toEqual(["CM-0012", "CM-0014", "CM-0015"]);
    const first = entry(shown, "CM-0012");
    expect(first.querySelector(".comments-replies [data-comment='CM-0013']")).not.toBeNull();
    expect(first.textContent).toContain("edited 2026-11-22 10:05 UTC, v2");
    // An agent's mark and its role; no Edit on an agent's comment, no Reply on a reply.
    const reply = entry(shown, "CM-0013");
    expect(within(reply).getByText("agent")).toBeTruthy();
    expect(within(reply).getByText("spec-writer").getAttribute("lang")).toBe("en");
    const tombstone = entry(shown, "CM-0014");
    expect(tombstone.textContent).toContain("Deleted by mira, 2026-11-22 10:09 UTC");
    expect(acts(tombstone)).toEqual([]);
    expect(within(tombstone).queryAllByRole("button")).toEqual([]);
  });

  it("renders a body only through src/markdown/: emphasis drawn, raw HTML shown as text, no element made of it", async () => {
    const shown = await part(client());
    const body = entry(shown, "CM-0012").querySelector(".comment-text");
    if (body === null) {
      throw new Error("no body");
    }
    await waitFor(() => {
      expect(body.querySelector(".markdown strong")?.textContent).toBe("1.2 m");
    });
    expect(body.querySelector("img")).toBeNull();
    expect(body.textContent).toContain("<img src=x onerror=steal(1)>");
  });

  it("pages back by next_before: Load older reads the page before, drawn above", async () => {
    const stub = client([]);
    stub.getComments.mockImplementation((_project, options) =>
      Promise.resolve(
        options.before === undefined
          ? aCommentList([aComment({ id: "CM-0030", body: "Newest page." })], { total: 2, next_before: "CM-0030" })
          : aCommentList([aComment({ id: "CM-0004", body: "Lev's own remark." })], { total: 2 }),
      ),
    );
    renderApp(stub, `#/alpha/board/${CARD.id}`);
    const dialog = await screen.findByRole("dialog", { name: CARD.title });
    await within(dialog).findByText("Newest page.");
    const shown = within(dialog).getByRole("region", { name: "Comments" });
    fireEvent.click(within(shown).getByRole("button", { name: "Load older" }));
    await within(shown).findByText("Lev's own remark.");
    expect(argsOf(stub.getComments)).toEqual([
      ["alpha", { on: "C-0007" }],
      ["alpha", { on: "C-0007", before: "CM-0030" }],
    ]);
    expect(Array.from(shown.querySelectorAll(".comment"), (article) => article.getAttribute("data-comment"))).toEqual(["CM-0004", "CM-0030"]);
    expect(within(shown).queryByRole("button", { name: "Load older" })).toBeNull();
  });

  it("reads the thread again on a comment.* event, its mentions with it (Live)", async () => {
    const stub = client();
    let emit: (event: QueueEvent) => void = () => undefined;
    stub.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    const shown = await part(stub);
    const reads = stub.getComments.mock.calls.length;
    const mentions = stub.getMentions.mock.calls.length;
    stub.state.comments.push(aComment({ id: "CM-0016", body: "A newer word." }));
    act(() => {
      emit({ seq: 40, type: "comment.created", payload: { id: "CM-0016", on: "C-0007", as: "mira" } });
    });
    await within(shown).findByText("A newer word.");
    expect(stub.getComments.mock.calls.length).toBe(reads + 1);
    expect(stub.getMentions.mock.calls.length).toBe(mentions + 1);
  });
});

describe("the composer", () => {
  it("previews through src/markdown/, refuses a blank one unsent, then posts once under the acting name", async () => {
    window.localStorage.setItem(ACTING_KEY, "mira");
    const stub = client();
    const shown = await part(stub);
    const send = within(shown).getByRole("button", { name: "Comment" });
    fireEvent.click(send);
    expect(within(shown).getByRole("alert").textContent).toBe("Write a comment first.");
    expect(stub.addComment).not.toHaveBeenCalled();
    fireEvent.change(within(shown).getByLabelText("New comment"), { target: { value: "Rail at **1.1 m**, @lev." } });
    fireEvent.click(within(shown).getByRole("button", { name: "Preview" }));
    await waitFor(() => {
      expect(shown.querySelector(".comment-preview .markdown strong")?.textContent).toBe("1.1 m");
    });
    expect(shown.textContent).toContain("Posted as mira");
    fireEvent.click(within(shown).getByRole("button", { name: "Write" }));
    fireEvent.click(send);
    await waitFor(() => {
      expect(argsOf(stub.addComment)).toEqual([["alpha", { on: "C-0007", body: "Rail at **1.1 m**, @lev.", reply_to: null, as: "mira" }]]);
    });
    await waitFor(() => {
      expect(within(shown).getByLabelText<HTMLTextAreaElement>("New comment").value).toBe("");
    });
  });

  it("keeps the text and says the daemon's words when a post is refused", async () => {
    const stub = client();
    stub.addComment.mockRejectedValue(new ClientError({ status: 400, message: "body: 8193 bytes; at most 8192" }));
    const shown = await part(stub);
    fireEvent.change(within(shown).getByLabelText("New comment"), { target: { value: "Too long, say." } });
    fireEvent.click(within(shown).getByRole("button", { name: "Comment" }));
    const refusal = await within(shown).findByRole("alert");
    expect(refusal.textContent).toContain("body: 8193 bytes; at most 8192");
    expect(within(shown).getByLabelText<HTMLTextAreaElement>("New comment").value).toBe("Too long, say.");
  });

  it("replies on a top-level comment alone, naming it", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    const stub = client();
    const shown = await part(stub);
    expect(acts(entry(shown, "CM-0013"))).not.toContain("Reply");
    fireEvent.click(act_(entry(shown, "CM-0012"), "Reply"));
    const field = await within(shown).findByLabelText("Reply to CM-0012");
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: "It stays at 1.1 m." } });
    const form = field.closest("form");
    if (form === null) {
      throw new Error("no form");
    }
    fireEvent.click(within(form).getByRole("button", { name: "Reply" }));
    await waitFor(() => {
      expect(argsOf(stub.addComment)).toEqual([["alpha", { on: "C-0007", body: "It stays at 1.1 m.", reply_to: "CM-0012", as: "lev" }]]);
    });
  });
});

describe("Edit, one's own (CM18: a 409 keeps the draft)", () => {
  it("stands on one's own comment alone: the acting name against the actor locally, never an agent's", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    const shown = await part(client());
    expect(acts(entry(shown, "CM-0015"))).toContain("Edit");
    expect(acts(entry(shown, "CM-0012"))).not.toContain("Edit");
    expect(acts(entry(shown, "CM-0013"))).not.toContain("Edit");
  });

  it("sends the version it opened with; a stale one's 409 keeps the draft, shows the version now stored above it, read only, and the next save goes on that version", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    const stub = client();
    const shown = await part(stub);
    // Changed meanwhile, in another tab: version 2 now.
    Object.assign(stub.state.comments[3] ?? {}, { version: 2, body: "Lev's own remark, revised." });
    const reads = stub.getComments.mock.calls.length;
    fireEvent.click(act_(entry(shown, "CM-0015"), "Edit"));
    const field = await within(shown).findByLabelText("Edit CM-0015");
    expect((field as HTMLTextAreaElement).value).toBe("Lev's own remark.");
    fireEvent.change(field, { target: { value: "My remark, sharper." } });
    fireEvent.click(within(entry(shown, "CM-0015")).getByRole("button", { name: "Save" }));
    const stale = await within(entry(shown, "CM-0015")).findByRole("alert");
    expect(stale.textContent).toContain("Changed since you opened it: the version now stored stands above your text, which is kept; Save goes on it.");
    expect(stale.textContent).toContain("CM-0015 is at version 2, not 1: nothing changed");
    expect(stub.getComments.mock.calls.length).toBeGreaterThan(reads);
    // The version a save would now overwrite, to read, above the kept draft; said once, by the alert alone.
    const stored = entry(shown, "CM-0015").querySelector(".comment-stored");
    expect(stored?.textContent).toContain("Now stored, version 2:");
    await waitFor(() => {
      expect(stored?.textContent).toContain("Lev's own remark, revised.");
    });
    expect(stored?.querySelector("textarea")).toBeNull();
    expect(document.querySelector('[aria-live="assertive"]')?.textContent).toBe("");
    expect(within(shown).getByLabelText<HTMLTextAreaElement>("Edit CM-0015").value).toBe("My remark, sharper.");
    fireEvent.click(within(entry(shown, "CM-0015")).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(argsOf(stub.editComment)).toEqual([
        ["alpha", "CM-0015", { body: "My remark, sharper.", version: 1, as: "lev" }],
        ["alpha", "CM-0015", { body: "My remark, sharper.", version: 2, as: "lev" }],
      ]);
    });
    await within(shown).findByText("My remark, sharper.");
    expect(entry(shown, "CM-0015").querySelector(".comment-stored")).toBeNull();
  });

  it("never goes on a version a live event brought in unseen: the save stays on the version it opened with", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    const stub = client();
    let emit: (event: QueueEvent) => void = () => undefined;
    stub.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    const shown = await part(stub);
    fireEvent.click(act_(entry(shown, "CM-0015"), "Edit"));
    const field = await within(shown).findByLabelText("Edit CM-0015");
    Object.assign(stub.state.comments[3] ?? {}, { version: 2, body: "Lev's own remark, revised." });
    act(() => {
      emit({ seq: 50, type: "comment.edited", payload: { id: "CM-0015", version: 2, as: "lev" } });
    });
    await waitFor(() => {
      expect(stub.getComments.mock.calls.length).toBeGreaterThan(1);
    });
    fireEvent.change(field, { target: { value: "Mine." } });
    fireEvent.click(within(entry(shown, "CM-0015")).getByRole("button", { name: "Save" }));
    await within(entry(shown, "CM-0015")).findByRole("alert");
    expect(argsOf(stub.editComment)).toEqual([["alpha", "CM-0015", { body: "Mine.", version: 1, as: "lev" }]]);
  });

  it("closes the edit of a comment deleted meanwhile and says the daemon's words, never as stale", async () => {
    window.localStorage.setItem(ACTING_KEY, "lev");
    const stub = client();
    stub.editComment.mockImplementation((_project, id) => {
      Object.assign(stub.state.comments[3] ?? {}, { body: null, deleted: { by: "mira", at: "2026-11-22T11:00:00Z" } });
      return Promise.reject(new ClientError({ status: 409, message: `${id} is deleted: nothing changed` }));
    });
    const shown = await part(stub);
    fireEvent.click(act_(entry(shown, "CM-0015"), "Edit"));
    fireEvent.change(await within(shown).findByLabelText("Edit CM-0015"), { target: { value: "Too late." } });
    fireEvent.click(within(entry(shown, "CM-0015")).getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(within(shown).queryByLabelText("Edit CM-0015")).toBeNull();
    });
    expect(document.querySelector('[aria-live="assertive"]')?.textContent).toBe("CM-0015 is deleted: nothing changed");
    await within(entry(shown, "CM-0015")).findByText(sentence(/^Deleted by mira, /));
    expect(shown.textContent).not.toContain("Changed since you opened it");
  });

  it("says another 409 as refused, in the daemon's words: a reply refused by its rule is never stale", async () => {
    const stub = client();
    stub.addComment.mockRejectedValue(new ClientError({ status: 409, message: "reply_to: CM-0013 is a reply: a reply names a top-level comment" }));
    const shown = await part(stub);
    fireEvent.click(act_(entry(shown, "CM-0012"), "Reply"));
    const field = await within(shown).findByLabelText("Reply to CM-0012");
    fireEvent.change(field, { target: { value: "Late reply." } });
    const form = field.closest("form");
    if (form === null) {
      throw new Error("no form");
    }
    fireEvent.click(within(form).getByRole("button", { name: "Reply" }));
    const refusal = await within(form).findByRole("alert");
    expect(refusal.textContent).toBe("The daemon did not take it.reply_to: CM-0013 is a reply: a reply names a top-level comment");
    expect((field as HTMLTextAreaElement).value).toBe("Late reply.");
  });
});

describe("Delete, through a dialog (CM18: ActOff where `allowed` lacks delete_comment)", () => {
  it("on a hub, stands on one's own and stays off on another's with its line when `allowed` lacks delete_comment", async () => {
    const shown = await part(onHub(client(), "lev", ["stage", "accept", "clear"]));
    await waitFor(() => {
      expect(act_(entry(shown, "CM-0012"), "Delete…").getAttribute("aria-disabled")).toBe("true");
    });
    const another = entry(shown, "CM-0012");
    expect(Array.from(another.children).some((child) => child.textContent === "Your login deletes only its own comments on this project.")).toBe(true);
    fireEvent.click(act_(another, "Delete…"));
    expect(screen.queryByRole("dialog", { name: "Delete CM-0012?" })).toBeNull();
    // Its own, and its own agent's (the same login): on, no line.
    for (const id of ["CM-0015", "CM-0013"]) {
      const own = entry(shown, id);
      expect([id, act_(own, "Delete…").getAttribute("aria-disabled")]).toEqual([id, "false"]);
    }
    expect(entry(shown, "CM-0015").textContent).not.toContain("Your login deletes only its own comments");
  });

  it("on a hub whose `allowed` lists delete_comment (an owner), stands on another's", async () => {
    const shown = await part(onHub(client(), "lev", ["stage", "accept", "clear", "group_clear", "confirm", "delete_comment"]));
    await waitFor(() => {
      expect(act_(entry(shown, "CM-0012"), "Delete…").getAttribute("aria-disabled")).toBe("false");
    });
    expect(shown.textContent).not.toContain("Your login deletes only its own comments");
  });

  it("locally deletes any after the dialog, which says every version's text is erased; Cancel deletes nothing", async () => {
    const stub = client();
    const shown = await part(stub);
    const trigger = act_(entry(shown, "CM-0012"), "Delete…");
    fireEvent.click(trigger);
    let dialog = await screen.findByRole("dialog", { name: "Delete CM-0012?" });
    expect(dialog.textContent).toContain("The text of every version of mira's comment is erased; the thread keeps who deleted it and when.");
    // Cancel takes the focus first: a stray Enter deletes nothing.
    expect(document.activeElement?.textContent).toBe("Cancel");
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(stub.deleteComment).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(document.activeElement).toBe(trigger);
    });
    fireEvent.click(trigger);
    dialog = await screen.findByRole("dialog", { name: "Delete CM-0012?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    await waitFor(() => {
      expect(argsOf(stub.deleteComment)).toEqual([["alpha", "CM-0012"]]);
    });
    await within(shown).findByText(sentence(/^Deleted by local, /));
    expect(screen.queryByRole("dialog", { name: "Delete CM-0012?" })).toBeNull();
  });

  it("keeps the dialog and says the daemon's words when the delete is refused", async () => {
    const stub = client();
    stub.deleteComment.mockRejectedValue(new ClientError({ status: 403, message: "refused: CM-0012 is mira's: only its author or an `owner` deletes it" }));
    const shown = await part(stub);
    fireEvent.click(act_(entry(shown, "CM-0012"), "Delete…"));
    const dialog = await screen.findByRole("dialog", { name: "Delete CM-0012?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain("refused: CM-0012 is mira's: only its author or an `owner` deletes it");
  });
});

describe("Turn into… (CM18: node_ids prefilled per target)", () => {
  it("asks a question from a task's comment: the body's text, the task's targets each checked, posted to the comment's door", async () => {
    window.localStorage.setItem(ACTING_KEY, "mira");
    const stub = client();
    const shown = await part(stub);
    fireEvent.click(act_(entry(shown, "CM-0015"), "Turn into…"));
    const menu = await screen.findByRole("menu", { name: "Turn CM-0015 into" });
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Ask a question…", "Record a decision…", "Report a discrepancy…"]);
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Ask a question…" }));
    const dialog = await screen.findByRole("dialog", { name: "Ask a question from CM-0015" });
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Question").value).toBe("Lev's own remark.");
    expect(within(dialog).getAllByRole("checkbox").map((box) => [box.parentElement?.textContent.trim(), (box as HTMLInputElement).checked])).toEqual([
      ["MEC-RAIL", true],
      ["MEC-POST", true],
    ]);
    fireEvent.change(within(dialog).getByLabelText("Working answer"), { target: { value: "1.1 m" } });
    fireEvent.change(within(dialog).getByLabelText("What another answer would cost"), { target: { value: "A rebuild" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask" }));
    await waitFor(() => {
      expect(argsOf(stub.raiseFromComment)).toEqual([
        ["alpha", "CM-0015", { kind: "question", node_ids: ["MEC-RAIL", "MEC-POST"], text: "Lev's own remark.", working_answer: "1.1 m", price_of_other: "A rebuild", as: "mira" }],
      ]);
    });
    expect(stub.raiseOnCard).not.toHaveBeenCalled();
    // The new item stands among the comment's items, an Inbox link with its state.
    const item = await within(entry(shown, "CM-0015")).findByRole("link", { name: "PR-0102" });
    expect(item.getAttribute("href")).toBe("#/alpha/inbox/PR-0102");
  });

  it("reports a discrepancy: summary, observed, documented and the gap required; no answer's keys sent", async () => {
    const stub = client();
    const shown = await part(stub);
    fireEvent.click(act_(entry(shown, "CM-0015"), "Turn into…"));
    fireEvent.click(within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Report a discrepancy…" }));
    const dialog = await screen.findByRole("dialog", { name: "Report a discrepancy from CM-0015" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Report" }));
    expect(within(dialog).getByRole("alert").textContent).toBe("Fill in the summary, what is observed, what is documented and the gap.");
    expect(stub.raiseFromComment).not.toHaveBeenCalled();
    fireEvent.change(within(dialog).getByLabelText("Observed in the code"), { target: { value: "The rail is 1.1 m" } });
    fireEvent.change(within(dialog).getByLabelText("Documented in the spec"), { target: { value: "MEC-RAIL says 1.2 m" } });
    fireEvent.change(within(dialog).getByLabelText("The gap"), { target: { value: "contradicts" } });
    fireEvent.click(within(dialog).getAllByRole("checkbox")[1] as HTMLElement);
    fireEvent.click(within(dialog).getByRole("button", { name: "Report" }));
    await waitFor(() => {
      expect(argsOf(stub.raiseFromComment)).toEqual([
        ["alpha", "CM-0015", { kind: "discrepancy", node_ids: ["MEC-RAIL"], text: "Lev's own remark.", gap_type: "contradicts", observed: "The rail is 1.1 m", documented: "MEC-RAIL says 1.2 m", as: null }],
      ]);
    });
  });

  it("prefills the text with the body's first 1 024 bytes, cut at a character boundary", async () => {
    // 600 two-byte letters: 1 200 bytes, so 512 letters fit.
    const long = "\u0436".repeat(600);
    const shown = await part(client([...thread(), aComment({ id: "CM-0020", body: long, created_at: "2026-11-22T11:00:00Z" })]));
    fireEvent.click(act_(entry(shown, "CM-0020"), "Turn into…"));
    fireEvent.click(within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Record a decision…" }));
    const dialog = await screen.findByRole("dialog", { name: "Record a decision from CM-0020" });
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("What had to be decided").value).toBe("\u0436".repeat(512));
  });

  it("frames a decision turned from a task's comment by the task's mandate, as the task's own dialog does", async () => {
    const stub = client();
    stub.getCard.mockImplementation((_project, id) => Promise.resolve({ ...structuredClone(CARD), id, mandate: "binding" as const }));
    const shown = await part(stub);
    fireEvent.click(act_(entry(shown, "CM-0015"), "Turn into…"));
    fireEvent.click(within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Record a decision…" }));
    const dialog = await screen.findByRole("dialog", { name: "Record a decision from CM-0015" });
    expect(dialog.textContent).toContain("This task is binding: a decision here departs from its brief, and its reviewer sees it as one.");
  });

  it("offers no Turn into… under a task with no target, nor on a deleted comment", async () => {
    const stub = client();
    stub.getCard.mockImplementation((_project, id) => Promise.resolve({ ...structuredClone(CARD), id, targets: [] }));
    const shown = await part(stub);
    expect(within(shown).queryAllByRole("button", { name: "Turn into…" })).toEqual([]);
  });
});

describe("what the live tail reads again (Live)", () => {
  it("reads every thread on proposal.created, the threads listing the item alone on its later events, none on a stage", async () => {
    const stub = client([...thread().slice(0, 3), aComment({ id: "CM-0015", actor: "lev", body: "Lev's own remark.", items: [{ id: "PR-0102", kind: "question", status: "open" }] })]);
    let emit: (event: QueueEvent) => void = () => undefined;
    stub.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    await part(stub);
    const reads = () => stub.getComments.mock.calls.length;
    const before = reads();
    act(() => {
      emit({ seq: 60, type: "proposal.approved", payload: { id: "PR-0200" } });
      emit({ seq: 61, type: "proposal.staged", payload: { id: "PR-0102", staged: null, staged_at: null } });
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(reads()).toBe(before);
    act(() => {
      emit({ seq: 62, type: "proposal.rejected", payload: { id: "PR-0102" } });
    });
    await waitFor(() => {
      expect(reads()).toBe(before + 1);
    });
    act(() => {
      emit({ seq: 63, type: "proposal.created", payload: { id: "PR-0300" } });
    });
    await waitFor(() => {
      expect(reads()).toBe(before + 2);
    });
  });
});

describe("a task's package on a comment's event (Live)", () => {
  it("reads the open task again: its package carries its cards' comments", async () => {
    const stub = client();
    stub.getTasks.mockResolvedValue({ tasks: [aTaskEntry({ id: "T-0001" })], notes: [] });
    stub.getTask.mockImplementation((_project, id) => Promise.resolve(aTaskPackage({ id })));
    let emit: (event: QueueEvent) => void = () => undefined;
    stub.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    renderApp(stub, "#/alpha/tasks/T-0001");
    await waitFor(() => {
      expect(stub.getTask).toHaveBeenCalled();
    });
    const reads = stub.getTask.mock.calls.length;
    act(() => {
      emit({ seq: 80, type: "comment.created", payload: { id: "CM-0031", on: "C-0007", as: "mira" } });
    });
    await waitFor(() => {
      expect(stub.getTask.mock.calls.length).toBe(reads + 1);
    });
  });
});

describe("the part in Russian", () => {
  it("words its chrome from ru.ts, the people's words as written", async () => {
    const shown = await part(client());
    act(() => {
      switchLanguage("ru");
    });
    expect(within(shown).getByRole("heading", { level: 3 }).textContent).toBe(ru.comments.titleCount_few.replace("{{count}}", "4"));
    expect(within(shown).getAllByRole("button", { name: ru.comments.reply }).length).toBeGreaterThan(0);
    expect(shown.textContent).toContain("Lev's own remark.");
  });
});

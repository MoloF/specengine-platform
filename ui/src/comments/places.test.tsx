import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { aComment, aNode, aProposal, entryOf } from "../test/builders";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";

// docs/features/comments.md "UI": the Comments part stands in the document view (modal and page) and
// in the proposal pane, open or closed, each reading the thread under what it shows, and Turn into…
// prefills `node_ids` as `docs/canon/comments.md` "Turning into an item" says: a node comment's node,
// a proposal comment's targets (a task comment's targets: CommentsPart.test.tsx). The part arrives in
// its own chunk; nothing else is read for it.

beforeAll(async () => {
  await import("./CommentsPart");
  await import("../board/CardItems");
  await import("../doc/DocBody");
});

/** The part under what a route shows, once its thread is drawn. */
async function partAt(stub: StubClient, hash: string, text: string): Promise<HTMLElement> {
  renderApp(stub, hash);
  await screen.findByText(text);
  return screen.getByRole("region", { name: "Comments" });
}

/** Turn a comment into a question, its text and working answer filled; what it posts. */
async function ask(shown: HTMLElement, id: string, stub: StubClient): Promise<unknown[][]> {
  const article = shown.querySelector<HTMLElement>(`[data-comment="${id}"]`);
  if (article === null) {
    throw new Error(`no ${id}`);
  }
  fireEvent.click(within(article).getByRole("button", { name: "Turn into…" }));
  fireEvent.click(within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Ask a question…" }));
  const dialog = await screen.findByRole("dialog", { name: `Ask a question from ${id}` });
  fireEvent.change(within(dialog).getByLabelText("Working answer"), { target: { value: "Yes" } });
  fireEvent.change(within(dialog).getByLabelText("What another answer would cost"), { target: { value: "A rework" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Ask" }));
  await waitFor(() => {
    expect(stub.raiseFromComment).toHaveBeenCalled();
  });
  return argsOf(stub.raiseFromComment);
}

describe("the document view's part", () => {
  it("reads the node's thread under its canonical ID on its page, a part at the page's level; a node comment turns into an item about that node", async () => {
    const stub = stubClient();
    stub.getNode.mockImplementation((_project, ref) => Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: "MEC-RAIL" })] }));
    stub.state.comments.push(aComment({ id: "CM-0040", on: { kind: "node", id: "MEC-RAIL" }, on_rev: 3, body: "The rail's height reads oddly." }));
    // The route names an alias: the thread is read under the holder's canonical ID.
    const shown = await partAt(stub, "#/alpha/tree/RAIL-OLD/page", "The rail's height reads oddly.");
    expect(within(shown).getByRole("heading", { level: 2 }).textContent).toBe("Comments · 1");
    expect(argsOf(stub.getComments)).toEqual([["alpha", { on: "MEC-RAIL" }]]);
    // The revision it was written at.
    expect(shown.querySelector('[data-comment="CM-0040"]')?.textContent).toContain("on rev 3");
    const posted = await ask(shown, "CM-0040", stub);
    expect(posted[0]?.[2]).toMatchObject({ kind: "question", node_ids: ["MEC-RAIL"] });
  });

  it("reads an ID-less document's thread under its path", async () => {
    const stub = stubClient();
    stub.getNode.mockImplementation((_project, ref) => Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: null, path: "docs/spec/notes.md" })] }));
    stub.state.comments.push(aComment({ id: "CM-0041", on: { kind: "node", id: "docs/spec/notes.md" }, body: "Notes need an ID." }));
    await partAt(stub, "#/alpha/tree/docs%2Fspec%2Fnotes.md/page", "Notes need an ID.");
    expect(argsOf(stub.getComments)).toEqual([["alpha", { on: "docs/spec/notes.md" }]]);
  });
});

describe("the proposal pane's part", () => {
  it("stands under an open proposal; its comment turns into an item about the proposal's targets", async () => {
    const stub = stubClient([aProposal({ id: "PR-0031", kind: "question", target_id: "MEC-A", target_ids: ["MEC-A", "MEC-B"], summary: "Rail?" })]);
    stub.state.comments.push(aComment({ id: "CM-0050", on: { kind: "proposal", id: "PR-0031" }, body: "Read with the rail decision." }));
    const shown = await partAt(stub, "#/alpha/inbox/PR-0031", "Read with the rail decision.");
    expect(within(shown).getByRole("heading", { level: 3 }).textContent).toBe("Comments · 1");
    const posted = await ask(shown, "CM-0050", stub);
    expect(posted[0]?.[2]).toMatchObject({ kind: "question", node_ids: ["MEC-A", "MEC-B"] });
  });

  it("stands under a closed proposal the inbox no longer lists, shown to read", async () => {
    const closed = aProposal({ id: "PR-0032", kind: "question", status: "applied", target_id: "MEC-C", target_ids: ["MEC-C"], decided_by: "mira", decided_at: "2026-11-21T10:00:00Z" });
    const stub = stubClient([closed]);
    stub.getInbox.mockImplementation(() => Promise.resolve({ proposals: [entryOf(aProposal({ id: "PR-0001" }))], notes: [] }));
    stub.state.comments.push(aComment({ id: "CM-0051", on: { kind: "proposal", id: "PR-0032" }, body: "Decided before we read the log." }));
    const shown = await partAt(stub, "#/alpha/inbox/PR-0032", "Decided before we read the log.");
    expect(argsOf(stub.getComments)).toEqual([["alpha", { on: "PR-0032" }]]);
    expect(shown.closest("[data-closed]")?.getAttribute("data-closed")).toBe("PR-0032");
    const posted = await ask(shown, "CM-0051", stub);
    expect(posted[0]?.[2]).toMatchObject({ node_ids: ["MEC-C"] });
  });
});

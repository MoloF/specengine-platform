import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError, type TraceOptions } from "../api/client";
import type { QueueEvent, TraceDocument } from "../api/types";
import { ACTING_KEY } from "../board/acting";
import { aCard, aCardWrite, aFlowNode, aFlowView, aNode, aTrace, aTraceCard, aTraceNode } from "../test/builders";
import { flowClient } from "../test/flowStub";
import { renderApp } from "../test/render";
import { TREE, treeClient } from "../test/treeStub";

// docs/features/trace-views.md "A node's cards", AC-08 on a stub: the block on the tree's pane and in
// the document view (Flow's panel holds none since doc-modal, DM-14): what it reads, shows and links,
// and that a Clear finishes on the page.

const SUSPECT = aTraceCard({ id: "C-0001", title: "The suspect card", flow: "work", column: "todo", mark: "suspect", on: "A" });

/** A flow on `name`, its trace read by `trace`; the node's holder in `path`, by default the file aTraceNode gives its row. */
function flowOn(name: string, trace: (options: TraceOptions | undefined) => TraceDocument, path = aTraceNode(name, []).path) {
  const client = flowClient(aFlowView([aFlowNode({ id: name, distance: 0 })], []));
  client.getTrace.mockImplementation((_project, options) => Promise.resolve(trace(options)));
  // The holder in the file its trace row names, as the daemon serves both: the block finds its row by ID and file.
  client.getNode.mockImplementation((_project, ref) => Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref, path })] }));
  return client;
}

/** The block of `name`, its read landed. */
async function block(client: ReturnType<typeof flowOn>, name = "A"): Promise<HTMLElement> {
  renderApp(client, `#/alpha/tree/${name}`);
  const found = await screen.findByRole("region", { name: "Tasks" });
  await waitFor(() => {
    expect(found.querySelector(".cards-state, .verbatim, .error-message")).not.toBeNull();
  });
  return found;
}

function type(scope: HTMLElement, label: string, value: string) {
  fireEvent.change(within(scope).getByLabelText(label), { target: { value } });
}

describe("what the block shows", () => {
  it("reads getTrace(ref): the rollup, each card's title, mark and place, a link to the Board", async () => {
    const trace = aTrace([aTraceNode("A", [SUSPECT, aTraceCard({ id: "C-0002", title: "An open one" }), aTraceCard({ id: "C-0003", mark: "accepted", title: "An accepted one" })], { rollup: "suspect" })]);
    const client = flowOn("A", () => trace);
    const shown = await block(client);
    expect(client.getTrace.mock.calls.map((call) => call[1])).toContainEqual({ ref: "A" });
    expect(shown.querySelector(".cards-state")?.textContent).toBe("Suspect");
    const line = within(shown).getByRole("link", { name: /C-0001 The suspect card/ });
    expect(line.getAttribute("href")).toBe("#/alpha/board/C-0001");
    expect(line.closest("li")?.textContent).toContain("Suspect");
    expect(line.closest("li")?.textContent).toContain("work/todo");
    expect(within(shown).getByRole("link", { name: /C-0002/ }).closest("li")?.textContent).toContain("Open");
    expect(within(shown).getByRole("link", { name: /C-0003/ }).closest("li")?.textContent).toContain("Accepted");
  });

  it("shows the rollup as a label and an icon; a word this build does not know raw and neutral", async () => {
    const shown = await block(flowOn("A", () => aTrace([aTraceNode("A", [], { rollup: "pondering" })])));
    const badge = shown.querySelector(".cards-state .badge");
    expect([badge?.textContent, badge?.getAttribute("data-tone"), badge?.querySelector("svg") !== null]).toEqual(["pondering", "trace-unknown", true]);
  });

  it("says a node with no card, and the next step", async () => {
    const shown = await block(flowOn("A", () => aTrace([aTraceNode("A", [], { rollup: "no card" })])));
    expect(shown.querySelector(".cards-state")?.textContent).toBe("No task");
    expect(within(shown).getByText("No task holds this yet. Next: press New task.")).toBeTruthy();
  });

  it("lists the nodes using it, each linking its node and saying its rollup; in the tree to the tree", async () => {
    const shown = await block(flowOn("A", () => aTrace([aTraceNode("A", [], { uses: [{ id: "B", rollup: "in work", cards: 2 }] })])));
    const use = within(shown).getByRole("link", { name: "B" });
    expect(use.getAttribute("href")).toBe("#/alpha/tree/B");
    expect(use.closest("li")?.textContent).toContain("In work");
    expect(use.closest("li")?.textContent).toContain("2 tasks");
  });

  it("says the daemon's notes on the read, and a REF it could not name in its own words", async () => {
    const noted = await block(flowOn("A", () => aTrace([aTraceNode("A", [])], { notes: ["C-0009: `GONE` no longer resolves"] })));
    expect(within(noted).getByText("C-0009: `GONE` no longer resolves")).toBeTruthy();
  });

  it("a read with no row for the node shows no other node's state, and says so; no card line claims none", async () => {
    const shown = await block(flowOn("A", () => aTrace([aTraceNode("OTHER", [], { rollup: "confirmed" })])));
    expect(shown.querySelector(".cards-state")?.textContent).toBe("This read holds no row for A: its state is not known.");
    expect(shown.querySelector(".badge")).toBeNull();
    expect(within(shown).queryByText(/No card holds this yet/)).toBeNull();
  });

  it("a failed re-read says what stands is the last read's, and Retry reads again", async () => {
    let trace = aTrace([aTraceNode("A", [], { rollup: "in work" })]);
    const client = flowOn("A", () => trace);
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    const shown = await block(client);
    const reads = client.getTrace.getMockImplementation();
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    act(() => {
      emit({ seq: 9, type: "card.moved", payload: { id: "C-0001" } });
    });
    const line = await within(shown).findByRole("alert");
    expect(line.textContent).toContain("the queue is locked");
    expect(line.textContent).toContain("What stands below is from the last read.");
    trace = aTrace([aTraceNode("A", [], { rollup: "confirmed" })]);
    client.getTrace.mockImplementation(reads as NonNullable<typeof reads>);
    const retry = within(shown).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(shown.querySelector(".cards-state")?.textContent).toBe("Confirmed");
    });
    expect(within(shown).queryByRole("alert")).toBeNull();
    // Retry went with the failure: focus lands on the block, never the body.
    await waitFor(() => {
      expect(document.activeElement).toBe(within(shown).getByRole("heading", { name: "Tasks" }));
    });
  });

  it.each([
    ["a node's rollup", () => aTrace([aTraceNode("A", [aTraceCard({ id: "C-0001", mark: "accepted" })], { rollup: "confirmed" })])],
    ["a group's counts", () => aTrace([aTraceNode("A", [], { rollup: "confirmed", counts: { no_card: 0, in_work: 0, suspect: 0, departed: 0, confirmed: 3 } }), aTraceNode("B", [])])],
    ["no row at all", () => aTrace([aTraceNode("OTHER", [])])],
  ])("a failed re-read draws no state row: %s stands nowhere until a read lands", async (_name, trace) => {
    const client = flowOn("A", trace);
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    const shown = await block(client);
    expect(shown.querySelector(".cards-state")).not.toBeNull();
    client.getTrace.mockRejectedValue(new ClientError({ status: 503, message: "the queue is locked" }));
    act(() => {
      emit({ seq: 9, type: "card.moved", payload: { id: "C-0001" } });
    });
    await within(shown).findByRole("alert");
    expect(shown.querySelector(".cards-state")).toBeNull();
  });

  it("a reason in the daemon's words, verbatim", async () => {
    const shown = await block(flowOn("A", () => aTrace([], { reason: "no node `A`" })));
    expect(within(shown).getByText("no node `A`")).toBeTruthy();
  });

  it("a failed read: the daemon's words and Retry; the panel stands; focus lands on the block", async () => {
    const client = flowOn("A", () => aTrace([]));
    client.getTrace.mockImplementation((_project, options) => (options?.ref === undefined ? Promise.resolve(aTrace([])) : Promise.reject(new ClientError({ status: 503, message: "the queue is of another schema" }))));
    const shown = await block(client);
    expect(within(shown).getByText("the queue is of another schema")).toBeTruthy();
    client.getTrace.mockImplementation(() => Promise.resolve(aTrace([aTraceNode("A", [], { rollup: "confirmed" })])));
    const retry = within(shown).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(shown.querySelector(".cards-state")?.textContent).toBe("Confirmed");
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(within(shown).getByRole("heading", { name: "Tasks" }));
    });
  });

  it("New card opens with the node as its target, and the new card links to the Board", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const client = flowOn("A", () => aTrace([aTraceNode("A", [])]));
    client.createCard.mockResolvedValue(aCardWrite("C-0042"));
    const shown = await block(client);
    fireEvent.click(within(shown).getByRole("button", { name: "New task" }));
    const dialog = await screen.findByRole("dialog", { name: "New task" });
    type(dialog, "Title", "From the panel");
    fireEvent.click(within(dialog).getByRole("button", { name: "Create task" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.createCard.mock.calls[0]?.[1]).toMatchObject({ title: "From the panel", target: ["A"] });
    expect(within(shown).getByRole("link", { name: "Open it on the Board" }).getAttribute("href")).toBe("#/alpha/board/C-0042");
  });
});

describe("Clear on the page", () => {
  function suspectClient() {
    let trace = aTrace([aTraceNode("A", [SUSPECT], { rollup: "suspect" })]);
    const client = flowOn("A", () => trace);
    client.getCard.mockResolvedValue(aCard({ id: "C-0001", title: "The suspect card", targets: ["A"], revision: "4" }));
    client.clearCard.mockImplementation((_project, id) => {
      trace = aTrace([aTraceNode("A", [{ ...SUSPECT, mark: "open", on: null }], { rollup: "in work" })]);
      return Promise.resolve(aCardWrite(id, { notes: ["the mark clears"] }));
    });
    return client;
  }

  it("a suspect card's Clear reads the card, takes the dialog, and finishes here: no reload, no Board (AC-08)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const client = suspectClient();
    const shown = await block(client);
    const button = within(shown).getByRole("button", { name: "Clear C-0001" });
    fireEvent.click(button);
    const dialog = await screen.findByRole("dialog", { name: "Clear C-0001" });
    expect(dialog.textContent).toContain("The brief changed at A.");
    type(dialog, "Reason", "Wording only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear task" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.clearCard.mock.calls[0]).toEqual(["alpha", "C-0001", { outcome: "no_effect", reason: "Wording only", revision: "4", as: "Ivo Brandt" }]);
    await waitFor(() => {
      expect(shown.querySelector(".cards-state")?.textContent).toBe("In work");
    });
    expect(within(shown).queryByRole("button", { name: /^Clear/ })).toBeNull();
    expect(window.location.hash).toBe("#/alpha/tree/A");
    expect(within(shown).getByText("the mark clears")).toBeTruthy();
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("C-0001 is cleared");
    // Its button went with its mark: focus lands on the block, never the body.
    await waitFor(() => {
      expect(document.activeElement).toBe(within(shown).getByRole("heading", { name: "Tasks" }));
    });
  });

  it("a refused clear keeps the dialog and the text, the daemon's words in an alert", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const client = suspectClient();
    client.clearCard.mockRejectedValue(new ClientError({ status: 409, message: "C-0001 is not suspect: there is no mark to clear; nothing changed" }));
    fireEvent.click(within(await block(client)).getByRole("button", { name: "Clear C-0001" }));
    const dialog = await screen.findByRole("dialog");
    type(dialog, "Reason", "Kept");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear task" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain("C-0001 is not suspect");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Reason").value).toBe("Kept");
  });

  it("shows a skeleton while the card is read for its dialog", async () => {
    const client = suspectClient();
    let land: (card: ReturnType<typeof aCard>) => void = () => undefined;
    client.getCard.mockImplementation(() => new Promise((resolve) => {
      land = resolve;
    }));
    const shown = await block(client);
    fireEvent.click(within(shown).getByRole("button", { name: "Clear C-0001" }));
    expect(await within(shown).findByRole("status", { name: "Reading C-0001" })).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    act(() => {
      land(aCard({ id: "C-0001", targets: ["A"] }));
    });
    expect(await screen.findByRole("dialog", { name: "Clear C-0001" })).toBeTruthy();
  });

  it("Esc in the dialog closes the dialog alone, and focus goes back to its button", async () => {
    const shown = await block(suspectClient());
    const button = within(shown).getByRole("button", { name: "Clear C-0001" });
    fireEvent.click(button);
    const dialog = await screen.findByRole("dialog");
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(window.location.hash).toBe("#/alpha/tree/A");
    await waitFor(() => {
      expect(document.activeElement).toBe(button);
    });
  });

  it("a card that cannot be read for its Clear says so in the daemon's words, and a second press reads again", async () => {
    const client = suspectClient();
    client.getCard.mockRejectedValueOnce(new ClientError({ status: 503, message: "the queue is locked" }));
    const shown = await block(client);
    fireEvent.click(within(shown).getByRole("button", { name: "Clear C-0001" }));
    expect((await within(shown).findByRole("alert")).textContent).toContain("the queue is locked");
    fireEvent.click(within(shown).getByRole("button", { name: "Clear C-0001" }));
    expect(await screen.findByRole("dialog", { name: "Clear C-0001" })).toBeTruthy();
    expect(client.getCard).toHaveBeenCalledTimes(2);
  });

  it("only a suspect card takes it: a correcting mark is held, a departed one answered elsewhere", async () => {
    const shown = await block(flowOn("A", () => aTrace([aTraceNode("A", [aTraceCard({ id: "C-0002", mark: "correcting", on: "A", correction: "C-0003" }), aTraceCard({ id: "C-0004", mark: "departed", on: "A", discrepancy: "PR-1" })], { rollup: "departed" })])));
    expect(within(shown).getByRole("link", { name: /C-0002/ }).closest("li")?.textContent).toContain("Correcting");
    expect(within(shown).getByRole("link", { name: /C-0004/ }).closest("li")?.textContent).toContain("Departed");
    expect(within(shown).queryByRole("button", { name: /^Clear/ })).toBeNull();
  });
});

describe("a group's block", () => {
  const counts = { no_card: 1, in_work: 0, suspect: 2, departed: 0, confirmed: 3 };
  /** A's file: last by path, so the block finds A's row wherever it stands. */
  const A_PATH = "docs/z.md";
  const group = () =>
    aTrace([
      aTraceNode("A", [aTraceCard({ id: "C-0001" })], { rollup: "no card", counts, path: A_PATH }),
      aTraceNode("B", [aTraceCard({ id: "C-0001", mark: "suspect", on: "B" }), aTraceCard({ id: "C-0002", mark: "suspect", on: "B" })], { rollup: "suspect" }),
      aTraceNode("C", [aTraceCard({ id: "C-0002", mark: "suspect", on: "C" })], { rollup: "suspect", uses: [{ id: "U", rollup: "no card", cards: 0 }] }),
    ]);

  it("says the counts served for the group, never its own rollup, whose cards may all sit under it", async () => {
    const shown = await block(flowOn("A", group, A_PATH));
    expect(shown.querySelector(".cards-state")?.textContent).toBe("3 confirmed, 2 suspect, 1 no task");
  });

  it("lists each card of what it holds once, the mark a person can clear standing", async () => {
    const shown = await block(flowOn("A", group, A_PATH));
    expect(within(shown).getAllByRole("link", { name: /C-000/ }).map((link) => link.textContent)).toEqual(["C-0001 Card C-0001", "C-0002 Card C-0002"]);
    expect(within(shown).getByRole("button", { name: "Clear C-0001" })).toBeTruthy();
    expect(within(shown).getByRole("link", { name: "U" })).toBeTruthy();
  });

  it("Clear all posts the group, a reason and a name, no effect only, and finishes here (AC-08)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    let trace = group();
    const client = flowOn("A", () => trace, A_PATH);
    client.clearCardGroup.mockImplementation((_project, body) => {
      trace = aTrace([aTraceNode("A", [], { rollup: "no card", counts: { ...counts, suspect: 0 }, path: A_PATH }), aTraceNode("B", [])]);
      return Promise.resolve({ group: body.group, cleared: ["C-0001", "C-0002"], notes: [] });
    });
    const shown = await block(client);
    fireEvent.click(within(shown).getByRole("button", { name: "Clear all" }));
    const dialog = await screen.findByRole("dialog", { name: "Clear all under A" });
    expect(dialog.textContent).toContain("2 suspect tasks");
    expect(within(dialog).queryByRole("radio")).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Say why the marks clear.");
    expect(client.clearCardGroup).not.toHaveBeenCalled();
    type(dialog, "Reason", "Numbering only");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.clearCardGroup.mock.calls).toEqual([["alpha", { group: "A", reason: "Numbering only", as: "Ivo Brandt" }]]);
    await waitFor(() => {
      expect(within(shown).queryByRole("button", { name: "Clear all" })).toBeNull();
    });
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("2 cleared under A");
  });

  it("a refused Clear all keeps the dialog, the daemon's words in an alert", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ivo Brandt");
    const client = flowOn("A", group, A_PATH);
    client.clearCardGroup.mockRejectedValue(new ClientError({ status: 409, message: "no suspect card under `A`: there is no mark to clear; nothing changed" }));
    fireEvent.click(within(await block(client)).getByRole("button", { name: "Clear all" }));
    const dialog = await screen.findByRole("dialog");
    type(dialog, "Reason", "Kept");
    fireEvent.click(within(dialog).getByRole("button", { name: "Clear all" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain("no suspect card under `A`");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Reason").value).toBe("Kept");
  });

  it("offers no Clear all on one node, nor on a group with nothing to clear", async () => {
    const single = await block(flowOn("A", () => aTrace([aTraceNode("A", [SUSPECT])])));
    expect(within(single).queryByRole("button", { name: "Clear all" })).toBeNull();
  });

  it("offers no Clear all on a group with no suspect card", async () => {
    const quiet = await block(flowOn("A", () => aTrace([aTraceNode("A", []), aTraceNode("B", [aTraceCard({ id: "C-0001" })])])));
    expect(within(quiet).queryByRole("button", { name: "Clear all" })).toBeNull();
  });
});

describe("live", () => {
  it("a card event reads the block again, with no reload", async () => {
    let trace = aTrace([aTraceNode("A", [], { rollup: "no card" })]);
    const client = flowOn("A", () => trace);
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = vi.fn((_project: string, onEvent: (event: QueueEvent) => void) => {
      emit = onEvent;
      return () => undefined;
    });
    const shown = await block(client);
    trace = aTrace([aTraceNode("A", [aTraceCard({ id: "C-0001", mark: "accepted" })], { rollup: "confirmed" })]);
    act(() => {
      emit({ seq: 9, type: "card.accepted", payload: { id: "C-0001" } });
    });
    await waitFor(() => {
      expect(shown.querySelector(".cards-state")?.textContent).toBe("Confirmed");
    });
  });
});

describe("in the Spec tree", () => {
  it("stands on the node's pane, its uses linking the tree, New card with the node as target (AC-08)", async () => {
    const name = TREE.find((row) => row.id !== null)?.id ?? "";
    const client = treeClient();
    client.getTrace.mockResolvedValue(aTrace([aTraceNode(name, [SUSPECT], { rollup: "suspect", uses: [{ id: "B", rollup: "in work", cards: 1 }] })]));
    renderApp(client, `#/alpha/tree/${name}`);
    const shown = await screen.findByRole("region", { name: "Tasks" });
    await waitFor(() => {
      expect(within(shown).getByRole("link", { name: /C-0001/ }).getAttribute("href")).toBe("#/alpha/board/C-0001");
    });
    expect(within(shown).getByRole("link", { name: "B" }).getAttribute("href")).toBe("#/alpha/tree/B");
    expect(within(shown).getByRole("heading", { level: 2, name: "Tasks" })).toBeTruthy();
    fireEvent.click(within(shown).getByRole("button", { name: "New task" }));
    expect(await screen.findByRole("dialog", { name: "New task" })).toBeTruthy();
  });
});

// DM-12 of docs/features/doc-modal.md: in the document view the block's head holds no New card (the
// view's own head does), and a node using it opens as a document.
describe("in the document view", () => {
  it("has no New card of its own, and a using node is a document link: its page the href, a plain click the modal", async () => {
    const client = treeClient();
    client.getTrace.mockResolvedValue(aTrace([aTraceNode("DOC-A", [SUSPECT], { rollup: "suspect", uses: [{ id: "DOC-C", rollup: "in work", cards: 1 }] })]));
    renderApp(client, "#/alpha/inbox?doc=DOC-A");
    const modal = await screen.findByRole("dialog", { name: "Title of DOC-A" });
    const shown = await within(modal).findByRole("region", { name: "Tasks" });
    const use = await within(shown).findByRole("link", { name: "DOC-C" });
    expect(use.getAttribute("href")).toBe("#/alpha/tree/DOC-C/page");
    expect(within(shown).queryByRole("button", { name: "New task" })).toBeNull();
    expect(within(shown).getByRole("heading", { level: 3, name: "Tasks" })).toBeTruthy();
    fireEvent.click(use);
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/inbox?doc=DOC-C");
    });
  });
});

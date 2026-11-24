import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Card, InSpec, Reference } from "../api/types";
import { switchLanguage } from "../i18n";
import { ru } from "../i18n/ru";
import { aB3, aCard, aCardList, aCardRow, aLinkReference, aNode, aProposal, aReference, entryOf } from "../test/builders";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";

// docs/canon/work-references-promotion-surfaces.md "UI" and docs/features/reference-promotion.md "Acceptance criteria", PR17, over the stub board: "Promote to canon…" on a
// stored file's tile alone, never a link, a deleted or a lost one; its Document preselected per
// target (a card's first target, a node, a proposal's target) or typed; one POST of the route's body
// under the acting name; a 200 linking the Inbox item it stored, with the daemon's notes, nothing else
// written; a 409 or a 503 keeping the dialog, the typed text and the daemon's line. "In the spec": the
// rows of `in_spec`, each its document as a link, its path and its text, plain, no thumbnail, player
// or download, no HTML sink; English and Russian.

beforeAll(async () => {
  await import("./ReferencesPart");
  await import("../board/CardItems");
  await import("../markdown/Markdown");
  await import("../doc/DocBody");
});

const CARD: Card = aCard({ id: "C-0007", title: "Rail height", targets: ["MEC-RAIL", "RULE-RAIL-GAP"] });

/** Every state a tile can be in: a stored image, a stored sound, a link, a file lost with its host, a deleted one. */
function tiles(): Reference[] {
  return [
    aReference({ id: "RF-0011", title: "Harbour at dusk", name: "harbour.png", matters: "The light on the water." }),
    aReference({ id: "RF-0012", title: "Foghorn", name: "foghorn.wav", b3: aB3("d"), media: "audio/wav", class: "audio", matters: "Two long blasts.", meta: { width: null, height: null, duration_ms: 2000, triangles: null } }),
    aLinkReference({ id: "RF-0013", title: "Moodboard" }),
    aReference({ id: "RF-0014", title: "Old pier", file: "lost" }),
    aReference({ id: "RF-0015", title: null, name: null, b3: null, meta: null, file: null, deleted: { by: "mira", at: "2026-11-23T11:00:00Z" } }),
  ];
}

/** The rows the card's targets' documents keep: one with text in Cyrillic, one with none. */
const IN_SPEC: InSpec[] = [
  // "Rail gap at night" in Russian, escaped: an entry's text is shown as written, any script.
  { node: "MEC-RAIL", document: "docs/spec/rail.md", path: "docs/spec/media/rail-gap.png", matters: "\u0417\u0430\u0437\u043e\u0440 \u0440\u0435\u043b\u044c\u0441\u0430 \u043d\u043e\u0447\u044c\u044e, <b>not</b> by day" },
  // An ID-less document: its `node` is its path.
  { node: "docs/spec/notes/yard.md", document: "docs/spec/notes/yard.md", path: "docs/spec/notes/yard-plan.pdf", matters: null },
];

function client(references: Reference[] = tiles()): StubClient {
  const stub = stubClient();
  stub.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: CARD.id, title: CARD.title, targets: CARD.targets })])));
  stub.getCard.mockImplementation((_project, id) => Promise.resolve(id === CARD.id ? structuredClone(CARD) : { id, reason: `no card ${id}` }));
  stub.state.references.push(...references);
  return stub;
}

/** The task's modal and its References part, once read. */
async function part(stub: StubClient): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/board/${CARD.id}`);
  const dialog = await screen.findByRole("dialog", { name: CARD.title });
  const heading = await within(dialog).findByRole("heading", { level: 3, name: /^References · / });
  const region = heading.closest("section");
  if (region === null) {
    throw new Error("no References region");
  }
  return region;
}

function tile(scope: HTMLElement, id: string): HTMLElement {
  const found = scope.querySelector<HTMLElement>(`[data-reference="${id}"]`);
  if (found === null) {
    throw new Error(`no ${id}`);
  }
  return found;
}

/** A tile's Promote to canon…, then its dialog. */
async function promoteFrom(shown: HTMLElement, id: string): Promise<HTMLElement> {
  fireEvent.click(within(tile(shown, id)).getByRole("button", { name: "Promote to canon…" }));
  return screen.findByRole("dialog", { name: `Promote ${id} to canon` });
}

describe("Promote to canon… (PR17)", () => {
  it("stands on a stored file's tile alone: never on a link, a file lost with its host or a deleted one (M: the button shown on a link)", async () => {
    const shown = await part(client());
    const offered = ["RF-0011", "RF-0012", "RF-0013", "RF-0014", "RF-0015"].map((id) => [id, within(tile(shown, id)).queryByRole("button", { name: "Promote to canon…" }) !== null]);
    expect(offered).toEqual([
      ["RF-0011", true],
      ["RF-0012", true],
      ["RF-0013", false],
      ["RF-0014", false],
      ["RF-0015", false],
    ]);
  });

  it("chooses the card's first target, prefills the name, what matters and the rationale, and posts the route's body: unchanged fields null, the daemon's defaults", async () => {
    const stub = client();
    const shown = await part(stub);
    const dialog = await promoteFrom(shown, "RF-0011");
    const document = within(dialog).getByRole("combobox", { name: "Document" });
    expect(document instanceof HTMLSelectElement ? [document.value, Array.from(document.options, (option) => option.textContent)] : null).toEqual(["0", ["MEC-RAIL", "RULE-RAIL-GAP", "Another document…"]]);
    expect(within(dialog).getByRole<HTMLInputElement>("textbox", { name: "File name" }).value).toBe("harbour.png");
    expect(within(dialog).getByRole<HTMLTextAreaElement>("textbox", { name: "What matters here" }).value).toBe("The light on the water.");
    const rationale = within(dialog).getByRole<HTMLTextAreaElement>("textbox", { name: "Rationale" });
    expect(rationale.value).toBe("Promote RF-0011 beside MEC-RAIL");
    // The default follows the Document chosen until the person types one.
    fireEvent.change(document, { target: { value: "1" } });
    expect(rationale.value).toBe("Promote RF-0011 beside RULE-RAIL-GAP");
    expect(dialog.textContent).toContain("Nothing is written now: a person confirms the proposal in the Inbox");
    fireEvent.click(within(dialog).getByRole("button", { name: "Propose" }));
    await within(dialog).findByText(/: a person confirms it in the Inbox$/);
    expect(argsOf(stub.promoteReference)).toEqual([["alpha", "RF-0011", { to: "RULE-RAIL-GAP", name: null, matters: null, rationale: "Promote RF-0011 beside RULE-RAIL-GAP", as: null }]]);
    // A proposal only: no reference written, none attached or deleted.
    expect([stub.addReference.mock.calls.length, stub.deleteReference.mock.calls.length, stub.putBlob.mock.calls.length]).toEqual([0, 0, 0]);
  });

  it("shows a 200 as the stored proposal, linked to its Inbox item, with the daemon's notes; the dialog keeps only its Close", async () => {
    const stub = client();
    const shown = await part(stub);
    const dialog = await promoteFrom(shown, "RF-0012");
    fireEvent.change(within(dialog).getByRole("textbox", { name: "File name" }), { target: { value: "horn.wav" } });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "What matters here" }), { target: { value: "The second blast." } });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Rationale" }), { target: { value: "The horn, in the spec" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Propose" }));
    const line = await within(dialog).findByText(/: a person confirms it in the Inbox$/);
    expect(line.textContent).toBe("Proposed as PR-0140: a person confirms it in the Inbox");
    expect(within(dialog).getByRole("link", { name: "PR-0140" }).getAttribute("href")).toBe("#/alpha/inbox/PR-0140");
    expect(within(dialog).getByRole("list", { name: "What the daemon notes" }).textContent).toBe("adds media/horn.wav: RF-0012, audio/wav, plain");
    expect(argsOf(stub.promoteReference)).toEqual([["alpha", "RF-0012", { to: "MEC-RAIL", name: "horn.wav", matters: "The second blast.", rationale: "The horn, in the spec", as: null }]]);
    expect(within(dialog).queryByRole("button", { name: "Propose" })).toBeNull();
    await waitFor(() => {
      expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Close" }));
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Promote RF-0012 to canon" })).toBeNull();
    });
  });

  it.each([
    [409, "RF-0011 is already proposed for `MEC-RAIL` by PR-0042 (open)"],
    [503, "spec: no [references] home takes kind `mechanic` (`MEC-RAIL`): add one to [[references.homes]] in specengine.toml; nothing stored"],
  ])("keeps the dialog, the typed text and the daemon's words on a %s", async (status, message) => {
    const stub = client();
    stub.promoteReference.mockImplementation(() => Promise.reject(new ClientError({ status, message })));
    const shown = await part(stub);
    const dialog = await promoteFrom(shown, "RF-0011");
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Rationale" }), { target: { value: "Keep the dusk" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Propose" }));
    const alert = await within(dialog).findByRole("alert");
    expect(alert.textContent).toContain(message);
    expect(within(dialog).getByRole<HTMLTextAreaElement>("textbox", { name: "Rationale" }).value).toBe("Keep the dusk");
    expect(within(dialog).getByRole("button", { name: "Propose" })).toBeTruthy();
    expect(within(dialog).queryByRole("link", { name: /^PR-/ })).toBeNull();
  });

  it("takes a typed REF for another document, and stops a blank one or a blank rationale before sending", async () => {
    const stub = client();
    const shown = await part(stub);
    const dialog = await promoteFrom(shown, "RF-0011");
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Document" }), { target: { value: "typed" } });
    const typed = within(dialog).getByRole("textbox", { name: "Document's ID or path" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Propose" }));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Name the document first.");
    fireEvent.change(typed, { target: { value: "docs/spec/notes/yard.md" } });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Rationale" }), { target: { value: "  " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Propose" }));
    await within(dialog).findByText("Give a rationale first.");
    expect(stub.promoteReference).not.toHaveBeenCalled();
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Rationale" }), { target: { value: "The yard's plan" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Propose" }));
    await within(dialog).findByText(/: a person confirms it in the Inbox$/);
    expect(argsOf(stub.promoteReference)).toEqual([["alpha", "RF-0011", { to: "docs/spec/notes/yard.md", name: null, matters: null, rationale: "The yard's plan", as: null }]]);
  });

  it("chooses the node in the document view, and the proposal's target in the proposal pane", async () => {
    const stub = stubClient([aProposal({ id: "PR-0031", kind: "question", target_id: "MEC-A", target_ids: ["MEC-A"], summary: "Rail?" })]);
    stub.getNode.mockImplementation((_project, ref) => Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: "MEC-RAIL" })] }));
    stub.state.references.push(aReference({ id: "RF-0050", on: { kind: "node", id: "MEC-RAIL" }, title: "Rail photo" }), aReference({ id: "RF-0060", on: { kind: "proposal", id: "PR-0031" }, title: "The rail log" }));
    renderApp(stub, "#/alpha/tree/MEC-RAIL/page");
    await screen.findByText("Rail photo");
    const onNode = screen.getByRole("region", { name: "References" });
    let dialog = await promoteFrom(onNode, "RF-0050");
    let document = within(dialog).getByRole("combobox", { name: "Document" });
    expect(document instanceof HTMLSelectElement ? [document.value, Array.from(document.options, (option) => option.textContent)] : null).toEqual(["0", ["MEC-RAIL", "Another document…"]]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Promote RF-0050 to canon" })).toBeNull();
    });
    stub.getInbox.mockImplementation(() => Promise.resolve({ proposals: [entryOf(aProposal({ id: "PR-0031", kind: "question", target_id: "MEC-A", target_ids: ["MEC-A"], summary: "Rail?" }))], notes: [] }));
    window.location.hash = "#/alpha/inbox/PR-0031";
    await screen.findByText("The rail log");
    dialog = await promoteFrom(screen.getByRole("region", { name: "References" }), "RF-0060");
    document = within(dialog).getByRole("combobox", { name: "Document" });
    expect(document instanceof HTMLSelectElement ? Array.from(document.selectedOptions, (option) => option.textContent) : null).toEqual(["MEC-A"]);
  });
});

describe("In the spec (PR17)", () => {
  it("lists the target's documents' entries as text: the document a link, the path, the text; no thumbnail, player or download (M: a preview drawn)", async () => {
    const stub = client([aLinkReference({ id: "RF-0013", title: "Moodboard" })]);
    stub.state.inSpec[CARD.id] = IN_SPEC;
    const shown = await part(stub);
    const group = await within(shown).findByRole("region", { name: "In the spec · 2" });
    const rows = within(group).getAllByRole("listitem");
    expect(rows.map((row) => row.textContent)).toEqual([
      "MEC-RAILFiledocs/spec/media/rail-gap.png\u0417\u0430\u0437\u043e\u0440 \u0440\u0435\u043b\u044c\u0441\u0430 \u043d\u043e\u0447\u044c\u044e, <b>not</b> by day",
      "docs/spec/notes/yard.mdFiledocs/spec/notes/yard-plan.pdf",
    ]);
    // The document opens as any document link does; an ID-less one by its path.
    expect(within(group).getAllByRole("link").map((link) => link.getAttribute("href"))).toEqual(["#/alpha/tree/MEC-RAIL/page", "#/alpha/tree/docs%2Fspec%2Fnotes%2Fyard.md/page"]);
    expect(group.querySelectorAll("img, audio, video, b, a[download], button")).toHaveLength(0);
    // People's tiles stay as they were, the group after them.
    expect(tile(shown, "RF-0013").compareDocumentPosition(group) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("shows no group when the documents keep none", async () => {
    const shown = await part(client());
    expect(within(shown).queryByRole("region", { name: /^In the spec/ })).toBeNull();
  });

  it("words the act and the group from ru.ts, the entries as written", async () => {
    const stub = client();
    stub.state.inSpec[CARD.id] = IN_SPEC;
    const shown = await part(stub);
    await within(shown).findByRole("region", { name: "In the spec · 2" });
    act(() => {
      switchLanguage("ru");
    });
    expect(within(tile(shown, "RF-0011")).getByRole("button", { name: ru.references.promote.action })).toBeTruthy();
    expect(within(shown).getByRole("region", { name: ru.references.inSpec.title_few.replace("{{count}}", "2") })).toBeTruthy();
    expect(shown.textContent).toContain("docs/spec/media/rail-gap.png");
  });
});

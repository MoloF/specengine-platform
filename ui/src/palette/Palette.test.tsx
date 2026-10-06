import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { Proposal, TaskListEntry } from "../api/types";
import { aProposal, aSearchHit, aSearchResults, aTaskEntry } from "../test/builders";
import { callsOf, homeClient } from "../test/homeStub";
import { renderApp } from "../test/render";

// docs/features/ui-home.md "Palette" and "Chord" on a stub client: AC-09 (the chord), AC-10 (one
// document listener), AC-11 (dialog and combobox), AC-12 (what opening, typing and a search read),
// AC-13 (matching and jumps), AC-14 (failures in their group).

/** Markup an author pasted; shown as text, never parsed (escaped so no source line spells a dialog call). */
const HOSTILE = "<img src=x onerror=\u0061lert(1)>";

const TASKS: TaskListEntry[] = [
  aTaskEntry({ id: "T-0107", status: "review", title: "Queue deep ships at the outer anchorage" }),
  aTaskEntry({ id: "T-0108", status: "ready", stale: true, title: "Speed limit on night passages" }),
  aTaskEntry({ id: "T-0110", status: "changes_requested", title: "Pilot boat dispatch" }),
  aTaskEntry({ id: "T-0104", status: "done", title: "Mooring crew of four" }),
];

const QUEUE: Proposal[] = [
  aProposal({ id: "PR-0041", summary: "Entry only inside the tide window", severity: "high" }),
  aProposal({ id: "PR-0042", summary: "Berth T-0108 draft", severity: "normal" }),
];

function client() {
  return homeClient(TASKS, QUEUE);
}

const LABEL = "Jump to a section, task, proposal, node or project";

function combobox(): HTMLInputElement {
  return screen.getByRole("combobox", { name: LABEL });
}

function listbox(): HTMLElement {
  return screen.getByRole("listbox", { name: "Places to jump to" });
}

function palette(): HTMLElement | null {
  return screen.queryByRole("dialog", { name: "Jump to" });
}

/** Each group's label and its options' text. */
function groups(): [string, string[]][] {
  return within(listbox())
    .queryAllByRole("group")
    .map((group) => [
      document.getElementById(group.getAttribute("aria-labelledby") ?? "")?.textContent ?? "",
      within(group)
        .queryAllByRole("option")
        .map((option) => option.textContent),
    ]);
}

function activeOption(): HTMLElement | null {
  return document.getElementById(combobox().getAttribute("aria-activedescendant") ?? "");
}

/** Whether the element sits in an inert subtree (jsdom keeps `inert` as a plain property). */
function isInert(element: Element): boolean {
  for (let node: Element | null = element; node !== null; node = node.parentElement) {
    if (node instanceof HTMLElement && node.inert) {
      return true;
    }
  }
  return false;
}

function politeRegion(): HTMLElement {
  const region = document.querySelector<HTMLElement>('[aria-live="polite"]');
  if (region === null) {
    throw new Error("no polite live region");
  }
  return region;
}

/** Cmd-K (or the given fields) on `target`; whether the event went unprevented, as dispatchEvent says. */
function chord(target: Element | Document = document.body, fields: Record<string, unknown> = { key: "k", code: "KeyK", metaKey: true }): boolean {
  return fireEvent.keyDown(target, fields);
}

function type(text: string) {
  fireEvent.change(combobox(), { target: { value: text } });
}

function key(name: string, fields: Record<string, unknown> = {}) {
  fireEvent.keyDown(combobox(), { key: name, ...fields });
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

async function home(stub = client(), hash = "#/alpha") {
  renderApp(stub, hash);
  await screen.findByRole("heading", { level: 1, name: "Overview" });
  await waitFor(() => {
    expect(document.querySelectorAll('.home-region[aria-busy="false"]')).toHaveLength(2);
  });
  return stub;
}

async function openPalette(target: Element | Document = document.body): Promise<HTMLElement> {
  expect(chord(target)).toBe(false);
  return screen.findByRole("dialog", { name: "Jump to" });
}

async function closePalette() {
  key("Escape");
  await waitFor(() => {
    expect(palette()).toBeNull();
  });
}

/** Waits for the hash jsdom changes in a task of its own. */
async function hashIs(hash: string) {
  await waitFor(() => {
    expect(window.location.hash).toBe(hash);
  });
}

describe("the chord (AC-09)", () => {
  it.each([
    ["#/alpha", "Overview"],
    ["#/alpha/inbox", "Inbox"],
    ["#/alpha/tasks", "Tasks"],
    ["#/alpha/tree", "Spec tree"],
    ["#/alpha/graph", "Graph"],
  ])("opens the palette on %s with Cmd-K and with Ctrl-K, from the body, the event prevented", async (hash, heading) => {
    renderApp(client(), hash);
    await screen.findByRole("heading", { level: 1, name: heading });
    expect(chord(document.body, { key: "k", metaKey: true })).toBe(false);
    expect(await screen.findByRole("dialog", { name: "Jump to" })).toBeTruthy();
    await closePalette();
    expect(chord(document.body, { key: "k", ctrlKey: true })).toBe(false);
    expect(await screen.findByRole("dialog", { name: "Jump to" })).toBeTruthy();
  });

  it("opens from the Inbox filter, a text field, and on a Cyrillic layout by the physical key", async () => {
    renderApp(client(), "#/alpha/inbox");
    const filter = await screen.findByLabelText("Filter");
    filter.focus();
    await openPalette(filter);
    await closePalette();
    expect(document.activeElement).toBe(filter);
    expect(chord(filter, { key: "\u043b", code: "KeyK", metaKey: true })).toBe(false);
    expect(await screen.findByRole("dialog", { name: "Jump to" })).toBeTruthy();
  });

  it("does nothing for Dvorak's Cmd-T on KeyK, with Alt or Shift, or during composition", async () => {
    await home();
    for (const fields of [
      { key: "t", code: "KeyK", metaKey: true },
      { key: "k", code: "KeyK", metaKey: true, altKey: true },
      { key: "K", code: "KeyK", ctrlKey: true, shiftKey: true },
      { key: "k", code: "KeyK", metaKey: true, isComposing: true },
      { key: "k", code: "KeyK" },
    ]) {
      expect([fields, chord(document.body, fields)]).toEqual([fields, true]);
    }
    await settle();
    expect(palette()).toBeNull();
  });

  it("leaves a plain k to the Inbox, which moves", async () => {
    renderApp(client(), "#/alpha/inbox/PR-0042");
    const list = await screen.findByRole("listbox", { name: "Proposals by severity, then age" });
    const selected = within(list).getByRole("option", { selected: true });
    selected.focus();
    expect(fireEvent.keyDown(selected, { key: "k", code: "KeyK" })).toBe(false);
    await hashIs("#/alpha/inbox/PR-0041");
    expect(palette()).toBeNull();
  });

  it("over a decision dialog: no palette, the event still prevented", async () => {
    renderApp(client(), "#/alpha/inbox");
    const list = await screen.findByRole("listbox", { name: "Proposals by severity, then age" });
    const selected = within(list).getByRole("option", { selected: true });
    selected.focus();
    fireEvent.keyDown(selected, { key: "a" });
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-0041" });
    const inside = dialog.querySelector<HTMLElement>("textarea, button") ?? dialog;
    expect(chord(inside)).toBe(false);
    await settle();
    expect(palette()).toBeNull();
    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
  });

  it("over the open palette: focus back in its field", async () => {
    await home();
    const dialog = await openPalette();
    const field = combobox();
    // A click on the panel's text leaves focus on the panel (tabIndex -1), off the field.
    dialog.focus();
    expect(document.activeElement).toBe(dialog);
    expect(chord(dialog)).toBe(false);
    expect(document.activeElement).toBe(field);
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
  });
});

describe("one document key listener (AC-10)", () => {
  it.each(["#/", "#/alpha", "#/alpha/inbox", "#/alpha/tasks/T-0107", "#/alpha/tree/R-1", "#/alpha/graph/R-1", "#/alpha/health", "#/alpha/nowhere"])(
    "adds exactly the shell's on %s, once, and removes the same function on unmount",
    async (hash) => {
      const added = vi.spyOn(document, "addEventListener");
      const addedOnWindow = vi.spyOn(window, "addEventListener");
      const removed = vi.spyOn(document, "removeEventListener");
      const { unmount } = renderApp(client(), hash);
      await screen.findByRole("heading", { level: 1 });
      await settle();
      const keyed = (calls: unknown[][]) => calls.filter(([kind]) => typeof kind === "string" && kind.startsWith("key"));
      const onDocument = keyed(added.mock.calls);
      expect(onDocument.map(([kind, , options]) => [kind, options])).toEqual([["keydown", true]]);
      expect(keyed(addedOnWindow.mock.calls)).toEqual([]);
      unmount();
      const gone = keyed(removed.mock.calls);
      expect(gone.map(([kind, , options]) => [kind, options])).toEqual([["keydown", true]]);
      expect(gone[0]?.[1]).toBe(onDocument[0]?.[1]);
    },
  );

  it("does nothing for a plain j, a or ? on the document", async () => {
    const stub = await home();
    const before = callsOf(stub);
    for (const name of ["j", "a", "?", "/"]) {
      expect(fireEvent.keyDown(document, { key: name })).toBe(true);
    }
    await settle();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.location.hash).toBe("#/alpha");
    expect(callsOf(stub)).toEqual(before);
  });
});

describe("the dialog and its combobox (AC-11)", () => {
  it("is a labelled modal dialog with a combobox that keeps focus while the active option moves", async () => {
    await home();
    const dialog = await openPalette();
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(within(dialog).getByRole("heading", { level: 2, name: "Jump to" })).toBeTruthy();
    const field = combobox();
    expect(document.activeElement).toBe(field);
    expect([field.getAttribute("aria-expanded"), field.getAttribute("aria-autocomplete"), field.getAttribute("aria-controls")]).toEqual([
      "true",
      "list",
      listbox().id,
    ]);
    const options = within(listbox()).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "Overview",
      "Inbox",
      "Tasks",
      "Spec tree",
      "Graph",
      "Health",
      "Questions",
      "Alpha alpha",
      "Beta beta",
    ]);
    expect(activeOption()).toBe(options[0]);
    expect(options.filter((option) => option.getAttribute("aria-selected") === "true")).toEqual([options[0]]);
    key("ArrowDown");
    key("ArrowDown");
    expect(activeOption()).toBe(options[2]);
    key("ArrowUp");
    expect(activeOption()).toBe(options[1]);
    key("End");
    expect(activeOption()).toBe(options[8]);
    key("ArrowDown");
    expect(activeOption()).toBe(options[8]);
    key("Home");
    expect(activeOption()).toBe(options[0]);
    key("ArrowUp");
    expect(activeOption()).toBe(options[0]);
    expect(options[0]?.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(field);
    // Two tab stops, the field and Close; the options are never one.
    const close = within(dialog).getByRole("button", { name: "Close" });
    key("Tab", { shiftKey: true });
    expect(document.activeElement).toBe(close);
    fireEvent.keyDown(close, { key: "Tab" });
    expect(document.activeElement).toBe(field);
    expect(options.every((option) => !option.hasAttribute("tabindex"))).toBe(true);
  });

  it("closes by its Close button, focus back on the Jump to button", async () => {
    await home();
    const button = screen.getByRole("button", { name: "Jump to" });
    button.focus();
    fireEvent.click(button);
    const dialog = await screen.findByRole("dialog", { name: "Jump to" });
    type("t-0108");
    fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => {
      expect(palette()).toBeNull();
    });
    await settle();
    expect(document.activeElement).toBe(button);
    expect(window.location.hash).toBe("#/alpha");
  });

  it("closes on a click on the scrim, focus kept in the field by the press and back on main after", async () => {
    await home();
    await openPalette(document.body);
    const scrim = document.querySelector<HTMLElement>("[data-dialog-scrim]");
    if (scrim === null) {
      throw new Error("no scrim");
    }
    expect(fireEvent.mouseDown(scrim)).toBe(false);
    expect(document.activeElement).toBe(combobox());
    fireEvent.click(scrim);
    await waitFor(() => {
      expect(palette()).toBeNull();
    });
    await settle();
    expect(document.activeElement).toBe(screen.getByRole("main"));
    expect(window.location.hash).toBe("#/alpha");
  });

  it("stays open on an Esc that ends an IME composition, closes on the next", async () => {
    await home();
    await openPalette();
    type("\u306f\u3044");
    expect(fireEvent.keyDown(combobox(), { key: "Escape", isComposing: true })).toBe(true);
    await settle();
    expect(palette()).not.toBeNull();
    expect(combobox().value).toBe("\u306f\u3044");
    await closePalette();
  });

  it("names each option's element by its key, escaped: the active one keeps its ID while an answer moves the list", async () => {
    const stub = client();
    let release: () => void = () => undefined;
    stub.getTasks.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = () => {
            resolve({ tasks: structuredClone(TASKS), notes: [] });
          };
        }),
    );
    renderApp(stub, "#/alpha/inbox");
    await screen.findByRole("listbox", { name: "Proposals by severity, then age" });
    await openPalette();
    type("T-01");
    await waitFor(() => {
      expect(groups().map(([label]) => label)).toEqual(["Tasks, loading", "Inbox", "Spec tree"]);
    });
    key("ArrowDown");
    const search = activeOption();
    expect(search?.textContent).toBe("Search the spec for 'T-01'");
    const id = search?.id ?? "";
    await act(async () => {
      release();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(groups()[0]?.[0]).toBe("Tasks");
    });
    expect(combobox().getAttribute("aria-activedescendant")).toBe(id);
    expect(activeOption()?.textContent).toBe("Search the spec for 'T-01'");
    const ids = within(listbox())
      .getAllByRole("option")
      .map((option) => option.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.filter((each) => /\s/.test(each))).toEqual([]);
  });

  it("gives a hit whose path has a space an ID without one, reachable as the active descendant", async () => {
    const stub = await home();
    const path = "docs/spec/\u043f\u0440\u0438\u043b\u0438\u0432 \u0438 \u043e\u0442\u043b\u0438\u0432.md";
    stub.search.mockResolvedValue(aSearchResults([aSearchHit({ id: null, path, title: null })]));
    await openPalette();
    type("\u043f\u0440\u0438\u043b\u0438\u0432");
    expect(activeOption()?.textContent).toBe("Search the spec for '\u043f\u0440\u0438\u043b\u0438\u0432'");
    key("Enter");
    await within(listbox()).findByRole("option", { name: path });
    key("End");
    expect(activeOption()?.textContent).toBe(path);
    expect(activeOption()?.id).not.toMatch(/\s/);
  });

  it("closes on Esc, focus back on the Jump to button that opened it", async () => {
    await home();
    const button = screen.getByRole("button", { name: "Jump to" });
    expect([button.getAttribute("aria-haspopup"), button.getAttribute("aria-keyshortcuts")]).toEqual(["dialog", "Meta+K Control+K"]);
    button.focus();
    fireEvent.click(button);
    await screen.findByRole("dialog", { name: "Jump to" });
    await closePalette();
    expect(document.activeElement).toBe(button);
  });

  it("closes on Esc, focus back on the task row the chord was pressed on", async () => {
    renderApp(client(), "#/alpha/tasks");
    await screen.findByRole("listbox", { name: "Tasks, what waits for you first" });
    const row = document.querySelector<HTMLElement>('[data-task="T-0108"]');
    if (row === null) {
      throw new Error("no row T-0108");
    }
    row.focus();
    await openPalette(row);
    await closePalette();
    expect(document.activeElement).toBe(row);
  });

  it("closes on Esc, focus on main when the chord came from the body", async () => {
    await home();
    expect(document.activeElement).toBe(document.body);
    await openPalette(document.body);
    await closePalette();
    expect(document.activeElement).toBe(screen.getByRole("main"));
  });

  it("counts the options in the polite live region, outside the inert page", async () => {
    await home();
    await openPalette();
    type("t-0108");
    await waitFor(() => {
      expect(politeRegion().textContent).toBe(`${String(within(listbox()).getAllByRole("option").length)} options`);
    });
    expect(isInert(politeRegion())).toBe(false);
    expect(isInert(screen.getByRole("main"))).toBe(true);
    type("Pilot boat dispatch");
    await waitFor(() => {
      expect(politeRegion().textContent).toBe(`${String(within(listbox()).getAllByRole("option").length)} options`);
    });
  });
});

describe("what the palette reads (AC-12)", () => {
  it("reads nothing on the home when opened or typed in", async () => {
    const stub = await home();
    const before = callsOf(stub);
    await openPalette();
    for (const text of "a long query typed!!".split("").map((_, index, all) => all.slice(0, index + 1).join(""))) {
      type(text);
    }
    expect(combobox().value).toBe("a long query typed!!");
    await settle();
    expect(callsOf(stub)).toEqual(before);
  });

  it("reads at most the tasks and the inbox once when opened elsewhere, nothing when opened again", async () => {
    const stub = client();
    renderApp(stub, "#/alpha/tree");
    await screen.findByRole("heading", { level: 1, name: "Spec tree" });
    await waitFor(() => {
      expect(stub.getInbox).toHaveBeenCalledTimes(1);
    });
    await openPalette();
    type("T-01");
    await waitFor(() => {
      expect(groups()[0]?.[0]).toBe("Tasks");
    });
    expect([stub.getInbox.mock.calls.length, stub.getTasks.mock.calls.length]).toEqual([1, 1]);
    await closePalette();
    await openPalette();
    type("T-01");
    await settle();
    expect([stub.getInbox.mock.calls.length, stub.getTasks.mock.calls.length]).toEqual([1, 1]);
  });

  it("searches the spec once per activation, the text as typed, spaces kept", async () => {
    const stub = await home();
    stub.search.mockResolvedValue(aSearchResults([aSearchHit({ id: "RULE-TIDE-WINDOW", title: "Entry window" })]));
    await openPalette();
    type("  harbour pilots ");
    expect(activeOption()?.textContent).toBe("Search the spec for '  harbour pilots '");
    key("Enter");
    await waitFor(() => {
      expect(groups().find(([label]) => label === "Spec tree")?.[1]).toEqual([
        "Search the spec for '  harbour pilots '",
        "Open 'harbour pilots' in the spec tree",
        "RULE-TIDE-WINDOW: Entry window",
      ]);
    });
    expect(stub.search.mock.calls).toEqual([["alpha", { query: "  harbour pilots " }]]);
    expect(activeOption()?.textContent).toBe("Search the spec for '  harbour pilots '");
    key("Enter");
    await waitFor(() => {
      expect(stub.search).toHaveBeenCalledTimes(2);
    });
    type("  harbour pilots");
    expect(groups().find(([label]) => label === "Spec tree")?.[1]).toHaveLength(2);
    expect(stub.search).toHaveBeenCalledTimes(2);
  });
});

describe("matching and jumps (AC-13)", () => {
  it("finds a decomposed title with a precomposed query", async () => {
    await home(homeClient([aTaskEntry({ id: "T-1", status: "draft", title: "Cafe\u0301 berth" })]));
    await openPalette();
    type("caf\u00e9");
    expect(groups()[0]).toEqual(["Tasks", ["T-1: Cafe\u0301 berthState: Draft"]]);
  });

  it("puts T-0108 first for t-0108, its group first; Enter jumps with one history entry and focuses its heading", async () => {
    await home();
    await openPalette();
    type("t-0108");
    expect(groups().map(([label]) => label)).toEqual(["Tasks", "Inbox", "Spec tree"]);
    expect(activeOption()?.textContent).toBe("T-0108: Speed limit on night passagesState: Ready");
    const before = window.history.length;
    key("Enter");
    await hashIs("#/alpha/tasks/T-0108");
    expect(window.history.length).toBe(before + 1);
    expect(palette()).toBeNull();
    const heading = await screen.findByRole("heading", { level: 1, name: /^T-0108/ });
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
  });

  it("focuses the next task's heading within the same view, T-0107 to T-0108", async () => {
    renderApp(client(), "#/alpha/tasks/T-0107");
    const first = await screen.findByRole("heading", { level: 1, name: /^T-0107/ });
    const before = window.history.length;
    await openPalette();
    type("T-0108");
    key("Enter");
    await hashIs("#/alpha/tasks/T-0108");
    expect(window.history.length).toBe(before + 1);
    const heading = await screen.findByRole("heading", { level: 1, name: /^T-0108/ });
    expect(heading).not.toBe(first);
    await waitFor(() => {
      expect(document.activeElement).toBe(heading);
    });
  });

  it("jumps by a click, and to the hash shown with no history entry, focus on its heading", async () => {
    await home();
    await openPalette();
    const before = window.history.length;
    fireEvent.click(within(listbox()).getByRole("option", { name: "Overview" }));
    await waitFor(() => {
      expect(palette()).toBeNull();
    });
    await settle();
    expect(window.location.hash).toBe("#/alpha");
    expect(window.history.length).toBe(before);
    expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1, name: "Overview" }));
  });

  it("opens a hit with no ID by its path", async () => {
    const stub = await home();
    stub.search.mockResolvedValue(aSearchResults([aSearchHit({ id: null, path: "docs/spec/tides/tide-cycle.md", title: null })]));
    await openPalette();
    type("cycle");
    key("Enter");
    const hit = await within(listbox()).findByRole("option", { name: "docs/spec/tides/tide-cycle.md" });
    fireEvent.click(hit);
    await hashIs("#/alpha/tree/docs%2Fspec%2Ftides%2Ftide-cycle.md");
  });

  it("opens the REF as typed, trimmed and unchecked, with no call", async () => {
    const stub = await home();
    const before = callsOf(stub);
    await openPalette();
    type(" MEC-TIDES#RULE-TIDE-WINDOW ");
    fireEvent.click(within(listbox()).getByRole("option", { name: "Open 'MEC-TIDES#RULE-TIDE-WINDOW' in the spec tree" }));
    await hashIs("#/alpha/tree/MEC-TIDES%23RULE-TIDE-WINDOW");
    expect(callsOf(stub).search).toBeUndefined();
    expect(callsOf(stub).getTasks).toBe(before.getTasks);
  });

  it("shows hostile text as text", async () => {
    await home(homeClient([aTaskEntry({ id: "T-1", title: HOSTILE })], [aProposal({ id: "PR-1", summary: HOSTILE })]));
    await openPalette();
    type("onerror");
    expect(groups()).toEqual([
      ["Tasks", [`T-1: ${HOSTILE}State: Draft`]],
      ["Inbox", [`PR-1: ${HOSTILE}Status: Open`]],
      ["Spec tree", ["Search the spec for 'onerror'", "Open 'onerror' in the spec tree"]],
    ]);
    expect(document.querySelector("img")).toBeNull();
  });

  it("says ten of a larger group", async () => {
    const many = Array.from({ length: 12 }, (_, index) => aTaskEntry({ id: `T-${String(200 + index)}` }));
    await home(homeClient(many));
    await openPalette();
    type("T-2");
    expect(groups()[0]?.[0]).toBe("Tasks, 10 of 12");
    expect(groups()[0]?.[1]).toHaveLength(10);
  });
});

describe("failures in their group (AC-14)", () => {
  it("says the tasks could not be read and keeps Sections and the Inbox", async () => {
    const stub = client();
    stub.getTasks.mockRejectedValue(new ClientError({ status: 503, message: "t" }));
    renderApp(stub, "#/alpha");
    await within(await screen.findByRole("region", { name: "Tasks" })).findByText("t");
    await within(screen.getByRole("region", { name: "Inbox" })).findByRole("link", { name: /^PR-0041/ });
    await openPalette();
    type("in");
    await waitFor(() => {
      expect(within(palette() ?? document.body).getByRole("alert").textContent).toBe("Tasks could not be read: t");
    });
    expect(groups().map(([label]) => label)).toEqual(["Sections", "Inbox", "Spec tree"]);
    expect(stub.getTasks).toHaveBeenCalledTimes(2);
    expect(combobox().value).toBe("in");
  });

  it("says tasks the daemon does not serve yet as not built: a status, no alert role, R-n7", async () => {
    const stub = client();
    const message = "Not served by the daemon yet: GET /api/projects/alpha/tasks is a missing endpoint";
    stub.getTasks.mockRejectedValue(new ClientError({ status: 501, message }, { notServed: true }));
    renderApp(stub, "#/alpha");
    await within(await screen.findByRole("region", { name: "Tasks" })).findByText(message);
    await within(screen.getByRole("region", { name: "Inbox" })).findByRole("link", { name: /^PR-0041/ });
    await openPalette();
    type("in");
    const dialog = palette() ?? document.body;
    await waitFor(() => {
      expect(within(dialog).getByText(message).closest("p")?.textContent).toBe(`Tasks: ${message}`);
    });
    expect(within(dialog).getByText(message).closest("p")?.getAttribute("role")).toBe("status");
    expect(within(dialog).queryByRole("alert")).toBeNull();
    expect(groups().map(([label]) => label)).toEqual(["Sections", "Inbox", "Spec tree"]);
  });

  it("says the search could not be read, stays open and keeps the text", async () => {
    const stub = await home();
    stub.search.mockRejectedValue(new ClientError({ status: 503, message: "s" }));
    await openPalette();
    type("harbour");
    expect(activeOption()?.textContent).toBe("Search the spec for 'harbour'");
    key("Enter");
    const dialog = palette() ?? document.body;
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Search could not be read: s");
    expect(palette()).not.toBeNull();
    expect(combobox().value).toBe("harbour");
    expect(groups().map(([label]) => label)).toEqual(["Spec tree"]);
    type("harbours");
    expect(within(dialog).queryByRole("alert")).toBeNull();
  });

  it("uses the route's project while the projects are on their way: Sections, Tasks and a jump work", async () => {
    const stub = client();
    stub.getProjects.mockReturnValue(new Promise<never>(() => undefined));
    renderApp(stub, "#/alpha/inbox");
    await screen.findByRole("listbox", { name: "Proposals by severity, then age" });
    await openPalette();
    expect(groups()).toEqual([
      ["Sections", ["Overview", "Inbox", "Tasks", "Spec tree", "Graph", "Health", "Questions"]],
      ["Projects, loading", []],
    ]);
    expect(within(palette() ?? document.body).queryByText("No project yet.")).toBeNull();
    type("t-0108");
    await waitFor(() => {
      expect(groups()[0]?.[0]).toBe("Tasks");
    });
    key("Home");
    expect(activeOption()?.textContent).toBe("T-0108: Speed limit on night passagesState: Ready");
    key("Enter");
    await hashIs("#/alpha/tasks/T-0108");
  });

  it("uses the route's project when the projects failed: their failure said, Sections and the Inbox listed", async () => {
    const stub = client();
    stub.getProjects.mockRejectedValue(new ClientError({ status: 503, message: "p" }));
    renderApp(stub, "#/alpha/inbox");
    await screen.findByText("Projects could not be loaded:");
    await openPalette();
    expect(within(palette() ?? document.body).getByRole("alert").textContent).toBe("Projects could not be read: p");
    expect(groups()).toEqual([["Sections", ["Overview", "Inbox", "Tasks", "Spec tree", "Graph", "Health", "Questions"]]]);
    fireEvent.click(within(listbox()).getByRole("option", { name: "Tasks" }));
    await hashIs("#/alpha/tasks");
    await openPalette();
    type("pr-0042");
    await waitFor(() => {
      expect(groups()[0]).toEqual(["Inbox", ["PR-0042: Berth T-0108 draftStatus: Open"]]);
    });
  });

  it("lists Projects alone when no project is served", async () => {
    const stub = homeClient([]);
    stub.getProjects.mockResolvedValue([]);
    renderApp(stub, "#/");
    await screen.findByRole("heading", { level: 1, name: "No projects" });
    await openPalette();
    expect(within(palette() ?? document.body).getByText("No project yet.")).toBeTruthy();
    expect(groups()).toEqual([]);
  });
});

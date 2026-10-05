import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { ShownLinks } from "../api/types";
import { aLink, aProposal, aSearchHit, aSearchResults, aTreeView } from "../test/builders";
import { renderApp } from "../test/render";
import { START_ROWS, TREE, treeClient, viewOf } from "../test/treeStub";
import type { StubClient } from "../test/stubClient";

// docs/features/ui-tree-node.md on a stub client: AC-01 (what a route reads), AC-03 (the tree's
// rows and ARIA), AC-04 (its keys), AC-11 (search), AC-12 (states of the tree and the search),
// AC-13 (archive reads again).

function tree(): HTMLElement {
  return screen.getByRole("tree", { name: "Spec tree" });
}

function rows(): HTMLElement[] {
  return within(tree()).getAllByRole("treeitem");
}

function nameOf(item: HTMLElement): string {
  return item.querySelector(".tree-row-name")?.textContent ?? "";
}

function row(name: string, nth = 0): HTMLElement {
  const found = rows().filter((item) => nameOf(item) === name)[nth];
  if (found === undefined) {
    throw new Error(`no row ${name}`);
  }
  return found;
}

function currentNames(): string[] {
  return rows()
    .filter((item) => item.getAttribute("aria-current") === "page")
    .map(nameOf);
}

async function openTree(client: StubClient, hash = "#/alpha/tree") {
  renderApp(client, hash);
  await screen.findByRole("tree", { name: "Spec tree" });
  return client;
}

/** The page's polite live region (src/ui/announcer.tsx), outside the app's root. */
function politeRegion(): HTMLElement {
  const region = document.querySelector<HTMLElement>('[aria-live="polite"]');
  if (region === null) {
    throw new Error("no polite live region");
  }
  return region;
}

/**
 * A click on a link as a browser takes it. jsdom follows every clicked hash link a task later,
 * modifiers or not; a browser opens a Cmd-, Ctrl- or Shift-clicked one elsewhere (Alt downloads
 * it), so this page stays: such a click's default is cancelled after the app has seen it.
 */
function clickLink(anchor: HTMLElement, modifiers: { metaKey?: boolean; ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean } = {}) {
  if (Object.values(modifiers).some(Boolean)) {
    window.addEventListener(
      "click",
      (event) => {
        event.preventDefault();
      },
      { once: true },
    );
  }
  fireEvent.click(anchor, modifiers);
}

/** Runs the tasks jsdom queued: a clicked link's navigation, a hashchange. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** The browser following a hash link: the hash set, its hashchange (a task of its own in jsdom) awaited. */
async function followTo(hash: string) {
  const changed = new Promise<void>((resolve) => {
    window.addEventListener(
      "hashchange",
      () => {
        resolve();
      },
      { once: true },
    );
  });
  await act(async () => {
    window.location.hash = hash;
    await changed;
  });
}

async function focused(element: HTMLElement) {
  await waitFor(() => {
    expect(document.activeElement).toBe(element);
  });
}

describe("what a route reads (AC-01)", () => {
  it("reads one tree and one node for #/alpha/tree/DOC-A; no bundle, no search", async () => {
    const client = await openTree(treeClient(), "#/alpha/tree/DOC-A");
    await screen.findByRole("tab", { name: "Text" });
    await waitFor(() => {
      expect(currentNames()).toEqual(["DOC-A"]);
    });
    expect(client.getTree).toHaveBeenCalledTimes(1);
    expect(client.getTree).toHaveBeenCalledWith("alpha");
    expect(client.getNode).toHaveBeenCalledTimes(1);
    expect(client.getNode).toHaveBeenCalledWith("alpha", "DOC-A");
    expect(client.getBundle).not.toHaveBeenCalled();
    expect(client.search).not.toHaveBeenCalled();
  });

  it("reads no node for #/alpha/tree and names the view in its h1", async () => {
    const client = await openTree(treeClient());
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Spec tree");
    expect(client.getNode).not.toHaveBeenCalled();
  });
});

describe("the tree's rows (AC-03)", () => {
  it("lists the rows in pre-order with the roots open; level is depth + 1", async () => {
    await openTree(treeClient());
    expect(rows().map(nameOf)).toEqual(START_ROWS);
    for (const item of rows()) {
      const node = TREE.find((candidate) => (candidate.id ?? candidate.path) === nameOf(item));
      expect([nameOf(item), item.getAttribute("aria-level")]).toEqual([nameOf(item), String((node?.depth ?? -9) + 1)]);
    }
  });

  it("gives each row its place among its siblings, aria-expanded on parents only", async () => {
    await openTree(treeClient());
    const place = (item: HTMLElement) => [item.getAttribute("aria-posinset"), item.getAttribute("aria-setsize")];
    expect(["DOC-A", "DOC-C", "DOC-D", "DOC-F"].map((name) => place(row(name)))).toEqual([
      ["1", "4"],
      ["2", "4"],
      ["3", "4"],
      ["4", "4"],
    ]);
    expect(["SEC-A1", "SEC-DUP", "docs/spec/b.md"].map((name) => place(row(name)))).toEqual([
      ["1", "3"],
      ["2", "3"],
      ["3", "3"],
    ]);
    expect(row("DOC-A").getAttribute("aria-expanded")).toBe("true");
    expect(row("SEC-A1").getAttribute("aria-expanded")).toBe("false");
    for (const leaf of ["SEC-DUP", "DOC-C", "DOC-E", "DOC-F"]) {
      expect([leaf, row(leaf).hasAttribute("aria-expanded")]).toEqual([leaf, false]);
    }
  });

  it("names the ID-less row by its path and shows a missing kind as -", async () => {
    await openTree(treeClient());
    const idless = row("docs/spec/b.md");
    expect(idless.textContent).toContain("Bee notes");
    expect(idless.querySelector(".kind-tag")?.textContent).toBe("Kind: -");
  });

  it("says mark, archived, status and the inbox count in words, a mark with its icon", async () => {
    const proposals = [
      aProposal({ id: "PR-1", target_id: "DOC-A" }),
      aProposal({ id: "PR-2", target_ids: ["R-9", "DOC-A"] }),
      aProposal({ id: "PR-3", target_ids: ["docs/spec/b.md"] }),
    ];
    await openTree(treeClient(proposals));
    expect(row("DOC-C").textContent).toContain("Parent dangling");
    expect(row("DOC-C").querySelector(".badge svg")).not.toBeNull();
    expect(row("DOC-D").textContent).toContain("Parent cycle");
    expect(row("DOC-F").textContent).toContain("archived");
    expect(row("DOC-A").textContent).toContain("Status: alpha-live");
    expect(row("DOC-A").textContent).toContain("rev 2");
    await waitFor(() => {
      expect(row("DOC-A").textContent).toContain("2 in inbox");
    });
    expect(row("docs/spec/b.md").textContent).toContain("1 in inbox");
    expect(row("DOC-C").textContent).not.toContain("in inbox");
  });

  it("nests the cycle by depth: the second member under the root marked parent cycle", async () => {
    await openTree(treeClient());
    const names = rows().map(nameOf);
    expect(names.indexOf("DOC-E")).toBe(names.indexOf("DOC-D") + 1);
    expect(row("DOC-D").getAttribute("aria-level")).toBe("1");
    expect(row("DOC-E").getAttribute("aria-level")).toBe("2");
  });

  it("finds a row's parent by depth, never by name: the second holder's child reveals under it", async () => {
    await openTree(treeClient(), "#/alpha/tree/SEC-DUP-KID");
    await waitFor(() => {
      expect(currentNames()).toEqual(["SEC-DUP-KID"]);
    });
    expect(row("docs/spec/b.md").getAttribute("aria-expanded")).toBe("true");
    expect(row("SEC-DUP", 1).getAttribute("aria-expanded")).toBe("true");
    expect(row("SEC-DUP", 0).hasAttribute("aria-expanded")).toBe(false);
    expect(row("SEC-DUP-KID").getAttribute("aria-level")).toBe("4");
  });

  it("says a cut tree was cut and offers a narrower root, read with `root`", async () => {
    const client = treeClient();
    client.getTree.mockImplementation((_project, options) =>
      Promise.resolve(
        options?.root === undefined
          ? aTreeView(TREE.slice(0, 4), { truncated: true })
          : aTreeView(TREE.slice(0, 4), { ref: options.root }),
      ),
    );
    await openTree(client);
    expect(screen.getByText("Showing the first 4 nodes: the list was cut")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Show DOC-A as root" }));
    await waitFor(() => {
      expect(client.getTree).toHaveBeenLastCalledWith("alpha", { root: "DOC-A" });
    });
    expect(await screen.findByText("Rooted at")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Whole tree" }));
    await waitFor(() => {
      expect(client.getTree).toHaveBeenLastCalledWith("alpha");
    });
  });

  it("lists the daemon's notes and what was left out below the tree", async () => {
    const client = treeClient();
    client.getTree.mockResolvedValue(aTreeView(TREE, { notes: ["a note from the daemon"], left_out: { generated: 1, tier3: 3 } }));
    await openTree(client);
    expect(screen.getByText("a note from the daemon")).toBeTruthy();
    expect(screen.getByText(/Left out: 1 generated, 3 archived/)).toBeTruthy();
  });
});

describe("the tree's keys (AC-04)", () => {
  it("is one tab stop", async () => {
    await openTree(treeClient());
    expect(rows().filter((item) => item.getAttribute("tabindex") === "0")).toHaveLength(1);
    expect(rows().filter((item) => item.tabIndex === 0).map(nameOf)).toEqual(["DOC-A"]);
  });

  it("moves with Down, Up, j, k, Home and End, pushing no history entry", async () => {
    await openTree(treeClient());
    const before = window.history.length;
    row("DOC-A").focus();
    fireEvent.keyDown(row("DOC-A"), { key: "ArrowDown" });
    await focused(row("SEC-A1"));
    fireEvent.keyDown(row("SEC-A1"), { key: "j" });
    await focused(row("SEC-DUP"));
    fireEvent.keyDown(row("SEC-DUP"), { key: "k" });
    await focused(row("SEC-A1"));
    fireEvent.keyDown(row("SEC-A1"), { key: "ArrowUp" });
    await focused(row("DOC-A"));
    fireEvent.keyDown(row("DOC-A"), { key: "End" });
    await focused(row("DOC-F"));
    fireEvent.keyDown(row("DOC-F"), { key: "Home" });
    await focused(row("DOC-A"));
    expect(window.history.length).toBe(before);
    expect(window.location.hash).toBe("#/alpha/tree");
    expect(rows().filter((item) => item.tabIndex === 0).map(nameOf)).toEqual(["DOC-A"]);
  });

  it("expands with Right, then enters; collapses with Left, then climbs", async () => {
    await openTree(treeClient());
    row("SEC-A1").focus();
    fireEvent.keyDown(row("SEC-A1"), { key: "ArrowRight" });
    await waitFor(() => {
      expect(row("SEC-A1").getAttribute("aria-expanded")).toBe("true");
    });
    expect(document.activeElement).toBe(row("SEC-A1"));
    fireEvent.keyDown(row("SEC-A1"), { key: "ArrowRight" });
    await focused(row("SEC-A1X"));
    fireEvent.keyDown(row("SEC-A1X"), { key: "ArrowLeft" });
    await focused(row("SEC-A1"));
    fireEvent.keyDown(row("SEC-A1"), { key: "ArrowLeft" });
    await waitFor(() => {
      expect(row("SEC-A1").getAttribute("aria-expanded")).toBe("false");
    });
    expect(rows().map(nameOf)).not.toContain("SEC-A1X");
    fireEvent.keyDown(row("SEC-A1"), { key: "ArrowLeft" });
    await focused(row("DOC-A"));
  });

  it("opens with Enter: one history entry, the node shown, focus kept on the row", async () => {
    const client = await openTree(treeClient());
    const before = window.history.length;
    row("SEC-A1").focus();
    fireEvent.keyDown(row("SEC-A1"), { key: "Enter" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/tree/SEC-A1");
    });
    expect(window.history.length).toBe(before + 1);
    await waitFor(() => {
      expect(currentNames()).toEqual(["SEC-A1"]);
    });
    expect(client.getNode).toHaveBeenCalledWith("alpha", "SEC-A1");
    expect(document.activeElement).toBe(row("SEC-A1"));
  });

  it("opens with a click: one history entry", async () => {
    await openTree(treeClient());
    const before = window.history.length;
    fireEvent.click(row("DOC-C"));
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/tree/DOC-C");
    });
    expect(window.history.length).toBe(before + 1);
  });

  it("ignores keys with Ctrl, Meta or Alt, keys from the search field and keys outside the tree", async () => {
    await openTree(treeClient());
    row("DOC-A").focus();
    for (const modifier of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }]) {
      fireEvent.keyDown(row("DOC-A"), { key: "ArrowDown", ...modifier });
      fireEvent.keyDown(row("DOC-A"), { key: "Enter", ...modifier });
    }
    fireEvent.keyDown(document.body, { key: "ArrowDown" });
    fireEvent.keyDown(document, { key: "j" });
    await act(async () => {
      await Promise.resolve();
    });
    expect(document.activeElement).toBe(row("DOC-A"));
    expect(window.location.hash).toBe("#/alpha/tree");
    const field = screen.getByLabelText("Search the spec");
    field.focus();
    for (const key of ["j", "k", "?", "ArrowDown"]) {
      fireEvent.keyDown(field, { key });
    }
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(field);
    expect(rows().filter((item) => item.tabIndex === 0).map(nameOf)).toEqual(["DOC-A"]);
  });

  describe("after a link in the node pane", () => {
    /** DOC-A's links name SEC-A1 and DOC-A itself; the default bundle's Targets name DOC-A. */
    function linkClient() {
      const links: ShownLinks = {
        outgoing: [
          aLink({ type: "zeta_type", written: "SEC-A1", name: "SEC-A1", line: 2 }),
          aLink({ type: "zeta_type", written: "DOC-A", name: "DOC-A", line: 3 }),
        ],
        incoming: [],
        left_out: { generated: 0, tier3: 0 },
        omitted: 0,
      };
      const client = treeClient();
      client.getNode.mockImplementation((_project, ref, options) => {
        const view = viewOf(ref, false);
        const withLinks = options?.with?.includes("links") === true;
        return Promise.resolve(withLinks ? { ...view, nodes: view.nodes.map((node) => ({ ...node, links })) } : view);
      });
      return client;
    }

    async function linksPanel(): Promise<HTMLElement> {
      fireEvent.click(await screen.findByRole("tab", { name: "Links" }));
      const panel = await screen.findByRole("tabpanel", { name: "Links" });
      await within(panel).findByRole("link", { name: "SEC-A1" });
      return panel;
    }

    async function enterOn(name: string) {
      row(name).focus();
      fireEvent.keyDown(row(name), { key: "Enter" });
      await waitFor(() => {
        expect(screen.getByRole("heading", { level: 1 }).textContent).toContain(name);
      });
      await settle();
    }

    it("focuses the new node's heading after a plain click that moves the page", async () => {
      await openTree(linkClient(), "#/alpha/tree/DOC-A");
      const panel = await linksPanel();
      clickLink(within(panel).getByRole("link", { name: "SEC-A1" }));
      await followTo("#/alpha/tree/SEC-A1");
      await waitFor(() => {
        expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1 }));
      });
      expect(document.activeElement?.textContent).toContain("SEC-A1");
    });

    it("keeps focus on the row after Enter when a link was Cmd-, Ctrl- or Shift-clicked, or named the open node", async () => {
      await openTree(linkClient(), "#/alpha/tree/DOC-A");
      const panel = await linksPanel();
      const toSection = within(panel).getByRole("link", { name: "SEC-A1" });
      for (const modifier of [{ metaKey: true }, { ctrlKey: true }, { shiftKey: true }, { altKey: true }]) {
        clickLink(toSection, modifier);
      }
      clickLink(within(panel).getByRole("link", { name: "DOC-A" }));
      fireEvent.click(screen.getByRole("tab", { name: "Bundle" }));
      const bundle = await screen.findByRole("tabpanel", { name: "Bundle" });
      const target = await within(bundle).findByRole("link", { name: "DOC-A" });
      expect(target.getAttribute("href")).toBe(window.location.hash);
      clickLink(target);
      await settle();
      expect(window.location.hash).toBe("#/alpha/tree/DOC-A");
      await enterOn("SEC-A1");
      expect(window.location.hash).toBe("#/alpha/tree/SEC-A1");
      expect(document.activeElement).toBe(row("SEC-A1"));
    });

    it("ends a follow on a hash change that opens no new node: Enter on a row then keeps focus there", async () => {
      // The same node by another spelling of its REF: the hash changes, the pane stays.
      await openTree(linkClient(), "#/alpha/tree/DOC%2DA");
      const panel = await linksPanel();
      clickLink(within(panel).getByRole("link", { name: "DOC-A" }));
      await followTo("#/alpha/tree/DOC-A");
      await settle();
      expect(screen.getByRole("tabpanel", { name: "Links" })).toBe(panel);
      await enterOn("SEC-A1");
      expect(document.activeElement).toBe(row("SEC-A1"));
    });
  });

  it("lists the tree's keys on ?", async () => {
    await openTree(treeClient());
    fireEvent.keyDown(row("DOC-A"), { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(dialog.textContent).toContain("Tree: expand, else go to the first child");
    expect(dialog.textContent).toContain("Search: back to the tree");
    expect(Array.from(dialog.querySelectorAll("kbd"), (key) => key.textContent)).toContain("Right arrow");
  });
});

describe("search (AC-11)", () => {
  function searchClient() {
    const client = treeClient();
    client.search.mockImplementation((_project, options) =>
      Promise.resolve(
        aSearchResults(
          [
            aSearchHit({
              id: "SEC-A1",
              path: "docs/spec/a.md",
              line: 12,
              snippet: {
                segments: [
                  { text: "the ", hit: false },
                  { text: "Widget", hit: true },
                  { text: " and **bold** text", hit: false },
                ],
                cut_start: true,
                cut_end: true,
              },
            }),
            aSearchHit({ id: "DOC-C", path: "docs/spec/c.md", line: 1, archived: true }),
          ],
          { query: options.query, archive: options.archive ?? false, tier3_left_out: 2, notes: ["search terms shorter than 3 characters dropped: `a`"] },
        ),
      ),
    );
    return client;
  }

  it("calls once per Enter, never per keystroke, the query as typed", async () => {
    const client = await openTree(searchClient());
    const field = screen.getByLabelText("Search the spec");
    for (const value of ["w", "wi", "a widget"]) {
      fireEvent.change(field, { target: { value } });
    }
    expect(client.search).not.toHaveBeenCalled();
    fireEvent.submit(screen.getByRole("search"));
    await screen.findByRole("listbox", { name: /Search hits/ });
    expect(client.search).toHaveBeenCalledTimes(1);
    expect(client.search).toHaveBeenCalledWith("alpha", { query: "a widget" });
    fireEvent.submit(screen.getByRole("search"));
    await waitFor(() => {
      expect(client.search).toHaveBeenCalledTimes(2);
    });
  });

  it("passes archive when it is on", async () => {
    const client = await openTree(searchClient());
    fireEvent.click(screen.getByLabelText("Include archive"));
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "widget" } });
    fireEvent.submit(screen.getByRole("search"));
    await waitFor(() => {
      expect(client.search).toHaveBeenCalledWith("alpha", { query: "widget", archive: true });
    });
  });

  it("marks hits, shows cut ends, counts, notes and archived matches; the ** stays text", async () => {
    await openTree(searchClient());
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "widget" } });
    fireEvent.submit(screen.getByRole("search"));
    const list = await screen.findByRole("listbox", { name: /Search hits/ });
    const first = within(list).getAllByRole("option")[0];
    if (first === undefined) {
      throw new Error("no hit");
    }
    expect(Array.from(first.querySelectorAll("mark"), (mark) => mark.textContent)).toEqual(["Widget"]);
    expect(first.querySelector(".snippet")?.textContent).toBe("…the Widget and **bold** text…");
    expect(first.querySelector("strong, b")).toBeNull();
    expect(screen.getByText("2 hits (limit 20)", { selector: ".search-count" })).toBeTruthy();
    expect(screen.getByText(/2 archived matches left out/)).toBeTruthy();
    expect(screen.getByText("search terms shorter than 3 characters dropped: `a`")).toBeTruthy();
    await waitFor(() => {
      expect(politeRegion().textContent).toBe("2 hits (limit 20)");
    });
    expect(screen.queryByRole("tree")).toBeNull();
  });

  it("goes back to the tree's row on Esc", async () => {
    await openTree(searchClient());
    row("SEC-DUP").focus();
    fireEvent.keyDown(row("SEC-DUP"), { key: "k" });
    await focused(row("SEC-A1"));
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "widget" } });
    fireEvent.submit(screen.getByRole("search"));
    const list = await screen.findByRole("listbox", { name: /Search hits/ });
    const first = within(list).getAllByRole("option")[0];
    first?.focus();
    fireEvent.keyDown(first ?? list, { key: "Escape" });
    await screen.findByRole("tree");
    await focused(row("SEC-A1"));
  });

  it("goes back from the search field on Esc too", async () => {
    await openTree(searchClient());
    const field = screen.getByLabelText("Search the spec");
    fireEvent.change(field, { target: { value: "widget" } });
    fireEvent.submit(screen.getByRole("search"));
    await screen.findByRole("listbox", { name: /Search hits/ });
    field.focus();
    fireEvent.keyDown(field, { key: "Escape" });
    await screen.findByRole("tree");
    await focused(row("DOC-A"));
  });

  it("moves through hits with the keys, opens one with Enter, and Esc lands on the opened node's row", async () => {
    await openTree(searchClient());
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "widget" } });
    fireEvent.submit(screen.getByRole("search"));
    const list = await screen.findByRole("listbox", { name: /Search hits/ });
    const options = within(list).getAllByRole("option");
    options[0]?.focus();
    fireEvent.keyDown(options[0] ?? list, { key: "j" });
    await waitFor(() => {
      expect(document.activeElement).toBe(within(list).getAllByRole("option")[1]);
    });
    const before = window.history.length;
    fireEvent.keyDown(document.activeElement ?? list, { key: "Enter" });
    await waitFor(() => {
      expect(window.location.hash).toBe("#/alpha/tree/DOC-C");
    });
    expect(window.history.length).toBe(before + 1);
    expect(document.activeElement?.getAttribute("role")).toBe("option");
    fireEvent.keyDown(document.activeElement ?? list, { key: "Escape" });
    await screen.findByRole("tree");
    await focused(row("DOC-C"));
  });

  it("says when nothing matches, with the next step; Enter on Back to tree restores it", async () => {
    const client = treeClient();
    client.search.mockResolvedValue(aSearchResults([], { query: "zzz" }));
    await openTree(client);
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "zzz" } });
    fireEvent.submit(screen.getByRole("search"));
    expect(await screen.findByText(/No node matches/)).toBeTruthy();
    expect(screen.getByText(/Next: every word of three or more letters/)).toBeTruthy();
    const back = screen.getByRole("button", { name: "Back to tree" });
    back.focus();
    // A browser clicks a focused button on Enter unless the keydown was prevented.
    expect(fireEvent.keyDown(back, { key: "Enter" })).toBe(true);
    fireEvent.click(back);
    expect(await screen.findByRole("tree")).toBeTruthy();
    expect(window.location.hash).toBe("#/alpha/tree");
  });

  it("leaves Enter on Back to tree to the button: the tree back, no hit opened, no history entry; the list's keys stay off it", async () => {
    const client = await openTree(searchClient());
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "widget" } });
    fireEvent.submit(screen.getByRole("search"));
    const list = await screen.findByRole("listbox", { name: /Search hits/ });
    const back = screen.getByRole("button", { name: "Back to tree" });
    back.focus();
    const before = window.history.length;
    for (const key of ["j", "ArrowDown", "k", "ArrowUp", "Home", "End"]) {
      expect(fireEvent.keyDown(back, { key })).toBe(true);
    }
    await act(async () => {
      await Promise.resolve();
    });
    expect(document.activeElement).toBe(back);
    expect(within(list).getAllByRole("option").map((option) => option.getAttribute("aria-selected"))).toEqual(["true", "false"]);
    // A browser clicks a focused button on Enter unless the keydown was prevented.
    const activates = fireEvent.keyDown(back, { key: "Enter" });
    expect(activates).toBe(true);
    if (activates) {
      fireEvent.click(back);
    }
    expect(await screen.findByRole("tree")).toBeTruthy();
    expect(screen.queryByRole("listbox")).toBeNull();
    await act(async () => {
      await Promise.resolve();
    });
    expect(window.history.length).toBe(before);
    expect(window.location.hash).toBe("#/alpha/tree");
    expect(client.getNode).not.toHaveBeenCalled();
  });

  it("refuses an empty query in the field, without a call", async () => {
    const client = await openTree(treeClient());
    fireEvent.submit(screen.getByRole("search"));
    expect((await screen.findByRole("alert")).textContent).toContain("Type what to search for");
    expect(client.search).not.toHaveBeenCalled();
  });

  it("shows the daemon's refusal verbatim with a Retry that searches again", async () => {
    const client = treeClient();
    const message = "spec: no search term of 3 or more characters; to read a node by its ID, use `spec show <ID>`";
    client.search.mockRejectedValue(new ClientError({ status: 503, message }));
    await openTree(client);
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "ab" } });
    fireEvent.submit(screen.getByRole("search"));
    expect(await screen.findByText(message)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(client.search).toHaveBeenCalledTimes(2);
    });
  });
});

describe("states of the tree and the search (AC-12)", () => {
  it("slow: the tree's skeleton is busy, the node's too", async () => {
    const client = treeClient();
    client.getTree.mockImplementation(() => new Promise(() => undefined));
    client.getNode.mockImplementation(() => new Promise(() => undefined));
    renderApp(client, "#/alpha/tree/DOC-A");
    const skeleton = await screen.findByLabelText("Loading the spec tree of alpha");
    expect(skeleton.getAttribute("aria-busy")).toBe("true");
    expect(skeleton.parentElement?.getAttribute("aria-busy")).toBe("true");
    expect((await screen.findByLabelText("Loading DOC-A")).getAttribute("aria-busy")).toBe("true");
    expect(screen.getByRole("article").getAttribute("aria-busy")).toBe("true");
  });

  it("slow: the search's skeleton is busy", async () => {
    const client = treeClient();
    client.search.mockImplementation(() => new Promise(() => undefined));
    await openTree(client);
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "widget" } });
    fireEvent.submit(screen.getByRole("search"));
    expect((await screen.findByLabelText("Searching for widget")).getAttribute("aria-busy")).toBe("true");
  });

  it("empty: the tree's note verbatim and the next step", async () => {
    const client = treeClient();
    const note = 'no document under [paths] spec "docs/spec"; give a ROOT';
    client.getTree.mockResolvedValue(aTreeView([], { notes: [note] }));
    renderApp(client, "#/alpha/tree");
    expect(await screen.findByRole("heading", { name: "No document in the tree" })).toBeTruthy();
    expect(screen.getByText(note)).toBeTruthy();
    expect(screen.getByText(/Next: write a spec document/)).toBeTruthy();
  });

  it("error: the tree says the daemon's words and Retry reads it again, the node meanwhile shown", async () => {
    const client = treeClient();
    const message = "spec index unavailable: the database is locked (code 5)";
    client.getTree.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/tree/DOC-A");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The spec tree could not be loaded${message}`);
    expect(await screen.findByRole("tab", { name: "Text" })).toBeTruthy();
    const retry = screen.getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await screen.findByRole("tree");
    expect(client.getTree).toHaveBeenCalledTimes(2);
    await waitFor(() => {
      expect(document.activeElement?.getAttribute("role")).toBe("treeitem");
    });
  });
});

describe("Include archive (AC-13)", () => {
  it("reads the tree again with archive, and the open links too", async () => {
    const client = await openTree(treeClient(), "#/alpha/tree/DOC-A");
    fireEvent.click(await screen.findByRole("tab", { name: "Links" }));
    await waitFor(() => {
      expect(client.getNode).toHaveBeenCalledWith("alpha", "DOC-A", { with: ["links"] });
    });
    fireEvent.click(screen.getByLabelText("Include archive"));
    await waitFor(() => {
      expect(client.getTree).toHaveBeenCalledWith("alpha", { archive: true });
    });
    await waitFor(() => {
      expect(client.getNode).toHaveBeenCalledWith("alpha", "DOC-A", { with: ["links"], archive: true });
    });
    // The text needs no archive: the daemon refuses archive without links.
    expect(client.getNode.mock.calls.filter((call) => call[2]?.archive === true && call[2].with === undefined)).toEqual([]);
    expect(rows().map(nameOf)).toEqual(START_ROWS);
  });
});

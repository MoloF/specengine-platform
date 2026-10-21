import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import { ClientError } from "../api/client";
import { App } from "../app/App";
import type { BindingRow, NodeView, ShownBindings, ShownLinks, ShownNode } from "../api/types";
import {
  aBindingRow,
  aBundle,
  aBundleItem,
  aBundleRow,
  aLink,
  aNode,
  aProposal,
  aSearchHit,
  aSearchResults,
  aTestedRow,
  aTreeView,
  someLayers,
} from "../test/builders";
import { takeConsoleCalls } from "../test/console";
import { renderApp } from "../test/render";
import { argsOf, type StubClient } from "../test/stubClient";
import { LINKS, textOf, TREE, treeClient, viewOf } from "../test/treeStub";

// docs/features/ui-tree-node.md on a stub client: AC-05 (unknown REF), AC-06 (the text), AC-07
// (no markup from data, hrefs only #/), AC-08 (links), AC-09 (proposals), AC-10 (bundle), AC-12
// (states of the node and its tabs), AC-13 (separate cache entries), AC-14 (ARIA tabs, one h1).

const HOSTILE = "<img src=x onerror=\u0061lert(1)>";

async function openNode(client: StubClient, ref = "DOC-A") {
  renderApp(client, `#/alpha/tree/${encodeURIComponent(ref)}`);
  await screen.findByRole("tab", { name: "Text" });
  return client;
}

function tab(name: string | RegExp): HTMLElement {
  return screen.getByRole("tab", { name });
}

async function openTab(name: string | RegExp): Promise<HTMLElement> {
  fireEvent.click(tab(name));
  const label = typeof name === "string" ? name : undefined;
  return label === undefined ? screen.getByRole("tabpanel") : screen.findByRole("tabpanel", { name: label });
}

/**
 * The Text tab's Source view: the text verbatim, numbered. ui-markdown made Rendered the default
 * (docs/features/ui-markdown.md "Implementation", the shipped criteria it changes): AC-06's verbatim
 * text is Source's.
 */
function showSource() {
  fireEvent.click(within(screen.getByRole("group", { name: "Show the text as" })).getByRole("button", { name: "Source" }));
}

function textLines(root: ParentNode = document): string {
  return Array.from(root.querySelectorAll(".text-line-content"), (line) => line.textContent).join("");
}

/** The shown headings' levels in document order. */
function headingLevels(): number[] {
  return screen.getAllByRole("heading").map((heading) => Number(heading.tagName.slice(1)));
}

/** One h1 first, and no level deeper than one below the heading before it. */
function expectNoSkippedLevel() {
  const levels = headingLevels();
  expect(levels[0]).toBe(1);
  expect(levels.filter((level) => level === 1)).toHaveLength(1);
  levels.forEach((level, at) => {
    expect(level, `heading ${String(at)} of ${levels.join(",")}`).toBeLessThanOrEqual((levels[at - 1] ?? 0) + 1);
  });
}

/** A link row's "as <written>", else null. */
function asOf(row: Element | null | undefined): string | null {
  const meta = row?.querySelector(".link-row-meta");
  const part = Array.from(meta?.children ?? []).find((child) => child.tagName === "SPAN" && child.textContent.startsWith("as "));
  return part?.querySelector(".mono")?.textContent ?? null;
}

function nodeOnly(view: NodeView) {
  return (_project: string, ref: string): Promise<NodeView> => Promise.resolve({ ...view, ref });
}

describe("the node's heading and text (AC-05, AC-06, AC-14)", () => {
  it("shows the text verbatim in Source, numbered from its first line, never trimmed", async () => {
    const client = treeClient();
    const text = "\n  ---\nid: DOC-A\n---\n\n## DOC-A: Title\n\n    indented, then trailing spaces   \n\n\n";
    client.getNode.mockImplementation(nodeOnly({ ref: "DOC-A", reason: null, notes: [], nodes: [aNode({ id: "DOC-A", line: 7, end_line: 16, text })] }));
    await openNode(client);
    showSource();
    expect(textLines()).toBe(text);
    const numbers = Array.from(document.querySelectorAll(".text-line-number"), (number) => number.textContent);
    expect(numbers[0]).toBe("7");
    expect(numbers).toHaveLength(text.split("\n").length - 1);
    expect(document.querySelector(".text-line-number")?.getAttribute("aria-hidden")).toBe("true");
  });

  it("keeps a 300-character token on one row that wraps", async () => {
    const token = "k7Qm2Xr9".repeat(38).slice(0, 300);
    const client = treeClient();
    client.getNode.mockImplementation(
      nodeOnly({ ref: "DOC-A", reason: null, notes: [], nodes: [aNode({ id: "DOC-A", text: `before\n${token}\nafter\n` })] }),
    );
    await openNode(client);
    showSource();
    const rowWithToken = Array.from(document.querySelectorAll(".text-line-content")).find((line) => line.textContent === `${token}\n`);
    expect(rowWithToken).toBeTruthy();
    const rule = /\.text-line-content\s*\{([^}]*)\}/.exec(appCss)?.[1] ?? "";
    expect(rule).toMatch(/white-space:\s*pre-wrap/);
    expect(rule).toMatch(/overflow-wrap:\s*anywhere/);
  });

  it("shows the header facts: kind, place, status, rev, size, span hash, flags", async () => {
    const client = treeClient();
    client.getNode.mockImplementation(
      nodeOnly({
        ref: "DOC-A",
        reason: null,
        notes: ["a note from show"],
        nodes: [
          aNode({
            id: "DOC-A",
            kind: "widget",
            title: "The A document",
            path: "docs/spec/a.md",
            line: 1,
            end_line: 40,
            status: "alpha-live",
            rev: 7,
            tokens_est: 321,
            span_hash: "b3:abc123",
            archived: true,
            utf8: false,
          }),
        ],
      }),
    );
    await openNode(client);
    const head = screen.getByRole("article").querySelector(".node-head");
    const text = head?.textContent ?? "";
    for (const part of ["DOC-A", "The A document", "widget", "docs/spec/a.md:1-40", "alpha-live", "7", "321 tokens", "b3:abc123", "archived", "not UTF-8", "a note from show"]) {
      expect([part, text.includes(part)]).toEqual([part, true]);
    }
  });

  it("holds two holders under one h1, the REF, an h2 and a text each", async () => {
    await openNode(treeClient(), "SEC-DUP");
    showSource();
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("SEC-DUP");
    const header = document.querySelector<HTMLElement>(".node-head");
    expect(header === null ? [] : within(header).getAllByRole("heading", { level: 2 })).toHaveLength(2);
    const holders = TREE.filter((row) => row.id === "SEC-DUP");
    expect(textLines()).toBe(holders.map(textOf).join(""));
    expect(document.querySelectorAll(".holder-text")).toHaveLength(2);
  });

  it("lists a cut node's lines, sections as links, holders and how many more", async () => {
    const client = treeClient();
    client.getNode.mockImplementation(
      nodeOnly({
        ref: "DOC-A",
        reason: null,
        notes: [],
        nodes: [
          aNode({
            id: "DOC-A",
            truncated: true,
            omitted: { lines: [40, 90], sections: ["SEC-A7", "SEC-A8"], sections_more: 3, holders: ["docs/spec/z.md:4"], holders_more: 1 },
          }),
        ],
      }),
    );
    await openNode(client);
    expect(screen.getByText("Lines 40-90 not shown")).toBeTruthy();
    expect(screen.getByRole("link", { name: "SEC-A7" }).getAttribute("href")).toBe("#/alpha/tree/SEC-A7");
    expect(screen.getByRole("link", { name: "SEC-A8" }).getAttribute("href")).toBe("#/alpha/tree/SEC-A8");
    expect(screen.getByText(/Sections not shown/).textContent).toContain(", 3 more");
    expect(screen.getByText(/Holders not shown/).textContent).toContain("docs/spec/z.md:4, 1 more");
  });

  it("jumps to a section's heading line from the list above the text, focusing it", async () => {
    const client = treeClient();
    const text = ["# DOC-A", "", "Intro.", "", "## SEC-A1: One", "", "Body."].join("\n");
    client.getNode.mockImplementation(
      nodeOnly({ ref: "DOC-A", reason: null, notes: [], nodes: [aNode({ id: "DOC-A", line: 1, end_line: 7, sections: ["SEC-A1"], text })] }),
    );
    await openNode(client);
    // The tree lists SEC-A1 at docs/spec/a.md:12, outside this node's lines 1-7: the heading line in
    // the text says where it is instead.
    fireEvent.click(within(screen.getByRole("navigation", { name: "Sections of DOC-A" })).getByRole("button"));
    await waitFor(() => {
      expect(document.activeElement?.getAttribute("data-line")).toBe("5");
    });
  });

  it("skips no heading level with one holder: Links and Bundle sections are h2 under the h1", async () => {
    await openNode(treeClient());
    const links = await openTab("Links");
    await within(links).findByText("R-404");
    expect(within(links).getByRole("heading", { name: /^Outgoing/ }).tagName).toBe("H2");
    expect(within(links).getByRole("heading", { name: /^Incoming/ }).tagName).toBe("H2");
    expectNoSkippedLevel();
    const bundle = await openTab("Bundle");
    await within(bundle).findByText(/tokens 12 of 2000/);
    expect(within(bundle).getByRole("heading", { name: "Targets" }).tagName).toBe("H2");
    expectNoSkippedLevel();
  });

  it("skips no heading level with two holders: each holder's text and links under an h2, the lists h3", async () => {
    await openNode(treeClient(), "SEC-DUP");
    expect(Array.from(document.querySelectorAll(".holder-text .holder-text-title"), (title) => title.tagName)).toEqual(["H2", "H2"]);
    expectNoSkippedLevel();
    const links = await openTab("Links");
    await within(links).findAllByText("R-404");
    expect(within(links).getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual([
      "docs/spec/a.md:30-39",
      "docs/spec/b.md:9-18",
    ]);
    expect(within(links).getAllByRole("heading", { name: /^(Outgoing|Incoming)/ }).map((heading) => heading.tagName)).toEqual([
      "H3",
      "H3",
      "H3",
      "H3",
    ]);
    expectNoSkippedLevel();
  });

  it("says why a REF names nothing, verbatim, with no tabs and the next step", async () => {
    renderApp(treeClient(), "#/alpha/tree/R-404");
    expect(await screen.findByText("`R-404` resolves to no ID and no alias")).toBeTruthy();
    expect(screen.getByText(/Next: search the spec for it/)).toBeTruthy();
  });

  it("follows the ARIA tabs pattern with manual activation", async () => {
    await openNode(treeClient());
    const list = screen.getByRole("tablist");
    const tabs = within(list).getAllByRole("tab");
    // UI-1 of ui-node-bindings: Bindings after Links, in 07 s3's order.
    expect(tabs.map((each) => each.textContent.replace(/\d+$/, "").trim())).toEqual(["Text", "Links", "Bindings", "Bundle", "Proposals"]);
    expect(tabs.map((each) => each.getAttribute("aria-selected"))).toEqual(["true", "false", "false", "false", "false"]);
    expect(tabs.map((each) => each.tabIndex)).toEqual([0, -1, -1, -1, -1]);
    for (const each of tabs) {
      const panel = document.getElementById(each.getAttribute("aria-controls") ?? "");
      expect(panel?.getAttribute("role")).toBe("tabpanel");
      expect(panel?.getAttribute("aria-labelledby")).toBe(each.id);
    }
    tabs[0]?.focus();
    fireEvent.keyDown(tabs[0] ?? list, { key: "ArrowRight" });
    expect(document.activeElement).toBe(tabs[1]);
    expect(tabs[0]?.getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(tabs[1] ?? list, { key: "End" });
    expect(document.activeElement).toBe(tabs[4]);
    fireEvent.keyDown(tabs[4] ?? list, { key: "ArrowRight" });
    expect(document.activeElement).toBe(tabs[0]);
    fireEvent.keyDown(tabs[0] ?? list, { key: "ArrowLeft" });
    expect(document.activeElement).toBe(tabs[4]);
    fireEvent.keyDown(tabs[4] ?? list, { key: "Home" });
    expect(document.activeElement).toBe(tabs[0]);
    fireEvent.keyDown(tabs[0] ?? list, { key: "ArrowRight", ctrlKey: true });
    expect(document.activeElement).toBe(tabs[0]);
    fireEvent.click(tabs[1] ?? list);
    expect(tabs[1]?.getAttribute("aria-selected")).toBe("true");
    expect(tabs[1]?.tabIndex).toBe(0);
  });
});

describe("markup in data stays text; hrefs come from the route (AC-07)", () => {
  it("renders a hostile title, text and snippet with no img, every href #/", async () => {
    const client = treeClient();
    client.getTree.mockResolvedValue(aTreeView(TREE.map((row) => (row.id === "DOC-A" ? { ...row, title: HOSTILE } : row))));
    client.getNode.mockImplementation(
      nodeOnly({
        ref: "DOC-A",
        reason: null,
        notes: [HOSTILE],
        nodes: [aNode({ id: "DOC-A", title: HOSTILE, text: `${HOSTILE}\n<a href="javascript:x">a</a>\n`, links: null })],
      }),
    );
    client.search.mockResolvedValue(
      aSearchResults([
        aSearchHit({
          id: "DOC-A",
          title: HOSTILE,
          snippet: { segments: [{ text: HOSTILE, hit: true }, { text: "<script>x()</script>", hit: false }], cut_start: false, cut_end: false },
        }),
      ]),
    );
    await openNode(client);
    expect(screen.getByText(HOSTILE, { selector: ".node-title-text" })).toBeTruthy();
    expect(screen.getByText(HOSTILE, { selector: ".tree-row-title" })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "img" } });
    fireEvent.submit(screen.getByRole("search"));
    const hits = await screen.findByRole("listbox", { name: /Search hits/ });
    expect(within(hits).getByText(HOSTILE, { selector: "mark" })).toBeTruthy();
    expect(document.querySelector("img")).toBeNull();
    expect(screen.getByRole("main").querySelector("script")).toBeNull();
    const hrefs = Array.from(screen.getByRole("main").querySelectorAll("a[href]"), (anchor) => anchor.getAttribute("href") ?? "");
    expect(hrefs.length).toBeGreaterThan(0);
    expect(hrefs.filter((href) => !href.startsWith("#/"))).toEqual([]);
  });

  it("anchors a resolved link by its name, never by its written form", async () => {
    const client = treeClient();
    await openNode(client);
    const panel = await openTab("Links");
    await within(panel).findByText("R-404");
    const hrefs = Array.from(panel.querySelectorAll("a[href]"), (anchor) => anchor.getAttribute("href"));
    expect(hrefs).toEqual(["#/alpha/tree/R-9", "#/alpha/tree/R-8", "#/alpha/tree/DOC-X"]);
  });
});

describe("links (AC-08)", () => {
  it("lists outgoing then incoming in the daemon's order, with type, place, at, as and state", async () => {
    await openNode(treeClient());
    const panel = await openTab("Links");
    await within(panel).findByText("R-404");
    const types = Array.from(panel.querySelectorAll(".link-row .kind-tag"), (tag) => tag.textContent);
    expect(types).toEqual([...LINKS.outgoing, ...LINKS.incoming].map((link) => link.type));
    const second = panel.querySelectorAll(".link-row")[1];
    expect(second?.textContent).toContain("at SEC-A1");
    expect(second?.textContent).toContain("as [[R-8]]");
    expect(second?.textContent).toContain("docs/spec/a.md:3");
    // Incoming, written `DOC-A` and landing on DOC-A: written as the node it lands on, no "as".
    const incoming = panel.querySelectorAll(".link-row")[LINKS.outgoing.length];
    expect(incoming?.textContent).toContain("DOC-X");
    expect(asOf(incoming)).toBeNull();
    expect(asOf(second)).toBe("[[R-8]]");
    const states = Array.from(panel.querySelectorAll(".link-row .badge"), (badge) => [
      badge.querySelector(".badge-label")?.textContent,
      badge.querySelector("svg") !== null,
    ]);
    expect(states).toEqual([
      ["Resolved", true],
      ["Resolved", true],
      ["Dangling", true],
      ["Skipped: another project", true],
      ["Not checked", true],
      ["Resolved", true],
    ]);
    expect(within(panel).getByText("weak")).toBeTruthy();
  });

  it("says \"as <written>\" when the written form is not the name of the node it names, either way", async () => {
    const links: ShownLinks = {
      outgoing: [
        aLink({ type: "zeta_type", written: "R-9", name: "R-9", line: 2 }),
        aLink({ type: "zeta_type", written: "docs/spec/r.md", name: "R-7", line: 3 }),
        // Written in DOC-A and naming it, its source: the other end is its target (superseded-by).
        aLink({ type: "eta_type", written: "DOC-A", name: "DOC-OLD", line: 4 }),
      ],
      incoming: [
        aLink({ type: "delta_type", written: "LEGACY-A", name: "DOC-X", path: "docs/spec/x.md", line: 5 }),
        aLink({ type: "delta_type", written: "SEC-A1", name: "DOC-X", path: "docs/spec/x.md", line: 6, at: "SEC-A1" }),
        aLink({ type: "delta_type", written: "DOC-A", name: "DOC-Y", path: "docs/spec/y.md", line: 7, at: "SEC-A1" }),
        // Written in DOC-NEW and naming it, the source landing here (superseded-by).
        aLink({ type: "eta_type", written: "DOC-NEW", name: "DOC-NEW", path: "docs/spec/n.md", line: 8 }),
      ],
      left_out: { generated: 0, tier3: 0 },
      omitted: 0,
    };
    const client = treeClient();
    client.getNode.mockImplementation((_project, ref, options) =>
      Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref, links: options?.with === undefined ? null : links })] }),
    );
    await openNode(client);
    const panel = await openTab("Links");
    await within(panel).findByText("DOC-NEW");
    expect(Array.from(panel.querySelectorAll(".link-row"), asOf)).toEqual([null, "docs/spec/r.md", null, "LEGACY-A", null, "DOC-A", null]);
  });

  it("anchors only resolved links; shows reasons verbatim; says what was left out", async () => {
    await openNode(treeClient());
    const panel = await openTab("Links");
    expect(await within(panel).findByText("`R-404` resolves to no ID and no alias")).toBeTruthy();
    expect(within(panel).getByText("`canon:` names docs/canon/x.md; not checked")).toBeTruthy();
    expect(within(panel).getByText("R-404").closest("a")).toBeNull();
    expect(within(panel).getByText("other:R-1").closest("a")).toBeNull();
    expect(within(panel).getByText(/Left out: 1 generated, 2 archived/)).toBeTruthy();
  });

  it("shows an outgoing link in the text: the Text tab, its line focused (Source)", async () => {
    await openNode(treeClient());
    showSource();
    const panel = await openTab("Links");
    await within(panel).findByText("R-404");
    const buttons = within(panel).getAllByRole("button", { name: /Show in text/ });
    expect(buttons).toHaveLength(LINKS.outgoing.length);
    fireEvent.click(buttons[2] ?? panel);
    await waitFor(() => {
      expect(tab("Text").getAttribute("aria-selected")).toBe("true");
    });
    await waitFor(() => {
      expect(document.activeElement?.getAttribute("data-line")).toBe("4");
    });
  });

  it("says when a node has no link, with the next step", async () => {
    const client = treeClient();
    client.getNode.mockImplementation((_project, ref, options) =>
      Promise.resolve({
        ref,
        reason: null,
        notes: [],
        nodes: [aNode({ id: ref, links: options?.with === undefined ? null : { outgoing: [], incoming: [], left_out: { generated: 0, tier3: 0 }, omitted: 0 } })],
      }),
    );
    await openNode(client);
    const panel = await openTab("Links");
    expect(await within(panel).findByText("No link is written in this node, and none lands on it.")).toBeTruthy();
    expect(within(panel).getByText(/Next: links come from front-matter fields/)).toBeTruthy();
  });

  it("names how many links a cut left out", async () => {
    const client = treeClient();
    client.getNode.mockImplementation((_project, ref, options) =>
      Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref, links: options?.with === undefined ? null : { ...LINKS, omitted: 4 } })] }),
    );
    await openNode(client);
    const panel = await openTab("Links");
    expect(await within(panel).findByText("4 links not shown")).toBeTruthy();
  });
});

describe("proposals (AC-09)", () => {
  const QUEUE = [
    aProposal({ id: "PR-1", target_id: "DOC-A", summary: "By its ID" }),
    aProposal({ id: "PR-2", target_ids: ["R-9", "DOC-A"], summary: "By a second target" }),
    aProposal({ id: "PR-3", target_ids: ["a/DOC-A"], summary: "By slug and ID" }),
    aProposal({ id: "PR-4", target_ids: ["R-5"], summary: "Elsewhere" }),
    aProposal({ id: "PR-5", target_ids: ["docs/spec/b.md"], summary: "By the path" }),
    aProposal({ id: "PR-6", target_ids: ["docs/spec/a.md"], summary: "A path never names an ID'd node" }),
  ];

  it("counts and lists exactly the items targeting the node, linked to the Inbox", async () => {
    await openNode(treeClient(QUEUE));
    await waitFor(() => {
      expect(tab(/^Proposals/).textContent).toBe("Proposals 3");
    });
    const panel = await openTab(/^Proposals/);
    const links = within(panel).getAllByRole("link").filter((link) => /^PR-/.test(link.textContent));
    expect(links.map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
      ["PR-1", "#/alpha/inbox/PR-1"],
      ["PR-2", "#/alpha/inbox/PR-2"],
      ["PR-3", "#/alpha/inbox/PR-3"],
    ]);
    expect(panel.textContent).toContain("By a second target");
  });

  it("matches an ID-less node by its path", async () => {
    await openNode(treeClient(QUEUE), "docs/spec/b.md");
    const panel = await openTab(/^Proposals/);
    expect(within(panel).getAllByRole("link").map((link) => link.textContent)).toContain("PR-5");
    expect(panel.textContent).not.toContain("PR-1");
  });

  it("says when nothing targets the node, with the next step", async () => {
    await openNode(treeClient([]));
    const panel = await openTab(/^Proposals/);
    expect(within(panel).getByText("No inbox item targets this node.")).toBeTruthy();
    expect(within(panel).getByRole("link", { name: /the Inbox lists every proposal/ }).getAttribute("href")).toBe("#/alpha/inbox");
  });

  it("keeps an inbox failure in the Proposals tab: the tree and the node stay", async () => {
    const client = treeClient(QUEUE);
    const message = "inbox unavailable: queue locked";
    client.getInbox.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    await openNode(client);
    showSource();
    const panel = await openTab(/^Proposals/);
    expect((await within(panel).findByRole("alert")).textContent).toContain(message);
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("tree")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1 }).textContent).toContain("DOC-A");
    expect(textLines()).toBe(textOf(TREE[0] ?? { id: "DOC-A", path: "", title: null }));
    fireEvent.click(within(panel).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(client.getInbox).toHaveBeenCalledTimes(2);
    });
    expect(await within(panel).findByText("By its ID")).toBeTruthy();
  });
});

describe("the bundle (AC-10)", () => {
  it("is read only when its tab opens, then once per submit", async () => {
    const client = await openNode(treeClient());
    expect(client.getBundle).not.toHaveBeenCalled();
    const panel = await openTab("Bundle");
    await within(panel).findByText(/tokens 12 of 2000/);
    expect(client.getBundle).toHaveBeenCalledTimes(1);
    expect(argsOf(client.getBundle).at(-1)).toEqual(["alpha", { node_ids: ["DOC-A"] }]);
    fireEvent.click(tab("Text"));
    fireEvent.click(tab("Bundle"));
    expect(client.getBundle).toHaveBeenCalledTimes(1);
    const field = within(panel).getByLabelText("Budget in tokens");
    fireEvent.change(field, { target: { value: "5000" } });
    fireEvent.submit(field.closest("form") ?? panel);
    await waitFor(() => {
      expect(client.getBundle).toHaveBeenCalledTimes(2);
    });
    expect(argsOf(client.getBundle).at(-1)).toEqual(["alpha", { node_ids: ["DOC-A"], budget: 5000 }]);
    await within(panel).findByText(/tokens 12 of 5000/);
    fireEvent.submit(field.closest("form") ?? panel);
    await waitFor(() => {
      expect(client.getBundle).toHaveBeenCalledTimes(3);
    });
  });

  it("shows a refusal verbatim and keeps the typed budget; refuses non-digits in the field without a call", async () => {
    const client = treeClient();
    const message =
      "spec: --budget 5 is below this bundle's minimum of 30 tokens (its title, target headers and not-included line): raise the budget to 30 or more";
    client.getBundle.mockImplementation((_project, options) =>
      options.budget === 5 ? Promise.reject(new ClientError({ status: 503, message })) : Promise.resolve(aBundle(options.node_ids)),
    );
    await openNode(client);
    const panel = await openTab("Bundle");
    await within(panel).findByText(/tokens 12 of/);
    const field = within(panel).getByLabelText("Budget in tokens");
    fireEvent.change(field, { target: { value: "5" } });
    fireEvent.submit(field.closest("form") ?? panel);
    expect((await within(panel).findByRole("alert")).textContent).toContain(message);
    expect((field as HTMLInputElement).value).toBe("5");
    const calls = client.getBundle.mock.calls.length;
    fireEvent.change(field, { target: { value: "12k" } });
    fireEvent.submit(field.closest("form") ?? panel);
    expect(await within(panel).findByText(/Digits only/)).toBeTruthy();
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(client.getBundle).toHaveBeenCalledTimes(calls);
  });

  it("refuses a budget above 4294967295 in the field without a call; sends 4294967295", async () => {
    const client = await openNode(treeClient());
    const panel = await openTab("Bundle");
    await within(panel).findByText(/tokens 12 of/);
    const field = within(panel).getByLabelText("Budget in tokens");
    const calls = client.getBundle.mock.calls.length;
    for (const value of ["4294967296", "9007199254740991", "99999999999999999999"]) {
      fireEvent.change(field, { target: { value } });
      fireEvent.submit(field.closest("form") ?? panel);
      expect((await within(panel).findByRole("alert")).textContent).toBe(
        "That number is too large to send; the daemon takes at most 4294967295.",
      );
      expect(client.getBundle).toHaveBeenCalledTimes(calls);
    }
    fireEvent.change(field, { target: { value: "4294967295" } });
    fireEvent.submit(field.closest("form") ?? panel);
    await waitFor(() => {
      expect(argsOf(client.getBundle).at(-1)).toEqual(["alpha", { node_ids: ["DOC-A"], budget: 4294967295 }]);
    });
    expect(within(panel).queryByRole("alert")).toBeNull();
  });

  it("shows the layers under the CLI's headings, items' fields, the tail and the body", async () => {
    const client = treeClient();
    client.getBundle.mockResolvedValue(
      aBundle(["DOC-A"], {
        layers: someLayers({
          targets: [aBundleItem({ name: "DOC-A", form: "text", status: "alpha-live" })],
          open_questions: [
            aBundleItem({
              name: "Q-1",
              form: "header",
              via: [{ type: "mentions", direction: "in" }],
              working_answer: { name: "R-9", written: "R-9", path: "docs/spec/q.md", line: 4, state: "resolved" },
            }),
          ],
          neighbours: [aBundleItem({ name: "N-1", form: "summary", via: [{ type: "depends_on", direction: "out" }] })],
        }),
        tail: [{ name: "T-1", title: "Left behind", path: "docs/spec/t.md", line: 1, tokens_est: 900, layer: "terms" }],
        more: 4,
        body: "# Bundle: DOC-A\n\n## Targets\n<b>not bold</b>\n",
      }),
    );
    await openNode(client);
    const panel = await openTab("Bundle");
    await within(panel).findByText("Open questions");
    expect(Array.from(panel.querySelectorAll(".bundle-layer h2"), (heading) => heading.textContent)).toEqual([
      "Targets",
      "Open questions",
      "Neighbours",
      "Not included",
    ]);
    expect(panel.textContent).toContain("via mentions incoming");
    expect(panel.textContent).toContain("Working answer:R-9docs/spec/q.md:4");
    expect(panel.textContent).toContain("Header and summary");
    expect(panel.textContent).toContain("4 more");
    expect(panel.textContent).toContain("not included 5");
    expect(panel.querySelector("details pre")?.textContent).toBe("# Bundle: DOC-A\n\n## Targets\n<b>not bold</b>\n");
    expect(panel.querySelector("b")).toBeNull();
  });
});

/** A non-Latin file name, escaped (ADR-0024): shown as written. */
const NON_LATIN_FILE = "src/\u0433\u0430\u0432\u0430\u043d\u044c.rs";

/** Layers 5 and 9 as the daemon sends them: an item, a keyless marker, a RON value, a moved and an ambiguous row, one not judged, one that cannot be verified; a test. */
function codeLayers() {
  return someLayers({
    targets: [aBundleItem({ name: "DOC-A", form: "text" })],
    criteria: [aBundleItem({ name: "AC-1", form: "text" })],
    bindings: [
      aBundleRow({ node: "DOC-A", relation: "binds_any", file: NON_LATIN_FILE, kind: null, symbol: null, item_kind: null, line: null, end_line: null, signature: null, layer: null, sync: "bad_marker", category: "unanchored", anchor: null }),
      aBundleRow({ node: "DOC-A", sync: "code_ahead", category: "code_changed", detail: "body", signature: "pub fn run(depth: u8) -> Result<(), Error>", tokens_est: 31 }),
      aBundleRow({ node: "SEC-A1", relation: "configures_any", file: "assets/data/a.ron", kind: "ron", symbol: "root.a.limit", item_kind: null, line: 4, end_line: null, signature: null, layer: null, sync: "spec_ahead", category: "marker_behind" }),
      aBundleRow({
        node: "SEC-A1",
        file: "src/b.rs",
        symbol: "crate::b::walk",
        sync: "ok",
        anchor: { state: "moved", from: { file: "src/old.rs", kind: "item", symbol: "crate::old::walk", item_kind: "function_item" }, candidates: null },
      }),
      aBundleRow({
        node: "SEC-A1",
        file: "src/c.rs",
        symbol: "crate::c::step",
        sync: "cannot_verify",
        category: "ambiguous_symbol",
        detail: "duplicate",
        anchor: {
          state: "ambiguous",
          from: null,
          candidates: [
            { file: "src/c.rs", kind: "item", symbol: "crate::c::step", item_kind: "function_item" },
            { file: "src/d.rs", kind: "item", symbol: "crate::d::step", item_kind: null },
          ],
        },
      }),
      aBundleRow({ node: "DOC-A", file: "src/e.rs", symbol: "crate::e::listed", sync: null, category: null, detail: null, anchor: null }),
    ],
    neighbours: [aBundleItem({ name: "N-1", form: "summary" })],
    tests: [aBundleRow({ node: "AC-1", relation: "checks_any", file: "tests/a.rs", symbol: "a::runs_once", line: 3, end_line: 9, signature: "fn runs_once()" })],
  });
}

/** A layer's section by its heading. */
function layerSection(panel: HTMLElement, heading: string): HTMLElement {
  const section = within(panel).getByRole("heading", { name: heading }).closest("section");
  if (section === null) {
    throw new Error(`no section for ${heading}`);
  }
  return section;
}

describe("the bundle's code rows (UI-2 of bundle-bindings)", () => {
  it("shows layers 5 and 9 as lines in canon order: node, relation, place, binding, layer, tokens, sync, signature, anchors", async () => {
    const client = treeClient();
    client.getBundle.mockResolvedValue(aBundle(["DOC-A"], { layers: codeLayers() }));
    await openNode(client);
    const panel = await openTab("Bundle");
    await within(panel).findByText("Bindings");
    expect(Array.from(panel.querySelectorAll(".bundle-layer h2"), (heading) => heading.textContent)).toEqual(["Targets", "Criteria", "Bindings", "Neighbours", "Tests"]);
    const bindings = layerSection(panel, "Bindings");
    const lines = Array.from(bindings.querySelectorAll("li"), (line) => Array.from(line.querySelectorAll("p"), (part) => part.textContent));
    expect(lines).toEqual([
      [`DOC-Abinds_any${NON_LATIN_FILE}`, "No symbol12 tokens", "Sync: Bad markerunanchored"],
      ["DOC-Aimplementssrc/a.rs:10-20", "item crate::a::run function_itemlayer a31 tokens", "Sync: Code aheadcode_changed body", "Signature:pub fn run(depth: u8) -> Result<(), Error>"],
      ["SEC-A1configures_anyassets/data/a.ron:4", "ron root.a.limit12 tokens", "Sync: Spec aheadmarker_behind"],
      ["SEC-A1implementssrc/b.rs:10-20", "item crate::b::walk function_itemlayer a12 tokens", "Sync: In sync", "Signature:pub fn run()", "Moved fromsrc/old.rs | item crate::old::walk function_item"],
      [
        "SEC-A1implementssrc/c.rs:10-20",
        "item crate::c::step function_itemlayer a12 tokens",
        "Sync: Cannot verifyambiguous_symbol duplicate",
        "Signature:pub fn run()",
        "Candidatesrc/c.rs | item crate::c::step function_item",
        "Candidatesrc/d.rs | item crate::d::step",
      ],
      ["DOC-Aimplementssrc/e.rs:10-20", "item crate::e::listed function_itemlayer a12 tokens", "Sync: Not judged", "Signature:pub fn run()"],
    ]);
    const tests = layerSection(panel, "Tests");
    expect(Array.from(tests.querySelectorAll("li > p"), (part) => part.textContent)).toEqual([
      "AC-1checks_anytests/a.rs:3-9",
      "item a::runs_once function_itemlayer a12 tokens",
      "Sync: In sync",
      "Signature:fn runs_once()",
    ]);
    expect(panel.textContent).not.toMatch(/undefined|NaN|\[object/);
  });

  it("links nothing in a row: neither its node, nor its symbol, nor its file is a tree node", async () => {
    const client = treeClient();
    client.getBundle.mockResolvedValue(aBundle(["DOC-A"], { layers: codeLayers() }));
    await openNode(client);
    const panel = await openTab("Bundle");
    await within(panel).findByText("Bindings");
    expect(layerSection(panel, "Bindings").querySelectorAll("a")).toHaveLength(0);
    expect(layerSection(panel, "Tests").querySelectorAll("a")).toHaveLength(0);
    // The nodes of the other layers stay links to the tree.
    expect(within(layerSection(panel, "Criteria")).getByRole("link", { name: "AC-1" }).getAttribute("href")).toBe("#/alpha/tree/AC-1");
    expect(within(layerSection(panel, "Neighbours")).getByRole("link", { name: "N-1" }).getAttribute("href")).toBe("#/alpha/tree/N-1");
  });

  it("draws a row not judged and one that cannot be verified each in its own role, neither like a row in sync", async () => {
    const client = treeClient();
    client.getBundle.mockResolvedValue(aBundle(["DOC-A"], { layers: codeLayers() }));
    await openNode(client);
    const panel = await openTab("Bundle");
    await within(panel).findByText("Bindings");
    const tone = (label: string) => within(panel).getAllByText(label)[0]?.closest(".badge")?.getAttribute("data-tone");
    expect([tone("In sync"), tone("Not judged"), tone("Cannot verify"), tone("Bad marker")]).toEqual(["sync-ok", "sync-not-judged", "sync-cannot-verify", "sync-bad-marker"]);
    // Besides its colour and icon, cannot-verify's badge is dotted and a row not judged dashed, as Health's are.
    const ruled = (style: string) =>
      Array.from(appCss.matchAll(new RegExp(`([^{}]*)\\{\\s*border-style: ${style};\\s*\\}`, "g")), (match) => match[1] ?? "").join(",");
    expect(ruled("dotted")).toContain(".tone-sync-cannot-verify");
    expect(ruled("dashed")).toContain(".tone-sync-not-judged");
    expect(ruled("dotted")).not.toContain(".tone-sync-ok");
    expect(ruled("dashed")).not.toContain(".tone-sync-ok");
  });

  it("names left-out rows in the tail with their place and layer, unlinked; a left-out node stays a link", async () => {
    const client = treeClient();
    client.getBundle.mockResolvedValue(
      aBundle(["DOC-A"], {
        tail: [
          { name: "crate::a::run", title: "implements DOC-A", path: "src/a.rs", line: 10, tokens_est: 31, layer: "bindings" },
          { name: "T-1", title: "Left behind", path: "docs/spec/t.md", line: 1, tokens_est: 900, layer: "terms" },
          { name: "tests/a.rs", title: "checks_any AC-1", path: "tests/a.rs", line: 1, tokens_est: 20, layer: "tests" },
        ],
        more: 2,
      }),
    );
    await openNode(client);
    const panel = await openTab("Bundle");
    await within(panel).findByText("Not included");
    const tail = within(layerSection(panel, "Not included"));
    const entries = tail.getAllByRole("listitem").map((entry) => [entry.textContent, entry.querySelectorAll("a").length]);
    expect(entries).toEqual([
      ["crate::a::run implements DOC-A | src/a.rs:10 | 31 tokens | Bindings", 0],
      ["T-1 Left behind | 900 tokens | Terms", 1],
      ["tests/a.rs checks_any AC-1 | tests/a.rs:1 | 20 tokens | Tests", 0],
      ["2 more", 0],
    ]);
    expect(tail.getByRole("link", { name: "T-1" }).getAttribute("href")).toBe("#/alpha/tree/T-1");
    expect(panel.textContent).toContain("not included 5");
  });

  it("renders two listed keyless rows on one node, relation and file without React's duplicate-key error (degrade causes 4-6)", async () => {
    // A row listed but not judged has no state and no category (docs/canon/code-reads.md "Degrade"):
    // two keyless ones on one node, relation and file differ in nothing else.
    const listed = { kind: null, symbol: null, item_kind: null, line: null, end_line: null, signature: null, layer: null, sync: null, category: null, detail: null, anchor: null };
    const client = treeClient();
    client.getBundle.mockResolvedValue(
      aBundle(["DOC-A"], { layers: someLayers({ bindings: [aBundleRow({ node: "DOC-A", ...listed }), aBundleRow({ node: "DOC-A", ...listed })] }) }),
    );
    await openNode(client);
    const panel = await openTab("Bundle");
    await within(panel).findByText("Bindings");
    expect(layerSection(panel, "Bindings").querySelectorAll("li")).toHaveLength(2);
    expect(within(panel).getAllByText("Not judged")).toHaveLength(2);
    expect(takeConsoleCalls()).toEqual([]);
  });
});

describe("states of the node and its tabs (AC-12)", () => {
  it("slow: links, bundle and proposals each have a busy skeleton", async () => {
    const client = treeClient();
    client.getBundle.mockImplementation(() => new Promise(() => undefined));
    client.getInbox.mockImplementation(() => new Promise(() => undefined));
    client.getNode.mockImplementation((_project, ref, options) =>
      options?.with === undefined
        ? Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref })] })
        : new Promise(() => undefined),
    );
    await openNode(client);
    await openTab("Links");
    expect((await screen.findByLabelText("Loading the links of DOC-A")).getAttribute("aria-busy")).toBe("true");
    await openTab("Bundle");
    expect((await screen.findByLabelText("Loading the bundle of DOC-A")).getAttribute("aria-busy")).toBe("true");
    await openTab(/^Proposals/);
    expect((await screen.findByLabelText("Loading the inbox")).getAttribute("aria-busy")).toBe("true");
  });

  it("error: the node says the daemon's words; Retry reads it again and focus lands on its heading", async () => {
    const client = treeClient();
    const message = "spec: `DOC-A` mixes scripts; IDs are Latin only: write it in Latin letters";
    client.getNode.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/tree/DOC-A");
    const article = await screen.findByRole("article");
    const alert = await within(article).findByRole("alert");
    expect(alert.textContent).toBe(`DOC-A could not be read${message}`);
    expect(screen.getByRole("tree")).toBeTruthy();
    const retry = within(article).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await screen.findByRole("tab", { name: "Text" });
    // The plain read twice; the links read beside it once (ui-markdown AC-05).
    expect(client.getNode.mock.calls.filter((call) => call[2] === undefined)).toHaveLength(2);
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1 }));
    });
  });

  it("error: links and bundle each fail alone, verbatim, and Retry calls again", async () => {
    const client = treeClient();
    client.getNode.mockImplementation((_project, ref, options) =>
      options?.with === undefined
        ? Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref })] })
        : Promise.reject(new ClientError({ status: 503, message: "links: index locked" })),
    );
    client.getBundle.mockRejectedValue(new ClientError({ status: 503, message: "bundle: index locked" }));
    await openNode(client);
    const links = await openTab("Links");
    expect((await within(links).findByRole("alert")).textContent).toContain("links: index locked");
    fireEvent.click(within(links).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(client.getNode.mock.calls.filter((call) => call[2] !== undefined)).toHaveLength(2);
    });
    const bundle = await openTab("Bundle");
    expect((await within(bundle).findByRole("alert")).textContent).toContain("bundle: index locked");
    fireEvent.click(within(bundle).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(client.getBundle).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByRole("tree")).toBeTruthy();
  });

  it("a region that throws shows its own fallback; the tree and the node stay", async () => {
    const client = treeClient();
    client.getBundle.mockResolvedValue({ ...aBundle(["DOC-A"]), layers: { targets: null } } as unknown as ReturnType<typeof aBundle>);
    window.history.replaceState(null, "", "/#/alpha/tree/DOC-A");
    const caught: unknown[] = [];
    render(<App client={client} scenario={null} />, {
      onCaughtError: (error: unknown) => {
        caught.push(error);
      },
    });
    await screen.findByRole("tab", { name: "Text" });
    const panel = await openTab("Bundle");
    expect(await within(panel).findByText(/The bundle could not be shown/)).toBeTruthy();
    expect(caught.length).toBeGreaterThan(0);
    expect(screen.getByRole("tree")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1 }).textContent).toContain("DOC-A");
  });
});

// docs/features/ui-node-bindings.md on a stub client: UI-1 (one read, on the tab's first opening),
// UI-2 (rows per holder in the daemon's order, each field), UI-3 (null, not judged, none), UI-4
// (the test evidence's three states), long lists, the commands, Read again and the read's failure.

/** The calls of the stub's node read that asked for bindings. */
function bindingsReads(client: StubClient) {
  return argsOf(client.getNode).filter(([, , options]) => (options as { with?: string[] } | undefined)?.with?.includes("bindings") === true);
}

/**
 * The stub's node reads, the bindings read answering each holder's bindings (as given, or per
 * holder) with `notes`; the plain and links reads as the stub's.
 */
function bindingsClient(bindings: ShownBindings | null | ((holder: ShownNode) => ShownBindings | null), notes: string[] = []) {
  const client = treeClient();
  client.getNode.mockImplementation((_project, ref, options) => {
    const view = viewOf(ref, options?.with?.includes("links") === true);
    if (options?.with?.includes("bindings") !== true || view.nodes.length === 0) {
      return Promise.resolve(view);
    }
    return Promise.resolve({
      ...view,
      notes,
      nodes: view.nodes.map((node) => ({ ...node, bindings: typeof bindings === "function" ? bindings(node) : bindings })),
    });
  });
  return client;
}

/** A holder's bindings: its rows, nothing left out, its test evidence `[]` unless given. */
function shown(rows: BindingRow[], tested: ShownBindings["tested"] = []): ShownBindings {
  return { rows, omitted: 0, tested };
}

/**
 * Rows in the daemon's (verify's) order, which neither sync nor file order gives: a moved row, a
 * keyless one, a RON value, an ambiguous one, a bare marker's not judged in a non-Latin file.
 */
function daemonRows(): BindingRow[] {
  return [
    aBindingRow({
      node: "DOC-A",
      file: "src/z.rs",
      symbol: "crate::z::run",
      sync: "code_ahead",
      category: "code_changed",
      detail: "body",
      anchor: { state: "moved", from: { file: "src/old.rs", kind: "item", symbol: "crate::old::run", item_kind: "function_item" }, candidates: null },
    }),
    aBindingRow({ node: "DOC-A", relation: "verifies", file: "src/a.rs", kind: null, symbol: null, item_kind: null, line: null, end_line: null, signature: null, layer: null, sync: "bad_marker", category: "unanchored", anchor: null }),
    aBindingRow({ node: "SEC-A1", relation: "configures", file: "data/a.ron", kind: "ron", symbol: "root.limit", item_kind: null, line: 4, end_line: null, signature: null, layer: null }),
    aBindingRow({
      node: "SEC-A1",
      file: "src/c.rs",
      symbol: "crate::c::step",
      sync: "cannot_verify",
      category: "ambiguous_symbol",
      detail: "duplicate",
      anchor: {
        state: "ambiguous",
        from: null,
        candidates: [
          { file: "src/c.rs", kind: "item", symbol: "crate::c::step", item_kind: "function_item" },
          { file: "src/d.rs", kind: "item", symbol: "crate::d::step", item_kind: null },
        ],
      },
    }),
    aBindingRow({ node: "", file: NON_LATIN_FILE, symbol: "crate::b::walk", sync: null, category: null, detail: null, anchor: null }),
  ];
}

/** The panel's binding lines, each as its paragraphs' text. */
function bindingLines(root: ParentNode): (string | null)[][] {
  return Array.from(root.querySelectorAll(".binding-line"), (line) => Array.from(line.querySelectorAll("p"), (part) => part.textContent));
}

/** A holder's part of the panel by its place. */
function holderPart(panel: HTMLElement, place: string): HTMLElement {
  const part = panel.querySelector<HTMLElement>(`[data-holder="${place}"]`);
  if (part === null) {
    throw new Error(`no holder ${place}`);
  }
  return part;
}

const NO_CODE_NOTE = "bindings: specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

describe("the Bindings tab's read (UI-1 of ui-node-bindings)", () => {
  it("reads nodes/<REF>?with=bindings once, when the tab first opens: none before, none on reopening", async () => {
    const client = bindingsClient(shown(daemonRows()));
    await openNode(client);
    await openTab("Links");
    await openTab("Bundle");
    await openTab("Text");
    await waitFor(() => {
      expect(client.getBundle).toHaveBeenCalledTimes(1);
    });
    expect(bindingsReads(client)).toEqual([]);
    const panel = await openTab("Bindings");
    await within(panel).findByText("Code bindings");
    expect(bindingsReads(client)).toEqual([["alpha", "DOC-A", { with: ["bindings"] }]]);
    await openTab("Text");
    await openTab("Bindings");
    await openTab(/^Proposals/);
    await openTab("Bindings");
    expect(within(panel).getAllByText("Code bindings")).toHaveLength(1);
    expect(bindingsReads(client)).toHaveLength(1);
  });

  it("reads again on Read again, the last answer kept up meanwhile; nothing else asks", async () => {
    const client = bindingsClient(shown(daemonRows()));
    await openNode(client);
    const panel = await openTab("Bindings");
    await within(panel).findByText("Code bindings");
    fireEvent.click(within(panel).getByRole("button", { name: "Read again" }));
    expect(within(panel).getByText("Code bindings")).toBeTruthy();
    await waitFor(() => {
      expect(bindingsReads(client)).toHaveLength(2);
    });
    await within(panel).findByRole("button", { name: "Read again" });
    expect(bindingsReads(client)).toHaveLength(2);
  });

  it("fails alone: the daemon's words, Retry reads again; the tree, the node and the other tabs stay", async () => {
    const client = bindingsClient(shown(daemonRows()));
    const message = "spec: the index is locked by another process";
    client.getNode.mockImplementation((_project, ref, options) =>
      options?.with?.includes("bindings") === true
        ? Promise.reject(new ClientError({ status: 503, message }))
        : Promise.resolve(viewOf(ref, options?.with?.includes("links") === true)),
    );
    await openNode(client);
    const panel = await openTab("Bindings");
    expect((await within(panel).findByRole("alert")).textContent).toBe(`The bindings of DOC-A could not be read${message}`);
    fireEvent.click(within(panel).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(bindingsReads(client)).toHaveLength(2);
    });
    expect(screen.getByRole("tree")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1 }).textContent).toContain("DOC-A");
  });

  it("slow: a busy skeleton until the read answers", async () => {
    const client = bindingsClient(shown([]));
    client.getNode.mockImplementation((_project, ref, options) =>
      options?.with?.includes("bindings") === true ? new Promise(() => undefined) : Promise.resolve(viewOf(ref, options?.with?.includes("links") === true)),
    );
    await openNode(client);
    await openTab("Bindings");
    expect((await screen.findByLabelText("Loading the bindings of DOC-A")).getAttribute("aria-busy")).toBe("true");
  });
});

describe("the Bindings tab's rows (UI-2 of ui-node-bindings)", () => {
  it("shows the rows in the daemon's order, never sorted, each field: node (\"\" as No ID), relation, the three place forms, binding or No symbol, layer, sync, signature, Moved from, Candidate", async () => {
    await openNode(bindingsClient(shown(daemonRows())));
    const panel = await openTab("Bindings");
    await within(panel).findByText("Code bindings");
    expect(bindingLines(panel)).toEqual([
      ["DOC-Aimplementssrc/z.rs:10-20", "item crate::z::run function_itemlayer a", "Sync: Code aheadcode_changed body", "Signature:pub fn run()", "Moved fromsrc/old.rs | item crate::old::run function_item"],
      ["DOC-Averifiessrc/a.rs", "No symbol", "Sync: Bad markerunanchored"],
      ["SEC-A1configuresdata/a.ron:4", "ron root.limit", "Sync: In sync"],
      [
        "SEC-A1implementssrc/c.rs:10-20",
        "item crate::c::step function_itemlayer a",
        "Sync: Cannot verifyambiguous_symbol duplicate",
        "Signature:pub fn run()",
        "Candidatesrc/c.rs | item crate::c::step function_item",
        "Candidatesrc/d.rs | item crate::d::step",
      ],
      [`No IDimplements${NON_LATIN_FILE}:10-20`, "item crate::b::walk function_itemlayer a", "Sync: Not judged", "Signature:pub fn run()"],
    ]);
    expect(within(panel).getByText("5 rows")).toBeTruthy();
    // Nothing in a row is a link: its node may be unresolved, its symbol is a qpath.
    expect(Array.from(panel.querySelectorAll(".binding-line a"))).toEqual([]);
    expect(panel.textContent).not.toMatch(/undefined|NaN|\[object/);
  });

  it("shows a binding row's daemon words on one line: a line feed in its file, signature or detail a space, another control \\u{..}, as Health's drift rows (UI-5 of ui-drift-evidence)", async () => {
    const LF = String.fromCharCode(10);
    const ESC = String.fromCharCode(0x1b);
    const row = aBindingRow({
      node: "DOC-A",
      file: `src/two${LF}lines.rs`,
      symbol: `crate::two${ESC}::run`,
      signature: `pub fn run(${LF}    tide: u32,${LF})`,
      sync: "code_ahead",
      category: "code_changed",
      detail: `body,${LF}deps`,
      anchor: null,
    });
    await openNode(bindingsClient(shown([row])));
    const panel = await openTab("Bindings");
    await within(panel).findByText("Code bindings");
    expect(bindingLines(panel)).toEqual([
      [
        "DOC-Aimplementssrc/two lines.rs:10-20",
        "item crate::two\\u{1b}::run function_itemlayer a",
        "Sync: Code aheadcode_changed body, deps",
        "Signature:pub fn run(     tide: u32, )",
      ],
    ]);
  });

  it("gives each holder of a REF its own rows under its place, h2 per holder, lists h3, no level skipped", async () => {
    const client = bindingsClient((holder) =>
      shown([aBindingRow({ node: "SEC-DUP", file: `src/${holder.path.replaceAll("/", "_")}.rs`, symbol: `crate::at_${String(holder.line)}` })]),
    );
    await openNode(client, "SEC-DUP");
    const panel = await openTab("Bindings");
    await within(panel).findAllByText("Code bindings");
    const holders = TREE.filter((row) => row.id === "SEC-DUP");
    expect(holders).toHaveLength(2);
    for (const holder of holders) {
      const part = holderPart(panel, `${holder.path}:${String(holder.line)}`);
      expect(within(part).getByRole("heading", { level: 2 }).textContent).toBe(`${holder.path}:${String(holder.line)}-${String(holder.line + 9)}`);
      expect(within(part).getByRole("heading", { level: 3, name: "Code bindings" })).toBeTruthy();
      expect(bindingLines(part).map((line) => line[1])).toEqual([`item crate::at_${String(holder.line)} function_itemlayer a`]);
    }
    expectNoSkippedLevel();
  });

  it("skips no heading level with one holder: its lists and the commands are h2 under the h1", async () => {
    await openNode(bindingsClient(shown(daemonRows(), [aTestedRow({ node: "DOC-A" })])));
    const panel = await openTab("Bindings");
    await within(panel).findByText("Code bindings");
    expect(within(panel).getByRole("heading", { name: "Code bindings" }).tagName).toBe("H2");
    expect(within(panel).getByRole("heading", { name: "Test evidence" }).tagName).toBe("H2");
    expect(within(panel).getByRole("heading", { name: "Commands for a terminal" }).tagName).toBe("H2");
    expectNoSkippedLevel();
  });

  it("draws a row not judged and one that cannot be verified each in its own role, neither like a row in sync", async () => {
    await openNode(bindingsClient(shown(daemonRows())));
    const panel = await openTab("Bindings");
    await within(panel).findByText("Code bindings");
    const tone = (label: string) => within(panel).getAllByText(label)[0]?.closest(".badge")?.getAttribute("data-tone");
    expect([tone("In sync"), tone("Not judged"), tone("Cannot verify")]).toEqual(["sync-ok", "sync-not-judged", "sync-cannot-verify"]);
  });

  it("shows the first 10 rows, then Show all, which shows every row and moves focus to the 11th", async () => {
    const rows = Array.from({ length: 25 }, (_, index) => aBindingRow({ node: "DOC-A", file: `src/f${String(index).padStart(2, "0")}.rs`, sync: "unlocked" }));
    await openNode(bindingsClient(shown(rows)));
    const panel = await openTab("Bindings");
    await within(panel).findByText("25 rows");
    expect(panel.querySelectorAll(".binding-line")).toHaveLength(10);
    expect(within(panel).getByText("The first 10 of 25, in spec verify's order.")).toBeTruthy();
    fireEvent.click(within(panel).getByRole("button", { name: "Show all 25" }));
    expect(panel.querySelectorAll(".binding-line")).toHaveLength(25);
    await waitFor(() => {
      expect(document.activeElement).toBe(panel.querySelectorAll(".binding-line")[10]);
    });
    expect(bindingLines(panel).map((line) => line[0])).toEqual(rows.map((row) => `DOC-Aimplements${row.file}:10-20`));
  });
});

describe("the Bindings tab's states (UI-3, UI-4 of ui-node-bindings)", () => {
  it("null: Not read and the daemon's note verbatim, never a number, never None", async () => {
    await openNode(bindingsClient(null, [NO_CODE_NOTE]));
    const panel = await openTab("Bindings");
    await within(panel).findByText(NO_CODE_NOTE);
    const part = holderPart(panel, "docs/spec/a.md:1");
    expect(part.textContent).toContain("Not read");
    expect(part.textContent).not.toMatch(/\d|None/);
    expect(within(panel).getByRole("list", { name: "Notes from the daemon on the bindings" }).textContent).toBe(NO_CODE_NOTE);
    expect(within(panel).queryByText("Code bindings")).toBeNull();
    expect(within(panel).queryByText("Test evidence")).toBeNull();
  });

  it("listed, never judged (sync null): each row Not judged, never In sync; the note verbatim", async () => {
    const note = "bindings: spec.lock:1: unclosed array, expected `]`";
    const listed = { sync: null, category: null, detail: null, anchor: null };
    await openNode(bindingsClient(shown([aBindingRow({ node: "DOC-A", ...listed }), aBindingRow({ node: "DOC-A", file: "src/b.rs", ...listed })], null), [note]));
    const panel = await openTab("Bindings");
    await within(panel).findByText(note);
    expect(within(panel).getAllByText("Not judged", { selector: ".badge-label" })).toHaveLength(2);
    expect(within(panel).queryByText("In sync")).toBeNull();
    expect(Array.from(panel.querySelectorAll(".binding-line .badge"), (badge) => badge.getAttribute("data-tone"))).toEqual(["sync-not-judged", "sync-not-judged"]);
  });

  it("zero rows: None, said as such", async () => {
    await openNode(bindingsClient(shown([])));
    const panel = await openTab("Bindings");
    const rows = (await within(panel).findByRole("heading", { name: "Code bindings" })).closest("section");
    expect(rows?.textContent).toBe("Code bindingsNone: no marker or lock entry binds code to this node or a section within it.");
    expect(panel.querySelectorAll(".binding-line")).toHaveLength(0);
  });

  it("test evidence: each entry with its state, the node linked; [] None; null Not judged, each its own words", async () => {
    const entries = [
      aTestedRow({ node: "DOC-A", state: "verified", category: null, rev: 2, node_rev: 2 }),
      aTestedRow({ node: "SEC-A1", state: "unverifiable", category: "predated", rev: 3, node_rev: null }),
    ];
    const evidence = async (tested: ShownBindings["tested"]) => {
      const client = bindingsClient(shown(daemonRows(), tested), tested === null ? ["tested: spec.lock: a [[tested]] block names no commit"] : []);
      await openNode(client);
      const panel = await openTab("Bindings");
      const heading = await within(panel).findByRole("heading", { name: "Test evidence" });
      const section = heading.closest("section");
      const text = section?.textContent ?? "";
      const links = Array.from(section?.querySelectorAll("a") ?? [], (link) => [link.textContent, link.getAttribute("href")]);
      cleanup();
      return { text, links };
    };
    const listed = await evidence(entries);
    expect(listed.text).toContain("Evidence: Verified");
    expect(listed.text).toContain("Evidence: Unverifiable");
    expect(listed.text).toContain("Vouched for rev 3");
    expect(listed.text).toContain("No revision now");
    expect(listed.links).toEqual([
      ["DOC-A", "#/alpha/tree/DOC-A"],
      ["SEC-A1", "#/alpha/tree/SEC-A1"],
    ]);
    const none = await evidence([]);
    expect(none.text).toBe("Test evidenceNone: no [[tested]] block covers this node or a section within it.");
    const notJudged = await evidence(null);
    expect(notJudged.text).toBe("Test evidenceNot judged: the [[tested]] entries were not judged on this read; the daemon\u2019s note above says why.");
    expect(new Set([listed.text, none.text, notJudged.text]).size).toBe(3);
  });
});

/** The commands block's first note, as shown. */
const NOTE_TEXT =
  "Copy a command and run it in the project\u2019s worktree; only spec lock accept asks [y/N], before it writes spec.lock, and spec propose drift queues an item for your decision. Nothing runs from here, and nothing waits on these rows.";

describe("the Bindings tab's commands (UI-5 of ui-node-bindings)", () => {
  /** The commands block's command texts and notes. */
  function commandsOf(panel: HTMLElement) {
    const block = within(panel).getByRole("heading", { name: "Commands for a terminal" }).closest("section");
    return {
      texts: Array.from(block?.querySelectorAll(".command-text") ?? [], (command) => command.textContent),
      notes: Array.from(block?.querySelectorAll(".owner-commands-note") ?? [], (note) => note.textContent),
      blocks: panel.querySelectorAll(".owner-commands").length,
    };
  }

  /** A stub answering any REF with `holders` nodes of ID `id`, each with bindings when asked. */
  function anyRefClient(id: string | null, holders: number) {
    const client = treeClient();
    client.getNode.mockImplementation((_project, ref, options) =>
      Promise.resolve({
        ref,
        reason: null,
        notes: [],
        nodes: Array.from({ length: holders }, (_, index) =>
          aNode({ id, path: `docs/features/f${String(index)}.md`, line: 5, bindings: options?.with?.includes("bindings") === true ? shown([]) : null }),
        ),
      }),
    );
    return client;
  }

  it("offers the REF's three commands to copy, once, fixed words and the REF; the note says which asks; nothing runs", async () => {
    const client = bindingsClient(shown(daemonRows()));
    await openNode(client);
    const panel = await openTab("Bindings");
    const list = await within(panel).findByRole("list", { name: "Commands for DOC-A" });
    expect(commandsOf(panel)).toEqual({
      texts: ["spec verify --node DOC-A", "spec propose drift DOC-A", "spec lock accept DOC-A"],
      notes: [NOTE_TEXT],
      blocks: 1,
    });
    expect(within(list).getAllByRole("button", { name: /^Copy/ })).toHaveLength(3);
    expect(Array.from(list.querySelectorAll(".command-meaning"), (meaning) => meaning.textContent)[1]).toBe(
      "Queue its drift: judges the node's own rows at HEAD (commit edits first) and queues an Inbox item for your decision",
    );
    // Reads only: the stub's writes are never called.
    expect(client.stageDecision).not.toHaveBeenCalled();
    expect(client.unstageDecision).not.toHaveBeenCalled();
  });

  it("builds them from the REF, never a holder's bare ID: a feature-scoped REF with several holders gets verify alone, once, with the note", async () => {
    await openNode(anyRefClient("AC-07", 2), "slug/AC-07");
    const panel = await openTab("Bindings");
    await within(panel).findByRole("list", { name: "Commands for slug/AC-07" });
    expect(commandsOf(panel)).toEqual({
      texts: ["spec verify --node slug/AC-07"],
      notes: [
        NOTE_TEXT,
        "Only verify is offered: slug/AC-07 names 2 nodes, and spec propose drift and spec lock accept each name one.",
      ],
      blocks: 1,
    });
    expect(panel.textContent).not.toMatch(/--node AC-07|drift AC-07|accept AC-07/);
  });

  it("gives one REF's two holders (SEC-DUP) verify alone; a feature-scoped REF with one holder all three", async () => {
    await openNode(bindingsClient(shown([])), "SEC-DUP");
    let panel = await openTab("Bindings");
    await within(panel).findByRole("list", { name: "Commands for SEC-DUP" });
    expect(commandsOf(panel).texts).toEqual(["spec verify --node SEC-DUP"]);
    cleanup();
    await openNode(anyRefClient("AC-07", 1), "slug/AC-07");
    panel = await openTab("Bindings");
    await within(panel).findByRole("list", { name: "Commands for slug/AC-07" });
    expect(commandsOf(panel).texts).toEqual(["spec verify --node slug/AC-07", "spec propose drift slug/AC-07", "spec lock accept slug/AC-07"]);
  });

  it("offers an ID-less document verify alone, by its path REF, with the note; none for a REF the check refuses, whatever the holder's ID", async () => {
    await openNode(bindingsClient(shown([])), "docs/spec/b.md");
    let panel = await openTab("Bindings");
    await within(panel).findByRole("list", { name: "Commands for docs/spec/b.md" });
    expect(commandsOf(panel)).toEqual({
      texts: ["spec verify --node docs/spec/b.md"],
      notes: [NOTE_TEXT, "docs/spec/b.md names a document with no ID; spec propose drift and spec lock accept need one."],
      blocks: 1,
    });
    cleanup();
    for (const refused of ["DOC-A#SEC-A1", "DOC-A; rm -rf ~"]) {
      await openNode(anyRefClient("DOC-A", 1), refused);
      panel = await openTab("Bindings");
      expect((await within(panel).findByText(/^No command for/)).textContent).toBe(`No command for ${refused}: only a REF of Latin letters, digits and . _ / - is put in a command.`);
      expect(panel.querySelectorAll(".command-text")).toHaveLength(0);
      cleanup();
    }
  });
});

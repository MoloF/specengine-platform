import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import { ClientError } from "../api/client";
import { App } from "../app/App";
import type { NodeView, ShownLinks } from "../api/types";
import { aBundle, aBundleItem, aLink, aNode, aProposal, aSearchHit, aSearchResults, aTreeView, someLayers } from "../test/builders";
import { renderApp } from "../test/render";
import type { StubClient } from "../test/stubClient";
import { LINKS, textOf, TREE, treeClient } from "../test/treeStub";

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
 * (docs/features/ui-markdown.md "Data", the shipped criteria it changes): AC-06's verbatim text is Source's.
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
    expect(tabs.map((each) => each.textContent.replace(/\d+$/, "").trim())).toEqual(["Text", "Links", "Bundle", "Proposals"]);
    expect(tabs.map((each) => each.getAttribute("aria-selected"))).toEqual(["true", "false", "false", "false"]);
    expect(tabs.map((each) => each.tabIndex)).toEqual([0, -1, -1, -1]);
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
    expect(document.activeElement).toBe(tabs[3]);
    fireEvent.keyDown(tabs[3] ?? list, { key: "ArrowRight" });
    expect(document.activeElement).toBe(tabs[0]);
    fireEvent.keyDown(tabs[0] ?? list, { key: "ArrowLeft" });
    expect(document.activeElement).toBe(tabs[3]);
    fireEvent.keyDown(tabs[3] ?? list, { key: "Home" });
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
    expect(client.getBundle).toHaveBeenLastCalledWith("alpha", { node_ids: ["DOC-A"] });
    fireEvent.click(tab("Text"));
    fireEvent.click(tab("Bundle"));
    expect(client.getBundle).toHaveBeenCalledTimes(1);
    const field = within(panel).getByLabelText("Budget in tokens");
    fireEvent.change(field, { target: { value: "5000" } });
    fireEvent.submit(field.closest("form") ?? panel);
    await waitFor(() => {
      expect(client.getBundle).toHaveBeenCalledTimes(2);
    });
    expect(client.getBundle).toHaveBeenLastCalledWith("alpha", { node_ids: ["DOC-A"], budget: 5000 });
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
      expect(client.getBundle).toHaveBeenLastCalledWith("alpha", { node_ids: ["DOC-A"], budget: 4294967295 });
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

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { NodeView, ShownLinks } from "../api/types";
import { aBundle, aLink, aNode, aProposal, aSearchHit, aSearchResults, aTaskPackage, aTaskRun } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { taskClient } from "../test/taskStub";
import { treeClient } from "../test/treeStub";

// docs/features/ui-markdown.md through the views on stub clients: AC-04 (anchors from the node's
// links read), AC-05 (what opening a node reads; the text first), AC-08 (jumps in the rendered
// text), AC-09 (Rendered by default, kept; Source as today), AC-12 (the verbatim fields), and the
// inbox's and the task's prose.

const PATH = "docs/spec/a.md";

/** DOC-A as a document: front-matter (lines 1-3), a link line (7), a {#ID} section (9). */
const DOC_TEXT = [
  "---",
  "id: DOC-A",
  "---",
  "",
  "# The A document",
  "",
  "See [the rule](docs/r9.md) and [gone](docs/gone.md).",
  "",
  "## Window {#SEC-W}",
  "",
  "Body   with  spaces kept in Source.  ",
  "",
].join("\n");

const DOC_LINKS: ShownLinks = {
  outgoing: [
    aLink({ type: "mentions", origin: "inline", written: "docs/r9.md", name: "R-9", path: PATH, line: 7 }),
    aLink({
      type: "mentions",
      origin: "inline",
      written: "docs/gone.md",
      name: null,
      path: PATH,
      line: 7,
      state: "dangling",
      reason: "`docs/gone.md` names no file",
    }),
  ],
  incoming: [],
  left_out: { generated: 0, tier3: 0 },
  omitted: 0,
};

function docView(ref: string, links: boolean): NodeView {
  return {
    ref,
    reason: null,
    notes: [],
    nodes: [aNode({ id: "DOC-A", path: PATH, line: 1, end_line: 12, sections: ["SEC-W"], text: DOC_TEXT, links: links ? DOC_LINKS : null })],
  };
}

/** A tree client whose DOC-A is the document above; its links read waits for `release` when held. */
function docClient(hold = false) {
  const client = treeClient();
  let release: () => void = () => undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  client.getNode.mockImplementation((_project, ref, options) => {
    if (ref !== "DOC-A") {
      return Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref, text: `## ${ref}\n\nOther text.\n` })] });
    }
    const links = options?.with?.includes("links") === true;
    if (links && hold) {
      return held.then(() => docView(ref, true));
    }
    return Promise.resolve(docView(ref, links));
  });
  return {
    client,
    release: async () => {
      await act(async () => {
        release();
        await held;
      });
    },
  };
}

function textPanel(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "Text" });
}

async function renderedText(): Promise<HTMLElement> {
  await screen.findByRole("heading", { name: "The A document" });
  const root = textPanel().querySelector<HTMLElement>(".markdown");
  if (root === null) {
    throw new Error("no rendered text");
  }
  return root;
}

function modeButton(name: "Rendered" | "Source", group = "Show the text as"): HTMLElement {
  return within(screen.getByRole("group", { name: group })).getByRole("button", { name });
}

describe("anchors from the node's links read (AC-04)", () => {
  it("anchors a link the read resolved to its name, shows a dangling one as text with its label and icon", async () => {
    const { client } = docClient();
    renderApp(client, "#/alpha/tree/DOC-A");
    const root = await renderedText();
    const anchor = await within(root).findByRole("link", { name: "the rule" });
    expect(anchor.getAttribute("href")).toBe("#/alpha/tree/R-9");
    const gone = within(root).getByText("gone");
    expect(gone.closest("a")).toBeNull();
    const label = gone.closest(".md-link")?.querySelector(".badge");
    expect(label?.querySelector(".badge-label")?.textContent).toBe("Dangling");
    expect(label?.querySelector("svg")).not.toBeNull();
    expect(Array.from(root.querySelectorAll("a"), (each) => each.getAttribute("href"))).toEqual(["#/alpha/tree/R-9"]);
  });

  it("anchors nothing before the read lands, then anchors without parsing the text again", async () => {
    const { client, release } = docClient(true);
    renderApp(client, "#/alpha/tree/DOC-A");
    const root = await renderedText();
    expect(root.querySelector("a")).toBeNull();
    expect(within(root).getByText("the rule")).toBeTruthy();
    const heading = within(root).getByRole("heading", { name: "The A document" });
    await release();
    expect((await within(root).findByRole("link", { name: "the rule" })).getAttribute("href")).toBe("#/alpha/tree/R-9");
    // The same heading element: the links landing re-rendered the anchors, not the parse.
    expect(within(root).getByRole("heading", { name: "The A document" })).toBe(heading);
  });
});

describe("what opening a node reads (AC-05)", () => {
  it("reads the node once plainly and once with links; the text shows first; the Links tab adds no read", async () => {
    const { client, release } = docClient(true);
    renderApp(client, "#/alpha/tree/DOC-A");
    await renderedText();
    expect(client.getNode.mock.calls.map((call) => call.slice(1))).toEqual([["DOC-A"], ["DOC-A", { with: ["links"] }]]);
    await release();
    fireEvent.click(screen.getByRole("tab", { name: "Links" }));
    const links = await screen.findByRole("tabpanel", { name: "Links" });
    await within(links).findByText("docs/gone.md");
    expect(client.getNode).toHaveBeenCalledTimes(2);
  });
});

describe("jumps in the rendered text (AC-08)", () => {
  it("Sections focuses the section's heading block, front-matter lines counted", async () => {
    const { client } = docClient();
    renderApp(client, "#/alpha/tree/DOC-A");
    await renderedText();
    fireEvent.click(within(screen.getByRole("navigation", { name: "Sections of DOC-A" })).getByRole("button", { name: /SEC-W/ }));
    await waitFor(() => {
      expect(document.activeElement?.getAttribute("data-line")).toBe("9");
    });
    expect(document.activeElement?.tagName).toBe("H3");
    expect(document.activeElement?.textContent).toBe("Window SEC-W");
  });

  it("Show in text focuses the block holding the link's line", async () => {
    const { client } = docClient();
    renderApp(client, "#/alpha/tree/DOC-A");
    await renderedText();
    fireEvent.click(screen.getByRole("tab", { name: "Links" }));
    const links = await screen.findByRole("tabpanel", { name: "Links" });
    fireEvent.click((await within(links).findAllByRole("button", { name: /Show in text/ }))[0] ?? links);
    await waitFor(() => {
      expect(document.activeElement?.getAttribute("data-line")).toBe("7");
    });
    expect(document.activeElement?.tagName).toBe("P");
    expect(screen.getByRole("tab", { name: "Text" }).getAttribute("aria-selected")).toBe("true");
  });
});

describe("Rendered and Source (AC-09)", () => {
  it("leaves focus on the switch after a jump: a new mode does not jump again (WCAG 3.2.2)", async () => {
    const { client } = docClient();
    renderApp(client, "#/alpha/tree/DOC-A");
    await renderedText();
    fireEvent.click(within(screen.getByRole("navigation", { name: "Sections of DOC-A" })).getByRole("button", { name: /SEC-W/ }));
    await waitFor(() => {
      expect(document.activeElement?.getAttribute("data-line")).toBe("9");
    });
    const source = modeButton("Source");
    source.focus();
    fireEvent.click(source);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(textPanel().querySelector(".text-line")).not.toBeNull();
    expect(document.activeElement).toBe(source);
    expect(textPanel().querySelector("[data-target]")).toBeNull();
    const rendered = modeButton("Rendered");
    rendered.focus();
    fireEvent.click(rendered);
    await within(textPanel()).findByRole("heading", { name: "The A document" });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(document.activeElement).toBe(rendered);
    expect(textPanel().querySelector("[data-target]")).toBeNull();
  });

  it("renders by default, shows Source as today's numbered text exactly, and keeps the choice across nodes", async () => {
    const { client } = docClient();
    renderApp(client, "#/alpha/tree/DOC-A");
    await renderedText();
    expect(modeButton("Rendered").getAttribute("aria-pressed")).toBe("true");
    expect(modeButton("Source").getAttribute("aria-pressed")).toBe("false");
    expect(textPanel().querySelector(".text-line")).toBeNull();
    fireEvent.click(modeButton("Source"));
    expect(modeButton("Source").getAttribute("aria-pressed")).toBe("true");
    expect(textPanel().querySelector(".markdown")).toBeNull();
    expect(Array.from(textPanel().querySelectorAll(".text-line-content"), (line) => line.textContent).join("")).toBe(DOC_TEXT);
    expect(textPanel().querySelector(".text-line-number")?.textContent).toBe("1");
    // Another node: Source still.
    window.location.hash = "#/alpha/tree/SEC-A1";
    await screen.findByRole("heading", { level: 1, name: /SEC-A1/ });
    await waitFor(() => {
      expect(textPanel().querySelector(".text-line-content")?.textContent).toBe("## SEC-A1\n");
    });
    expect(modeButton("Source").getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(modeButton("Rendered"));
    expect(await within(textPanel()).findByRole("heading", { name: "SEC-A1" })).toBeTruthy();
  });
});

describe("the verbatim fields (AC-12)", () => {
  const MARKUP = "## Not a heading **not bold** [not](a.md) <b>x</b>";

  it("keeps diffs, conflicts, evidence and notes as sent", async () => {
    const diff = ["--- base docs/spec/a.md", "+++ proposed docs/spec/a.md", "@@ -1,2 +1,2 @@", ` ${MARKUP}`, "-**old**", "+**new**"].join("\n");
    const proposal = aProposal({
      id: "PR-1",
      kind: "discrepancy",
      target_ids: ["R-1"],
      summary: "Summary",
      diff,
      conflict: `<<<<<<< current\n${MARKUP}\n=======\n**x**\n>>>>>>> proposed`,
      evidence: [{ file: "src/a.rs", qpath: null, lines: "1-2", observed: `\`code\` ${MARKUP}`, documented: "**doc**" }],
      notes: [MARKUP],
    });
    renderApp(stubClient([proposal]), "#/alpha/inbox/PR-1");
    const card = await screen.findByRole("article");
    const figure = await within(card).findByRole("figure", { name: "Section diff" });
    expect(figure.querySelector("pre.diff")?.textContent.includes(` ${MARKUP}`)).toBe(true);
    expect(figure.querySelector("strong, h1, h2, h3, h4, h5, h6, a")).toBeNull();
    const conflict = Array.from(card.querySelectorAll("pre.node-text")).find((pre) => pre.textContent.startsWith("<<<<<<<"));
    expect(conflict?.textContent).toBe(proposal.conflict);
    const evidence = card.querySelector(".evidence");
    expect(evidence?.textContent).toContain(`\`code\` ${MARKUP}`);
    expect(evidence?.textContent).toContain("**doc**");
    expect(evidence?.querySelector("strong, code, a")).toBeNull();
    expect(within(card).getByText(MARKUP, { selector: ".note-list li" })).toBeTruthy();
  });

  it("keeps search snippets, the bundle body and a task's package as sent", async () => {
    const client = treeClient();
    client.search.mockResolvedValue(aSearchResults([aSearchHit({ id: "DOC-A", snippet: { segments: [{ text: MARKUP, hit: false }], cut_start: false, cut_end: false } })]));
    client.getBundle.mockImplementation((_project, options) => Promise.resolve(aBundle(options.node_ids, { body: `# Bundle\n\n${MARKUP}\n` })));
    renderApp(client, "#/alpha/tree/DOC-A");
    await screen.findByRole("tab", { name: "Text" });
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: "x" } });
    fireEvent.submit(screen.getByRole("search"));
    const hits = await screen.findByRole("listbox", { name: /Search hits/ });
    expect(hits.textContent).toContain(MARKUP);
    expect(hits.querySelector("strong, h2, a[href]")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Bundle" }));
    const bundle = await screen.findByRole("tabpanel", { name: "Bundle" });
    await waitFor(() => {
      expect(bundle.querySelector(".bundle-body pre")?.textContent).toBe(`# Bundle\n\n${MARKUP}\n`);
    });
  });

  it("keeps a task's package JSON and spec changes as sent", async () => {
    const pkg = aTaskPackage({
      id: "T-0001",
      plan: MARKUP,
      snapshot_diff: [{ id: "R-1", path: PATH, span_hash: "b3:00", diff: `@@ -1 +1 @@\n-${MARKUP}\n+**new**`, cut: false }],
    });
    renderApp(taskClient([], [pkg]), "#/alpha/tasks/T-0001");
    await screen.findByRole("tab", { name: /^Overview/, selected: true });
    fireEvent.click(screen.getByRole("tab", { name: /^Package/ }));
    expect(document.querySelector(".task-json")?.textContent).toBe(JSON.stringify(pkg, null, 2));
  });
});

describe("prose in the inbox and the tasks", () => {
  it("renders a target's current section with its own Rendered/Source switch, and the proposal's prose", async () => {
    const proposal = aProposal({
      id: "PR-2",
      kind: "question",
      target_ids: ["R-1"],
      summary: "Which window?",
      working_answer: "Use **90 minutes**.",
      price_of_other: "Rework of `RULE-X`.",
      rationale: "## Why\n\nBecause.",
      options: [{ label: "Wide", effect: "A *wider* window", price: "**More** traffic" }],
      decision_note: "Deferred: _later_.",
    });
    const client = stubClient([proposal]);
    client.getNode.mockImplementation((_project, ref) =>
      Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref, line: 5, end_line: 8, text: "## R-1 {#R-1}\n\nThe **rule**  text.\n" })] }),
    );
    renderApp(client, "#/alpha/inbox/PR-2");
    const card = await screen.findByRole("article");
    const group = "Show the current section of R-1 as";
    const rendered = await within(card).findByRole("heading", { name: "R-1 R-1" });
    expect(rendered.tagName).toBe("H4");
    expect(within(card).getByText("rule").tagName).toBe("STRONG");
    expect(within(card).getByText("90 minutes").tagName).toBe("STRONG");
    expect(within(card).getByText("wider").tagName).toBe("EM");
    expect(within(card).getByText("More").tagName).toBe("STRONG");
    expect(within(card).getByText("later").tagName).toBe("EM");
    expect(within(card).getByRole("heading", { name: "Why" }).tagName).toBe("H4");
    expect(within(card).getByText("RULE-X").tagName).toBe("CODE");
    fireEvent.click(modeButton("Source", group));
    expect(card.querySelector(".target pre.node-text")?.textContent).toBe("## R-1 {#R-1}\n\nThe **rule**  text.\n");
    expect(modeButton("Source", group).getAttribute("aria-pressed")).toBe("true");
  });

  it("renders a task's goal, plan and run summary; the plan's headings under the task's h1", async () => {
    const pkg = aTaskPackage({
      id: "T-0002",
      goal: "Ship the **window**.",
      plan: "## Steps\n\n1. Read\n2. Write `code`\n",
      runs: [aTaskRun({ run: 1, ended_at: "2026-10-02T10:00:00Z", outcome: "completed", summary: "Done with *care*." })],
    });
    renderApp(taskClient([], [pkg]), "#/alpha/tasks/T-0002");
    await screen.findByRole("tab", { name: /^Overview/, selected: true });
    expect((await screen.findByText("window")).tagName).toBe("STRONG");
    fireEvent.click(screen.getByRole("tab", { name: /^Plan/ }));
    expect((await screen.findByRole("heading", { name: "Steps" })).tagName).toBe("H2");
    const plan = screen.getByRole("tabpanel", { name: /^Plan/ });
    expect(within(plan).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["Read", "Write code"]);
    fireEvent.click(screen.getByRole("tab", { name: /^Runs/ }));
    expect((await screen.findByText("care")).tagName).toBe("EM");
  });
});

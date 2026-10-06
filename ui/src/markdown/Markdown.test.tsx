import { cleanup, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import type { ShownLink } from "../api/types";
import { aLink } from "../test/builders";
import { Markdown, type MarkdownProps } from "./Markdown";

// docs/features/ui-markdown.md on the renderer itself: AC-03 (a hostile corpus: library anchors
// bypass the source scan, so the DOM is checked), AC-04 (anchors only from the links read), AC-06
// (external links and images as text), AC-07 (headings), AC-08 (`data-line`), AC-10
// (front-matter), AC-11 (tables, task items, code), AC-13 (code points kept, long tokens wrap).
// Non-Latin text is escaped (ADR-0024); dialog calls are spelled with an escape.

const PATH = "docs/spec/a.md";

function show(source: string, props: Partial<MarkdownProps> = {}) {
  const { container } = render(
    <Markdown source={source} links={null} project="alpha" baseLevel={1} frontMatter={false} path={PATH} {...props} />,
  );
  const root = container.querySelector<HTMLElement>(".markdown");
  if (root === null) {
    throw new Error("no .markdown");
  }
  return root;
}

function inline(written: string, line: number, fields: Partial<ShownLink> = {}): ShownLink {
  return aLink({ type: "mentions", origin: "inline", written, name: null, path: PATH, line, state: "resolved", ...fields });
}

/** Every element's attribute names, by tag, deduplicated. */
function attributesOf(root: Element): string[] {
  const names = new Set<string>();
  for (const element of root.querySelectorAll("*")) {
    for (const attribute of Array.from(element.attributes)) {
      names.add(`${element.tagName.toLowerCase()}[${attribute.name}]`);
    }
  }
  return [...names].sort();
}

describe("a hostile corpus stays text (AC-03)", () => {
  const source = [
    "# Hostile",
    "",
    "<script>\u0061lert(1)</script>",
    "",
    "Inline <img src=x onerror=\u0061lert(1)> and <iframe src=\"https://evil.example/\"></iframe> here.",
    "",
    "<style>body { display: none }</style>",
    "",
    "<form action=\"https://evil.example/\"><input name=\"x\"></form>",
    "",
    "[js](javascript:\u0061lert(1)) [data](data:text/html;base64,PHNjcmlwdD4=) [ok](docs/ok.md \"t\\\" onmouseover=\\\"x\")",
    "",
    "<a href=\"javascript:x\" onclick=\"y\">raw anchor</a>",
    "",
    "A footnote[^n] and https://autolink.example/x and <https://angle.example/> and someone@mail.example.",
    "",
    "[^n]: The note, with <b onclick=\"z\">markup</b>.",
    "",
    "![tracker](https://evil.example/pixel.png)",
  ].join("\n");
  const links = [inline("docs/ok.md", 11, { name: "R-OK" })];

  it("creates no active element, no id or on* attribute, and anchors only to #/", () => {
    const root = show(source, { links });
    for (const tag of ["script", "img", "iframe", "style", "form", "input", "object", "embed", "link", "meta", "base", "svg image"]) {
      expect([tag, root.querySelector(tag)]).toEqual([tag, null]);
    }
    const attributes = attributesOf(root);
    expect(attributes.filter((name) => /\[(id|on[a-z]+|src\w*|action|style|title|target|name)\]$/.test(name))).toEqual([]);
    const anchors = Array.from(root.querySelectorAll("a"));
    expect(anchors.map((anchor) => anchor.getAttribute("href"))).toEqual(["#/alpha/tree/R-OK"]);
    // An anchor writes its href and class, nothing it was handed (the parse's title, data, node).
    expect(attributes.filter((name) => name.startsWith("a["))).toEqual(["a[class]", "a[href]"]);
  });

  it("shows the markup as text", () => {
    const text = show(source, { links }).textContent;
    for (const part of [
      "<script>\u0061lert(1)</script>",
      "<img src=x onerror=\u0061lert(1)>",
      '<iframe src="https://evil.example/">',
      "<style>body { display: none }</style>",
      '<form action="https://evil.example/"><input name="x">',
      "javascript:\u0061lert(1)",
      "data:text/html;base64,PHNjcmlwdD4=",
      '<a href="javascript:x" onclick="y">',
      "raw anchor",
      '<b onclick="z">',
      "https://autolink.example/x",
      "Image: tracker",
      "https://evil.example/pixel.png",
    ]) {
      expect([part, text.includes(part)]).toEqual([part, true]);
    }
  });

  it("gives a footnote no generated heading, id or in-page link", () => {
    const root = show("Text[^1].\n\n[^1]: The note.\n");
    expect(root.querySelectorAll("h1, h2, h3, h4, h5, h6, section, a")).toHaveLength(0);
    expect(root.querySelector(".md-footnote-ref")?.textContent).toBe("[1]");
    expect(root.querySelector(".md-footnote")?.textContent).toContain("The note.");
  });

  it("keeps raw HTML's whitespace as written: an indented <pre> block, inline tags", () => {
    const root = show("<pre>\n  keep\n    this\n</pre>\n\nInline <b>  spaced </b> tag.\n");
    expect(root.querySelector(".md-raw-html")?.textContent).toBe("<pre>\n  keep\n    this\n</pre>");
    expect(root.querySelector("pre")).toBeNull();
    expect(Array.from(root.querySelectorAll(".md-raw-inline"), (part) => part.textContent)).toEqual(["<b>", "</b>"]);
    expect(root.querySelectorAll("p")[1]?.textContent).toBe("Inline <b>  spaced </b> tag.");
  });

  it("drops empty named anchors as the core reads them: self-closed, or with other attributes", () => {
    const root = show('Before <a name="x"/> and <a id="y" class="z"></a> and <a NAME=w ></a> after.\n\n<a href="docs/a.md"></a> <a id=""></a>\n');
    const [first, second] = Array.from(root.querySelectorAll("p"), (paragraph) => paragraph.textContent);
    expect(first).toBe("Before  and  and  after.");
    // No id or name, or an empty one: no anchor to the core, so shown.
    expect(second).toBe('<a href="docs/a.md"></a> <a id=""></a>');
  });

  it("drops comments and empty named anchors; keeps every other tag as text", () => {
    const root = show('<a id="kept-out"></a>\n\n## Title <!-- hidden -->\n\n<!-- a block comment -->\n\n<a name="x"></a> Text <a id=y>raw</a>\n');
    const text = root.textContent;
    expect(text).not.toContain("kept-out");
    expect(text).not.toContain("hidden");
    expect(text).not.toContain("block comment");
    expect(text).not.toContain('name="x"');
    expect(text).toContain("<a id=y>raw</a>");
    expect(root.querySelectorAll("p")).toHaveLength(1);
  });
});

describe("links (AC-04, AC-06)", () => {
  const source = "Intro.\n\nSee [the rule](docs/r9.md#SEC-1), [gone](docs/gone.md) and [later](docs/later.md).\n";

  it("anchors a link the links read resolved, by its name; any other state is text with its label and icon", () => {
    const links = [
      inline("docs/r9.md#SEC-1", 3, { name: "R-9" }),
      inline("docs/gone.md", 3, { state: "dangling", reason: "`docs/gone.md` names no file" }),
    ];
    const root = show(source, { links });
    expect(within(root).getByRole("link", { name: "the rule" }).getAttribute("href")).toBe("#/alpha/tree/R-9");
    const gone = within(root).getByText("gone").closest(".md-link");
    expect(gone?.closest("a")).toBeNull();
    expect(gone?.querySelector(".badge-label")?.textContent).toBe("Dangling");
    expect(gone?.querySelector(".badge svg")).not.toBeNull();
    // Not listed at all: text, no label.
    const later = within(root).getByText("later");
    expect(later.closest("a")).toBeNull();
    expect(later.querySelector(".badge")).toBeNull();
  });

  it("matches the place and the bytes: another line, path, origin or written form is no anchor", () => {
    for (const other of [
      inline("docs/r9.md#SEC-1", 4, { name: "R-9" }),
      inline("docs/r9.md#SEC-1", 3, { name: "R-9", path: "docs/spec/other.md" }),
      inline("docs/r9.md#SEC-1", 3, { name: "R-9", origin: "frontmatter" }),
      inline("docs/r9.md", 3, { name: "R-9" }),
    ]) {
      const root = show(source, { links: [other] });
      expect(root.querySelector("a")).toBeNull();
      cleanup();
    }
  });

  it("counts the destination's line from the text's first line in the file, front-matter included", () => {
    const text = "---\nid: DOC-A\n---\n\n[r](docs/r.md)\n";
    const root = show(text, { links: [inline("docs/r.md", 14, { name: "R-1" })], frontMatter: true, firstLine: 10 });
    expect(within(root).getByRole("link", { name: "r" }).getAttribute("href")).toBe("#/alpha/tree/R-1");
  });

  it("anchors only when every entry at the link's place resolved to the same node: `[R-9](R-9)`", () => {
    // The core lists the text's ID as a mention and the destination as a file link, both inline.
    const mention = inline("R-9", 1, { name: "R-9" });
    const file = inline("R-9", 1, { name: null, state: "unchecked", reason: "`R-9` names no Markdown file" });
    const text = show("See [R-9](R-9).\n", { links: [mention, file] });
    expect(text.querySelector("a")).toBeNull();
    expect(text.querySelector(".md-link-unresolved .badge-label")?.textContent).toBe("Not checked");
    cleanup();
    const both = show("See [R-9](R-9).\n", { links: [mention, { ...file, name: "R-9", state: "resolved", reason: null }] });
    expect(within(both).getByRole("link", { name: "R-9" }).getAttribute("href")).toBe("#/alpha/tree/R-9");
    cleanup();
    const apart = show("See [R-9](R-9).\n", { links: [mention, { ...file, name: "R-8", state: "resolved", reason: null }] });
    expect(apart.querySelector("a")).toBeNull();
    expect(apart.querySelector(".badge")).toBeNull();
  });

  it("reads past a leading BOM: offsets, lines and anchors as without it", () => {
    for (const frontMatter of [true, false]) {
      const root = show("\uFEFF# T\n\nSee [r](docs/r.md).\n", { links: [inline("docs/r.md", 3, { name: "R-1" })], frontMatter });
      expect(within(root).getByRole("link", { name: "r" }).getAttribute("href")).toBe("#/alpha/tree/R-1");
      expect(root.querySelector(".md-heading")?.getAttribute("data-line")).toBe("1");
      expect(root.querySelector(".md-url")).toBeNull();
      cleanup();
    }
  });

  it("reads a reference link at its definition", () => {
    const root = show("Use [the ref][r].\n\n[r]: <docs/ref file.md>\n", { links: [inline("docs/ref file.md", 3, { name: "R-REF" })] });
    expect(within(root).getByRole("link", { name: "the ref" }).getAttribute("href")).toBe("#/alpha/tree/R-REF");
  });

  it("shows no anchor before the links read lands", () => {
    const root = show(source, { links: null });
    expect(root.querySelector("a")).toBeNull();
    expect(root.textContent).toContain("the rule");
  });

  it("shows an external link's text and URL, an autolink and a mailto as text, an image as its alt and path", () => {
    const root = show("[docs](https://example.com/a?b=1) <https://angle.example/> [mail](mailto:a@b.example) ![a](p.png) ![](q.png)\n");
    expect(root.querySelector("a")).toBeNull();
    expect(root.querySelector("img")).toBeNull();
    expect(root.querySelector(".md-link-external")?.textContent).toBe("docs https://example.com/a?b=1");
    expect(Array.from(root.querySelectorAll(".md-url"), (url) => url.textContent)).toEqual([
      "https://example.com/a?b=1",
      "https://angle.example/",
      "mailto:a@b.example",
    ]);
    expect(Array.from(root.querySelectorAll(".md-image"), (image) => image.textContent)).toEqual(["Image: a p.png", "Image q.png"]);
    expect(/\.md-url,\s*\.md-image-path\s*\{[^}]*font-family:\s*var\(--font-mono\)/.test(appCss)).toBe(true);
  });

  it("links no bare ID or ID#SECTION in prose", () => {
    const root = show("See R-9 and R-9#SEC-1.\n", { links: [inline("R-9", 1, { name: "R-9" })] });
    expect(root.querySelector("a")).toBeNull();
  });
});

describe("headings (AC-07)", () => {
  it("shifts the headings below the view's own: no h1, the shallowest one level below", () => {
    const root = show("# T\n\n## Sub\n\n#### Deep\n");
    expect(Array.from(root.querySelectorAll("h1, h2, h3, h4, h5, h6"), (heading) => heading.tagName)).toEqual(["H2", "H3", "H5"]);
    const under = show("## Only\n\n### Below\n", { baseLevel: 3 });
    expect(Array.from(under.querySelectorAll(".md-heading"), (heading) => heading.tagName)).toEqual(["H4", "H5"]);
    const capped = show("# A\n\n###### F\n", { baseLevel: 5 });
    expect(Array.from(capped.querySelectorAll(".md-heading"), (heading) => heading.tagName)).toEqual(["H6", "H6"]);
  });

  it("hides a trailing {...} and shows its #ID as a label, with no id attribute", () => {
    const root = show("## W {#RULE-X}\n\n### V { .wide #RULE-Y data-x=1 }\n\n### U {.only-class}\n");
    const [w, v, u] = Array.from(root.querySelectorAll<HTMLElement>(".md-heading"));
    expect(w?.textContent).toBe("W RULE-X");
    expect(w?.querySelector(".md-heading-id")?.textContent).toBe("RULE-X");
    expect(v?.querySelector(".md-heading-id")?.textContent).toBe("RULE-Y");
    expect(u?.textContent).toBe("U");
    expect(root.textContent).not.toContain("{");
    expect(root.querySelector("[id]")).toBeNull();
  });

  it("puts one space before the label: none for a heading that is only its block, one past a dropped comment", () => {
    const root = show("## T2 <!-- c --> {#Y}\n\n## {#ONLY}\n\n## T3 {#Z} <!-- after -->\n");
    expect(Array.from(root.querySelectorAll(".md-heading"), (heading) => heading.textContent)).toEqual(["T2 Y", "ONLY", "T3 Z"]);
  });
});

describe("lines (AC-08)", () => {
  it("puts the file line of each block on it, front-matter lines counted", () => {
    const root = show("---\nid: DOC-A\n---\n\n# T\n\nPara\nnext\n\n- one\n- two\n", { frontMatter: true, firstLine: 20 });
    expect(Array.from(root.querySelectorAll<HTMLElement>("[data-line]"), (block) => `${block.tagName}:${block.dataset.line ?? ""}`)).toEqual([
      "PRE:20",
      "H2:24",
      "P:26",
      "UL:29",
      "LI:29",
      "LI:30",
    ]);
  });

  it("focuses the block holding a jumped-to line", () => {
    const text = "# T\n\nOne\ntwo\nthree\n\n## S\n";
    const { rerender } = render(<Markdown source={text} links={null} project="alpha" baseLevel={1} frontMatter={false} firstLine={5} target={null} />);
    rerender(<Markdown source={text} links={null} project="alpha" baseLevel={1} frontMatter={false} firstLine={5} target={{ line: 8, nonce: 1 }} />);
    expect(document.activeElement?.tagName).toBe("P");
    expect(document.activeElement?.getAttribute("data-line")).toBe("7");
    rerender(<Markdown source={text} links={null} project="alpha" baseLevel={1} frontMatter={false} firstLine={5} target={{ line: 11, nonce: 2 }} />);
    expect(document.activeElement?.tagName).toBe("H3");
    expect(document.querySelectorAll("[data-target]")).toHaveLength(1);
  });
});

describe("front-matter (AC-10)", () => {
  it("shows a document's front-matter as one verbatim block, never a rule or a heading", () => {
    const block = "---\nid: DOC-A\ntitle: **not bold**\n---\n";
    const root = show(`${block}\nBody.\n`, { frontMatter: true });
    expect(root.querySelector(".md-front-matter")?.textContent).toBe(block);
    expect(root.querySelector("hr")).toBeNull();
    expect(root.querySelectorAll(".md-heading")).toHaveLength(0);
    expect(root.querySelector("strong")).toBeNull();
  });

  it("renders an unclosed one as markdown, and a section's text never takes one", () => {
    const unclosed = show("---\nid: DOC-A\n\nBody.\n", { frontMatter: true });
    expect(unclosed.querySelector(".md-front-matter")).toBeNull();
    expect(unclosed.querySelector("hr")).not.toBeNull();
    const section = show("---\nid: x\n---\n", { frontMatter: false });
    expect(section.querySelector(".md-front-matter")).toBeNull();
  });
});

describe("tables, task items, code (AC-11)", () => {
  it("gives a table header cells and a named, focusable scroll region", () => {
    const root = show("| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n");
    const region = within(root).getByRole("region", { name: "Table at line 1" });
    expect(region.tabIndex).toBe(0);
    expect(Array.from(region.querySelectorAll("th"), (cell) => cell.textContent)).toEqual(["A", "B"]);
    expect(region.querySelectorAll("tbody tr")).toHaveLength(2);
  });

  it("shows a task item's state as an icon and a word, never a form control", () => {
    const root = show("- [x] shipped\n- [ ] waiting\n");
    expect(root.querySelector("input")).toBeNull();
    const states = Array.from(root.querySelectorAll(".md-task-state"), (state) => [state.textContent, state.querySelector("svg") !== null]);
    expect(states).toEqual([
      ["Done", true],
      ["Not done", true],
    ]);
  });

  it("keeps a code block's whitespace and shows its info string as a label", () => {
    const code = "  fn main() {\n\tlet x = 1;   \n\n  }";
    const root = show(`\`\`\`rust edition=2024\n${code}\n\`\`\`\n`);
    expect(root.querySelector(".md-code-info")?.textContent).toBe("rust edition=2024");
    expect(root.querySelector("pre code")?.textContent).toBe(`${code}\n`);
    expect(root.querySelector("pre code")?.getAttribute("class")).toBeNull();
  });
});

describe("text as sent (AC-13)", () => {
  // Decomposed (NFD): e + U+0308, i + U+0306; never composed on the way.
  const decomposed = "\u0417\u0435\u043b\u0435\u0308\u043d\u044b\u0438\u0306";

  it("keeps a decomposed heading's code points", () => {
    const root = show(`## ${decomposed} {#RULE-BASIN}\n\n${decomposed}\n`);
    const heading = root.querySelector(".md-heading");
    expect(heading?.firstChild?.textContent).toBe(decomposed);
    expect(heading?.textContent.includes(decomposed.normalize("NFC"))).toBe(false);
    expect(root.querySelector("p")?.textContent).toBe(decomposed);
  });

  it("wraps a 300-character token in text, a table cell and code", () => {
    const token = "k7Qm2Xr9".repeat(38).slice(0, 300);
    const root = show(`${token}\n\n| A |\n|---|\n| ${token} |\n\n\`\`\`\n${token}\n\`\`\`\n`);
    expect(root.querySelector("p")?.textContent).toBe(token);
    expect(root.querySelector("td")?.textContent).toBe(token);
    expect(root.querySelector("pre code")?.textContent).toBe(`${token}\n`);
    const rule = (selector: string) => new RegExp(`(^|\\n|,)\\s*${selector.replaceAll(".", "\\.")}\\s*[,{][^}]*\\}`).exec(appCss)?.[0] ?? "";
    expect(rule(".markdown")).toMatch(/overflow-wrap:\s*anywhere/);
    expect(rule(".md-table td")).toMatch(/overflow-wrap:\s*anywhere/);
    expect(rule(".md-code-block")).toMatch(/white-space:\s*pre-wrap/);
    expect(rule(".md-code-block")).toMatch(/overflow-wrap:\s*anywhere/);
  });

  it("takes every colour from tokens: no literal in the renderer's styles", () => {
    const markdownRules = appCss.slice(appCss.indexOf("Rendered markdown"));
    expect(markdownRules.length).toBeGreaterThan(100);
    expect(markdownRules).not.toMatch(/#[0-9a-f]{3,8}\b|\b(rgb|rgba|hsl|hsla|oklch)\(/i);
  });
});

describe("prose under a view's heading", () => {
  it("renders inline markup and lists as elements, the text otherwise as sent", () => {
    render(<Markdown source={"A **bold** _step_ with `code`.\n\n1. First\n2. Second\n"} links={null} project="alpha" baseLevel={2} frontMatter={false} />);
    expect(screen.getByText("bold").tagName).toBe("STRONG");
    expect(screen.getByText("step").tagName).toBe("EM");
    expect(screen.getByText("code").tagName).toBe("CODE");
    expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toEqual(["First", "Second"]);
  });
});

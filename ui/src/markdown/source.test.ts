import { describe, expect, it } from "vitest";
import {
  anchorTag,
  attributeBlock,
  definitionDestination,
  inlineDestination,
  isEmptyNamedAnchor,
  isLocalDestination,
  isNamedAnchorOpen,
  lineIndex,
  splitFrontMatter,
  withoutComments,
} from "./source";

// The pure readings beside the parse (docs/features/ui-markdown.md "Description and interactions"): the
// front-matter split as the core's `front_matter.rs` `split`, the destination scan as the core's
// `markdown.rs` (AC-04, AC-10).

describe("splitFrontMatter (AC-10)", () => {
  it("splits a closed block, its lines counted, the body after its closing line", () => {
    const text = "---\nid: DOC-A\nkind: widget\n---\n\n# Title\n";
    expect(splitFrontMatter(text)).toEqual({ block: "---\nid: DOC-A\nkind: widget\n---\n", lines: 4, body: "\n# Title\n" });
  });

  it("keeps CRLF endings in the block and closes on `---\\r`", () => {
    const text = "---\r\nid: DOC-A\r\n---\r\nBody\r\n";
    expect(splitFrontMatter(text)).toEqual({ block: "---\r\nid: DOC-A\r\n---\r\n", lines: 3, body: "Body\r\n" });
  });

  it("passes a BOM before the opening line", () => {
    expect(splitFrontMatter("\uFEFF---\na: 1\n---\nB")).toEqual({ block: "---\na: 1\n---\n", lines: 3, body: "B" });
  });

  it("closes on a last line with no ending", () => {
    expect(splitFrontMatter("---\na: 1\n---")).toEqual({ block: "---\na: 1\n---", lines: 3, body: "" });
  });

  it("leaves the whole text as body when unclosed, not first, or not exactly `---`", () => {
    for (const text of ["---\nid: DOC-A\n\n# Title\n", "\n---\na: 1\n---\n", "--- \na: 1\n---\n", "----\na\n----\n", "# Title\n---\n"]) {
      expect(splitFrontMatter(text)).toEqual({ block: null, lines: 0, body: text });
    }
  });
});

describe("the destination as written (AC-04)", () => {
  function inline(source: string): string | null {
    const place = inlineDestination(source, 0, source.length, 1);
    return place === null ? null : source.slice(place.start, place.end);
  }

  it("reads the bytes between `](` and the title or the closing paren", () => {
    expect(inline("[t](docs/r9.md)")).toBe("docs/r9.md");
    expect(inline('[t](docs/r9.md "Title")')).toBe("docs/r9.md");
    expect(inline("[t](  docs/r9.md#SEC-1  )")).toBe("docs/r9.md#SEC-1");
    expect(inline("[t](docs/a(b).md)")).toBe("docs/a(b).md");
  });

  it("drops the angle brackets and keeps escapes and encodings as written", () => {
    expect(inline("[t](<docs/my file.md>)")).toBe("docs/my file.md");
    expect(inline("[t](docs/R\\-12.md)")).toBe("docs/R\\-12.md");
    expect(inline("[t](docs/a%20b.md)")).toBe("docs/a%20b.md");
    expect(inline("[t](docs/\u0434\u043e\u043a.md)")).toBe("docs/\u0434\u043e\u043a.md");
  });

  it("passes brackets in the text: the first `](` after the text's end", () => {
    const source = "[a ](b) c](docs/x.md)";
    const place = inlineDestination(source, 0, source.length, source.indexOf(" c]") + 2);
    expect(place === null ? null : source.slice(place.start, place.end)).toBe("docs/x.md");
  });

  it("reads a definition's destination after its label's `]:`, on its line or the next", () => {
    const one = "[ref]: docs/ref.md 'T'";
    const two = "[ref]:\n  <docs/ref two.md>";
    const read = (source: string) => {
      const place = definitionDestination(source, 0, source.length);
      return place === null ? null : source.slice(place.start, place.end);
    };
    expect(read(one)).toBe("docs/ref.md");
    expect(read(two)).toBe("docs/ref two.md");
    expect(read("[r\\]x]: docs/e.md")).toBe("docs/e.md");
  });

  it("tells a place in the project from a URL", () => {
    for (const local of ["docs/a.md", "../b.md#x", "#SEC-1", "a.md?x=1", "R-9"]) {
      expect([local, isLocalDestination(local)]).toEqual([local, true]);
    }
    for (const other of ["", "  ", "//host/a.md", "https://example.com", "mailto:a@b.c", "javascript:x", "data:text/html,x", "#", "?q"]) {
      expect([other, isLocalDestination(other)]).toEqual([other, false]);
    }
  });

  it("numbers lines from 1 by offset", () => {
    const line = lineIndex("a\nbb\n\nc");
    expect([0, 1, 2, 4, 5, 6].map(line)).toEqual([1, 1, 2, 2, 3, 4]);
  });
});

describe("raw HTML and heading attributes, read in one pass (n1, n2 of the review)", () => {
  it("drops comments, an unclosed one to the end; `<!-->` and `<!--->` are whole", () => {
    expect(withoutComments("a<!-- x -->b<!---->c<!-->d<!--->e")).toBe("abcde");
    expect(withoutComments("keep <!-- open")).toBe("keep ");
    expect(withoutComments("<b>no comment</b>")).toBe("<b>no comment</b>");
  });

  it("reads an a start tag as the core's a_tag: its id and name values, its length", () => {
    expect(anchorTag('<a id="x" class=\'y\' name=z>rest')).toEqual({ names: ["x", "z"], length: 27, selfClosing: false });
    expect(anchorTag('<A NAME="n"/>')).toEqual({ names: ["n"], length: 13, selfClosing: true });
    expect(anchorTag('<a href="x" / >')).toEqual({ names: [], length: 15, selfClosing: false });
    expect(anchorTag("<abbr id=x>")).toBeNull();
    expect(anchorTag("<a>")).toBeNull();
    expect(anchorTag('<a id="open')).toBeNull();
    expect(isNamedAnchorOpen(' <a id="y" class="z"> ')).toBe(true);
    expect(isNamedAnchorOpen('<a href="z">')).toBe(false);
    expect(isEmptyNamedAnchor('<a name="x"/>')).toBe(true);
    expect(isEmptyNamedAnchor('<a id="y" class="z">\n</a>')).toBe(true);
    expect(isEmptyNamedAnchor('<a id="y">text</a>')).toBe(false);
    expect(isEmptyNamedAnchor('<a id="">\n</a>')).toBe(false);
  });

  it("finds a trailing {...} block from the end", () => {
    expect(attributeBlock("W \t{#RULE-X .c}  ")).toEqual({ before: "W", inner: "#RULE-X .c" });
    expect(attributeBlock("{#ONLY}")).toEqual({ before: "", inner: "#ONLY" });
    expect(attributeBlock("W {a}b}")).toBeNull();
    expect(attributeBlock("W }")).toBeNull();
    expect(attributeBlock("W {x} y")).toBeNull();
  });

  it("stays linear on long blank runs and many unclosed comments", () => {
    const blanks = `## a${" ".repeat(100_000)}b`;
    const comments = "<!--".repeat(100_000);
    const started = performance.now();
    expect(attributeBlock(blanks)).toBeNull();
    expect(attributeBlock(`a${" ".repeat(100_000)}{#X}`)).toEqual({ before: "a", inner: "#X" });
    expect(withoutComments(comments)).toBe("");
    expect(performance.now() - started).toBeLessThan(500);
  });
});

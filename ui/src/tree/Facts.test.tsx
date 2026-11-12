import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { aNode } from "../test/builders";
import { enT } from "../test/text";
import { Facts, factsOf } from "./Facts";
import factsSource from "./Facts.tsx?raw";

// DS7 of docs/features/design-sweep.md "Acceptance criteria": one list of a holder's facts feeds both
// renderings, the Spec tree's PropertyList and the document view's `dl.facts`; Facts lists no row of
// its own.

const LABEL_KEYS = ["tree.node.kind", "tree.node.where", "tree.node.status", "tree.node.rev", "tree.node.size", "tree.node.spanHash", "tree.node.flags"];

describe("one holder's facts (DS7 of design-sweep)", () => {
  it("lists kind, place, status, rev, size, span hash and flags, null where the holder has none", () => {
    const section = aNode({ id: "SEC-A1", kind: "section", status: null, rev: null, archived: false, utf8: true });
    expect(factsOf(section, enT).map((fact) => [fact.key, fact.label, fact.value === null])).toEqual([
      ["kind", "Kind", false],
      ["where", "Where", false],
      ["status", "Status", true],
      ["rev", "Rev", true],
      ["size", "Size", false],
      ["spanHash", "Span hash", false],
      ["flags", "Flags", true],
    ]);
  });

  it("renders dl.facts from the same rows, a null one left out, the span hash wide", () => {
    const holder = aNode({ id: "DOC-A", kind: "widget", path: "docs/spec/a.md", line: 1, end_line: 40, status: "alpha-live", rev: 7, tokens_est: 321, span_hash: "b3:abc123", archived: true, utf8: false });
    const { container } = render(<Facts holder={holder} />);
    const list = container.querySelector("dl.facts.node-facts");
    expect(Array.from(list?.querySelectorAll(".fact") ?? [], (fact) => [fact.querySelector("dt")?.textContent, fact.querySelector("dd")?.textContent, fact.classList.contains("fact-wide")])).toEqual([
      ["Kind", "widget", false],
      ["Where", "docs/spec/a.md:1-40", false],
      ["Status", "alpha-live", false],
      ["Rev", "7", false],
      ["Size", "321 tokens", false],
      ["Span hash", "b3:abc123", true],
      ["Flags", "archivednot UTF-8", false],
    ]);
    const bare = render(<Facts holder={aNode({ id: "SEC-A1", status: null, rev: null, archived: false, utf8: true })} />).container;
    expect(Array.from(bare.querySelectorAll(".fact dt"), (term) => term.textContent)).toEqual(["Kind", "Where", "Size", "Span hash"]);
  });

  it("names each fact's label once, in factsOf, and Facts draws factsOf's rows", () => {
    // M: factsOf dropped from Facts, which then lists its own rows.
    expect(LABEL_KEYS.map((key) => [key, factsSource.split(`t("${key}")`).length - 1])).toEqual(LABEL_KEYS.map((key) => [key, 1]));
    const component = factsSource.slice(factsSource.indexOf("export function Facts("));
    expect(component).toContain("factsOf(holder, t)");
  });
});

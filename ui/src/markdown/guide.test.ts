import { describe, expect, it } from "vitest";
import { aCited } from "../test/builders";
import { citedEntries, codeSpans } from "./cited";
import { guideCitations, linkRuns } from "./guide";
import tableText from "./guide-citations.json?raw";

// docs/features/specengine-guide.md "The UI", GU14: a `guide:<topic>[#<ID>]` citation links only when its
// topic is one the served index holds, standing alone and outside code; the rule is read as a Latin ID
// after the `#`. M: linking an unknown topic (any `guide:<word>`) turns "links only a served topic" red.

const TOPICS = ["start", "records", "cards", "documents"];

/** What each citation found opens, in order. */
function opened(text: string, topics: readonly string[] = TOPICS, code: readonly (readonly [number, number])[] = []): string[] {
  return guideCitations(text, topics, code).map((found) => `${text.slice(found.start, found.end)} -> ${found.topic}${found.rule === null ? "" : `#${found.rule}`}`);
}

describe("guide citations (GU14 of specengine-guide)", () => {
  it("links only a served topic: an unknown one stays text, whatever it looks like (M: linking an unknown topic)", () => {
    expect(opened("See guide:records#GD-RECORDS-ONE-FILE and guide:nope#GD-NOPE-X, guide:charts too.")).toEqual(["guide:records#GD-RECORDS-ONE-FILE -> records#GD-RECORDS-ONE-FILE"]);
    expect(opened("guide:records", [])).toEqual([]);
  });

  it("reads a topic alone, or with its rule, and leaves the sentence's own punctuation outside", () => {
    expect(opened("Read guide:start.")).toEqual(["guide:start -> start"]);
    expect(opened("(guide:cards#GD-CARDS-HANDOFF), then guide:documents#GD-DOCUMENTS-CITE-THE-GUIDE.")).toEqual([
      "guide:cards#GD-CARDS-HANDOFF -> cards#GD-CARDS-HANDOFF",
      "guide:documents#GD-DOCUMENTS-CITE-THE-GUIDE -> documents#GD-DOCUMENTS-CITE-THE-GUIDE",
    ]);
    // A trailing `-` is never a rule's: it stays out, and the citation no longer stands alone.
    expect(opened("guide:cards#GD-CARDS- next")).toEqual([]);
    // A `#` with no rule after it: the topic alone.
    expect(opened("guide:cards# and guide:cards#")).toEqual(["guide:cards -> cards", "guide:cards -> cards"]);
    expect(opened("[[guide:records#GD-RECORDS-ONE-FILE]]")).toEqual(["guide:records#GD-RECORDS-ONE-FILE -> records#GD-RECORDS-ONE-FILE"]);
  });

  it("links a citation standing alone only: nothing of a word or an ID runs on into it from either side", () => {
    expect(opened("myguide:start xguide:records")).toEqual([]);
    expect(opened("guide:starts guide:start_up guide:start-up")).toEqual([]);
    expect(opened("slug/guide:start")).toEqual([]);
    // A letter of another script, a rule's next character: no link at all (\u0416 is a Cyrillic letter).
    expect(opened("guide:cards#GD-X\u0416 guide:cards#\u0416")).toEqual([]);
    // The longest served topic that stands alone wins.
    expect(opened("guide:start-here guide:start", ["start", "start-here"])).toEqual(["guide:start-here -> start-here", "guide:start -> start"]);
  });

  it("leaves a citation inside a code span as text", () => {
    const text = "Cite `guide:records#GD-RECORDS-ONE-FILE`, then guide:records.";
    expect(opened(text, TOPICS, codeSpans(text))).toEqual(["guide:records -> records"]);
  });

  it("cuts a text into runs: the guide's citations, the served IDs between them, the rest text; joined, the runs are the text", () => {
    const text = "Per R-12 and guide:cards#GD-CARDS-HANDOFF; `R-12` and guide:nope stay.";
    const runs = linkRuns(text, citedEntries([aCited("R-12", { name: "DOC/R-12" })]), TOPICS, codeSpans(text));
    expect(runs.map((run) => run.text).join("")).toBe(text);
    expect(runs.filter((run) => run.name !== null || run.guide !== null).map((run) => [run.text, run.name, run.guide])).toEqual([
      ["R-12", "DOC/R-12", null],
      ["guide:cards#GD-CARDS-HANDOFF", null, { topic: "cards", rule: "GD-CARDS-HANDOFF" }],
    ]);
    expect(linkRuns("", [], TOPICS)).toEqual([{ text: "", name: null, guide: null }]);
    expect(linkRuns("plain", [], [])).toEqual([{ text: "plain", name: null, guide: null }]);
  });

  it("asks `accept` of each citation: a decoded escape is never one", () => {
    expect(guideCitations("guide:start guide:cards", TOPICS, [], (start) => start > 0).map((found) => found.topic)).toEqual(["cards"]);
  });
});

// docs/features/release-tail.md RT10: the left boundary before `guide` is the grammar's rule (5), the
// one rule (7) holds (crates/specengine-model `grammar.rs`): a letter or digit of any script, `_ - . /
// :` before it means no citation; `#`, `@` or `(` leaves one. One table of odd texts, read here and by
// the model's test (a Rust test compares its copy with this file byte for byte): `refs` are the IDs
// the daemon keeps under `[ids]` R and S, `guide_link` whether the UI links `guide:x#R-1`. Where the
// daemon keeps R-1 a reference the UI links no guide; where it reads a citation standing alone on
// its right, the UI links it. M: the old left boundary in guide.ts (`standsAlone`'s, which stops on
// `# @` and not on `. :`) turns "after # or @ ... links" and "after . or : ... links none" red.

/** One row of the shared table. */
interface TableRow {
  text: string;
  refs: string[];
  guide_link: boolean;
}

/** The shared table, its shape checked: exactly `{text, refs, guide_link}` per row. */
function table(): TableRow[] {
  const parsed: unknown = JSON.parse(tableText);
  if (!Array.isArray(parsed)) {
    throw new Error("guide-citations.json: not a list");
  }
  return parsed.map((row: unknown) => {
    if (typeof row !== "object" || row === null || Object.keys(row).join() !== "text,refs,guide_link") {
      throw new Error(`guide-citations.json: not {text, refs, guide_link}: ${JSON.stringify(row)}`);
    }
    const { text, refs, guide_link: link } = row as Record<string, unknown>;
    if (typeof text !== "string" || !Array.isArray(refs) || !refs.every((ref) => typeof ref === "string") || typeof link !== "boolean") {
      throw new Error(`guide-citations.json: a field of the wrong type: ${JSON.stringify(row)}`);
    }
    return { text, refs, guide_link: link };
  });
}

/** What the UI links of a text as a guide citation, topic `x` served. */
function linked(text: string): string[] {
  return guideCitations(text, ["x"]).map((found) => text.slice(found.start, found.end));
}

describe("the guide's left boundary, the grammar's (RT10 of release-tail)", () => {
  const rows = table();
  const byText = new Map(rows.map((row) => [row.text, row]));

  it("covers #, @, ., :, /, - and ( before guide, and the RT9 forms, each text once", () => {
    expect(byText.size).toBe(rows.length);
    for (const before of ["#", "@", ".", ":", "/", "-", "("]) {
      expect([before, rows.some((row) => row.text.startsWith(`${before}guide:x#R-1`))]).toEqual([before, true]);
    }
    for (const form of ["guide:x#R-1", "guide:x#R-1#S-2", "guide:x#R-1#S-2@3", "guide:x#R-1 S-2", "R-1#S-2"]) {
      expect([form, byText.has(form)]).toEqual([form, true]);
    }
  });

  it.each(table().map((row) => [row.text, row] as const))("%s: the UI links the guide exactly as the table says", (_text, row) => {
    expect(linked(row.text)).toEqual(row.guide_link ? ["guide:x#R-1"] : []);
  });

  it("agrees with the daemon on every row: R-1 kept a reference is never a guide link, a guide link never leaves R-1 a reference", () => {
    for (const row of rows) {
      expect([row.text, row.refs.includes("R-1") && row.guide_link]).toEqual([row.text, false]);
    }
    expect(rows.filter((row) => row.refs.includes("R-1")).length).toBeGreaterThan(0);
    expect(rows.filter((row) => row.guide_link).length).toBeGreaterThan(0);
  });

  it("after # or @ a citation links (M: the old boundary); after . or : none does (M: the old boundary)", () => {
    expect(linked("#guide:x#R-1 and @guide:x")).toEqual(["guide:x#R-1", "guide:x"]);
    expect(linked(".guide:x#R-1 :guide:x")).toEqual([]);
    // A letter of another script, or a letter or digit of a surrogate pair (U+1D400, U+1D7CF), runs on as a Latin one does.
    expect(linked("\u0436guide:x \ud835\udc00guide:x \ud835\udfcfguide:x")).toEqual([]);
    // A symbol outside the planes' letters is no letter: the citation after it stands (U+1F600).
    expect(linked("\ud83d\ude00guide:x")).toEqual(["guide:x"]);
  });
});

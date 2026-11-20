import { describe, expect, it } from "vitest";
import { aCited } from "../test/builders";
import { citedEntries, codeSpans } from "./cited";
import { guideCitations, linkRuns } from "./guide";

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

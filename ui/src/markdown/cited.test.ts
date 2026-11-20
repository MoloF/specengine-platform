import { describe, expect, it } from "vitest";
import type { Cited } from "../api/types";
import { citedRuns, codeSpans, verbatimIn, verbatimPlaces, type CitedRun } from "./cited";

// docs/features/department-practices.md "Acceptance criteria", DP21 and "The UI": the link kind
// `cited`. A text's runs link only the strings the daemon served, outside code; an ID-shaped token it
// did not serve stays text (the spec's mutation: linking any ID-shaped token turns this red).

function cite(written: string, name: string = written): Cited {
  return { written, name, title: null, path: "docs/practices/art.md" };
}

/** The runs as `[text, name]`, a plain run's name null. */
function shown(runs: readonly CitedRun[]): [string, string | null][] {
  return runs.map((run) => [run.text, run.name]);
}

describe("the cited runs of a text (DP21)", () => {
  it("links each served string and nothing else: an unserved ID-shaped token stays text", () => {
    const runs = citedRuns("Keep PRA-ART-LOD; PRA-NOPE-X is no rule, and PRA-ART-LOD again.", [cite("PRA-ART-LOD", "art/PRA-ART-LOD")]);
    expect(shown(runs)).toEqual([
      ["Keep ", null],
      ["PRA-ART-LOD", "art/PRA-ART-LOD"],
      ["; PRA-NOPE-X is no rule, and ", null],
      ["PRA-ART-LOD", "art/PRA-ART-LOD"],
      [" again.", null],
    ]);
    // Joined, the runs are the text, byte for byte.
    expect(runs.map((run) => run.text).join("")).toBe("Keep PRA-ART-LOD; PRA-NOPE-X is no rule, and PRA-ART-LOD again.");
  });

  it("links nothing with no served list, nor in an empty text", () => {
    expect(shown(citedRuns("PRA-ART-LOD and R-12", []))).toEqual([["PRA-ART-LOD and R-12", null]]);
    expect(shown(citedRuns("", [cite("R-12")]))).toEqual([["", null]]);
    // An empty `written` never matches.
    expect(shown(citedRuns("R-12", [cite("")]))).toEqual([["R-12", null]]);
  });

  it("never links inside a code span, but links the same string outside it", () => {
    const text = "Use `PRA-ART-LOD` as PRA-ART-LOD says, ``a `PRA-ART-LOD` b`` too.";
    const runs = citedRuns(text, [cite("PRA-ART-LOD")], codeSpans(text));
    expect(runs.filter((run) => run.name !== null)).toHaveLength(1);
    expect(shown(runs)).toEqual([
      ["Use `PRA-ART-LOD` as ", null],
      ["PRA-ART-LOD", "PRA-ART-LOD"],
      [" says, ``a `PRA-ART-LOD` b`` too.", null],
    ]);
  });

  it("reads code spans as CommonMark does: a run to the next run of as many backticks; an unmatched run is text", () => {
    expect(codeSpans("a `b` c")).toEqual([[2, 5]]);
    expect(codeSpans("``a ` b`` c `d")).toEqual([[0, 9]]);
    expect(codeSpans("no code ` here")).toEqual([]);
    const text = "open ` PRA-ART-LOD";
    expect(shown(citedRuns(text, [cite("PRA-ART-LOD")], codeSpans(text)))).toEqual([
      ["open ` ", null],
      ["PRA-ART-LOD", "PRA-ART-LOD"],
    ]);
  });

  it("links an occurrence standing alone only: never part of a longer ID, another project's, a section's or a revision's", () => {
    const served = [cite("PRA-ART")];
    for (const text of ["PRA-ART-LOD", "XPRA-ART", "PRA-ART_2", "slug/PRA-ART", "DOC#PRA-ART", "PRA-ART#X", "PRA-ART@3", "PRA-ART/x", "\u0416PRA-ART", "PRA-ART\u0416"]) {
      expect([text, shown(citedRuns(text, served))]).toEqual([text, [[text, null]]]);
    }
    for (const text of ["(PRA-ART)", "PRA-ART.", "PRA-ART, then", "\u00abPRA-ART\u00bb", "PRA-ART#", "PRA-ART\u2014x"]) {
      expect([text, shown(citedRuns(text, served)).filter(([, name]) => name !== null)]).toEqual([text, [["PRA-ART", "PRA-ART"]]]);
    }
  });

  it("takes the longest served string at a place, each string's first name", () => {
    const runs = citedRuns("See DOC#PRA-ART here.", [cite("PRA-ART"), cite("DOC#PRA-ART", "DOC#PRA-ART"), cite("PRA-ART", "other")]);
    expect(shown(runs)).toEqual([
      ["See ", null],
      ["DOC#PRA-ART", "DOC#PRA-ART"],
      [" here.", null],
    ]);
    expect(shown(citedRuns("PRA-ART", [cite("PRA-ART", "first"), cite("PRA-ART", "second")]))).toEqual([["PRA-ART", "first"]]);
  });

  it("keeps any script around a link: Cyrillic and CJK prose, a surrogate pair before an ID", () => {
    const text = "\u041f\u0440\u0430\u0432\u0438\u043b\u043e PRA-ART \u2014 \u6f6e \ud83c\udf0a PRA-ART";
    expect(shown(citedRuns(text, [cite("PRA-ART")]))).toEqual([
      ["\u041f\u0440\u0430\u0432\u0438\u043b\u043e ", null],
      ["PRA-ART", "PRA-ART"],
      [" \u2014 \u6f6e \ud83c\udf0a ", null],
      ["PRA-ART", "PRA-ART"],
    ]);
  });
});

describe("a decoded markdown text's verbatim places (DP21: each verbatim occurrence)", () => {
  /** Whether `written` at its first place in `value` stands in `raw` as written. */
  function verbatim(raw: string, value: string, written: string): boolean {
    const at = value.indexOf(written);
    return verbatimIn(verbatimPlaces(raw, value))(at, at + written.length);
  }

  it("reads an escape or a character reference as no verbatim occurrence", () => {
    expect(verbatim("R\\-12", "R-12", "R-12")).toBe(false);
    expect(verbatim("R&#45;12", "R-12", "R-12")).toBe(false);
    expect(verbatim("R&#x2d;12", "R-12", "R-12")).toBe(false);
    expect(verbatim("R&hyphen;12", "R\u201012", "R\u201012")).toBe(false);
    // The same string written plainly beside an escaped one is verbatim.
    const raw = "R\\-12 or R-12";
    const value = "R-12 or R-12";
    const places = verbatimIn(verbatimPlaces(raw, value));
    expect([places(0, 4), places(8, 12)]).toEqual([false, true]);
  });

  it("passes over what the text drops: a continuation line's indent, a block quote's marker, an escaped backslash", () => {
    expect(verbatim("a\n   R-12", "a\nR-12", "R-12")).toBe(true);
    expect(verbatim("a\n> R-12", "a\nR-12", "R-12")).toBe(true);
    expect(verbatim("\\\\ R-12", "\\ R-12", "R-12")).toBe(true);
    // An unknown named reference stays as written, and so does what follows it.
    expect(verbatim("&nope; R-12", "&nope; R-12", "R-12")).toBe(true);
  });

  it("links nothing in a text it cannot align with its source", () => {
    expect(verbatimPlaces("abc", "abd")).toBeNull();
    expect(verbatimIn(null)(0, 1)).toBe(false);
  });

  it("drops an unaccepted match from the runs, the rest of the text unchanged", () => {
    const runs = citedRuns("R-12 or R-12", [cite("R-12")], [], verbatimIn(verbatimPlaces("R\\-12 or R-12", "R-12 or R-12")));
    expect(shown(runs)).toEqual([
      ["R-12 or ", null],
      ["R-12", "R-12"],
    ]);
  });
});

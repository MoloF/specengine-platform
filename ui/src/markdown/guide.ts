import { GUIDE_PREFIX, type GuideRef } from "../app/routes";
import { runsOf, standsAlone, type CitedEntries } from "./cited";

// Citations of SpecEngine's own guide (docs/features/specengine-guide.md "The UI"; ADR-0085): in a text,
// `guide:<topic>` or `guide:<topic>#<ID>` whose topic is one of the guide's, as the daemon's build
// serves its index, standing alone and outside code, links to that topic's page, at the rule when one
// is named. Nothing else becomes one: a topic the served index does not hold stays text, whatever it
// looks like, so a citation of a page this build lacks never links to nothing. The rule is not checked
// against the page here (the index names no rules); the page says when it has no such rule. Shared by
// the plain texts (`CitedText`) and the markdown renderer (`./plugin`), with the served `cited` IDs
// (`./cited`) matched in what is left between citations.

/** A rule ID after the topic's `#`: Latin letters, digits, `_` and `-`, a letter or digit first (ADR-0009). */
const RULE = /^[A-Za-z0-9][A-Za-z0-9_-]*/;

/** The longest a rule ID read here may be: past it the token is cut, and stands alone no more. */
const RULE_MAX = 128;

/** A citation found in a text: where it stands and what it opens. */
export interface GuideCitation extends GuideRef {
  start: number;
  end: number;
}

/** A run of a text: as written, a served ID's document link (`name`), or a guide citation (`guide`). */
export interface LinkRun {
  text: string;
  /** The document view's REF; null for plain text and a guide citation. */
  name: string | null;
  /** The guide's topic and rule; null for plain text and an ID's link. */
  guide: GuideRef | null;
}

/** The known topics, each once, longest first, an empty one dropped. */
function longestFirst(topics: readonly string[]): string[] {
  return [...new Set(topics)].filter((topic) => topic !== "").sort((a, b) => b.length - a.length);
}

/** The citation starting at `at` (its `guide:`), the longest known topic that stands alone, with its rule when one follows; else null. */
function citationAt(text: string, at: number, topics: readonly string[]): GuideCitation | null {
  const from = at + GUIDE_PREFIX.length;
  for (const topic of topics) {
    if (!text.startsWith(topic, from)) {
      continue;
    }
    const end = from + topic.length;
    const rule = text[end] === "#" ? RULE.exec(text.slice(end + 1, end + 1 + RULE_MAX))?.[0].replace(/[-_]+$/, "") : undefined;
    if (rule !== undefined && rule !== "") {
      const ruleEnd = end + 1 + rule.length;
      if (standsAlone(text, at, ruleEnd)) {
        return { start: at, end: ruleEnd, topic, rule };
      }
      continue;
    }
    if (standsAlone(text, at, end)) {
      return { start: at, end, topic, rule: null };
    }
  }
  return null;
}

/**
 * The guide citations of a text, in order: each `guide:` whose topic is one of `topics`, standing
 * alone, outside `code` (sorted `[start, end)` spans) and, given `accept`, one it accepts.
 */
export function guideCitations(text: string, topics: readonly string[], code: readonly (readonly [number, number])[] = [], accept?: (start: number, end: number) => boolean): GuideCitation[] {
  const known = longestFirst(topics);
  const found: GuideCitation[] = [];
  if (known.length === 0) {
    return found;
  }
  let span = 0;
  for (let at = text.indexOf(GUIDE_PREFIX); at >= 0; at = text.indexOf(GUIDE_PREFIX, at + 1)) {
    let inside = code[span];
    while (inside !== undefined && inside[1] <= at) {
      span += 1;
      inside = code[span];
    }
    if (inside !== undefined && at >= inside[0]) {
      continue;
    }
    const citation = citationAt(text, at, known);
    if (citation === null || (inside !== undefined && citation.end > inside[0]) || (accept !== undefined && !accept(citation.start, citation.end))) {
      continue;
    }
    found.push(citation);
    at = citation.end - 1;
  }
  return found;
}

/**
 * A text cut into runs: its guide citations (`guideCitations`), and between them each served `cited`
 * string's link (`runsOf`), everything else text, in order; joined, the runs are the text.
 */
export function linkRuns(text: string, entries: CitedEntries, topics: readonly string[], code: readonly (readonly [number, number])[] = [], accept?: (start: number, end: number) => boolean): LinkRun[] {
  const runs: LinkRun[] = [];
  let plain = 0;
  const between = (end: number) => {
    if (end <= plain) {
      return;
    }
    const offset = plain;
    const shifted = code.filter(([start, stop]) => stop > offset && start < end).map(([start, stop]) => [Math.max(start, offset) - offset, Math.min(stop, end) - offset] as const);
    const kept = accept === undefined ? undefined : (start: number, stop: number) => accept(start + offset, stop + offset);
    for (const run of runsOf(text.slice(offset, end), entries, shifted, kept)) {
      runs.push({ text: run.text, name: run.name, guide: null });
    }
  };
  for (const citation of guideCitations(text, topics, code, accept)) {
    between(citation.start);
    runs.push({ text: text.slice(citation.start, citation.end), name: null, guide: { topic: citation.topic, rule: citation.rule } });
    plain = citation.end;
  }
  between(text.length);
  if (runs.length === 0) {
    runs.push({ text, name: null, guide: null });
  }
  return runs;
}

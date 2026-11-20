import type { Card, CardNotFound, CardPractice, Cited, Proposal } from "../api/types";
import type { MockCorpus } from "./build";
import { nameOf, resolveRef } from "./corpus";
import { libraryOf } from "./library";
import type { Scenario } from "./scenario";

// A card's practices and the IDs its texts cite, as the daemon's `GET .../cards/:id?practices=true`
// serves them, and a review's cited IDs, as `GET .../proposals/:id` does
// (docs/features/department-practices.md "Acceptance criteria", DP9, DP14, DP13):
// the practices are the library's live rows whose `disciplines` name the card's discipline, by path,
// at most 32, the rest one note; a cited ID is an ID-shaped run the corpus resolves, to its first
// holder, first-seen order, at most 64, an unresolved one left out. `practices-unread`: the read fails,
// both null and the note says why. Nothing is cached: every read computes anew.

/** The practice documents a card lists (`PRACTICES_MAX`). */
const PRACTICES_MAX = 32;

/** The resolved citations a card or a proposal lists (`CITED_MAX`). */
const CITED_MAX = 64;

/** A discipline as a flow names one: `[a-z0-9_-]`, 1 to 64 bytes. */
const DISCIPLINE = /^[a-z0-9_-]{1,64}$/;

/** An ID-shaped run, as the mock's corpora write IDs: a Latin capital, then hyphenated runs of capitals and digits. */
const ID_SHAPED = /(?<![\p{L}\p{N}_\-/#@])[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+(?![\p{L}\p{N}_])/gu;

/** Why the `practices-unread` scenario's read fails: the index's words. */
const UNREAD_CAUSE = "the index at /tmp/specengine/harbor-sim/index.sqlite could not be opened: database is locked";

/** The IDs `texts` cite, each resolved by the corpus to its first holder, in first-seen order, at most 64; one unresolved is left out. */
export function citedIn(corpus: MockCorpus, texts: readonly (string | null)[]): Cited[] {
  const seen = new Set<string>();
  const cited: Cited[] = [];
  for (const text of texts) {
    for (const match of (text ?? "").matchAll(ID_SHAPED)) {
      const written = match[0];
      if (cited.length >= CITED_MAX || seen.has(written)) {
        continue;
      }
      seen.add(written);
      const resolved = resolveRef(corpus, written);
      const holder = "holders" in resolved ? resolved.holders[0] : undefined;
      if (holder !== undefined) {
        cited.push({ written, name: nameOf(holder), title: holder.title, path: holder.path });
      }
    }
  }
  return cited;
}

/** A practice's rules: its document's direct child ID sections, else 1, the document itself. */
function rulesOf(corpus: MockCorpus, path: string): number {
  const file = corpus.documents.find((candidate) => candidate.path === path);
  const rules = file?.sections.filter((section) => section.within === null).length ?? 0;
  return rules === 0 ? 1 : rules;
}

/** The practices of one discipline, by path, at most 32, and the note past them; a card with none, or one off the grammar, gets none. */
export function practicesOf(corpus: MockCorpus, slug: string, scenario: Scenario, discipline: string | null): { practices: CardPractice[]; note: string | null } {
  if (discipline === null || !DISCIPLINE.test(discipline)) {
    return { practices: [], note: null };
  }
  const rows = libraryOf(corpus, slug, scenario, false)
    .rows.filter((row) => !row.archived && row.class !== "generated" && row.disciplines.includes(discipline))
    .sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  const practices = rows.slice(0, PRACTICES_MAX).map((row) => ({ name: row.name, title: row.title, path: row.path, disciplines: [...row.disciplines], rules: rulesOf(corpus, row.path) }));
  const first = rows[PRACTICES_MAX];
  const note = first === undefined ? null : `practices: ${String(rows.length - PRACTICES_MAX)} more practice documents of ${discipline}, from ${first.path}`;
  return { practices, note };
}

/** The texts a card's cited scan reads: title, goal, criteria, each item's served text, each history note. */
function cardTexts(card: Card): (string | null)[] {
  const items = card.items.flatMap((item) => [item.summary, item.gist, item.matters, item.working_answer, item.why, item.decision_note]);
  const notes = (card.history ?? []).map((event) => {
    const note = typeof event.payload === "object" && event.payload !== null ? (event.payload as Record<string, unknown>).note : null;
    return typeof note === "string" ? note : null;
  });
  return [card.title, card.goal, ...card.criteria, ...items, ...notes];
}

/** A card read with `?practices=true`: its practices and cited IDs filled, or, under `practices-unread`, both null with the note. */
export function withPractices(answer: Card | CardNotFound, corpus: MockCorpus, slug: string, scenario: Scenario): Card | CardNotFound {
  if (!("title" in answer)) {
    return answer;
  }
  if (scenario === "practices-unread") {
    return { ...answer, practices: null, cited: null, notes: [...answer.notes, `practices: not read: ${UNREAD_CAUSE}`] };
  }
  const { practices, note } = practicesOf(corpus, slug, scenario, answer.discipline);
  return { ...answer, practices, cited: citedIn(corpus, cardTexts(answer)), notes: note === null ? answer.notes : [...answer.notes, note] };
}

/**
 * A review as `proposals/:id` serves it: the IDs its rationale, summary, gist, why it matters, working
 * answer, why, option labels and staged or recorded answer cite; the exit-1 document keeps null, and
 * under `practices-unread` null with the note `cited unknown: <cause>`.
 */
export function withCited(review: Proposal, corpus: MockCorpus, scenario: Scenario): Proposal {
  if (review.id === null) {
    return review;
  }
  if (scenario === "practices-unread") {
    return { ...review, cited: null, notes: [...review.notes, `cited unknown: ${UNREAD_CAUSE}`] };
  }
  const answer = review.choice !== null && "answer" in review.choice ? review.choice.answer : null;
  const texts = [review.rationale, review.summary, review.gist, review.matters, review.working_answer, review.why, ...review.options.map((option) => option.label), answer];
  return { ...review, cited: citedIn(corpus, texts) };
}

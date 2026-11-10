import type { CardFlow } from "../api/types";
import type { Translate } from "../i18n";
import type { LozengeLook } from "../ui/Lozenge";
import { categoryMark, columnTitle, notInTheFlow } from "./labels";

// A card's status category (docs/features/design-foundation.md "Status categories"): its column's
// place in its served flow, looked up and never judged by a word, so no column, flow or event word is
// spelled here (`policy.test.ts`). The first rule that matches wins: a flow not served, or a column it
// does not list, is stranded; `accept.to` is done; the flow's `return` is returned; its first column
// is to do; any other is in progress. Cancellation leaves the category as it is: the Cancelled badge
// says it.

/** The five categories. */
const CATEGORIES = { todo: true, doing: true, done: true, returned: true, stranded: true };

export type StatusCategory = keyof typeof CATEGORIES;

/** Each category by itself, so none is spelled twice (a word a task state shares is never quoted in src/board/). */
const CATEGORY = Object.fromEntries(Object.keys(CATEGORIES).map((word) => [word, word])) as { readonly [K in StatusCategory]: K };

export function categoryOf(flow: CardFlow | undefined, column: string): StatusCategory {
  if (flow === undefined || !flow.columns.some((candidate) => candidate.id === column)) {
    return CATEGORY.stranded;
  }
  if (column === flow.accept.to) {
    return CATEGORY.done;
  }
  if (column === flow.return) {
    return CATEGORY.returned;
  }
  if (column === flow.columns[0]?.id) {
    return CATEGORY.todo;
  }
  return CATEGORY.doing;
}

/** A column's lozenge: its category's appearance and icon, its served title, the raw ID when stranded. */
export function statusLook(flow: CardFlow | undefined, column: string): LozengeLook {
  return { ...categoryMark(categoryOf(flow, column)), label: columnTitle(flow, column) };
}

/** The board's "Not in the flow" head: stranded's look, its words. */
export function outsideLook(t: Translate): LozengeLook {
  return { ...categoryMark(CATEGORY.stranded), label: notInTheFlow(t) };
}

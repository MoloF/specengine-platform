import type { StoredReview } from "./build";

// The lines `spec approve`'s first checks refuse a deciding item's flags with, judged by its fields
// (docs/features/plain-questions.md "Approve, stage, form, round"), shared by the decision route and the
// round's answers. Each route frames them as the daemon serves them: the decision route a 400 with the
// CLI's `spec: ` and `; nothing changed`, a round answer's reason with neither.

/** `--option` on an item holding no options: a question's without them, a decision's. */
export function optionOnNone(review: Pick<StoredReview, "id" | "kind">): string {
  const article = /^[aeiou]/.test(review.kind) ? "an" : "a";
  return `\`--option\` names an option, and \`${review.id}\` is ${article} ${review.kind} with none: its working answer, or \`--answer T\`, answers it`;
}

/** `--option` with `--answer` on an item taking either: a question with options and a price of another answer. */
export function optionAndAnswer(id: string): string {
  return `\`--option\` and \`--answer\` each name the owner's choice, and \`${id}\` takes one: give either`;
}

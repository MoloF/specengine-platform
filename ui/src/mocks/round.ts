import { ClientError } from "../api/client";
import type {
  Author,
  RoundAnswerBody,
  RoundAnswerEntry,
  RoundAnswersOutcome,
  RoundItem,
  RoundSheet,
  RoundTarget,
  RoundTask,
  Stage,
  StageBody,
} from "../api/types";
import type { MockProject, StoredReview } from "./build";
import { resolveRef } from "./corpus";
import { packageOf, type MockTasks } from "./tasks";

// The question round over a mock project (`docs/features/ui-round.md` "Data", `docs/canon/round.md`
// "Selection", "Order", "JSON"): the sheet as `spec round new --json` gives it, and the answers'
// POST as the daemon stages them, each answer refused alone in words like the CLI's. The mock's
// own vocabulary: the three deciding kinds and the three change kinds are named here, in src/mocks/ only.

/** The kinds a round lists (their kind `decides()`); a decision as a question (`docs/features/kind-decision.md`). */
const DECIDING = new Set(["question", "discrepancy", "decision"]);

/** The change proposals a round notes, never lists. */
const CHANGES = new Set(["update", "create", "interpretation"]);

/** A task state that no longer waits on anything. */
const SETTLED_TASK = new Set(["done", "cancelled"]);

const SEVERITY_RANK = new Map([
  ["high", 0],
  ["normal", 1],
  ["low", 2],
]);

/** `--limit`'s default and its most. */
export const ROUND_LIMIT_DEFAULT = 20;
export const ROUND_LIMIT_MAX = 200;

/** The most answers one POST takes. */
const ANSWERS_MAX = 200;

/** The caps of a stage's texts, in UTF-8 bytes (`docs/canon/decision-staging.md` "The stage"). */
const ANSWER_MAX_BYTES = 2048;
const REASON_MAX_BYTES = 4096;

/** IDs named in a note or a duplicate refusal, then `and <k> more`. */
const NAMED_MAX = 10;

/** A character a stage never carries: C0 but LF and TAB, DEL, C1, the bidirectional controls. */
const ESCAPED = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u;

const byNumber = new Intl.Collator("en", { numeric: true });

function bytesOf(text: string): number {
  return new TextEncoder().encode(text).length;
}

/** `a, b and c`, at most ten named, then `and <k> more`. */
function named(values: readonly string[]): string {
  if (values.length <= 1) {
    return values.join("");
  }
  if (values.length > NAMED_MAX) {
    return `${values.slice(0, NAMED_MAX).join(", ")} and ${String(values.length - NAMED_MAX)} more`;
  }
  return `${values.slice(0, -1).join(", ")} and ${values.at(-1) ?? ""}`;
}

function isPath(target: string): boolean {
  return target.includes("/") || target.endsWith(".md");
}

/** A stored target resolved in the corpus now: its holder's path and title, or nothing when gone. */
function targetOf(project: MockProject, review: StoredReview, stored: string): RoundTarget {
  const resolved = resolveRef(project.corpus, stored);
  const holder = "holders" in resolved ? resolved.holders[0] : undefined;
  if (holder !== undefined) {
    return { id: isPath(stored) ? null : stored, path: holder.path, title: holder.title };
  }
  const path = isPath(stored) ? stored : stored === review.target_id ? review.target_path : null;
  return { id: isPath(stored) ? null : stored, path, title: null };
}

/** The tasks waiting on a proposal: not done or cancelled, their package listing it; by ID. */
function waitingOn(project: MockProject, tasks: MockTasks, id: string): RoundTask[] {
  return tasks.tasks
    .filter((task) => !SETTLED_TASK.has(task.status))
    .filter((task) => packageOf(task, project.proposals).open_proposals.some((listed) => listed.id === id))
    .sort((a, b) => byNumber.compare(a.id, b.id))
    .map((task) => ({ id: task.id, status: task.status, title: task.title }));
}

const NO_AUTHOR: Author = { type: "human", role: null, model: null, run: null };

function itemOf(project: MockProject, tasks: MockTasks, review: StoredReview): RoundItem {
  return {
    id: review.id,
    kind: review.kind,
    severity: review.severity,
    gap_type: review.gap_type,
    summary: review.summary ?? "",
    working_answer: review.working_answer,
    price_of_other: review.price_of_other,
    options: review.options,
    recommendation: review.recommendation,
    evidence: review.evidence,
    targets: review.target_ids.map((stored) => targetOf(project, review, stored)),
    tasks: waitingOn(project, tasks, review.id),
    worktree: review.worktree ?? project.project.root,
    branch: review.branch,
    author: review.author ?? NO_AUTHOR,
    staged: review.staged,
    staged_at: review.staged_at,
    created_at: review.created_at,
    updated_at: review.updated_at,
  };
}

/** Round order: severity high, normal, low, then none; most waiting tasks first; oldest first; the ID's number. */
function roundOrder(a: RoundItem, b: RoundItem): number {
  const rank = (item: RoundItem) => SEVERITY_RANK.get(item.severity ?? "") ?? SEVERITY_RANK.size;
  return rank(a) - rank(b) || b.tasks.length - a.tasks.length || a.created_at.localeCompare(b.created_at) || byNumber.compare(a.id, b.id);
}

/** About how many characters the sheet gives an item: its texts, options, targets and tasks. */
function itemChars(item: RoundItem): number {
  const texts = [item.id, item.kind, item.summary, item.working_answer ?? "", item.price_of_other ?? ""];
  const options = item.options.map((option) => `  - [0] ${option.label} | ${option.effect} | ${option.price}`);
  const targets = item.targets.map((target) => `${target.title ?? ""} (${target.id ?? target.path ?? ""}); `);
  const waiting = item.tasks.map((task) => `${task.title ?? ""} (${task.id}); `);
  return [...texts, ...options, ...targets, ...waiting].reduce((sum, text) => sum + Array.from(text).length, 40);
}

/** `spec round new [--limit N] --json` over a mock project; an out-of-range limit as the daemon's 400. */
export function roundSheetOf(project: MockProject, tasks: MockTasks, limit: number | undefined): RoundSheet {
  const wanted = limit ?? ROUND_LIMIT_DEFAULT;
  if (!Number.isInteger(wanted) || wanted < 1 || wanted > ROUND_LIMIT_MAX) {
    throw new ClientError({ status: 400, message: `--limit ${String(wanted)}: the limit is 1 to ${String(ROUND_LIMIT_MAX)}` });
  }
  const reviews = project.proposals.map(({ review }) => review);
  const listed = reviews
    .filter((review) => review.status === "open" && DECIDING.has(review.kind))
    .map((review) => itemOf(project, tasks, review))
    .sort(roundOrder);
  const items = listed.slice(0, wanted);
  const notes: string[] = [];
  const more = listed.length - items.length;
  if (more > 0) {
    notes.push(`${String(more)} more item(s) not on this sheet: spec round new --limit ${String(Math.min(listed.length, ROUND_LIMIT_MAX))}`);
  }
  const held = reviews.filter((review) => review.status === "approved" && DECIDING.has(review.kind)).map((review) => review.id);
  if (held.length > 0) {
    notes.push(
      `${String(held.length)} held item(s) not on this sheet (approved: an apply in progress or stopped): ${named(held)}; spec approve <ID> completes each`,
    );
  }
  const changes = reviews.filter((review) => review.status === "open" && CHANGES.has(review.kind)).map((review) => review.id);
  if (changes.length > 0) {
    notes.push(`${String(changes.length)} open change proposal(s) not on this sheet: ${named(changes)}; spec inbox lists them`);
  }
  const characters = items.reduce((sum, item) => sum + itemChars(item), 0);
  return { project: project.project.slug, items, more, minutes: Math.ceil(characters / 1000), notes };
}

/** Why an answer is refused before the queue is read: none, or the stage's own flaw. */
function stageFlaw(review: StoredReview, stage: StageBody): string | null {
  const id = review.id;
  if (stage.decision === "reject") {
    if (stage.reason.trim() === "") {
      return `\`${id}\` needs a reason: the reason is blank`;
    }
    if (bytesOf(stage.reason) > REASON_MAX_BYTES) {
      return `the reason: ${String(bytesOf(stage.reason))} bytes; at most ${String(REASON_MAX_BYTES)}`;
    }
    const char = ESCAPED.exec(stage.reason)?.[0];
    return char === undefined ? null : `the reason holds U+${char.codePointAt(0)?.toString(16).toUpperCase().padStart(4, "0") ?? ""}: a reason never carries it`;
  }
  const last = review.options.length - 1;
  if (stage.option !== null) {
    if (last < 0) {
      return `--option: \`${id}\` has no options`;
    }
    if (review.options[stage.option] === undefined) {
      return `option ${String(stage.option)}: \`${id}\` has options 0-${String(last)}`;
    }
  } else if (last >= 0) {
    return `\`${id}\` takes one of its options by number, 0-${String(last)}, or a reject with its reason`;
  }
  if (stage.answer !== null) {
    if (bytesOf(stage.answer) > ANSWER_MAX_BYTES) {
      return `the answer: ${String(bytesOf(stage.answer))} bytes; at most ${String(ANSWER_MAX_BYTES)}`;
    }
    const char = ESCAPED.exec(stage.answer)?.[0];
    if (char !== undefined) {
      return `the answer holds U+${char.codePointAt(0)?.toString(16).toUpperCase().padStart(4, "0") ?? ""}: an answer never carries it`;
    }
  }
  return null;
}

/** Why the queue now refuses an answer on `review`, read at `read`: none, or the reason. */
function queueFlaw(review: StoredReview | undefined, id: string, read: string): string | null {
  if (review === undefined) {
    return `no proposal \`${id}\` in this project's queue`;
  }
  if (review.status === "applied") {
    return `\`${id}\` is applied: commit ${review.applied_commit ?? "unknown"}`;
  }
  if (review.status === "approved") {
    return `\`${id}\` is held (approved: an apply in progress or stopped): spec approve ${id} completes it`;
  }
  if (review.status !== "open") {
    return `\`${id}\` is ${review.status}`;
  }
  if (CHANGES.has(review.kind)) {
    return `\`${id}\` is a change proposal (${review.kind}): \`spec approve ${id}\` decides it`;
  }
  if (!DECIDING.has(review.kind)) {
    return `\`${id}\` is no question, discrepancy or decision (${review.kind})`;
  }
  if (review.updated_at !== read) {
    return `\`${id}\` changed since it was read: updated at ${review.updated_at}, the choice was made on the one of ${read}; read it again`;
  }
  return null;
}

/** The stored stage of a stage body: less `updated_at`; an approve's `span_hash` null (a deciding kind). */
function storedOf(stage: StageBody): Stage {
  if (stage.decision === "approve") {
    return { decision: "approve", option: stage.option, answer: stage.answer, canon: stage.canon, note: stage.note, span_hash: null };
  }
  return { decision: "reject", reason: stage.reason };
}

/**
 * The round POST over a mock project (`docs/features/ui-round.md` "Data"): each answer refused
 * alone by its first refusal (an ID answered twice: each, naming the positions), the rest staged at
 * one clock reading `at`, replacing any staged choice; the outcomes in request order. Over 200
 * answers: 400, nothing staged.
 */
export function stageAnswers(project: MockProject, answers: readonly RoundAnswerBody[], at: string): RoundAnswersOutcome {
  const slug = project.project.slug;
  if (answers.length > ANSWERS_MAX) {
    throw new ClientError({ status: 400, message: `${String(answers.length)} answers; at most ${String(ANSWERS_MAX)} in one request; nothing changed` });
  }
  const positions = new Map<string, number[]>();
  answers.forEach(({ id }, index) => {
    positions.set(id, [...(positions.get(id) ?? []), index + 1]);
  });
  const outcomes: RoundAnswerEntry[] = answers.map(({ id, stage }) => {
    const twice = positions.get(id) ?? [];
    const index = project.proposals.findIndex(({ review }) => review.id === id);
    const stored = project.proposals[index];
    const reason =
      twice.length > 1
        ? `answered at positions ${named(twice.map(String))}: give one answer per item`
        : (queueFlaw(stored?.review, id, stage.updated_at) ?? (stored === undefined ? null : stageFlaw(stored.review, stage)));
    if (reason !== null || stored === undefined) {
      return { id, outcome: "refused", stage: null, staged_at: null, replaced: null, reason: reason ?? `no proposal \`${id}\`` };
    }
    const staged = storedOf(stage);
    const replaced = stored.review.staged_at;
    project.proposals[index] = { ...stored, review: { ...stored.review, staged, staged_at: at, updated_at: at } };
    return { id, outcome: "staged", stage: staged, staged_at: at, replaced, reason: null };
  });
  return { project: slug, answers: outcomes, notes: [] };
}

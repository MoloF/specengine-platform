import type { RevisionChange } from "../api/types";
import { fakeHex, proposal, stamp, type MockProject, type StoredReview } from "./build";
import { nodeViewOf } from "./corpus";
import { WEATHER_DECISION } from "./harbor-sim/fixtures";

// The `editorial` scenario (docs/features/rev-discipline.md "UI", "Review document, stage, queue"):
// every read as `normal`, but harbor-sim keeps a spec.lock whose entries vouch for the nodes its
// changes alter, so each `revisions` entry counts the lock entries an editorial approval refreshes
// (MEC-CARGO-CRANES none: no entry of it); PR-0053, a wording fix an agent called editorial in its
// rationale, has an editorial approve staged; PR-0054, a wording fix of the decision DEC-0012,
// whose closed class `decision` admits no `rev`, lists its raise as refused with the daemon's
// reason, `to` null, and the review's note for it; ledger-api's spec.lock is unreadable, so its
// counts are null and its review says why in a note. The numbers are the daemon's, written here as
// such: `to` is never worked out from `rev`.

/** The wording fix's ID, after harbor-sim's last proposal. */
export const WORDING_FIX = "PR-0053";

/** The node it rewords, its revision now and the one a meaning approval would write. */
export const REWORDED = { id: "RULE-PILOT-REQ", rev: 1, to: 2 } as const;

/** The owner's note on the staged editorial approve. */
export const WORDING_NOTE = "Wording only: the rule says the same.";

/** The decision's wording fix, after PR-0053: its raise refused, nothing staged. */
export const DECISION_FIX = "PR-0054";

/** The decision it rewords: a document of the closed class `decision`, no `rev` written, so revision 1. */
export const CLOSED_NODE = { id: WEATHER_DECISION, rev: 1 } as const;

/** Why a meaning approval cannot raise it, as apply's step 6 words it (`revisions`' `refused`). */
export const CLOSED_REFUSAL = `\`${WEATHER_DECISION}\`: class \`decision\` is closed and does not admit \`rev\`: admit it in [classes], or approve with --editorial`;

/** The review's note for the refused raise, as `spec review` adds one per refused node. */
export const CLOSED_NOTE = `meaning approve not applicable (step 6): ${CLOSED_REFUSAL}`;

/** harbor-sim's spec.lock: the entries vouching for each node's text now, by ID; any other node has none. */
export const HARBOR_LOCK: ReadonlyMap<string, number> = new Map([
  ["RULE-BERTH-DRAFT", 2],
  ["RULE-HARBOR-CLOCK", 1],
  [REWORDED.id, 1],
]);

/** ledger-api's note on each change that alters a node: its spec.lock cannot be read. */
export const UNREADABLE_LOCK_NOTE =
  "spec.lock is unreadable (line 12: `spec_hash` is not a b3: hash): the lock entries an editorial approval would refresh are not counted (mock scenario: editorial)";

/** One wording fix: the node it rewords, its line before and after, and what the review says of it. */
interface Fix {
  id: string;
  node: string;
  before: string;
  after: string;
  rationale: string;
  revision: RevisionChange;
  run: string;
  /** Minutes ago it was raised, and the editorial approve's note staged on it, if any. */
  raised: number;
  staged: { note: string; minutesAgo: number } | null;
  notes: string[];
}

/** A wording fix over the corpus as it is: its base the node's span now, one line reworded. */
function wordingFix(project: MockProject, now: number, fix: Fix): StoredReview {
  const node = nodeViewOf(project.corpus, fix.node).nodes[0];
  if (node === undefined) {
    throw new Error(`the editorial scenario needs ${fix.node} in ${project.project.slug}`);
  }
  const baseLines = node.text.replace(/\n$/, "").split("\n");
  const at = baseLines.indexOf(fix.before);
  if (at < 0) {
    throw new Error(`${fix.node} no longer holds the line ${fix.id} rewords`);
  }
  const newText = node.text.replace(fix.before, fix.after);
  const count = String(baseLines.length);
  const diff = [
    `--- base ${node.path}`,
    `+++ proposed ${node.path}`,
    `@@ -1,${count} +1,${count} @@`,
    ...baseLines.map((line, index) => (index === at ? `-${line}\n+${fix.after}` : ` ${line}`)),
  ].join("\n");
  const stagedAt = fix.staged === null ? null : stamp(now, fix.staged.minutesAgo);
  return proposal({
    id: fix.id,
    project: project.project.slug,
    kind: "update",
    target_id: fix.node,
    target_path: node.path,
    worktree: `/work/${project.project.slug}`,
    base_commit: "3b5d7f9a1c3e5b7d9f1a3c5e7b9d1f3a5c7e9b1d",
    base_hash: node.span_hash,
    base_text: node.text,
    new_text: newText,
    patch_hash: `b3:${fakeHex(`${fix.node}\n${node.span_hash}\n${newText}`, 64)}`,
    rationale: fix.rationale,
    diff,
    preview: "applies",
    revisions: [fix.revision],
    staged:
      fix.staged === null
        ? null
        : { decision: "approve", option: null, answer: null, canon: null, editorial: true, note: fix.staged.note, span_hash: node.span_hash },
    staged_at: stagedAt,
    notes: fix.notes,
    author: { type: "agent", role: "spec-writer", model: "claude-opus-5-5", run: fix.run },
    created_at: stamp(now, fix.raised),
    updated_at: stagedAt ?? stamp(now, fix.raised),
  }).review;
}

/** PR-0053: a comma in RULE-PILOT-REQ, its raise 1 -> 2, an editorial approve staged. */
function pilotFix(project: MockProject, now: number): StoredReview {
  return wordingFix(project, now, {
    id: WORDING_FIX,
    node: REWORDED.id,
    before: "Without a free pilot the ship waits outside the fairway.",
    after: "Without a free pilot, the ship waits outside the fairway.",
    rationale: "Editorial: a comma after the condition; the rule says the same.\nEvery rev kept as read: approval sets revisions.",
    revision: { id: REWORDED.id, rev: REWORDED.rev, to: REWORDED.to, lock_entries: HARBOR_LOCK.get(REWORDED.id) ?? 0, refused: null },
    run: "R-2281",
    raised: 9,
    staged: { note: WORDING_NOTE, minutesAgo: 4 },
    notes: [],
  });
}

/** PR-0054: one word in DEC-0012; its closed class refuses the raise, so `to` is null; nothing staged. */
function decisionFix(project: MockProject, now: number): StoredReview {
  return wordingFix(project, now, {
    id: DECISION_FIX,
    node: CLOSED_NODE.id,
    before: "Ships and pilot boats react to them.",
    after: "Ships and pilot boats react to both.",
    rationale: "Editorial: \"both\" names wind and visibility; the decision says the same.\nEvery rev kept as read: approval sets revisions.",
    revision: { id: CLOSED_NODE.id, rev: CLOSED_NODE.rev, to: null, lock_entries: HARBOR_LOCK.get(CLOSED_NODE.id) ?? 0, refused: CLOSED_REFUSAL },
    run: "R-2284",
    raised: 3,
    staged: null,
    notes: [CLOSED_NOTE],
  });
}

/** A review whose revisions count the lock entries `count` gives each node. */
function counted(review: StoredReview, count: (id: string) => number | null): StoredReview {
  return review.revisions === null ? review : { ...review, revisions: review.revisions.map((entry) => ({ ...entry, lock_entries: count(entry.id) })) };
}

/** Turns the normal projects into the `editorial` scenario's, in place. */
export function editorialScenario(projects: MockProject[], now: number): void {
  for (const project of projects) {
    if (project.project.slug === "harbor-sim") {
      project.proposals = [
        ...project.proposals.map(({ review }) => ({ review: counted(review, (id) => HARBOR_LOCK.get(id) ?? 0) })),
        { review: pilotFix(project, now) },
        { review: decisionFix(project, now) },
      ];
    } else if (project.project.slug === "ledger-api") {
      project.proposals = project.proposals.map(({ review }) => {
        if (review.revisions === null || review.revisions.length === 0) {
          return { review };
        }
        return { review: { ...counted(review, () => null), notes: [...review.notes, UNREADABLE_LOCK_NOTE] } };
      });
    }
  }
}

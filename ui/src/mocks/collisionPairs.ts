import type { Evidence, Pair } from "../api/types";
import { frontMatter, proposal, specFile, stamp, type MockDocument, type MockProject } from "./build";
import { logItemEvent, type MockBoard } from "./cards";

// The `collision-pairs` scenario (docs/features/collision-pairs.md "Acceptance criteria", PP9, PP11):
// every read as `normal`, but harbor-sim's spec holds the crew's sprint and stamina, two documents of
// `fixtures/spec-a`, and its queue five `contradicts` discrepancies on them, each pair as the daemon
// would serve it, stored here as served, never worked out. PR-0061 is the spec's example: "Cost" and
// "Regeneration", one quote each, the code's under Other evidence, raised on C-0001 so a task's Answer...
// shows it. PR-0062 names two targets in one file, its quote under both. PR-0063's second target is gone
// from the spec: no title, no path, no quote. PR-0064 names three targets: no pair, the flat list as
// before (also on C-0001). PR-0065 is served a grouping no file match gives, which only the mock serves:
// its sprint quote cites the sprint file exactly, yet its pair lists that quote under Other evidence, so
// the page is seen to draw the served positions and match no file itself.

/** The spec's example item. */
export const PAIR_EXAMPLE = "PR-0061";

/** Two targets in one file. */
export const PAIR_SHARED = "PR-0062";

/** A side gone from the spec. */
export const PAIR_GONE = "PR-0063";

/** Three targets: no pair served. */
export const PAIR_THREE = "PR-0064";

/** A served grouping no file match gives (the mock's alone). */
export const PAIR_AS_SERVED = "PR-0065";

/** The task the example and the three-target item are raised for. */
export const PAIR_CARD = "C-0001";

/** The two files, as `fixtures/spec-a` has them. */
export const SPRINT_FILE = "docs/spec/movement/sprint.md";
export const STAMINA_FILE = "docs/spec/movement/stamina.md";

/** A target named nowhere in the spec now. */
export const GONE_TARGET = "RULE-STAM-PAUSE";

/** The two documents, added to harbor-sim's spec for this scenario. */
export function collisionDocuments(): MockDocument[] {
  return [
    specFile({
      path: SPRINT_FILE,
      id: "MEC-SPRINT",
      kind: "mechanic",
      title: "Sprint",
      status: "accepted",
      parent: "DOM-HARBOR",
      lines: [
        ...frontMatter({ id: "MEC-SPRINT", kind: "mechanic", status: "accepted", parent: "DOM-HARBOR" }),
        "",
        "# Sprint",
        "",
        "Hold the sprint key to run; each second drains stamina (see MEC-STAMINA).",
        "",
        "## Cost {#RULE-SPRINT-COST rev=3}",
        "",
        "Sprinting costs 12 units/s, drained per second.",
        "Sprinting pauses regeneration.",
      ],
      sections: [{ id: "RULE-SPRINT-COST", kind: "rule", title: "Cost", rev: 3 }],
    }),
    specFile({
      path: STAMINA_FILE,
      id: "MEC-STAMINA",
      kind: "mechanic",
      title: "Stamina",
      status: "accepted",
      parent: "DOM-HARBOR",
      lines: [
        ...frontMatter({ id: "MEC-STAMINA", kind: "mechanic", status: "accepted", parent: "DOM-HARBOR" }),
        "",
        "# Stamina",
        "",
        "Stamina limits sprinting. Drained while sprinting, restored at rest.",
        "",
        "## Regeneration {#RULE-STAM-REGEN}",
        "",
        "- Base rate 10 units/s; refill starts after 1.5 s.",
        "- Regeneration never pauses.",
      ],
      sections: [{ id: "RULE-STAM-REGEN", kind: "rule", title: "Regeneration" }],
    }),
  ];
}

/** The example's three quotes, a fresh copy per item: the sprint file's, the stamina file's, the code's. */
function evidenceOf(): Evidence[] {
  return [
    { file: SPRINT_FILE, qpath: null, lines: "18-20", observed: "Sprinting pauses regeneration", documented: "Cost is drained per second" },
    { file: STAMINA_FILE, qpath: null, lines: "21-23", observed: "Regeneration never pauses", documented: "Refill starts after 1.5 s" },
    { file: "src/stamina.rs", qpath: "stamina::regen", lines: "40-52", observed: "regen runs during sprint", documented: "a pause is specified" },
  ];
}

const OPTIONS = [
  { label: "Pause while sprinting", effect: "Sprint drains faster", price: "Regeneration rewritten" },
  { label: "Never pause", effect: "Sprint lasts longer", price: "Sprint cost rewritten" },
];

const COST = { id: "RULE-SPRINT-COST", title: "Cost", path: SPRINT_FILE };
const REGEN = { id: "RULE-STAM-REGEN", title: "Regeneration", path: STAMINA_FILE };

/** Each item's targets, quotes and pair as served (`null`: none, three targets). */
const ITEMS: readonly { id: string; targets: string[]; summary: string; evidence: Evidence[]; pair: Pair | null; card: string | null; minutes: number }[] = [
  {
    id: PAIR_EXAMPLE,
    targets: [COST.id, REGEN.id],
    summary: "Sprint cost and regeneration disagree on when stamina refills",
    evidence: evidenceOf(),
    pair: { left: { ...COST, evidence: [0] }, right: { ...REGEN, evidence: [1] }, other: [2] },
    card: PAIR_CARD,
    minutes: 44,
  },
  {
    id: PAIR_SHARED,
    targets: ["MEC-STAMINA", REGEN.id],
    summary: "The stamina page and its regeneration rule disagree on a pause",
    evidence: evidenceOf(),
    pair: { left: { id: "MEC-STAMINA", title: "Stamina", path: STAMINA_FILE, evidence: [1] }, right: { ...REGEN, evidence: [1] }, other: [0, 2] },
    card: null,
    minutes: 43,
  },
  {
    id: PAIR_GONE,
    targets: [COST.id, GONE_TARGET],
    summary: "Sprint cost contradicts the pause rule, since removed",
    evidence: evidenceOf(),
    pair: { left: { ...COST, evidence: [0] }, right: { id: GONE_TARGET, title: null, path: null, evidence: [] }, other: [1, 2] },
    card: null,
    minutes: 42,
  },
  {
    id: PAIR_THREE,
    targets: [COST.id, REGEN.id, "MEC-SPRINT"],
    summary: "Sprint, its cost and regeneration disagree on when stamina refills",
    evidence: evidenceOf(),
    pair: null,
    card: PAIR_CARD,
    minutes: 41,
  },
  {
    id: PAIR_AS_SERVED,
    targets: [COST.id, REGEN.id],
    summary: "Sprint cost and regeneration disagree, the sprint quote served apart",
    evidence: evidenceOf(),
    pair: { left: { ...COST, evidence: [] }, right: { ...REGEN, evidence: [1] }, other: [0, 2] },
    card: null,
    minutes: 40,
  },
];

/** Adds the two documents to harbor-sim's spec, the five discrepancies to its queue and the two on C-0001 to its board. */
export function collisionPairsScenario(harbor: MockProject, board: MockBoard | undefined, at: number): void {
  harbor.corpus = { ...harbor.corpus, documents: [...harbor.corpus.documents, ...collisionDocuments()] };
  for (const item of ITEMS) {
    const raised = stamp(at, item.minutes);
    harbor.proposals.push(
      proposal({
        id: item.id,
        project: harbor.project.slug,
        kind: "discrepancy",
        severity: "high",
        gap_type: "contradicts",
        target_id: item.targets[0] ?? null,
        target_path: item.pair?.left.path ?? SPRINT_FILE,
        target_ids: item.targets,
        summary: item.summary,
        evidence: item.evidence,
        pair: item.pair,
        options: OPTIONS.map((option) => ({ ...option })),
        recommendation: 0,
        author: { type: "agent", role: "rust-developer", model: "claude-opus-5-5", run: "R-2301" },
        card_id: item.card,
        created_at: raised,
      }),
    );
    if (board !== undefined && item.card !== null) {
      logItemEvent(board, item.id, "proposal.created", { as: "agent" }, raised);
    }
  }
}

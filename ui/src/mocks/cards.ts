import { ClientError, CARD_REFUSED, CARD_USAGE, NO_SUCH_CARD, type CardsOptions } from "../api/client";
import type {
  Card,
  CardItem,
  CardAcceptBody,
  CardAssignBody,
  CardCancelBody,
  CardClearBody,
  CardClearGroupBody,
  CardClearGroupWrite,
  CardCreateBody,
  CardEditBody,
  CardEvent,
  CardFlow,
  CardFrozen,
  CardList,
  CardMoveBody,
  CardNotFound,
  CardRaiseBody,
  CardRow,
  CardSplit,
  CardSplitBody,
  CardSplitChild,
  CardSplitParent,
  CardWrite,
  IntakeDocument,
  IntakeMatch,
  Mandate,
  TraceCard,
  TreeNode,
} from "../api/types";
import { proposal, stamp, type MockProposal, type StoredReview } from "./build";
import type { Scenario } from "./scenario";
import { linkedTaskOf, type MockTasks } from "./tasks";

// The mock's cards (`docs/canon/cards.md`), in memory until a reload: two invented boards and the
// daemon's rules in its words, so the Board's refusals can be seen without one. harbor-sim prints an
// almanac, two trades on two flows, the plates' `blocked` a column like any other; ledger-api keeps
// the default flow, and one card a column left behind. Marks are joined per read as the daemon's
// (`docs/features/card-trace.md`): a node's hash now against a card's frozen entry. The boards hold
// no section text, so each target is one member of its `within`, and `within_hash` is its `norm_hash`,
// the same string (`docs/features/trace-truth.md` "Data"). In harbor-sim DOM-BERTHS and MEC-MOORING changed after their cards read them, so C-0002 (open) and
// C-0005 (accepted) are suspect; C-0010 was accepted as departing, PR-0041 its open discrepancy;
// C-0008 stands on MEC-OLD-SLIPWAY, a target gone, which no node of a trace read holds; C-0011 and
// C-0012 are accepted and stand, so the Flow has something confirmed. The plates' `blocked` is its
// return column (`docs/canon/card-flows.md` "The rule"), a word of data like any other: C-0008 and
// C-0015 were each returned with a reason. Hand-offs run
// both ways and hold no target, so no trace read changes: C-0001 has a child handed off (C-0013),
// one taken in its own flow (C-0019), one accepted (C-0016) and one cancelled (C-0017); C-0003's
// was returned (C-0015), C-0004's accepted (C-0014), so C-0004, in proof, is ready beside Accept
// (docs/features/handoff-progress.md "Acceptance criteria", HP-4), and the plate C-0009 handed a
// caption to copy (C-0018).
// Cards link `T-` tasks of src/mocks/tasks.ts (`docs/canon/card-tasks.md` "The rule"), each read
// when the card is: C-0001 links T-0107 (plan review), T-0109 (in progress, its spec changed) and
// T-0106, whose row does not read; C-0004 links T-0104 (done); the accepted C-0012 keeps T-0111
// (ready, its staleness unknown, a note); ledger-api's C-0102 links T-0033 (plan review) and C-0104
// T-0031 (in progress). Every other card links none.
// A card's items are the queue's questions, discrepancies and decisions naming it, read with the card
// (docs/canon/card-questions.md "The rule"): C-0008 was sent back with a question, PR-0055, open, and
// names a decision, PR-0056, applied; their `proposal.*` events stand in the board's log beside the
// cards' and join the card's history by `seq`. Two of them stand out of time order, as two clocks'
// writes may (docs/features/card-questions.md "Acceptance criteria", U1): PR-0055 was raised a
// moment after the return, its `at` thirty seconds before it.
// A subtask's mandate (docs/canon/card-mandate.md "The rule"; docs/features/task-mandate.md
// "Acceptance criteria", V2, V3): C-0001's plate C-0013, reviewed by Mira Holt, is binding, and its
// glossary C-0019 is the copy desk's to decide; every other card holds none. Each holds one of
// C-0001's targets, so Ask and Record stand on it, and C-0013's plates flow opens its return column
// to it. Each recorded one decision, open: PR-0057 on C-0019 with its why, PR-0058 on C-0013, a
// departure from a binding brief; C-0001 reads both in its `subtask_items`, by ID.

/** A stored card: the document less what a read adds, its links as the task IDs the row keeps. */
export type StoredCard = Omit<Card, "mark" | "on" | "discrepancy" | "correction" | "tasks" | "items" | "subtask_items" | "allowed" | "history" | "context" | "notes"> & { tasks: string[] };

interface MockEvent extends CardEvent {
  id: string;
}

/** One project's board state. */
export interface MockBoard {
  flows: CardFlow[];
  cards: StoredCard[];
  events: MockEvent[];
  seq: number;
  /** The nodes whose meaning changed: each one's `norm_hash` now; any other reads as first frozen. */
  changed: Map<string, string>;
  /** The targets that no longer resolve. */
  gone: ReadonlySet<string>;
  /** The project's split templates, `[[card.split]]` as its config writes them (docs/canon/card-split.md "The rule"). */
  splits: readonly MockSplit[];
}

/** One `[[card.split]]` row: a node kind and the disciplines its work usually needs. */
export interface MockSplit {
  kind: string;
  disciplines: readonly string[];
}

/** A node of a split's scope, as the tree read serves it: its REF and its kind, none for a node with none. */
export interface MockScopeNode {
  target: string;
  kind: string | null;
}

/** Whether a discrepancy is `applied` or `rejected`. */
export type Settled = (discrepancy: string) => boolean;

const MINUTE_DAY = 24 * 60;

/** What `as` records when no name is sent. */
const LOCAL = "local";

/** Who a tool acts as: no one's name either. */
const AGENT = "agent";

/** How a cleared value is written. */
const CLEAR = "-";

/** At most this many bytes in a person's name, `CARD_PERSON_MAX`. */
const NAME_MAX_BYTES = 128;

/** At most this many bytes in a note or a reason, `NOTE_MAX`. */
const NOTE_MAX_BYTES = 4096;

/** harbor-sim's target gone: its document was deleted by hand. */
const GONE_TARGET = "MEC-OLD-SLIPWAY";

/** The `conflict` scenario's other hand. */
const ELSEWHERE = "Dana Reyes";

/** `mandate_problem`'s refusal: a mandate with no reviewer to change it (docs/canon/card-mandate.md "The rule"). */
const MANDATE_NEEDS_REVIEWER = "mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer";

/** The two mandates a card may hold, as stored. */
const MANDATES: readonly Mandate[] = ["binding", "open"];

/** `mandate_change_problem`: only the card's stored reviewer, compared exactly, changes its mandate; none, no one. */
function mandateChangeProblem(id: string, reviewer: string | null, actor: string): string | null {
  if (reviewer === null) {
    return `mandate: ${id} has no reviewer, who alone changes its mandate: name one with \`spec card assign ${id} --reviewer N\` first; nothing changed`;
  }
  return reviewer === actor ? null : `mandate: only ${id}'s reviewer \`${reviewer}\` changes its mandate, not \`${actor}\`; nothing changed`;
}

/** Every ordered pair of the columns but `accept.to`, as the reader expands a flow with no `moves`. */
function everyMove(columns: readonly string[], closing: string) {
  const open = columns.filter((column) => column !== closing);
  return open.flatMap((from) => open.filter((to) => to !== from).map((to) => ({ from, to, by: [] })));
}

const COPY: CardFlow = {
  name: "copy",
  title: "Copy desk",
  // Two disciplines on one desk, so a split's row shows its raw discipline beside the flow's title (docs/features/department-split.md "Acceptance criteria", U2).
  disciplines: ["copy", "captions"],
  columns: [
    { id: "draft", title: "Draft" },
    { id: "edit", title: "Edit" },
    { id: "proof", title: "Proof" },
    { id: "filed", title: "Filed" },
  ],
  moves: [
    { from: "draft", to: "edit", by: ["desk"] },
    { from: "edit", to: "proof", by: [] },
  ],
  accept: { from: "proof", to: "filed" },
  return: null,
};

const PLATES: CardFlow = {
  name: "plates",
  title: "plates",
  disciplines: ["plates"],
  columns: ["queued", "blocked", "checked", "struck"].map((id) => ({ id, title: id })),
  moves: everyMove(["queued", "blocked", "checked", "struck"], "struck"),
  accept: { from: "checked", to: "struck" },
  return: "blocked",
};

const PLAIN: CardFlow = {
  name: "plain",
  title: "plain",
  disciplines: [],
  columns: [
    { id: "backlog", title: "Backlog" },
    { id: "ready", title: "Ready" },
    { id: "in-progress", title: "In progress" },
    { id: "review", title: "Review" },
    { id: "done", title: "Done" },
  ],
  moves: everyMove(["backlog", "ready", "in-progress", "review", "done"], "done"),
  accept: { from: "review", to: "done" },
  return: null,
};

/** A node's `norm_hash` as first read. */
function hashOf(id: string): string {
  return `b3:${id.toLowerCase()}`;
}

/** The hash a node reads now. */
function hashNow(board: MockBoard, id: string): string {
  return board.changed.get(id) ?? hashOf(id);
}

/** An entry frozen at `hash`: one member, so both hashes are the same string. */
function frozenAt(id: string, hash: string | null): CardFrozen {
  return { id, rev: 1, norm_hash: hash, within_hash: hash, also: [] };
}

/** A frozen entry as a card reads the node now; a gone target has no hash to read. */
function frozenNow(board: MockBoard, id: string): CardFrozen {
  return frozenAt(id, board.gone.has(id) ? null : hashNow(board, id));
}

/** The hash an entry is compared by: its `within_hash`, else, on an entry holding none, its `norm_hash`. */
function compared({ within_hash, norm_hash }: CardFrozen): string | null {
  return within_hash ?? norm_hash;
}

/** A calendar day `days` from `at`, as `--due` writes one. */
function dayFrom(at: number, days: number): string {
  return stamp(at, -days * MINUTE_DAY).slice(0, 10);
}

/** A stored card; omitted fields as a new card has them. */
function aCard(at: number, fields: Partial<StoredCard> & Pick<StoredCard, "id" | "flow" | "column" | "position" | "title">): StoredCard {
  return {
    goal: null,
    discipline: null,
    targets: [],
    snapshot: null,
    criteria: [],
    assignee: null,
    reviewer: null,
    mandate: null,
    estimate: null,
    due: null,
    parent: null,
    corrects: null,
    evidence: [],
    tasks: [],
    acceptance: null,
    cancellation: null,
    created_by: "Mira Holt",
    created_at: stamp(at, 3 * MINUTE_DAY),
    updated_at: stamp(at, 2 * MINUTE_DAY),
    revision: "1",
    ...fields,
  };
}

function harborCards(at: number): StoredCard[] {
  return [
    aCard(at, {
      id: "C-0001",
      flow: "copy",
      column: "draft",
      position: 1,
      title: "Write the tide table foreword",
      goal: "One page a skipper reads before the first table: what the numbers mean and where they stop being true.",
      discipline: "copy",
      targets: ["MEC-TIDE-TABLES", "MEC-TIDES", "DOM-WATER"],
      criteria: ["Names the datum the tables use", "Says what a surge does to them"],
      assignee: "Mira Holt",
      reviewer: "Ivo Brandt",
      estimate: "3",
      due: dayFrom(at, -2),
      tasks: ["T-0107", "T-0109", "T-0106"],
    }),
    aCard(at, {
      id: "C-0002",
      flow: "copy",
      column: "draft",
      position: 2,
      title: "Caption the berth plans",
      discipline: "copy",
      targets: ["DOM-BERTHS"],
      estimate: "1-2",
      due: dayFrom(at, 6),
    }),
    aCard(at, {
      id: "C-0003",
      flow: "copy",
      column: "edit",
      position: 1,
      title: "Night passage rules, plain words",
      goal: "The rule as a pilot would say it over the radio.",
      discipline: "copy",
      targets: ["MEC-NIGHT-PASSAGE"],
      assignee: "Tomas Oakes",
      reviewer: "Ivo Brandt",
      estimate: "2",
      due: dayFrom(at, 1),
      evidence: [{ kind: "note", value: "Second read done with the pilots' office", by: "Tomas Oakes", at: stamp(at, 26 * 60) }],
      revision: "3",
    }),
    aCard(at, {
      id: "C-0004",
      flow: "copy",
      column: "proof",
      position: 1,
      title: "Proof the lock opening hours",
      discipline: "copy",
      targets: ["MEC-LOCK-A", "MEC-LOCK-B"],
      criteria: ["Every hour checked against the lock master's sheet"],
      assignee: "Mira Holt",
      reviewer: "Ivo Brandt",
      estimate: "1",
      due: dayFrom(at, 0),
      evidence: [{ kind: "url", value: "https://example.org/locks/hours-2026.pdf", by: "Mira Holt", at: stamp(at, 5 * 60) }],
      tasks: ["T-0104"],
      revision: "5",
    }),
    aCard(at, {
      id: "C-0005",
      flow: "copy",
      column: "filed",
      position: 1,
      title: "Mooring fees table",
      discipline: "copy",
      targets: ["MEC-MOORING"],
      assignee: "Tomas Oakes",
      reviewer: "Mira Holt",
      estimate: "1",
      due: dayFrom(at, -9),
      acceptance: { by: "Mira Holt", at: stamp(at, 4 * MINUTE_DAY), note: "Matches the harbour office's rates.", verdict: "briefed", departs: [], discrepancy: null },
      revision: "6",
    }),
    aCard(at, {
      id: "C-0010",
      flow: "copy",
      column: "filed",
      position: 2,
      title: "Deep-draft entry notice",
      discipline: "copy",
      targets: ["RULE-TIDE-WINDOW"],
      assignee: "Lena Fisk",
      reviewer: "Ivo Brandt",
      estimate: "1",
      acceptance: { by: "Ivo Brandt", at: stamp(at, MINUTE_DAY), note: "The window here is an hour wider than the rule.", verdict: "departs", departs: ["RULE-TIDE-WINDOW"], discrepancy: "PR-0041" },
      revision: "4",
    }),
    aCard(at, {
      id: "C-0006",
      flow: "copy",
      column: "draft",
      position: 0,
      title: "Advert for the old quays tour",
      discipline: "copy",
      assignee: "Lena Fisk",
      due: dayFrom(at, -20),
      cancellation: { by: "Ivo Brandt", at: stamp(at, 9 * MINUTE_DAY), reason: "The tour stops this season." },
      revision: "2",
    }),
    aCard(at, {
      id: "C-0007",
      flow: "plates",
      column: "queued",
      position: 1,
      title: "Fairway chart, sheet 2",
      discipline: "plates",
      targets: ["MEC-FAIRWAY"],
      assignee: "Noor Ainsley",
      reviewer: "Tomas Oakes",
      estimate: "4",
      due: dayFrom(at, 10),
    }),
    aCard(at, {
      id: "C-0008",
      flow: "plates",
      column: "blocked",
      position: 1,
      title: "Crane reach diagram",
      goal: "Waiting on the survey's new numbers; the plate is set otherwise.",
      discipline: "plates",
      targets: ["MEC-CARGO-CRANES", GONE_TARGET],
      assignee: "Noor Ainsley",
      estimate: "2",
    }),
    aCard(at, {
      id: "C-0009",
      flow: "plates",
      column: "checked",
      position: 1,
      title: "Harbour overview, colour plate",
      discipline: "plates",
      targets: ["DOM-HARBOR"],
      assignee: "Lena Fisk",
      reviewer: "Noor Ainsley",
      estimate: "3",
      due: dayFrom(at, 2),
      evidence: [{ kind: "build", value: "plate-run-41", by: "Lena Fisk", at: stamp(at, 8 * 60) }],
    }),
    aCard(at, {
      id: "C-0011",
      flow: "copy",
      column: "filed",
      position: 3,
      title: "High water note",
      discipline: "copy",
      targets: ["RULE-HIGH-WATER"],
      assignee: "Tomas Oakes",
      reviewer: "Mira Holt",
      estimate: "1",
      acceptance: { by: "Mira Holt", at: stamp(at, 5 * MINUTE_DAY), note: null, verdict: "briefed", departs: [], discrepancy: null },
      revision: "5",
    }),
    aCard(at, {
      id: "C-0012",
      flow: "copy",
      column: "filed",
      position: 4,
      title: "Spring window table",
      discipline: "copy",
      targets: ["RULE-SPRING-WINDOW"],
      assignee: "Mira Holt",
      reviewer: "Ivo Brandt",
      estimate: "2",
      acceptance: { by: "Ivo Brandt", at: stamp(at, 2 * MINUTE_DAY), note: null, verdict: "briefed", departs: [], discrepancy: null },
      tasks: ["T-0111"],
      revision: "5",
    }),
    aCard(at, {
      id: "C-0013",
      flow: "plates",
      column: "queued",
      position: 2,
      title: "Tide table plate",
      goal: "The tables set as one plate, the datum line printed under them.",
      discipline: "plates",
      targets: ["MEC-TIDE-TABLES"],
      parent: "C-0001",
      reviewer: "Mira Holt",
      mandate: "binding",
    }),
    aCard(at, {
      id: "C-0014",
      flow: "plates",
      column: "struck",
      position: 2,
      title: "Lock hours plate",
      discipline: "plates",
      parent: "C-0004",
      assignee: "Noor Ainsley",
      reviewer: "Mira Holt",
      acceptance: { by: "Mira Holt", at: stamp(at, 6 * 60), note: null, verdict: "briefed", departs: [], discrepancy: null },
      revision: "4",
    }),
    aCard(at, {
      id: "C-0015",
      flow: "plates",
      column: "blocked",
      position: 2,
      title: "Night passage chart",
      discipline: "plates",
      parent: "C-0003",
      assignee: "Lena Fisk",
      reviewer: "Tomas Oakes",
      revision: "3",
    }),
    aCard(at, {
      id: "C-0016",
      flow: "plates",
      column: "struck",
      position: 1,
      title: "Tide table cover",
      discipline: "plates",
      parent: "C-0001",
      assignee: "Noor Ainsley",
      reviewer: "Mira Holt",
      acceptance: { by: "Mira Holt", at: stamp(at, MINUTE_DAY), note: null, verdict: "briefed", departs: [], discrepancy: null },
      revision: "4",
    }),
    aCard(at, {
      id: "C-0017",
      flow: "plates",
      column: "queued",
      position: 0,
      title: "Second tide plate",
      discipline: "plates",
      parent: "C-0001",
      reviewer: "Mira Holt",
      cancellation: { by: "Mira Holt", at: stamp(at, MINUTE_DAY), reason: "One plate holds both tables." },
      revision: "2",
    }),
    aCard(at, {
      id: "C-0018",
      flow: "copy",
      column: "draft",
      position: 3,
      title: "Caption the harbour overview",
      discipline: "copy",
      parent: "C-0009",
      reviewer: "Noor Ainsley",
    }),
    aCard(at, {
      id: "C-0019",
      flow: "copy",
      column: "edit",
      position: 2,
      title: "Tide table glossary",
      discipline: "copy",
      targets: ["DOM-WATER"],
      parent: "C-0001",
      assignee: "Tomas Oakes",
      reviewer: "Mira Holt",
      mandate: "open",
      revision: "2",
    }),
  ];
}

/** The moves harbor-sim's cards made before the page opened, each with its note: C-0015's return, then C-0008's. */
function harborMoves(at: number): { id: string; type: string; at: string; payload: Record<string, unknown> }[] {
  return [
    { id: "C-0015", type: "card.moved", at: stamp(at, 30 * 60), payload: { id: "C-0015", from: "queued", to: "checked", position: 2, as: "Lena Fisk", note: null } },
    {
      id: "C-0015",
      type: "card.moved",
      at: stamp(at, 6 * 60),
      payload: { id: "C-0015", from: "checked", to: "blocked", position: 2, as: "Tomas Oakes", note: "The buoy numbers on the plate disagree with MEC-NIGHT-PASSAGE: the second light is green, not red." },
    },
    {
      id: "C-0008",
      type: "card.moved",
      at: stamp(at, 600),
      payload: { id: "C-0008", from: "queued", to: "blocked", position: 1, as: "Noor Ainsley", note: "The survey's crane reach numbers are not in MEC-CARGO-CRANES yet; the diagram waits on them." },
    },
  ];
}

/** The events of C-0008's two items (src/mocks/harbor-sim/fixtures.ts): PR-0055 asked with its return, PR-0056 recorded, confirmed and applied. */
function harborItemEvents(at: number): { id: string; type: string; at: string; payload: Record<string, unknown> }[] {
  return [
    { id: "PR-0055", type: "proposal.created", at: stamp(at, 600.5), payload: { id: "PR-0055", as: "Noor Ainsley" } },
    { id: "PR-0056", type: "proposal.created", at: stamp(at, 570), payload: { id: "PR-0056", as: "Tomas Oakes" } },
    { id: "PR-0056", type: "proposal.approved", at: stamp(at, 540), payload: { id: "PR-0056", record: "DEC-0013" } },
    { id: "PR-0056", type: "proposal.applied", at: stamp(at, 540), payload: { id: "PR-0056", commit: "b7c9d1e3f5a7b9c1d3e5f7a9b1c3d5e7f9a1b3c5" } },
    // The subtasks' two decisions (task-mandate): C-0019's, then C-0013's.
    { id: "PR-0057", type: "proposal.created", at: stamp(at, 240), payload: { id: "PR-0057", as: "Tomas Oakes" } },
    { id: "PR-0058", type: "proposal.created", at: stamp(at, 180), payload: { id: "PR-0058", as: "Noor Ainsley" } },
  ];
}

function ledgerCards(at: number): StoredCard[] {
  return [
    aCard(at, {
      id: "C-0101",
      flow: "plain",
      column: "backlog",
      position: 1,
      title: "Write the refund runbook",
      targets: ["SVC-REFUNDS"],
      estimate: "2",
    }),
    aCard(at, {
      id: "C-0102",
      flow: "plain",
      column: "ready",
      position: 1,
      title: "Publish the idempotency rule for partners",
      targets: ["POL-IDEMPOTENCY"],
      assignee: "Ana Weiss",
      due: dayFrom(at, 4),
      tasks: ["T-0033"],
    }),
    aCard(at, {
      id: "C-0103",
      flow: "plain",
      column: "in-progress",
      position: 1,
      title: "Book the finance review",
      assignee: "Omar Lind",
      reviewer: "Ana Weiss",
      due: dayFrom(at, -1),
    }),
    aCard(at, {
      id: "C-0104",
      flow: "plain",
      column: "review",
      position: 1,
      title: "Refund window, customer wording",
      targets: ["POL-REFUND-WINDOW", "POL-REFUND-RULES"],
      assignee: "Omar Lind",
      reviewer: "Ana Weiss",
      estimate: "1",
      tasks: ["T-0031"],
    }),
    aCard(at, {
      id: "C-0105",
      flow: "plain",
      column: "parked",
      position: 1,
      title: "Old settlement report",
      assignee: "Ana Weiss",
    }),
  ];
}

/** The `large` scenario's column: past the Board's cap. */
function largeColumn(at: number): StoredCard[] {
  return Array.from({ length: 230 }, (_, index) =>
    aCard(at, {
      id: `C-${String(1000 + index).padStart(4, "0")}`,
      flow: "copy",
      column: "edit",
      position: index + 2,
      title: `Chart note ${String(index + 1)}`,
      discipline: "copy",
    }),
  );
}

/**
 * harbor-sim's split templates: a mechanic's work goes to copy and the plates, a rule's to the
 * plates; captions has none, so its subtask takes the parent's targets. ledger-api writes none.
 */
const HARBOR_SPLITS: readonly MockSplit[] = [
  { kind: "mechanic", disciplines: ["copy", "plates"] },
  { kind: "rule", disciplines: ["plates"] },
];

/** A project's board for a scenario; `empty` keeps the columns and no card. */
export function mockBoard(slug: string, scenario: Scenario, at: number): MockBoard {
  const harbor = slug === "harbor-sim";
  const flows = structuredClone(harbor ? [COPY, PLATES] : [PLAIN]);
  const board = (cards: StoredCard[], events: MockEvent[], changed: Map<string, string>): MockBoard => ({
    flows,
    cards,
    events,
    seq: events.length,
    changed,
    gone: new Set(harbor ? [GONE_TARGET] : []),
    splits: harbor ? HARBOR_SPLITS : [],
  });
  if (scenario === "empty") {
    return board([], [], new Map());
  }
  const cards = [...(harbor ? harborCards(at) : ledgerCards(at)), ...(harbor && scenario === "large" ? largeColumn(at) : [])].map((card) => ({
    ...card,
    snapshot: card.targets.map((id) => frozenAt(id, hashOf(id))),
  }));
  const created = cards.map((card) => ({
    id: card.id,
    type: "card.created",
    at: card.created_at,
    payload: { id: card.id, flow: card.flow, column: card.column, position: card.position, assignee: card.assignee, reviewer: card.reviewer, mandate: card.mandate, as: card.created_by },
  }));
  const events = [...created, ...(harbor ? [...harborMoves(at), ...harborItemEvents(at)] : [])].map((event, index): MockEvent => ({ ...event, seq: index + 1 }));
  return board(cards, events, new Map(harbor ? [["DOM-BERTHS", "b3:dom-berths-2"], ["MEC-MOORING", "b3:mec-mooring-2"]] : []));
}

function flowOf(board: MockBoard, name: string): CardFlow | undefined {
  return board.flows.find((flow) => flow.name === name);
}

function closed(card: StoredCard): boolean {
  return card.acceptance !== null || card.cancellation !== null;
}

function holds(flow: CardFlow, column: string): boolean {
  return flow.columns.some((candidate) => candidate.id === column);
}

/** What a read says is open: a closed card none; a column the flow left, its first column. */
function allowedOf(board: MockBoard, card: StoredCard) {
  const flow = flowOf(board, card.flow);
  if (flow === undefined || closed(card)) {
    return [];
  }
  if (!holds(flow, card.column)) {
    return [{ to: flow.columns[0]?.id ?? card.column, by: [] }];
  }
  return flow.moves.filter((move) => move.from === card.column).map(({ to, by }) => ({ to, by: [...by] }));
}

function rowOf({ id, flow, column, position, title, discipline, targets, assignee, reviewer, mandate, estimate, due, parent, corrects, updated_at, revision, acceptance, cancellation }: StoredCard): CardRow {
  return { id, flow, column, position, title, discipline, targets, assignee, reviewer, mandate, estimate, due, parent, corrects, updated_at, revision, accepted: acceptance !== null, cancelled: cancellation !== null, mark: null, on: null, discrepancy: null, correction: null };
}

/** A row with its mark over all its targets; a cancelled card, which a read ignores, has none. */
function markedRow(board: MockBoard, card: StoredCard, settled: Settled): CardRow {
  if (card.cancellation !== null) {
    return rowOf(card);
  }
  const { mark, on, discrepancy, correction } = markOver(board, card, card.targets, settled);
  return { ...rowOf(card), mark, on, discrepancy, correction };
}

/** A card's place in the list: flow, column by config, cancelled last, position. */
function placeOf(board: MockBoard, card: StoredCard): readonly number[] {
  const flow = board.flows.findIndex((candidate) => candidate.name === card.flow);
  const columns = board.flows[flow]?.columns ?? [];
  const column = columns.findIndex((candidate) => candidate.id === card.column);
  return [flow < 0 ? board.flows.length : flow, column < 0 ? columns.length : column, card.cancellation === null ? 0 : 1, card.position];
}

function byPlace(board: MockBoard) {
  return (a: StoredCard, b: StoredCard): number => {
    const left = placeOf(board, a);
    const right = placeOf(board, b);
    const at = left.findIndex((value, index) => value !== right[index]);
    return at < 0 ? 0 : (left[at] ?? 0) - (right[at] ?? 0);
  };
}

/** `spec card list [--marks] --json`, the browser view. */
export function cardListOf(board: MockBoard, options: CardsOptions, settled: Settled): CardList {
  const listed = board.cards
    .filter((card) => (options.with_cancelled === true || card.cancellation === null) && (options.parent === undefined || card.parent === options.parent))
    .sort(byPlace(board));
  const stranded = new Map<string, StoredCard[]>();
  for (const card of listed) {
    const flow = flowOf(board, card.flow);
    if (flow !== undefined && !holds(flow, card.column)) {
      const key = `${card.flow}/${card.column}`;
      stranded.set(key, [...(stranded.get(key) ?? []), card]);
    }
  }
  const notes = Array.from(stranded, ([key, group]) => `${String(group.length)} card(s) in ${key}, from ${group[0]?.id ?? ""}: the column \`${group[0]?.column ?? ""}\` is no longer in the flow \`${group[0]?.flow ?? ""}\`: served last`);
  const rows = listed.map((card) => (options.marks === true ? markedRow(board, card, settled) : rowOf(card)));
  return { flows: structuredClone(board.flows), cards: rows, notes, truncated: false, omitted: null };
}

/** The queue's prefix of a proposal's events, as stored. */
const ITEM_EVENT = "proposal.";

/** An item of a card as its read serves it: the row's 16 keys (docs/features/card-questions.md "Acceptance criteria", K1). */
function itemOf(review: StoredReview): CardItem {
  return {
    id: review.id,
    kind: review.kind,
    status: review.status,
    summary: review.summary,
    working_answer: review.working_answer,
    why: review.why,
    author: review.author ?? { type: "human", role: null, model: null, run: null },
    created_at: review.created_at,
    updated_at: review.updated_at,
    staged_at: review.staged_at,
    decided_by: review.decided_by,
    decided_at: review.decided_at,
    decision_note: review.decision_note,
    record_id: review.record_id,
    record_path: review.record_path,
    record_title: review.record_title,
  };
}

/** A proposal's ID number, as the queue orders its rows. */
function idNumber(id: string): number {
  return Number(id.slice(id.indexOf("-") + 1));
}

/**
 * `spec card show C --history --json`, uncut, as `GET cards/:id` serves it: no `--marks`, so its four
 * keys null; each linked task read now from `tasks`, its notes after the card's own, in link order;
 * its items, the queue's rows naming it, by ID number, and their `proposal.*` events beside its own
 * `card.*` ones, by `seq`; an unknown C its exit-1 document.
 */
export function cardShowOf(board: MockBoard, id: string, tasks: MockTasks, proposals: readonly MockProposal[] = []): Card | CardNotFound {
  const card = board.cards.find((candidate) => candidate.id === id);
  if (card === undefined) {
    return { id, reason: `no card ${id}` };
  }
  const items = proposals
    .filter(({ review }) => review.card_id === id)
    .map(({ review }) => itemOf(review))
    .sort((a, b) => idNumber(a.id) - idNumber(b.id));
  // Its subtasks' items, cancelled ones' too, no grandchild's: every item whose card names this one as `parent`, by ID, each its card first (`subtask_items`).
  const children = new Set(board.cards.filter((candidate) => candidate.parent === id).map((candidate) => candidate.id));
  const subtaskItems = proposals
    .filter(({ review }) => review.card_id !== null && children.has(review.card_id))
    .map(({ review }) => ({ card_id: review.card_id ?? "", ...itemOf(review) }))
    .sort((a, b) => idNumber(a.id) - idNumber(b.id));
  const named = new Set(items.map((item) => item.id));
  const history = board.events
    .filter((event) => (event.id === id && !event.type.startsWith(ITEM_EVENT)) || (event.type.startsWith(ITEM_EVENT) && named.has(event.id)))
    .map(({ seq, type, at, payload }) => ({ seq, type, at, payload }));
  const linked = card.tasks.map((task) => linkedTaskOf(tasks, task));
  return structuredClone({
    ...card,
    mark: null,
    on: null,
    discrepancy: null,
    correction: null,
    tasks: linked.map(({ task }) => task),
    items,
    subtask_items: subtaskItems,
    allowed: allowedOf(board, card),
    history,
    context: null,
    notes: linked.flatMap(({ notes }) => notes),
  });
}

/** One event of a proposal in the board's log: a card naming it reads it in its history. */
export function logItemEvent(board: MockBoard, id: string, type: string, payload: Record<string, unknown>, at: string): void {
  board.seq += 1;
  board.events.push({ id, seq: board.seq, type, at, payload: { id, ...payload } });
}

/** At most this many bytes in a question's or decision's text, `TEXT_MAX`; its answer, price and why `ANSWER_MAX`. */
const TEXT_MAX_BYTES = 1024;
const ANSWER_MAX_BYTES = 2048;

/** The most targets an item names (`agent-intake.md` "Rules"). */
const NODES_MAX = 16;

/** What `as` names when none is sent. */
const NO_NAME = LOCAL;

/** A text as the intake's dedup compares it: white space runs one space, trimmed, lower case. */
function normalized(text: string | null): string {
  return (text ?? "").replace(/\s+/gu, " ").trim().toLowerCase();
}

/** A required or given text off its grammar: blank, or over its cap; the intake's line, else null. */
function textProblem(key: string, text: string | null | undefined, cap: number, required: boolean): string | null {
  if (text === null || text === undefined) {
    return required ? `${key}: missing: give a value` : null;
  }
  if (text.trim() === "") {
    return `${key}: blank: give a value`;
  }
  const bytes = new TextEncoder().encode(text).length;
  return bytes > cap ? `${key}: ${String(bytes)} bytes; at most ${String(cap)}` : null;
}

/** How a hit is named in `distinct_from`: its ID, else its path. */
function nameOf(match: IntakeMatch): string {
  return match.id ?? match.path ?? "";
}

/**
 * `card_raise` over the mock's queue (docs/features/card-questions.md "Acceptance criteria", L3): the
 * card read first (404), the body's kind and `why` (400), the acting name (400), then the intake's
 * rules in their words (409), then the dedup: `corpus` the page's one corpus hit when the scenario
 * gives one; the queue's questions and decisions on a shared target with the same text are hits,
 * with other text related. Stored only when every hit is named in `distinct_from`: the next ID, the
 * card named, a person its author, `proposal.created` with the acting name; else nothing stored.
 */
export function raiseOnCardIn(board: MockBoard, queue: MockProposal[], project: string, id: string, body: CardRaiseBody, at: string, corpus: IntakeMatch | null): IntakeDocument {
  if (!board.cards.some((card) => card.id === id)) {
    throw new ClientError({ status: NO_SUCH_CARD, message: `no card ${id}` });
  }
  if (body.kind !== "question" && body.kind !== "decision") {
    throw new ClientError({ status: CARD_USAGE, message: "`kind`: a card raises a `question` or a `decision`" });
  }
  const why = body.why ?? null;
  if (body.kind === "question" && why !== null) {
    throw new ClientError({ status: CARD_USAGE, message: "`why` goes with a `decision`" });
  }
  checkName(body.as);
  const refusal = [
    body.node_ids.length === 0 || body.node_ids.length > NODES_MAX ? `node_ids: ${String(body.node_ids.length)} IDs; 1 to ${String(NODES_MAX)}` : null,
    textProblem("text", body.text, TEXT_MAX_BYTES, true),
    textProblem("working_answer", body.working_answer, ANSWER_MAX_BYTES, true),
    textProblem("price_of_other", body.price_of_other, ANSWER_MAX_BYTES, true),
    textProblem("why", why, ANSWER_MAX_BYTES, false),
  ].find((problem): problem is string => problem !== null);
  if (refusal !== undefined) {
    throw refused(refusal);
  }
  const shared = queue.filter(({ review }) => (review.kind === "question" || review.kind === "decision") && review.target_ids.some((target) => body.node_ids.includes(target)));
  const asQueued = (review: StoredReview): IntakeMatch => ({
    id: review.id,
    source: "queue",
    status: review.status,
    path: null,
    answer: review.status === "rejected" ? review.decision_note : null,
    record: review.record_id,
  });
  const hits = [...(corpus === null ? [] : [corpus]), ...shared.filter(({ review }) => normalized(review.summary) === normalized(body.text)).map(({ review }) => asQueued(review))];
  const related = shared.filter(({ review }) => normalized(review.summary) !== normalized(body.text)).map(({ review }) => ({ ...asQueued(review), answer: null, record: null }));
  const named = body.distinct_from ?? [];
  if (hits.some((hit) => !named.includes(nameOf(hit)))) {
    return { id: null, created: false, hits, related, linked: null, diagnostics: [], notes: [] };
  }
  const highest = queue.reduce((most, { review }) => Math.max(most, idNumber(review.id)), 0);
  const made = `PR-${String(highest + 1).padStart(4, "0")}`;
  queue.push(
    proposal({
      id: made,
      project,
      kind: body.kind,
      created_at: at,
      target_id: body.node_ids[0] ?? null,
      target_ids: [...body.node_ids],
      severity: body.severity ?? null,
      summary: body.text,
      working_answer: body.working_answer,
      price_of_other: body.price_of_other,
      why,
      distinct_from: [...named],
      author: { type: "human", role: null, model: null, run: null },
      card_id: id,
    }),
  );
  logItemEvent(board, made, "proposal.created", { as: body.as ?? NO_NAME }, at);
  return { id: made, created: true, hits, related, linked: null, diagnostics: [], notes: [] };
}

/** What cannot stand in a person's name: a control character, a line or paragraph separator, a bidirectional mark. */
const NOT_IN_A_NAME = /[\u0000-\u001f\u007f-\u009f\u061c\u200e\u200f\u2028\u2029\u202a-\u202e\u2066-\u2069]/u;

/** A person's name off its grammar: a 400 in the CLI's line, before anything is read. */
function checkPerson(key: string, name: string | null | undefined): void {
  if (name === null || name === undefined) {
    return;
  }
  const trimmed = name.trim();
  const bytes = new TextEncoder().encode(trimmed).length;
  const bad = NOT_IN_A_NAME.exec(trimmed)?.[0];
  if (bytes > NAME_MAX_BYTES) {
    throw new ClientError({ status: CARD_USAGE, message: `spec: ${key}: ${String(bytes)} bytes; at most ${String(NAME_MAX_BYTES)}` });
  }
  if (bad !== undefined) {
    const code = (bad.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, "0");
    throw new ClientError({ status: CARD_USAGE, message: `spec: ${key}: U+${code} in a name: one line, no control character or bidirectional mark` });
  }
}

/** The acting name, as the flag `--as` judges it. */
function checkName(as: string | null): void {
  checkPerson("--as", as);
}

function refused(reason: string): ClientError {
  return new ClientError({ status: CARD_REFUSED, message: reason });
}

/** The card a write is for: unknown 404, closed 409, a lost compare-and-set 409, in the daemon's words. */
function writable(board: MockBoard, id: string, revision: string | undefined): StoredCard {
  const card = board.cards.find((candidate) => candidate.id === id);
  if (card === undefined) {
    throw new ClientError({ status: NO_SUCH_CARD, message: `no card ${id}` });
  }
  if (closed(card)) {
    const state = card.acceptance === null ? "cancelled" : "accepted";
    throw refused(`${id} is ${state}: it takes no write; a correction is a new card, \`spec card new --corrects ${id}\`; nothing changed`);
  }
  if (revision !== undefined && revision !== card.revision) {
    throw refused(`\`${id}\` changed since this run read it: it is in ${card.column} since ${card.updated_at}; nothing changed`);
  }
  return card;
}

/** One event of one card, and the row's new `revision` and `updated_at`. */
function changed(board: MockBoard, card: StoredCard, type: string, payload: Record<string, unknown>, at: string): CardWrite {
  card.revision = String(Number(card.revision) + 1);
  card.updated_at = at;
  board.seq += 1;
  board.events.push({ id: card.id, seq: board.seq, type, at, payload: { id: card.id, ...payload } });
  return writeOf(board, card);
}

function writeOf(board: MockBoard, { id, flow, column, position, revision }: StoredCard): CardWrite {
  const card = board.cards.find((candidate) => candidate.id === id);
  return { id, flow, column, position, revision, allowed: card === undefined ? [] : allowedOf(board, card), notes: [] };
}

/** The cards standing in a column, by position. */
function standing(board: MockBoard, flow: string, column: string): StoredCard[] {
  return board.cards.filter((card) => card.flow === flow && card.column === column && card.cancellation === null).sort((a, b) => a.position - b.position);
}

/** Positions dense from 1, in the order given. */
function renumber(cards: readonly StoredCard[]): void {
  cards.forEach((card, index) => {
    card.position = index + 1;
  });
}

function nextId(board: MockBoard): string {
  const highest = board.cards.reduce((most, card) => Math.max(most, Number(card.id.slice(2))), 0);
  return `C-${String(highest + 1).padStart(4, "0")}`;
}

/** The flow the new-card rule gives a discipline: the first listing it, else the one listing none; else why none does. */
function flowFor(board: MockBoard, discipline: string | null): CardFlow | string {
  const flow = board.flows.find((candidate) => discipline !== null && candidate.disciplines.includes(discipline)) ?? board.flows.find((candidate) => candidate.disciplines.length === 0);
  if (flow !== undefined && flow.columns[0] !== undefined) {
    return flow;
  }
  const asked = discipline === null ? "a card without a discipline" : `the discipline \`${discipline}\``;
  return `no flow takes ${asked}: none lists it, and every \`[[card.flow]]\` lists its disciplines`;
}

/** `card_new`: the first flow listing the discipline, else the one flow listing none; goes last. */
export function createCardIn(board: MockBoard, body: CardCreateBody, at: string): CardWrite {
  checkName(body.as);
  checkPerson("assignee", body.assignee);
  if (body.reviewer === CLEAR) {
    throw refused(`reviewer: \`${CLEAR}\` clears a name: a new card has none`);
  }
  checkPerson("reviewer", body.reviewer);
  if (body.reviewer === LOCAL || body.reviewer === AGENT) {
    throw refused(`reviewer: \`${body.reviewer}\` can be no reviewer: a person accepts a result`);
  }
  // `mandate_problem`, after the reviewer's: a mandate goes with a reviewer, who alone changes it.
  if (body.mandate !== undefined && body.reviewer === undefined) {
    throw refused(MANDATE_NEEDS_REVIEWER);
  }
  if (body.parent !== undefined && !board.cards.some((card) => card.id === body.parent)) {
    throw refused(`parent: no card ${body.parent}`);
  }
  if (body.title.trim() === "") {
    throw refused("title: blank; a card has a title");
  }
  if (body.estimate !== undefined && !/^\d+(-\d+)?$/.test(body.estimate)) {
    throw refused(`estimate: \`${body.estimate}\` is not \`N\` or \`N-M\``);
  }
  const flow = flowFor(board, body.discipline ?? null);
  if (typeof flow === "string") {
    throw refused(flow);
  }
  const column = flow.columns[0]?.id ?? "";
  const as = body.as ?? LOCAL;
  const card: StoredCard = {
    id: nextId(board),
    flow: flow.name,
    column,
    position: standing(board, flow.name, column).length + 1,
    title: body.title,
    goal: body.goal ?? null,
    discipline: body.discipline ?? null,
    targets: body.target ?? [],
    snapshot: (body.target ?? []).map((target) => frozenNow(board, target)),
    criteria: body.criterion ?? [],
    assignee: body.assignee ?? null,
    reviewer: body.reviewer ?? null,
    mandate: body.mandate ?? null,
    estimate: body.estimate ?? null,
    due: body.due ?? null,
    parent: body.parent ?? null,
    corrects: body.corrects ?? null,
    evidence: [],
    tasks: [],
    acceptance: null,
    cancellation: null,
    created_by: as,
    created_at: at,
    updated_at: at,
    revision: "1",
  };
  board.cards.push(card);
  board.seq += 1;
  board.events.push({ id: card.id, seq: board.seq, type: "card.created", at, payload: { id: card.id, flow: card.flow, column, position: card.position, assignee: card.assignee, reviewer: card.reviewer, mandate: card.mandate, as } });
  return writeOf(board, card);
}

/** Why a flow refuses the move, the daemon's two lines; null when it stands. */
function moveRefusal(board: MockBoard, card: StoredCard, to: string, as: string): string | null {
  const flow = flowOf(board, card.flow);
  if (flow === undefined || to === card.column) {
    return null;
  }
  const open = allowedOf(board, card);
  const move = open.find((candidate) => candidate.to === to);
  if (move === undefined) {
    const allowed = [...open.map((candidate) => `\`${candidate.to}\``), ...(flow.accept.from === card.column ? ["`spec card accept`"] : [])];
    return `${card.id} is in ${card.column}: no move to \`${to}\`; allowed: ${allowed.length === 0 ? "nothing" : allowed.join(", ")}; nothing changed`;
  }
  if (move.by.length > 0 && !move.by.includes(as)) {
    return `${card.id} is in ${card.column}: the move to \`${to}\` is ${move.by.map((name) => `\`${name}\``).join(" or ")}'s, not \`${as}\`'s; nothing changed`;
  }
  return null;
}

/** A move's note as `cancel --reason` takes one: not blank, at most NOTE_MAX bytes, stored as sent, as the core stores it; none is null. */
function noteOf(note: string | undefined): string | null {
  if (note === undefined) {
    return null;
  }
  if (note.trim() === "") {
    throw refused("note: blank; say why it moves");
  }
  const bytes = new TextEncoder().encode(note).length;
  if (bytes > NOTE_MAX_BYTES) {
    throw refused(`note: ${String(bytes)} bytes; at most ${String(NOTE_MAX_BYTES)}`);
  }
  return note;
}

/**
 * `card_move`: leaves its column, closes it up, opens the target or goes last. A move entering its
 * flow's `return` column from another one needs a note; a reorder inside it, or a move out, none.
 */
export function moveCardIn(board: MockBoard, id: string, body: CardMoveBody, at: string): CardWrite {
  checkName(body.as);
  const card = writable(board, id, body.revision);
  const as = body.as ?? LOCAL;
  const to = body.to ?? card.column;
  const reason = moveRefusal(board, card, to, as);
  if (reason !== null) {
    throw refused(reason);
  }
  const note = noteOf(body.note);
  const back = flowOf(board, card.flow)?.return ?? null;
  if (note === null && to === back && card.column !== back) {
    throw refused(`${id} enters \`${to}\`, its flow's return column: say why with \`--note\`; nothing changed`);
  }
  const from = card.column;
  const left = standing(board, card.flow, from).filter((other) => other !== card);
  const target = from === to ? left : standing(board, card.flow, to);
  const place = body.before === undefined ? target.length : target.findIndex((other) => other.id === body.before);
  if (place < 0) {
    throw refused(`--before: ${body.before ?? ""} is not in ${to}; nothing changed`);
  }
  card.column = to;
  renumber(left);
  renumber([...target.slice(0, place), card, ...target.slice(place)]);
  return changed(board, card, "card.moved", { from, to, position: card.position, as, note }, at);
}

/** A value as `edit` and `assign` take it: absent keeps `stored`, `-` clears. */
function field(written: string | undefined, stored: string | null): string | null {
  if (written === undefined) {
    return stored;
  }
  return written === CLEAR ? null : written;
}

function list(written: string[] | undefined, stored: string[]): string[] {
  if (written === undefined) {
    return stored;
  }
  return written.length === 1 && written[0] === CLEAR ? [] : written;
}

/** `card_edit`: the fields asked, evidence appended. */
export function editCardIn(board: MockBoard, id: string, body: CardEditBody, at: string): CardWrite {
  checkName(body.as);
  // A body's mandate off its two words fails decoding, as any typed key: a 400, nothing read.
  if (body.mandate !== undefined && !MANDATES.includes(body.mandate)) {
    throw new ClientError({ status: CARD_USAGE, message: `spec: mandate: unknown variant \`${body.mandate}\`, expected \`binding\` or \`open\`` });
  }
  const card = writable(board, id, body.revision);
  const as = body.as ?? LOCAL;
  const reestimate = body.estimate !== undefined && body.estimate !== CLEAR && card.estimate !== null;
  const asked = (["title", "goal", "target", "criterion", "due", "parent", "corrects", "evidence", "mandate"] as const).some((key) => body[key] !== undefined);
  if (reestimate && asked) {
    throw new ClientError({ status: CARD_USAGE, message: `spec: \`spec card edit\`: ${id} holds an estimate: --estimate takes --reason and no other field` });
  }
  if (!reestimate && body.reason !== undefined) {
    throw new ClientError({ status: CARD_USAGE, message: "spec: `spec card edit`: --reason goes with --estimate on a card holding an estimate" });
  }
  if (reestimate && (body.reason === undefined || body.reason.trim() === "")) {
    throw refused(body.reason === undefined ? `${id} holds an estimate: --reason says why it changes; nothing changed` : "reason: blank; say why the estimate changes");
  }
  if (body.estimate !== undefined && body.estimate !== CLEAR && !/^\d+(-\d+)?$/.test(body.estimate)) {
    throw refused(`estimate: \`${body.estimate}\` is not \`N\` or \`N-M\``);
  }
  const evidence = (body.evidence ?? []).map((written) => {
    const split = written.indexOf("=");
    const [kind, value] = [written.slice(0, split), written.slice(split + 1)];
    if (kind === "url" && !/^https?:\/\//.test(value)) {
      throw refused(`evidence: \`${value}\` is no http:// or https:// url`);
    }
    return { kind, value, by: as, at };
  });
  // The store's `mandate_change_problem`, against the row under its lock, whatever else the edit holds.
  const mandateProblem = body.mandate === undefined ? null : mandateChangeProblem(id, card.reviewer, as);
  if (mandateProblem !== null) {
    throw refused(mandateProblem);
  }
  const fields = (["title", "goal", "target", "criterion", "estimate", "due", "parent", "corrects", "evidence", "mandate"] as const).filter((key) => body[key] !== undefined);
  if (reestimate) {
    const old = card.estimate;
    card.estimate = body.estimate ?? card.estimate;
    return changed(board, card, "card.reestimated", { old, new: card.estimate, reason: body.reason, as }, at);
  }
  card.title = body.title ?? card.title;
  card.goal = field(body.goal, card.goal);
  card.targets = list(body.target, card.targets);
  card.snapshot = card.targets.map((target) => card.snapshot?.find((entry) => entry.id === target) ?? frozenNow(board, target));
  card.criteria = list(body.criterion, card.criteria);
  card.estimate = field(body.estimate, card.estimate);
  card.due = field(body.due, card.due);
  card.parent = field(body.parent, card.parent);
  card.corrects = field(body.corrects, card.corrects);
  card.evidence = [...card.evidence, ...evidence];
  card.mandate = body.mandate ?? card.mandate;
  // `mandate` joins the payload only when the edit holds it: every other edit's stays `{id, fields, as}`.
  return changed(board, card, "card.edited", body.mandate === undefined ? { fields, as } : { fields, as, mandate: body.mandate }, at);
}

/** `card_assign`: `local` and `agent` are no one's name, so neither can review. */
export function assignCardIn(board: MockBoard, id: string, body: CardAssignBody, at: string): CardWrite {
  checkName(body.as);
  checkPerson("assignee", body.assignee);
  checkPerson("reviewer", body.reviewer);
  const card = writable(board, id, body.revision);
  if (body.reviewer === LOCAL || body.reviewer === AGENT) {
    throw refused(`reviewer: \`${body.reviewer}\` can be no reviewer: a person accepts a result`);
  }
  card.assignee = field(body.assignee, card.assignee);
  card.reviewer = field(body.reviewer, card.reviewer);
  return changed(board, card, "card.assigned", { assignee: card.assignee, reviewer: card.reviewer, as: body.as ?? LOCAL }, at);
}

/** `card_cancel`: out of its column at `position` 0, the column closing up. */
export function cancelCardIn(board: MockBoard, id: string, body: CardCancelBody, at: string): CardWrite {
  checkName(body.as);
  const card = writable(board, id, body.revision);
  const as = body.as ?? LOCAL;
  card.cancellation = { by: as, at, reason: body.reason };
  card.position = 0;
  renumber(standing(board, card.flow, card.column));
  return changed(board, card, "card.cancelled", { column: card.column, as }, at);
}

/** The verdict as asked: one of the two, a departure with its three texts; else the CLI's usage line. */
function checkVerdict(body: CardAcceptBody): void {
  const departing = (body.departs ?? []).length > 0;
  const texts = [body.summary, body.observed, body.documented];
  const usage = (line: string) => new ClientError({ status: CARD_USAGE, message: `spec: ${line}` });
  if (body.briefed === true && (departing || body.gap_type !== undefined || texts.some((text) => text !== undefined))) {
    throw usage("`spec card accept`: --briefed takes no --departs, nor its texts");
  }
  if (body.briefed !== true && !departing) {
    throw usage("`spec card accept` asks one of --briefed and --departs REF...");
  }
  if (departing && texts.some((text) => text === undefined)) {
    throw usage("`spec card accept`: --departs takes --summary, --observed and --documented");
  }
}

/**
 * `card_accept`: in `accept.from`, the name its reviewer, never its assignee; the rest of the core's
 * order. Either verdict freezes the snapshot anew, a correction's also that of the card it corrects;
 * `discrepancy` makes the item of a departure, in the same step.
 */
export function acceptCardIn(board: MockBoard, id: string, body: CardAcceptBody, at: string, discrepancy: (card: StoredCard) => string): CardWrite {
  checkName(body.as);
  checkVerdict(body);
  const card = writable(board, id, undefined);
  const flow = flowOf(board, card.flow);
  if (flow === undefined || card.column !== flow.accept.from) {
    throw refused(`${id} is in ${card.column}: \`accept\` needs ${flow?.accept.from ?? ""}; nothing changed`);
  }
  if (card.assignee === body.as) {
    throw refused(`${id}: \`${body.as}\` is its assignee, who never accepts a result; nothing changed`);
  }
  if (card.reviewer === null) {
    throw refused(`${id} has no reviewer: \`spec card assign ${id} --reviewer N\` names one; nothing changed`);
  }
  if (card.reviewer !== body.as) {
    throw refused(`${id}: its reviewer is \`${card.reviewer}\`, and \`--as\` says \`${body.as}\`; nothing changed`);
  }
  writable(board, id, body.revision);
  const departs = body.departs ?? [];
  const stranger = departs.findIndex((target) => !card.targets.includes(target));
  if (stranger >= 0) {
    throw refused(`departs[${String(stranger)}]: \`${departs[stranger] ?? ""}\` is no direct target of ${id}; nothing changed`);
  }
  const made = departs.length === 0 ? null : discrepancy(card);
  const from = card.column;
  const to = flow.accept.to;
  const left = standing(board, card.flow, from).filter((other) => other !== card);
  card.column = to;
  card.acceptance = { by: body.as, at, note: body.note, verdict: made === null ? "briefed" : "departs", departs, discrepancy: made };
  card.snapshot = card.targets.map((target) => frozenNow(board, target));
  const corrected = board.cards.find((candidate) => candidate.id === card.corrects);
  if (corrected !== undefined) {
    corrected.snapshot = corrected.targets.map((target) => frozenNow(board, target));
  }
  renumber(left);
  card.position = standing(board, card.flow, to).length;
  const answer = changed(board, card, "card.accepted", { from, to, position: card.position, verdict: card.acceptance.verdict, discrepancy: made, as: body.as }, at);
  return made === null ? answer : { ...answer, notes: [`${id} departs: its discrepancy is ${made}`] };
}

/** The direct targets `card` is suspect on: its frozen hash, or an `also`, is not what the node reads now, or the node is gone. */
function suspectOn(board: MockBoard, card: StoredCard): string[] {
  return card.targets.filter((target) => {
    const frozen = card.snapshot?.find((entry) => entry.id === target);
    if (frozen === undefined || compared(frozen) === null) {
      return false;
    }
    const now = hashNow(board, target);
    return board.gone.has(target) || (now !== compared(frozen) && !frozen.also.includes(now));
  });
}

/** The live correction of a card: one not closed that corrects it. */
function liveCorrection(board: MockBoard, id: string): string | null {
  return board.cards.find((card) => card.corrects === id && !closed(card))?.id ?? null;
}

/** Why a person cannot clear with `reason`: a reason, under a person's name. */
function clearProblem(as: string, reason: string): string | null {
  if (reason.trim() === "") {
    return "reason: blank; say why the mark clears";
  }
  return as === LOCAL || as === AGENT ? `--as: \`${as}\` clears no mark: a person answers one; nothing changed` : null;
}

/** Each suspect entry frozen again, and the entries as moved, `card.cleared`'s `nodes`. */
function cleared(board: MockBoard, card: StoredCard) {
  const suspect = suspectOn(board, card);
  const nodes = (card.snapshot ?? []).filter((entry) => suspect.includes(entry.id)).map((entry) => ({ id: entry.id, rev: 1, from: compared(entry), to: compared(frozenNow(board, entry.id)) }));
  card.snapshot = (card.snapshot ?? []).map((entry) => (suspect.includes(entry.id) ? frozenNow(board, entry.id) : entry));
  return nodes;
}

/**
 * `card_clear`: no effect freezes each suspect entry again; a needed correction, for an accepted card
 * alone, makes a new card with `corrects`, the targets, flow and discipline, first in that flow, and
 * leaves the original's snapshot, so its mark is held until the correction is accepted.
 */
export function clearCardIn(board: MockBoard, id: string, body: CardClearBody, at: string): CardWrite {
  const correcting = body.outcome === "needs_correction";
  if (correcting && body.estimate === undefined) {
    throw new ClientError({ status: CARD_USAGE, message: "spec: `spec card clear`: --needs-correction takes --estimate E" });
  }
  if (!correcting && (body.estimate !== undefined || body.title !== undefined)) {
    throw new ClientError({ status: CARD_USAGE, message: "spec: `spec card clear`: --no-effect takes no --estimate and no --title" });
  }
  checkName(body.as);
  const card = board.cards.find((candidate) => candidate.id === id);
  if (card === undefined) {
    throw new ClientError({ status: NO_SUCH_CARD, message: `no card ${id}` });
  }
  const problem = clearProblem(body.as, body.reason);
  if (problem !== null) {
    throw refused(problem);
  }
  if (card.cancellation !== null) {
    throw refused(`${id} is cancelled: it takes no write; a correction is a new card, \`spec card new --corrects ${id}\`; nothing changed`);
  }
  if (suspectOn(board, card).length === 0) {
    throw refused(`${id} is not suspect: there is no mark to clear; nothing changed`);
  }
  const held = liveCorrection(board, id);
  if (held !== null) {
    throw refused(`${id} reads correcting ${held}: its mark is held until that card is accepted or cancelled; nothing changed`);
  }
  if (body.revision !== card.revision) {
    throw refused(`\`${id}\` changed since this run read it: it is in ${card.column} since ${card.updated_at}; nothing changed`);
  }
  if (!correcting) {
    const nodes = cleared(board, card);
    return changed(board, card, "card.cleared", { outcome: "no_effect", nodes, correction: null, reason: body.reason, as: body.as }, at);
  }
  if (card.acceptance === null) {
    throw refused(`${id} is not accepted: a correction answers an accepted card; nothing changed`);
  }
  const first = flowOf(board, card.flow)?.columns[0]?.id ?? card.column;
  const made = createCardIn(board, { title: body.title ?? `Correct ${card.title}`, ...(card.discipline === null ? {} : { discipline: card.discipline }), target: card.targets, estimate: body.estimate, corrects: id, as: body.as }, at);
  const correction = board.cards.find((candidate) => candidate.id === made.id);
  if (correction !== undefined) {
    correction.flow = card.flow;
    correction.column = first;
    correction.snapshot = (card.snapshot ?? []).map((entry) => ({ ...entry }));
  }
  board.seq += 1;
  board.events.push({ id, seq: board.seq, type: "card.cleared", at, payload: { id, outcome: "needs_correction", nodes: [], correction: made.id, reason: body.reason, as: body.as } });
  return { ...writeOf(board, card), notes: [`${made.id ?? ""} corrects ${id}: ${card.flow}/${first}`] };
}

/** `card_clear_group`: every suspect card with a target in `held`, no effect, one event each. */
export function clearCardGroupIn(board: MockBoard, body: CardClearGroupBody, at: string, held: ReadonlySet<string>): CardClearGroupWrite {
  checkName(body.as);
  const problem = clearProblem(body.as, body.reason);
  if (problem !== null) {
    throw refused(problem);
  }
  const cards = board.cards.filter((card) => card.cancellation === null && card.targets.some((target) => held.has(target)) && suspectOn(board, card).length > 0 && liveCorrection(board, card.id) === null);
  if (cards.length === 0) {
    throw refused(`no suspect card under \`${body.group}\`: there is no mark to clear; nothing changed`);
  }
  for (const card of cards) {
    const nodes = cleared(board, card);
    changed(board, card, "card.cleared", { outcome: "no_effect", nodes, correction: null, reason: body.reason, as: body.as }, at);
  }
  return { group: body.group, cleared: cards.map((card) => card.id), notes: [] };
}

/** A card's mark over the targets `on`, first match: departed, suspect (correcting while a correction lives), accepted, open. */
export function markOver(board: MockBoard, card: StoredCard, on: readonly string[], settled: Settled): TraceCard {
  const { id, flow, column, title } = card;
  const row = { id, flow, column, title, on: null, discrepancy: null, correction: null };
  const { acceptance } = card;
  const departed = acceptance?.departs.find((target) => on.includes(target));
  if (acceptance !== null && departed !== undefined && (acceptance.discrepancy === null || !settled(acceptance.discrepancy))) {
    return { ...row, mark: "departed", on: departed, discrepancy: acceptance.discrepancy };
  }
  const suspect = suspectOn(board, card).find((target) => on.includes(target));
  if (suspect !== undefined) {
    const correction = liveCorrection(board, id);
    return correction === null ? { ...row, mark: "suspect", on: suspect } : { ...row, mark: "correcting", on: suspect, correction };
  }
  return { ...row, mark: acceptance === null ? "open" : "accepted" };
}

/** The targets gone a card is suspect on, as a read's note says them. */
export function goneUnder(board: MockBoard, card: StoredCard): string[] {
  return suspectOn(board, card).filter((target) => board.gone.has(target));
}

/** The `conflict` scenario: someone else got there first, once. */
export function changeElsewhere(board: MockBoard, id: string, at: string): void {
  const card = board.cards.find((candidate) => candidate.id === id);
  if (card !== undefined && !closed(card)) {
    changed(board, card, "card.edited", { fields: ["goal"], as: ELSEWHERE }, at);
  }
}

// A split (docs/canon/card-split.md "The rule";
// docs/features/department-split.md "Acceptance criteria", U2): the daemon's plan and write over the
// mock's board, in its words. The scope is each parent target and the nodes under it to depth 2, the
// tree read's rows, handed in by the client.

/** At most this many subtasks in one split, and this many targets a subtask takes from its kinds. */
const SPLIT_CHILDREN_MAX = 32;
const SPLIT_TARGETS_MAX = 64;

/** At most this many bytes in a flow's, a discipline's or a workspace's name, `MAX_NAME_BYTES`. */
const CONFIG_NAME_MAX_BYTES = 64;

/** Why a discipline is no name, `name_problem`'s words: 1 to 64 bytes of lower-case ASCII letters, digits, `_` and `-`; null when it is one. */
function nameProblem(name: string): string | null {
  const bytes = new TextEncoder().encode(name).length;
  if (bytes === 0 || bytes > CONFIG_NAME_MAX_BYTES) {
    return `\`${name}\` is ${String(bytes)} bytes long; a name has 1 to ${String(CONFIG_NAME_MAX_BYTES)}`;
  }
  return /^[a-z0-9_-]+$/.test(name) ? null : `\`${name}\` is not a name: lower-case ASCII letters, digits, \`_\` and \`-\``;
}

/** Why the disciplines a write names make no split, `chosen_problem`'s words: none, past 32, then each in order off a name or named twice. */
function chosenProblem(chosen: readonly string[]): string | null {
  if (chosen.length === 0) {
    return `child: none named; name 1 to ${String(SPLIT_CHILDREN_MAX)}`;
  }
  if (chosen.length > SPLIT_CHILDREN_MAX) {
    return `child: ${String(chosen.length)} named; a split makes 1 to ${String(SPLIT_CHILDREN_MAX)}`;
  }
  for (const [at, discipline] of chosen.entries()) {
    const problem = nameProblem(discipline);
    if (problem !== null) {
      return `child: ${problem}`;
    }
    if (chosen.slice(0, at).includes(discipline)) {
      return `child: \`${discipline}\` is named twice`;
    }
  }
  return null;
}

/**
 * A split's scope under one target, from the tree read's rows to depth 2 (`split_scope`): the root
 * named as the target, each other row by its ID, else, a document, by its path; an ID-less section is
 * passed over, and the mock's sections all hold an ID. Each with its kind.
 */
export function splitScopeOf(target: string, rows: readonly TreeNode[]): MockScopeNode[] {
  return rows.map((row) => ({ target: row.depth === 0 ? target : (row.id ?? row.path), kind: row.kind }));
}

/** The plan over a scope: the suggested disciplines, each child's flow, column and targets, the notes; or a refusal's reason. */
function splitPlanIn(board: MockBoard, scope: readonly MockScopeNode[], parentTargets: readonly string[], chosen: readonly string[] | null): { children: CardSplitChild[]; notes: string[] } | string {
  // Only what a write names is judged: a plan of every discipline the flows list is never capped.
  const problem = chosen === null ? null : chosenProblem(chosen);
  if (problem !== null) {
    return problem;
  }
  const matched = board.splits.filter((row) => scope.some((node) => node.kind === row.kind));
  const suggested = [...new Set(matched.flatMap((row) => row.disciplines))];
  const asked = chosen ?? [...new Set(board.flows.flatMap((flow) => flow.disciplines))];
  const notes: string[] = [];
  const children: CardSplitChild[] = [];
  for (const discipline of asked) {
    const flow = flowFor(board, discipline);
    if (typeof flow === "string") {
      return `child: ${flow}`;
    }
    const kinds = matched.filter((row) => row.disciplines.includes(discipline)).map((row) => row.kind);
    const found = scope.filter((node) => node.kind !== null && kinds.includes(node.kind)).map((node) => node.target);
    if (found.length > SPLIT_TARGETS_MAX) {
      notes.push(`subtask \`${discipline}\`: ${String(found.length)} nodes of its kinds; the first 64 taken`);
    }
    const byKinds = found.length > 0;
    children.push({
      id: null,
      discipline,
      flow: flow.name,
      flow_title: flow.title,
      column: flow.columns[0]?.id ?? "",
      targets: byKinds ? found.slice(0, SPLIT_TARGETS_MAX) : [...parentTargets],
      from: byKinds ? "kinds" : "parent",
      suggested: suggested.includes(discipline),
      mandate: null,
    });
  }
  return { children, notes };
}

/**
 * `split_mandates`: each `<discipline>=<mandate>` of a split, a discipline of its `--child` list once,
 * by discipline; else the refusal's line (docs/features/task-mandate.md "Acceptance criteria", M2).
 */
function splitMandates(given: readonly string[], children: readonly string[]): Map<string, Mandate> | string {
  const mandates = new Map<string, Mandate>();
  for (const entry of given) {
    const at = entry.indexOf("=");
    const [discipline, value] = at < 0 ? [entry, ""] : [entry.slice(0, at), entry.slice(at + 1)];
    const mandate = MANDATES.find((word) => word === value);
    if (mandate === undefined) {
      return `mandate: \`${entry}\` is not \`<discipline>=binding\` or \`<discipline>=open\``;
    }
    const problem = nameProblem(discipline);
    if (problem !== null) {
      return `mandate: ${problem}`;
    }
    if (!children.includes(discipline)) {
      return `mandate: \`${discipline}\` is no --child of this split`;
    }
    if (mandates.has(discipline)) {
      return `mandate: \`${discipline}\` is named twice`;
    }
    mandates.set(discipline, mandate);
  }
  return mandates;
}

/** A card a split makes, last in its flow's first column. */
function splitCard(board: MockBoard, fields: { flow: CardFlow; title: string; goal: string | null; discipline: string | null; targets: readonly string[]; reviewer: string; mandate: Mandate | null; parent: string | null; as: string }, at: string): StoredCard {
  const column = fields.flow.columns[0]?.id ?? "";
  const card: StoredCard = {
    id: nextId(board),
    flow: fields.flow.name,
    column,
    position: standing(board, fields.flow.name, column).length + 1,
    title: fields.title,
    goal: fields.goal,
    discipline: fields.discipline,
    targets: [...fields.targets],
    snapshot: fields.targets.map((target) => frozenNow(board, target)),
    criteria: [],
    assignee: null,
    reviewer: fields.reviewer,
    mandate: fields.mandate,
    estimate: null,
    due: null,
    parent: fields.parent,
    corrects: null,
    evidence: [],
    tasks: [],
    acceptance: null,
    cancellation: null,
    created_by: fields.as,
    created_at: at,
    updated_at: at,
    revision: "1",
  };
  board.cards.push(card);
  board.seq += 1;
  board.events.push({ id: card.id, seq: board.seq, type: "card.created", at, payload: { id: card.id, flow: card.flow, column, position: card.position, assignee: null, reviewer: card.reviewer, mandate: card.mandate, as: fields.as } });
  return card;
}

/**
 * `card_split`: the usage rules (400), the stored parent (409 `parent: no card C`), the plan (409, a
 * child's line), the reviewer's chain on a write (409), then the parent and each child in one go. A
 * dry run writes nothing and judges no person. `meetsConflict`, asked of a write that passed every
 * rule: true when it meets a config changed elsewhere, its first child's flow gone, and stores
 * nothing. A stored parent is read, never written.
 */
export function splitCardsIn(board: MockBoard, body: CardSplitBody, scopeOf: (target: string) => MockScopeNode[], at: string, meetsConflict: () => boolean): CardSplit {
  checkName(body.as);
  checkPerson("reviewer", body.reviewer);
  const children = body.child === null || body.child.length === 0 ? null : body.child;
  if (body.parent !== null && ((body.target ?? []).length > 0 || body.title !== null || body.goal !== null || body.discipline !== null)) {
    throw new ClientError({ status: CARD_USAGE, message: "spec: --parent C splits under a stored card: give no --target, --title, --goal or --discipline with it" });
  }
  if (body.parent === null && body.title === null) {
    throw new ClientError({ status: CARD_USAGE, message: "spec: a split makes a parent card: give --title T, or --parent C to split under a stored card" });
  }
  if (children === null && !body.dry_run) {
    throw new ClientError({ status: CARD_USAGE, message: "spec: name each subtask's discipline with --child, or plan with --dry-run" });
  }
  const given = body.mandate ?? [];
  if (given.length > 0 && children === null) {
    throw new ClientError({ status: CARD_USAGE, message: "spec: --mandate names a subtask: give its --child too" });
  }
  if (body.title !== null && body.title.trim() === "") {
    throw refused("title: blank; a card has a title");
  }
  // At step 2's fields, a dry run's too: each mandate a discipline of the children once.
  const mandates = splitMandates(given, children ?? []);
  if (typeof mandates === "string") {
    throw refused(mandates);
  }
  const stored = body.parent === null ? null : (board.cards.find((card) => card.id === body.parent) ?? null);
  if (body.parent !== null && stored === null) {
    throw refused(`parent: no card ${body.parent}`);
  }
  const as = body.as ?? LOCAL;
  const reviewer = body.reviewer ?? stored?.reviewer ?? body.as;
  if (!body.dry_run) {
    const named = reviewer ?? LOCAL;
    if (named === CLEAR) {
      throw refused(`reviewer: \`${CLEAR}\` clears a name: a new card has none`);
    }
    if (named === LOCAL || named === AGENT) {
      throw refused(`reviewer: \`${named}\` can be no reviewer: a person accepts a result`);
    }
  }
  const targets = stored?.targets ?? body.target ?? [];
  // Each node once, at its first place.
  const scope: MockScopeNode[] = [];
  for (const node of targets.flatMap(scopeOf)) {
    if (!scope.some((held) => held.target === node.target)) {
      scope.push(node);
    }
  }
  const planned = splitPlanIn(board, scope, targets, children);
  if (typeof planned === "string") {
    throw refused(planned);
  }
  // Each child its own mandate, by discipline, never by place; one named by none holds none.
  const plan = { ...planned, children: planned.children.map((child) => ({ ...child, mandate: mandates.get(child.discipline) ?? null })) };
  const parentFlow = stored === null ? flowFor(board, body.discipline) : (flowOf(board, stored.flow) ?? `no flow \`${stored.flow}\``);
  if (typeof parentFlow === "string") {
    throw refused(parentFlow);
  }
  if (!body.dry_run && meetsConflict()) {
    throw refused(`child: no flow takes the discipline \`${plan.children[0]?.discipline ?? ""}\`: none lists it, and every \`[[card.flow]]\` lists its disciplines`);
  }
  const shape = (card: { id: string | null; title: string; discipline: string | null; flow: string; column: string; targets: string[] }, created: boolean): CardSplitParent => ({
    id: card.id,
    created,
    title: card.title,
    discipline: card.discipline,
    flow: card.flow,
    flow_title: flowOf(board, card.flow)?.title ?? card.flow,
    column: card.column,
    targets: [...card.targets],
  });
  if (body.dry_run) {
    const parent = stored ?? { id: null, title: body.title ?? "", discipline: body.discipline, flow: parentFlow.name, column: parentFlow.columns[0]?.id ?? "", targets: [...targets] };
    return { dry_run: true, parent: shape({ ...parent, id: null }, stored === null), children: plan.children, reviewer: reviewer ?? null, notes: plan.notes };
  }
  const named = reviewer ?? LOCAL;
  const parent =
    stored ??
    splitCard(board, { flow: parentFlow, title: body.title ?? "", goal: body.goal, discipline: body.discipline, targets, reviewer: named, mandate: null, parent: null, as }, at);
  const made = plan.children.map((child) => {
    const flow = flowOf(board, child.flow) ?? parentFlow;
    const card = splitCard(board, { flow, title: parent.title, goal: parent.goal, discipline: child.discipline, targets: child.targets, reviewer: named, mandate: child.mandate, parent: parent.id, as }, at);
    return { ...child, id: card.id };
  });
  return { dry_run: false, parent: shape(parent, stored === null), children: made, reviewer: named, notes: plan.notes };
}

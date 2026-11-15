import type { FeedAuto, FeedDocument, FeedEntry, FeedGateObserved, FeedHeld, FeedNeedsMe, FeedOpen, FeedFailedRun, FeedUnlanded, RunOutcome } from "../api/types";
import type { FeedOptions } from "../api/client";
import { fakeHex, proposal, stamp, type MockProposal } from "./build";
import type { MockBoard } from "./cards";
import type { MockTasks } from "./tasks";

// The feed of the mock projects (docs/features/auto-feed.md "Data"), served as `spec feed --json`
// would serve it: entries oldest first, `since` a seq, the newest 100 without it, at most 512, the
// task, department and Needs me cuts as the spec words them. harbor-sim had a night under auto mode:
// pack 812 started at 22:00 on four agent tasks and one line; T-0104 ran, its update and create
// applied by rule (PR-0035, PR-0038, each with `delegated`), its land held once by the check and
// landed after; T-0107's approval held for an excluded kind; T-0109 claimed, then a usage limit
// paused the runner for three hours (no event: the runner logs it alone), then its run failed and
// was held; T-0112 approved by rule, its run held past the pack's `--until`; an agent wrote outside
// its zones once; PR-0051 held for an excluded zone. ledger-api has no `[auto]` table: its feed lists
// the manual events and says why auto is off. Times are relative to the mock's clock.

/** The newest entries served without `since`, and the most one read serves (`spec feed`'s `FEED_DEFAULT`, `FEED_MAX`). */
export const FEED_DEFAULT = 100;
export const FEED_MAX = 512;

/** harbor-sim's rule hash: a stable fake `b3:`, not BLAKE3. */
export const HARBOR_RULE = `b3:${fakeHex("harbor-sim auto rule", 64)}`;

/** The delegated applies of the night, applied rows with `delegated` (docs/features/hub-auto.md "Description and interactions" 4). */
export const DELEGATED_IDS = ["PR-0035", "PR-0038"] as const;

/** The pack of the night. */
const PACK = 812;

const HARBOR_AUTO: FeedAuto = { state: "on", reason: null, why: null, rule: HARBOR_RULE };

const LEDGER_AUTO: FeedAuto = { state: "off", reason: "off", why: "no [auto] table in specengine.toml@HEAD", rule: null };

/** A commit of the night, stable for its text. */
function commitOf(text: string): string {
  return fakeHex(`commit ${text}`, 40);
}

/** One night's event, its keys as an entry's but `title`, `status` and `lane`, which the queue gives now. */
export interface NightEvent {
  seq: number;
  /** Minutes before the mock's clock. */
  ago: number;
  type: string;
  task?: string;
  proposal?: string;
  card?: string;
  line?: string;
  by?: "rule";
  outcome?: RunOutcome;
  summary?: string;
  commit?: string;
  reason?: string;
  detail?: string;
  path?: string;
  /** auto.held's act, for Needs me. */
  act?: string;
  /** gate.observed's zone and branch, for Needs me. */
  zone?: string | null;
  branch?: string | null;
  /** task.run_reported's run. */
  run?: number;
}

/** harbor-sim's events: an afternoon by hand, then the night. */
function harborEvents(): NightEvent[] {
  return [
    { seq: 1181, ago: 1185, type: "card.moved", card: "C-0003" },
    { seq: 1182, ago: 1170, type: "proposal.created", proposal: "PR-0051" },
    { seq: 1183, ago: 1100, type: "task.planned", task: "T-0107" },
    { seq: 1184, ago: 1020, type: "proposal.staged", proposal: "PR-0042" },
    { seq: 1185, ago: 700, type: "card.edited", card: "C-0001" },
    { seq: 1186, ago: 600, type: "auto.started" },
    { seq: 1187, ago: 599, type: "auto.bound", task: "T-0113", line: `${String(PACK)}/1` },
    { seq: 1188, ago: 598, type: "task.approved", task: "T-0104", by: "rule" },
    {
      seq: 1189,
      ago: 597,
      type: "auto.held",
      task: "T-0107",
      act: "approve",
      reason: "excluded-kind",
      detail: "`SC-OUTER-ANCHORAGE` has the kind scenario, which the rule excludes",
    },
    { seq: 1190, ago: 596, type: "task.claimed", task: "T-0104" },
    { seq: 1191, ago: 594, type: "task.approved", task: "T-0109", by: "rule" },
    { seq: 1192, ago: 593, type: "task.claimed", task: "T-0109" },
    { seq: 1193, ago: 560, type: "task.run_reported", task: "T-0104", run: 2, outcome: "completed", summary: "Crew counted per berth again; mooring waits for four at night too." },
    { seq: 1194, ago: 559, type: "proposal.approved", task: "T-0104", proposal: "PR-0035", by: "rule" },
    { seq: 1195, ago: 559, type: "proposal.applied", task: "T-0104", proposal: "PR-0035", by: "rule", commit: commitOf("PR-0035") },
    { seq: 1196, ago: 558, type: "proposal.approved", task: "T-0104", proposal: "PR-0038", by: "rule" },
    { seq: 1197, ago: 558, type: "proposal.applied", task: "T-0104", proposal: "PR-0038", by: "rule", commit: commitOf("PR-0038") },
    { seq: 1198, ago: 557, type: "task.completed", task: "T-0104" },
    {
      seq: 1199,
      ago: 556,
      type: "auto.held",
      task: "T-0104",
      act: "land",
      reason: "check",
      detail: "spec check: 1 error(s); the first docs/spec/berths/mooring.md:12: ref-dangling",
    },
    { seq: 1200, ago: 551, type: "gate.observed", path: "src/sim/tide_clock.rs", zone: "code", branch: "auto-T-0109" },
    // A usage limit paused the runner here for three hours: the runner's log says so, the queue has no event.
    { seq: 1201, ago: 365, type: "task.run_reported", task: "T-0109", run: 2, outcome: "failed", summary: "The spring tide table has no entry for berth 7; the loader stops there." },
    { seq: 1202, ago: 364, type: "auto.held", task: "T-0109", act: "run", reason: "not-done", detail: "run 2 ended failed: the agent reported the task not done" },
    { seq: 1203, ago: 362, type: "task.approved", task: "T-0112", by: "rule" },
    { seq: 1204, ago: 361, type: "auto.held", proposal: "PR-0051", act: "apply", reason: "excluded-zone", detail: "docs/spec/mechanics/cargo-cranes.md lies in the zone narrative, which the rule excludes" },
    { seq: 1205, ago: 300, type: "auto.landed", task: "T-0104", commit: commitOf("land T-0104") },
    { seq: 1206, ago: 241, type: "auto.held", task: "T-0112", act: "run", reason: "until", detail: "the pack's --until passed: no leg starts after it" },
  ];
}

/** ledger-api's events: by hand, no rule. */
function ledgerEvents(): NightEvent[] {
  return [
    { seq: 301, ago: 2900, type: "task.created", task: "T-0033" },
    { seq: 302, ago: 2880, type: "task.claimed", task: "T-0031" },
    { seq: 303, ago: 1500, type: "task.planned", task: "T-0033" },
    { seq: 304, ago: 1440, type: "task.run_reported", task: "T-0031", run: 1, outcome: "partial", summary: "Refund window enforced for cards; bank transfers still at 30 days." },
  ];
}

/** The lane of each night's agent task (`docs/features/auto-land.md` "Lanes"): T-0112's targets lie under the Galley's berths module. */
const LANES: Readonly<Record<string, string>> = { "T-0112": "galley" };

/** The delegated rows of the night, applied by the rule: an update and a create on T-0104. */
export function delegatedProposals(slug: string, at: number): MockProposal[] {
  const decided = stamp(at, 559);
  const rows = [
    { id: "PR-0035", kind: "update", target: "RULE-MOOR-CREW", summary: null },
    { id: "PR-0038", kind: "create", target: "MEC-MOORING", summary: null },
  ];
  return rows.map((row) => {
    const commit = commitOf(row.id);
    const made = proposal({
      id: row.id,
      project: slug,
      kind: row.kind,
      status: "applied",
      target_id: row.target,
      target_ids: [row.target],
      target_path: "docs/spec/berths/mooring.md",
      task_id: "T-0104",
      decided_by: "Mira Holt <mira@harbor.example.invalid>",
      decided_at: decided,
      applied_commit: commit,
      created_at: stamp(at, 562),
      updated_at: decided,
    });
    made.review.delegated = { by: "rule", rule: HARBOR_RULE, commit };
    return made;
  });
}

/** What the feed's reader needs of a project now: its tasks, its cards and its open proposals. */
export interface FeedSources {
  tasks: MockTasks;
  board: MockBoard | undefined;
  proposals: readonly MockProposal[];
}

/** An event as an entry: the queue's title, status and lane now. */
function entryOf(event: NightEvent, at: number, sources: FeedSources): FeedEntry {
  const task = event.task === undefined ? undefined : sources.tasks.tasks.find((row) => row.id === event.task);
  const card = event.card === undefined ? undefined : sources.board?.cards.find((row) => row.id === event.card);
  const raised = event.proposal === undefined ? undefined : sources.proposals.find((row) => row.review.id === event.proposal);
  const title = task?.title ?? card?.title ?? raised?.review.target_id ?? null;
  const status = task?.status ?? (event.task === undefined ? (raised?.review.status ?? null) : null);
  return {
    seq: event.seq,
    at: stamp(at, event.ago),
    type: event.type,
    task: event.task ?? null,
    proposal: event.proposal ?? null,
    card: event.card ?? null,
    line: event.line ?? null,
    title,
    status,
    by: event.by ?? null,
    outcome: event.outcome ?? null,
    summary: event.summary ?? null,
    commit: event.commit ?? null,
    reason: event.reason ?? null,
    detail: event.detail ?? null,
    path: event.path ?? null,
    lane: event.task === undefined ? null : (LANES[event.task] ?? "general"),
  };
}

/** Whether an event belongs to `task`: its own, or one of a proposal bound to it. */
function namesTask(event: NightEvent, task: string, sources: FeedSources): boolean {
  if (event.task === task) {
    return true;
  }
  const raised = event.proposal === undefined ? undefined : sources.proposals.find((row) => row.review.id === event.proposal);
  return raised?.review.task_id === task;
}

/** Needs me over the events: the holds never landed since, the open proposals, the window's failed runs and gate writes, the pack's unlanded tasks. */
function needsOf(events: readonly NightEvent[], window: readonly NightEvent[], at: number, sources: FeedSources): FeedNeedsMe {
  const last = new Map<string, NightEvent>();
  for (const event of events.filter((candidate) => candidate.type.startsWith("auto.") && candidate.type !== "auto.started")) {
    last.set(event.task ?? event.proposal ?? event.line ?? "", event);
  }
  const held: FeedHeld[] = [...last.values()]
    .filter((event) => event.type === "auto.held")
    .map((event) => ({
      seq: event.seq,
      at: stamp(at, event.ago),
      pack: PACK,
      task: event.task ?? null,
      line: event.task === undefined && event.proposal === undefined ? (event.line ?? null) : null,
      proposal: event.proposal ?? null,
      act: event.act ?? "",
      reason: event.reason ?? "",
      detail: event.detail ?? null,
    }));
  const open: FeedOpen[] = sources.proposals
    .filter(({ review }) => review.status === "open")
    .map(({ review }) => ({ id: review.id, kind: review.kind, target_id: review.target_id, task_id: review.task_id, created_at: review.created_at }))
    .sort((a, b) => a.id.localeCompare(b.id, "en", { numeric: true }));
  const failed_runs: FeedFailedRun[] = window
    .filter((event) => event.type === "task.run_reported" && event.outcome !== undefined && event.outcome !== "completed")
    .map((event) => ({ task: event.task ?? "", run: event.run ?? 1, outcome: event.outcome ?? "failed", summary: event.summary ?? null, ended_at: stamp(at, event.ago) }));
  const gate_observed: FeedGateObserved[] = window
    .filter((event) => event.type === "gate.observed")
    .map((event) => ({ seq: event.seq, at: stamp(at, event.ago), path: event.path ?? "", zone: event.zone ?? null, branch: event.branch ?? null }));
  const unlanded: FeedUnlanded[] = sources.tasks.tasks
    .filter((row) => row.status === "done" && events.some((event) => event.type === "task.approved" && event.task === row.id && event.by === "rule") && !last.has(row.id))
    .map((row) => ({ task: row.id, status: row.status, pack: PACK }));
  return { held, open, failed_runs, gate_observed, unlanded };
}

/**
 * The `feed` scenario's morning on harbor-sim, after the night: one event at a time on the live tail
 * (MockClient.subscribe), each a few seconds after the last, so the view appends it and marks it New.
 */
export const MORNING: readonly Omit<NightEvent, "ago">[] = [
  { seq: 1207, type: "task.claimed", task: "T-0112" },
  { seq: 1208, type: "task.run_reported", task: "T-0112", run: 1, outcome: "completed", summary: "Berth 4's draft now comes from the new table." },
  { seq: 1209, type: "task.completed", task: "T-0112" },
  { seq: 1210, type: "auto.landed", task: "T-0112", commit: commitOf("land T-0112") },
];

/** A live event's payload as the tail carries it: the keys the feed reads, enough for a re-read. */
export function payloadOf(event: Omit<NightEvent, "ago">): Record<string, unknown> {
  return event.type.startsWith("task.") ? { id: event.task, run: event.run ?? null } : { pack: PACK, task: event.task ?? null, commit: event.commit ?? null };
}

/** The project's feed as `spec feed --json` serves it for the options asked; an empty queue for a project with no events; `live` the events the tail added since. */
export function feedOf(slug: string, at: number, sources: FeedSources, options: FeedOptions, empty: boolean, live: readonly NightEvent[] = []): FeedDocument {
  const events = empty ? [] : [...(slug === "harbor-sim" ? harborEvents() : ledgerEvents()), ...live];
  const departments = slug === "harbor-sim" ? ["galley", "foundry", "harbourmaster", "general"] : ["general"];
  const notes: string[] = [];
  let kept = events.filter((event) => options.task === undefined || namesTask(event, options.task, sources));
  if (options.department !== undefined) {
    if (!departments.includes(options.department)) {
      notes.push(`department \`${options.department}\` is not declared: no entry`);
      kept = [];
    } else {
      kept = kept.filter((event) => event.task !== undefined && (LANES[event.task] ?? "general") === options.department);
    }
  }
  const since = options.since;
  const above = since === undefined ? kept.slice(-FEED_DEFAULT) : kept.filter((event) => event.seq > since);
  const window = above.slice(0, FEED_MAX);
  const entries = window.map((event) => entryOf(event, at, sources));
  return {
    auto: slug === "harbor-sim" ? HARBOR_AUTO : LEDGER_AUTO,
    after: since ?? null,
    last_seq: entries.at(-1)?.seq ?? since ?? null,
    more: above.length > window.length,
    entries,
    needs_me: options.needs_me === true ? needsOf(events, window, at, sources) : null,
    notes,
  };
}

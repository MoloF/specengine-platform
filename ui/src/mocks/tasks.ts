import type {
  Author,
  SnapshotDiff,
  SnapshotNode,
  TaskAssumption,
  TaskList,
  TaskNotFound,
  TaskPackage,
  TaskProposal,
  TaskRun,
  TaskTarget,
} from "../api/types";
import type { MockProposal, StoredReview } from "./build";

// The tasks of the mock projects (docs/features/ui-tasks.md "Data", Mock): the draft package of
// docs/features/task-package.md as `spec task list --json` and `spec task show T --json` print it.
// One builder, fixed UTC times (never the clock), no word of a tool chain or of this repository's
// roles: role and profile values are project vocabulary, shown verbatim.
//
// harbor-sim holds the seven states a task reaches: T-0104 done, T-0105 cancelled, T-0107 in plan
// review with two bound proposals, T-0108 ready with its spec changed, T-0109 in progress with an
// open run and a cut diff, T-0110 sent back with an owner note, T-0111 ready with its approval
// place gone, T-0112 ready and unchanged, T-0113 a draft without a title; the list skips T-0106,
// an unreadable row. ledger-api: T-0031 in progress, T-0033 in plan review, both with a profile.
// `large` adds T-0200 to T-0519, the seven states in turn, T-0200 at every cap of the package;
// each other one in progress claimed with run 1 open, as T-0109.

/** A task as the mock stores it: the package less what the core computes per read from the queue. */
export type StoredTask = Omit<TaskPackage, "open_proposals" | "assumptions">;

/** One project's task rows and the list's own notes (a skipped row). */
export interface MockTasks {
  tasks: StoredTask[];
  notes: string[];
}

const pad = (value: number, width = 2) => String(value).padStart(width, "0");

/** A stored UTC time in October 2026, from fixed numbers only. */
function utc(day: number, hour: number, minute = 0): string {
  return `2026-10-${pad(day)}T${pad(hour)}:${pad(minute)}:00Z`;
}

/** A fake `b3:` hash, stable for its text (FNV-1a, repeated): not BLAKE3. */
function fakeHash(text: string): string {
  let hash = 0x811c9dc5;
  let out = "";
  for (let round = 0; out.length < 64; round += 1) {
    for (const char of `${text}:${String(round)}`) {
      hash ^= char.charCodeAt(0);
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    out += hash.toString(16).padStart(8, "0");
  }
  return `b3:${out.slice(0, 64)}`;
}

function commitOf(text: string): string {
  return fakeHash(`commit ${text}`).slice(3, 43);
}

/** The one builder: a whole stored task in the package's key order; omitted keys null or empty. */
export function task(fields: Pick<StoredTask, "id" | "project" | "status" | "created_at"> & Partial<StoredTask>): StoredTask {
  return {
    schema_version: fields.schema_version ?? 1,
    id: fields.id,
    project: fields.project,
    status: fields.status,
    title: fields.title ?? null,
    goal: fields.goal ?? null,
    profile: fields.profile ?? null,
    stale: fields.stale ?? null,
    targets: fields.targets ?? [],
    criteria: fields.criteria ?? [],
    affected_nodes: fields.affected_nodes ?? [],
    plan: fields.plan ?? null,
    owner_notes: fields.owner_notes ?? [],
    bindings: fields.bindings ?? [],
    spec_snapshot: fields.spec_snapshot ?? null,
    snapshot_diff: fields.snapshot_diff ?? null,
    claim: fields.claim ?? null,
    runs: fields.runs ?? [],
    bundle: fields.bundle ?? null,
    author: fields.author ?? null,
    created_at: fields.created_at,
    updated_at: fields.updated_at ?? fields.created_at,
    notes: fields.notes ?? [],
  };
}

function target(id: string | null, path: string | null, kind: string | null, title: string | null): TaskTarget {
  return { id, path, kind, title };
}

function snapshotNode(id: string, path: string): SnapshotNode {
  return { id, path, span_hash: fakeHash(`${id} at approval`) };
}

function snapshotOf(at: string, worktree: string, branch: string, nodes: SnapshotNode[]) {
  return { at, place: { worktree, root_rel: "", branch, commit: commitOf(`${worktree} ${at}`) }, nodes };
}

function changeOf(node: SnapshotNode, hunks: string[], cut = false): SnapshotDiff {
  const diff = [`--- snapshot ${node.path}`, `+++ current ${node.path}`, ...hunks].join("\n") + "\n";
  return { id: node.id, path: node.path, span_hash: node.span_hash, diff, cut };
}

/** A run as `report` closes it, or open (`ended_at` null). */
function run(fields: Pick<TaskRun, "run" | "role" | "started_at"> & Partial<TaskRun>): TaskRun {
  return {
    run: fields.run,
    role: fields.role,
    started_at: fields.started_at,
    ended_at: fields.ended_at ?? null,
    outcome: fields.outcome ?? null,
    summary: fields.summary ?? null,
    changed_files: fields.changed_files ?? [],
  };
}

const OWNER: Author = { type: "human", role: null, model: null, run: null };

function agent(role: string, runId: string): Author {
  return { type: "agent", role, model: "claude-opus-5-5", run: runId };
}

// harbor-sim

const HARBOR = "harbor-sim";
const TIDE_FILE = "docs/spec/tides/tide-cycle.md";
const DRAFT_FILE = "docs/spec/berths/draft-limits.md";
const NIGHT_FILE = "docs/spec/fairway/night.md";
const TABLES_FILE = "docs/spec/tides/tide-tables.md";
const PILOT_FILE = "docs/spec/pilotage/boarding.md";
const BERTHS_FILE = "docs/spec/berths/README.md";
const SIM_CODER = "sim-coder";

const T0107_PLAN = [
  "## Plan",
  "",
  "1. Read the tide window from MEC-TIDES#RULE-TIDE-WINDOW in the entry system.",
  "2. Hold deep ships at the outer anchorage until the window opens:",
  "   - ships over 11 m draft only;",
  "   - log each hold with the expected window.",
  "3. Probe: a 12 m ship arriving at low water waits, then enters 90 minutes before high water.",
  "",
  "Out of scope: berth assignment (DOM-BERTHS).",
].join("\n");

const T0110_PLAN = [
  "1. Dispatch a pilot boat when a ship over 120 m reaches the approach.",
  "2. Board at the fairway buoy; the ship waits outside the fairway until then.",
  "",
  "Open: does one boat serve both basins?",
].join("\n");

/** Tide-table rows as the diff of T-0109 changes them: a new column per row, cut at 8 192 B. */
function tableDiff(node: SnapshotNode): SnapshotDiff {
  const lines = [`--- snapshot ${node.path}`, `+++ current ${node.path}`, "@@ -12,140 +12,140 @@"];
  for (let day = 1; day <= 140; day += 1) {
    const date = `2026-${pad(1 + Math.floor((day - 1) / 28))}-${pad(1 + ((day - 1) % 28))}`;
    lines.push(`-| ${date} | HW ${pad((3 + day) % 24)}:12 | LW ${pad((9 + day) % 24)}:25 |`);
    lines.push(`+| ${date} | HW ${pad((3 + day) % 24)}:12 | LW ${pad((9 + day) % 24)}:25 | slack ${pad(day % 60)} min |`);
  }
  let diff = "";
  for (const line of lines) {
    if (diff.length + line.length + 1 > 8192) {
      break;
    }
    diff += `${line}\n`;
  }
  return { id: node.id, path: node.path, span_hash: node.span_hash, diff, cut: true };
}

function harborTasks(): MockTasks {
  const tideWindow = target("RULE-TIDE-WINDOW", TIDE_FILE, "rule", "Entry only inside the tide window");
  const berthDraft = target("RULE-BERTH-DRAFT", DRAFT_FILE, "rule", "Draft limit at a berth");

  const t0104Nodes = [snapshotNode("MEC-MOORING", "docs/spec/berths/mooring.md"), snapshotNode("RULE-MOOR-CREW", "docs/spec/berths/mooring.md")];
  const t0108Passage = snapshotNode("MEC-NIGHT-PASSAGE", NIGHT_FILE);
  const t0108Lights = snapshotNode("RULE-NIGHT-LIGHTS", NIGHT_FILE);
  const t0109Tides = snapshotNode("MEC-TIDES", TIDE_FILE);
  const t0109Tables = snapshotNode("MEC-TIDE-TABLES", TABLES_FILE);
  const t0111Nodes = [snapshotNode("RULE-SPRING-WINDOW", "docs/spec/tides/spring-window.md")];
  const t0112Nodes = [snapshotNode("RULE-BERTH-DRAFT", DRAFT_FILE), snapshotNode("DOM-BERTHS", BERTHS_FILE)];
  const goneNote = "snapshot place /work/harbor-sim/T-0111 is gone (worktree removed); stale unknown";

  return {
    notes: ["T-0106: unreadable row (bad JSON in criteria); skipped"],
    tasks: [
      task({
        id: "T-0104",
        project: HARBOR,
        status: "done",
        title: "Mooring crew of four",
        goal: "Mooring takes a crew of four and 20 simulated minutes.",
        stale: false,
        targets: [
          target("MEC-MOORING", "docs/spec/berths/mooring.md", "mechanic", "Mooring"),
          target("RULE-MOOR-CREW", "docs/spec/berths/mooring.md", "rule", "Mooring crew"),
        ],
        criteria: [{ ref: null, text: "A berth with fewer than four free crew keeps the ship waiting at anchor." }],
        plan: "1. Count free crew per berth.\n2. Start mooring only with four.",
        spec_snapshot: snapshotOf(utc(1, 9, 30), "/work/harbor-sim/T-0104", "task/T-0104", t0104Nodes),
        snapshot_diff: [],
        claim: { at: utc(1, 10), role: SIM_CODER, worktree: "/work/harbor-sim/T-0104", branch: "task/T-0104" },
        runs: [
          run({
            run: 1,
            role: SIM_CODER,
            started_at: utc(1, 10),
            ended_at: utc(1, 15, 40),
            outcome: "completed",
            summary: "Crew counted per berth; mooring starts with four.",
            changed_files: ["src/sim/mooring.rs", "tests/mooring_crew.rs"],
          }),
        ],
        bundle: { node_ids: ["MEC-MOORING", "RULE-MOOR-CREW"], budget: 10000, bundle_hash: fakeHash("bundle T-0104") },
        author: OWNER,
        created_at: utc(1, 8),
        updated_at: utc(1, 16, 5),
      }),
      task({
        id: "T-0105",
        project: HARBOR,
        status: "cancelled",
        title: "Restore the old quays",
        goal: "Bring the archived quays back into the simulation.",
        targets: [target("MEC-OLD-QUAYS", "docs/spec/archive/old-quays.md", "mechanic", "Old quays")],
        author: OWNER,
        created_at: utc(1, 11),
        updated_at: utc(2, 9, 15),
      }),
      task({
        id: "T-0107",
        project: HARBOR,
        status: "review",
        title: "Queue deep ships at the outer anchorage",
        goal: "Deep ships wait at the outer anchorage until the tide window opens, then enter in arrival order.",
        targets: [tideWindow, berthDraft],
        criteria: [
          {
            ref: "RULE-TIDE-WINDOW",
            text: "A ship with more than 11 m draft enters only from 90 minutes before\nto 60 minutes after high water.",
          },
          { ref: null, text: "Ships held at the anchorage keep their arrival order." },
        ],
        affected_nodes: ["MEC-TIDES"],
        plan: T0107_PLAN,
        author: agent("harbour-planner", "R-2201"),
        created_at: utc(3, 8, 20),
        updated_at: utc(5, 17, 45),
      }),
      task({
        id: "T-0108",
        project: HARBOR,
        status: "ready",
        title: "Speed limit on night passages",
        goal: "Ships in the fairway at night keep to the night speed limit.",
        stale: true,
        targets: [
          target("MEC-NIGHT-PASSAGE", NIGHT_FILE, "mechanic", "Night passage"),
          target("RULE-NIGHT-LIGHTS", NIGHT_FILE, null, null),
        ],
        criteria: [{ ref: null, text: "A ship over the night limit slows down before the first buoy." }],
        plan: "1. Read the night limit.\n2. Slow ships down at the first buoy.",
        spec_snapshot: snapshotOf(utc(4, 9), "/work/harbor-sim", "main", [t0108Passage, t0108Lights]),
        snapshot_diff: [
          changeOf(t0108Passage, [
            "@@ -5,3 +5,4 @@",
            " Night passages run from sunset to sunrise.",
            "-Ships keep to 6 knots in the fairway.",
            "+Ships keep to 5 knots in the fairway.",
            "+Tugs keep to 8 knots.",
          ]),
          changeOf(t0108Lights, [
            "@@ -1,4 +0,0 @@",
            "-## RULE-NIGHT-LIGHTS: Lights at night",
            "-",
            "-Every buoy of the fairway is lit from sunset to sunrise.",
            "-A dark buoy closes the fairway until it is lit again.",
          ]),
        ],
        author: OWNER,
        created_at: utc(3, 14),
        updated_at: utc(4, 9),
      }),
      task({
        id: "T-0109",
        project: HARBOR,
        status: "in_progress",
        title: "One loader for both tide-table formats",
        goal: "The tide-table loader reads the documented format; the sample format is decided by the owner.",
        stale: true,
        targets: [target("MEC-TIDES", TIDE_FILE, "mechanic", "Tide cycle"), target("MEC-TIDE-TABLES", TABLES_FILE, "mechanic", "Tide tables")],
        criteria: [{ ref: "MEC-TIDE-TABLES", text: "The tide table of a scenario lists high and low water per day." }],
        plan: "1. Read the documented format.\n2. Keep the sample format behind a flag until PR-0045 is decided.",
        spec_snapshot: snapshotOf(utc(2, 13), "/work/harbor-sim/T-0109", "task/T-0109", [t0109Tides, t0109Tables]),
        snapshot_diff: [tableDiff(t0109Tables)],
        claim: { at: utc(2, 14), role: SIM_CODER, worktree: "/work/harbor-sim/T-0109", branch: "task/T-0109" },
        runs: [run({ run: 1, role: SIM_CODER, started_at: utc(2, 14) })],
        bundle: { node_ids: ["MEC-TIDES", "MEC-TIDE-TABLES"], budget: 10000, bundle_hash: fakeHash("bundle T-0109") },
        author: agent("harbour-planner", "R-2190"),
        created_at: utc(2, 9),
        updated_at: utc(2, 14),
      }),
      task({
        id: "T-0110",
        project: HARBOR,
        status: "changes_requested",
        title: "Pilot boat dispatch",
        goal: "A pilot reaches every ship over 120 m at the fairway buoy.",
        targets: [
          target("RULE-PILOT-REQ", PILOT_FILE, "rule", "Pilot required above 120 m"),
          target("MEC-PILOTAGE", PILOT_FILE, "mechanic", "Pilot boarding"),
        ],
        criteria: [
          { ref: "RULE-PILOT-REQ", text: "Every ship longer than 120 m takes a pilot at the fairway buoy." },
          { ref: "MEC-PILOTAGE#AC-03", text: null },
        ],
        plan: T0110_PLAN,
        owner_notes: [{ at: utc(4, 16, 10), note: "Split the dispatch from the boarding.\nKeep the 120 m threshold as written." }],
        author: agent("harbour-planner", "R-2215"),
        created_at: utc(3, 10),
        updated_at: utc(4, 16, 10),
      }),
      task({
        id: "T-0111",
        project: HARBOR,
        status: "ready",
        title: "A wider window at spring tides",
        goal: "Spring tides widen the entry window.",
        targets: [target("RULE-SPRING-WINDOW", "docs/spec/tides/spring-window.md", "rule", "Spring tides: a wider window?")],
        spec_snapshot: snapshotOf(utc(4, 11), "/work/harbor-sim/T-0111", "task/T-0111", t0111Nodes),
        notes: [goneNote],
        author: OWNER,
        created_at: utc(3, 16),
        updated_at: utc(4, 11),
      }),
      task({
        id: "T-0112",
        project: HARBOR,
        status: "ready",
        title: "Berth 4 draft data",
        goal: "The scenario data and RULE-BERTH-DRAFT agree on berth 4.",
        stale: false,
        targets: [berthDraft, target("DOM-BERTHS", BERTHS_FILE, "domain", "Berths and moorings")],
        criteria: [{ ref: "RULE-BERTH-DRAFT", text: "Berth 4 is limited to 12.0 m draft." }],
        plan: "Align berths.ron with the spec once PR-0044 is decided.",
        spec_snapshot: snapshotOf(utc(5, 9), "/work/harbor-sim", "main", t0112Nodes),
        snapshot_diff: [],
        author: agent("balance-checker", "R-2230"),
        created_at: utc(4, 15),
        updated_at: utc(5, 9),
      }),
      task({
        id: "T-0113",
        project: HARBOR,
        status: "draft",
        targets: [
          target("MEC-LOCK-A", "docs/spec/locks/lock-a.md", "mechanic", "Lock A"),
          target("MEC-LOCK-B", "docs/spec/locks/lock-b.md", "mechanic", "Lock B"),
          target("MEC-FAIRWAY", "docs/spec/fairway/README.md", "mechanic", "Fairway"),
          target("DOM-HARBOR", "docs/spec/harbor.md", "domain", "Harbor"),
        ],
        author: OWNER,
        created_at: utc(5, 18, 30),
      }),
    ],
  };
}

// ledger-api: another project's words for roles and its profile.

const LEDGER = "ledger-api";
const LEDGER_PROFILE = "ledger-http-service";

function ledgerTasks(): MockTasks {
  const windowNode = snapshotNode("POL-REFUND-WINDOW", "docs/spec/refunds/rules.md");
  return {
    notes: [],
    tasks: [
      task({
        id: "T-0031",
        project: LEDGER,
        status: "in_progress",
        title: "Refund window of 60 days",
        goal: "Refunds are accepted for 60 days after the charge.",
        profile: LEDGER_PROFILE,
        stale: false,
        targets: [target("POL-REFUND-WINDOW", "docs/spec/refunds/rules.md", "policy", "Refund window")],
        criteria: [{ ref: "POL-REFUND-WINDOW", text: "A refund request after the window is refused with 422." }],
        plan: "1. Read the window from the policy.\n2. Refuse later requests with 422.",
        spec_snapshot: snapshotOf(utc(2, 10), "/srv/ledger-api", "main", [windowNode]),
        snapshot_diff: [],
        claim: { at: utc(2, 11), role: "backend-dev", worktree: "/srv/ledger-api/wt/T-0031", branch: "feat/refund-window" },
        runs: [run({ run: 1, role: "backend-dev", started_at: utc(2, 11) })],
        bundle: { node_ids: ["POL-REFUND-WINDOW"], budget: 10000, bundle_hash: fakeHash("bundle T-0031") },
        author: agent("product-lead", "L-0418"),
        created_at: utc(1, 15),
        updated_at: utc(2, 11),
      }),
      task({
        id: "T-0033",
        project: LEDGER,
        status: "review",
        title: "Idempotent refund creation",
        goal: "POST /v1/refunds is idempotent per key.",
        profile: LEDGER_PROFILE,
        targets: [
          target("EP-REFUND-CREATE", "docs/spec/refunds/endpoints.md", "endpoint", "POST /v1/refunds"),
          target("POL-IDEMPOTENCY", "docs/spec/api/idempotency.md", "policy", "Idempotency keys"),
        ],
        criteria: [{ ref: "POL-IDEMPOTENCY", text: "A repeated key returns the first answer." }],
        plan: "1. Store the key with the answer.\n2. Return the stored answer for a repeated key.",
        author: agent("api-reviewer", "L-0420"),
        created_at: utc(4, 9),
        updated_at: utc(5, 13, 5),
      }),
    ],
  };
}

// large: T-0200 to T-0519.

/** Exactly `bytes` ASCII bytes: `head`, then plain harbour words. */
function sized(head: string, bytes: number): string {
  const words = " the pilot boat waits at the buoy while the tide turns and the berth crew stands by";
  let text = head;
  while (text.length < bytes) {
    text += words;
  }
  return text.slice(0, bytes);
}

const REACHED = ["in_progress", "draft", "review", "changes_requested", "ready", "done", "cancelled"] as const;

function generatedStep(index: number): string {
  const d = 1 + (Math.floor(index / 36) % 12);
  const m = 1 + (Math.floor(index / 6) % 6);
  const k = 1 + (index % 6);
  return `MEC-GEN-${pad(d)}-${pad(m)}-${pad(k)}`;
}

function generatedRule(index: number): string {
  return `RULE-GEN-${generatedStep(index).slice("MEC-GEN-".length)}-${String(1 + (index % 2))}`;
}

function stepPath(id: string): string {
  const [d, m, k] = id.slice("MEC-GEN-".length).split("-");
  return `docs/spec/gen/d${d ?? "00"}/m${m ?? "00"}/s${k ?? "00"}.md`;
}

/** T-0200: every field at the package's cap (task-package "Data", Caps). */
function taskAtCaps(): StoredTask {
  const steps = Array.from({ length: 64 }, (_, index) => generatedStep(index));
  const rules = Array.from({ length: 64 }, (_, index) => generatedRule(index + 64));
  const nodes = [...steps, ...rules].map((id) => snapshotNode(id, stepPath(id.replace(/^RULE-GEN-(\d\d-\d\d-\d\d)-\d$/, "MEC-GEN-$1"))));
  const diffs = nodes.slice(0, 6).map((node) => {
    const lines = [`--- snapshot ${node.path}`, `+++ current ${node.path}`, "@@ -1,200 +1,200 @@"];
    for (let line = 1; line <= 200; line += 1) {
      lines.push(`-Step line ${pad(line, 3)}: the crew waits for the tide.`, `+Step line ${pad(line, 3)}: the crew waits for the pilot boat.`);
    }
    let diff = "";
    for (const text of lines) {
      if (diff.length + text.length + 1 > 8192) {
        break;
      }
      diff += `${text}\n`;
    }
    return { id: node.id, path: node.path, span_hash: node.span_hash, diff, cut: true };
  });
  const worktree = "/work/harbor-sim/T-0200";
  return task({
    id: "T-0200",
    project: HARBOR,
    status: "in_progress",
    title: sized("Every cap at once:", 256),
    goal: sized("Goal at its cap:", 4096),
    profile: sized("harbour-profile-", 64).replaceAll(" ", "-"),
    stale: true,
    targets: steps.map((id) => target(id, stepPath(id), "mechanic", `Generated step ${id.slice("MEC-GEN-".length)}`)),
    criteria: Array.from({ length: 32 }, (_, index) => ({ ref: null, text: sized(`Criterion ${String(index + 1)}:`, 1024) })),
    affected_nodes: rules,
    plan: sized("Plan at its cap:\n", 16384),
    owner_notes: [{ at: utc(1, 12), note: sized("Owner note at its cap:", 4096) }],
    spec_snapshot: snapshotOf(utc(1, 13), worktree, "task/T-0200", nodes),
    snapshot_diff: diffs,
    claim: { at: utc(1, 14), role: SIM_CODER, worktree, branch: "task/T-0200" },
    runs: [
      run({
        run: 1,
        role: SIM_CODER,
        started_at: utc(1, 14),
        ended_at: utc(2, 9),
        outcome: "partial",
        summary: sized("Summary at its cap:", 4096),
        changed_files: Array.from(
          { length: 256 },
          (_, index) => `${sized(`src/sim/generated/file-${pad(index + 1, 3)}/`, 508).replaceAll(" ", "-")}.txt`,
        ),
      }),
    ],
    bundle: { node_ids: steps, budget: 10000, bundle_hash: fakeHash("bundle T-0200") },
    author: OWNER,
    created_at: utc(1, 11),
    updated_at: utc(2, 9),
  });
}

function generatedTask(number: number): StoredTask {
  const index = number - 200;
  const status = REACHED[index % REACHED.length] ?? "draft";
  const step = generatedStep(index);
  const approved = status === "ready" || status === "in_progress" || status === "done";
  const id = `T-${pad(number, 4)}`;
  const worktree = `/work/harbor-sim/${id}`;
  const branch = `task/${id}`;
  const updated = utc(3, index % 24, index % 60);
  // In progress as T-0109 is: claimed in its worktree at its last update, run 1 open, its bundle.
  const claimed = status === "in_progress";
  return task({
    id,
    project: HARBOR,
    status,
    title: `Generated task ${String(number)}: ${step}`,
    stale: approved ? false : null,
    targets: [target(step, stepPath(step), "mechanic", `Generated step ${step.slice("MEC-GEN-".length)}`)],
    spec_snapshot: approved ? snapshotOf(utc(2, index % 24), worktree, branch, [snapshotNode(step, stepPath(step))]) : null,
    snapshot_diff: approved ? [] : null,
    claim: claimed ? { at: updated, role: SIM_CODER, worktree, branch } : null,
    runs: claimed ? [run({ run: 1, role: SIM_CODER, started_at: updated })] : [],
    bundle: claimed ? { node_ids: [step], budget: 10000, bundle_hash: fakeHash(`bundle ${id}`) } : null,
    created_at: utc(1, index % 24, index % 60),
    updated_at: updated,
  });
}

/** The `large` scenario's tasks of harbor-sim: 320 more, T-0200 at every cap. */
export function largeTasks(): StoredTask[] {
  return [taskAtCaps(), ...Array.from({ length: 319 }, (_, index) => generatedTask(201 + index))];
}

/** Each mock project's tasks, built anew per call from fixed values. */
export function mockTasks(slug: string): MockTasks {
  if (slug === HARBOR) {
    return harborTasks();
  }
  if (slug === LEDGER) {
    return ledgerTasks();
  }
  return { tasks: [], notes: [] };
}

// Reads, as the core answers them.

const LISTED_PROPOSAL = new Set(["open", "approved"]);

/** The nodes a task's proposals are matched on: the snapshot, else targets, criteria references, affected nodes. */
function nodesOf(stored: StoredTask): Set<string> {
  if (stored.spec_snapshot !== null) {
    return new Set(stored.spec_snapshot.nodes.map((node) => node.id));
  }
  const named = [
    ...stored.targets.flatMap((item) => item.id ?? item.path ?? []),
    ...stored.criteria.flatMap((item) => item.ref ?? []),
    ...stored.affected_nodes,
  ];
  return new Set(named);
}

function summaryOf(proposal: StoredReview): string {
  if (proposal.kind === "question") {
    return proposal.summary ?? "";
  }
  const first = proposal.rationale?.split("\n")[0];
  return first ?? proposal.summary ?? "";
}

function assumptionOf(proposal: StoredReview): TaskAssumption | null {
  if (proposal.kind === "question" && proposal.working_answer !== null) {
    return { proposal: proposal.id, text: proposal.working_answer };
  }
  if (proposal.kind === "discrepancy" && proposal.recommendation !== null) {
    const option = proposal.options[proposal.recommendation];
    return option === undefined ? null : { proposal: proposal.id, text: option.label };
  }
  return null;
}

/** `spec task show T --json`: the stored task with its proposals and assumptions read from the queue now. */
export function packageOf(stored: StoredTask, proposals: readonly MockProposal[]): TaskPackage {
  const nodes = nodesOf(stored);
  const listed = proposals
    .filter(({ review }) => LISTED_PROPOSAL.has(review.status))
    .filter(({ review, task_id }) => task_id === stored.id || review.target_ids.some((id) => nodes.has(id)))
    .sort((a, b) => a.review.id.localeCompare(b.review.id));
  const open: TaskProposal[] = listed.map(({ review, task_id }) => ({
    id: review.id,
    kind: review.kind,
    status: review.status,
    target_ids: [...review.target_ids],
    task_id,
    summary: summaryOf(review),
  }));
  return {
    schema_version: stored.schema_version,
    id: stored.id,
    project: stored.project,
    status: stored.status,
    title: stored.title,
    goal: stored.goal,
    profile: stored.profile,
    stale: stored.stale,
    targets: stored.targets,
    criteria: stored.criteria,
    affected_nodes: stored.affected_nodes,
    plan: stored.plan,
    assumptions: listed.flatMap(({ review }) => assumptionOf(review) ?? []),
    open_proposals: open,
    owner_notes: stored.owner_notes,
    bindings: stored.bindings,
    spec_snapshot: stored.spec_snapshot,
    snapshot_diff: stored.snapshot_diff,
    claim: stored.claim,
    runs: stored.runs,
    bundle: stored.bundle,
    author: stored.author,
    created_at: stored.created_at,
    updated_at: stored.updated_at,
    notes: stored.notes,
  };
}

/** `spec task list --json`: every row by number; a gone approval place adds `<id>: <note>`. */
export function taskListOf(store: MockTasks): TaskList {
  const tasks = [...store.tasks].sort((a, b) => a.id.localeCompare(b.id, "en", { numeric: true }));
  const placeNotes = tasks.flatMap((stored) =>
    stored.stale === null && stored.spec_snapshot !== null ? stored.notes.map((note) => `${stored.id}: ${note}`) : [],
  );
  return {
    tasks: tasks.map((stored) => ({
      id: stored.id,
      status: stored.status,
      title: stored.title,
      targets: stored.targets.flatMap((item) => item.id ?? item.path ?? []),
      stale: stored.stale,
      updated_at: stored.updated_at,
    })),
    notes: [...store.notes, ...placeNotes].sort((a, b) => a.localeCompare(b, "en", { numeric: true })),
  };
}

/** The exit-1 document of `spec task show` for a T the repository lacks (the daemon's 404). */
export function taskNotFound(id: string): TaskNotFound {
  return { id, reason: `no task ${id} in this repository` };
}

import { ClientError } from "../api/client";
import type {
  DeliveryCancelled,
  DeliveryCounts,
  DeliveryDocument,
  DeliveryGroup,
  DeliveryItem,
  DeliveryLate,
  DeliveryReestimate,
  DeliveryRelease,
  DeliveryReturn,
  DeliveryTimeline,
  LeadDays,
} from "../api/types";
import { stamp } from "./build";
import type { Scenario } from "./scenario";

// What was delivered, what is overdue and why, as the daemon's `GET .../delivery?as_of=D` serves it
// (docs/features/delivery-analytics.md "Acceptance criteria", DA-11, DA-12): stored task facts and
// the core's judge over them at the day asked, as src/mocks/roadmap.ts computes its sums. Each task's
// group is placed by hand where the plan would place it (a target's group, else its parent's), its
// departments are those whose flows list its flow, and its reasons stand as the queue would hold
// them; the judge counts each row, keeps each list's oldest 16 and orders the late tasks. The facts
// are this file's own: the Board's mock times are no history.
//
// harbor-sim: the releases keel, hull and sails, the pool, the unplaced items and a group word this
// build does not know (`milestone`), then no release, holding a cancelled task alone; its two
// departments and a third whose flow holds no task, a row of zeros. C-0001 is overdue with every
// reason: 17 returns (one more than listed, one with no note), a re-estimate, a cancelled subtask,
// a question, a discrepancy and a decision; C-0008 is overdue, its estimate cleared; C-0005 was
// delivered late, its first estimate set from none; C-0004 and C-0003 fall due today and tomorrow,
// overdue once the day moves past them. `empty`: no task. ledger-api has no `[roadmap]`: every task
// in no release, with the note. The spec's own scenario is held too, its JSON at 2026-11-20
// (src/mocks/delivery.test.ts).
//
// Asked with `timeline` (docs/features/delivery-timeline.md "Acceptance criteria", DT1, DT2, DT5,
// DT9), the document carries every task not cancelled on one axis, as the core's judge step 7 lays
// them out: by group, then creation, then ID number; a marker per group holding one, its first take
// and last accept; the axis from the first creation day to the greatest of the day asked and every
// stored day of a row. harbor-sim's rows hold every shape the view draws: an overdue, a late, an
// untaken row, a subtask, an accept from the first column (C-0012, its work segment empty) and the
// unknown `milestone`; `empty` an empty timeline. ledger-api's C-0101, made after every accept and
// due, ends its axis at that creation on a day asked before it. `large`: harbor-sim's 2 000 tasks
// over five releases and four departments, the view's paging at its bar.

/** Each list of a late task keeps its oldest this many, `REASONS_MAX`. */
const REASONS_MAX = 16;

const DAY_MS = 86_400_000;

const MINUTE_DAY = 24 * 60;

/** A task as the queue holds it: its group placed, its reasons gathered, oldest first. */
interface StoredTask {
  id: string;
  title: string;
  flow: string;
  column: string;
  parent: string | null;
  /** Its group's place among the project's groups; null: no release. */
  group: number | null;
  created_at: string;
  taken_at: string | null;
  accepted_at: string | null;
  cancelled: boolean;
  due: string | null;
  estimate: string | null;
  returns: DeliveryReturn[];
  reestimates: DeliveryReestimate[];
  cancelled_subtasks: DeliveryCancelled[];
  items: DeliveryItem[];
}

/** A group of the plan, in plan order. */
interface StoredGroup {
  group: DeliveryGroup;
  release: string | null;
}

/** A served department: its flows, null listing every flow. */
interface StoredDepartment {
  name: string;
  title: string;
  flows: readonly string[] | null;
}

/** A project's delivery facts. */
interface StoredDelivery {
  groups: readonly StoredGroup[];
  departments: readonly StoredDepartment[];
  tasks: readonly StoredTask[];
  notes: readonly string[];
}

/** A task with no move, accept, cancel, due, estimate or reason, unless given. */
function task(fields: Partial<StoredTask> & Pick<StoredTask, "id" | "title" | "flow" | "column" | "group" | "created_at">): StoredTask {
  return {
    parent: null,
    taken_at: null,
    accepted_at: null,
    cancelled: false,
    due: null,
    estimate: null,
    returns: [],
    reestimates: [],
    cancelled_subtasks: [],
    items: [],
    ...fields,
  };
}

/** A day's number since 1970-01-01 from `YYYY-MM-DD` or a timestamp's date part; null when it names no calendar day. */
function unixDays(text: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(text);
  if (match === null) {
    return null;
  }
  const at = Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return new Date(at).toISOString().slice(0, 10) === match[0] ? Math.round(at / DAY_MS) : null;
}

/** Days from made to accepted; null while open or when a time does not read. */
function leadOf(stored: StoredTask): number | null {
  const accepted = stored.accepted_at === null ? null : unixDays(stored.accepted_at);
  const created = unixDays(stored.created_at);
  return accepted === null || created === null ? null : accepted - created;
}

/** Accepted on a day after its due. */
function deliveredLate(stored: StoredTask): boolean {
  return stored.accepted_at !== null && stored.due !== null && stored.accepted_at.slice(0, 10) > stored.due;
}

/** Open past its due at `asOf`: a due equal to the day is not. */
function overdue(stored: StoredTask, asOf: string): boolean {
  return !stored.cancelled && stored.accepted_at === null && stored.due !== null && stored.due < asOf;
}

/** The lower of the two middles, and the most. */
function leadDays(values: readonly number[]): LeadDays | null {
  const sorted = [...values].sort((a, b) => a - b);
  const median = sorted[Math.floor((sorted.length - 1) / 2)];
  const max = sorted.at(-1);
  return median === undefined || max === undefined ? null : { median, max };
}

/** A row's counts over its tasks, cancelled ones included. */
function countsOf(tasks: readonly StoredTask[], asOf: string): DeliveryCounts {
  const live = tasks.filter((stored) => !stored.cancelled);
  const delivered = live.filter((stored) => stored.accepted_at !== null);
  return {
    tasks: live.length,
    delivered: delivered.length,
    delivered_late: delivered.filter(deliveredLate).length,
    open: live.length - delivered.length,
    overdue: live.filter((stored) => overdue(stored, asOf)).length,
    cancelled: tasks.length - live.length,
    lead_days: leadDays(delivered.flatMap((stored) => leadOf(stored) ?? [])),
  };
}

function inDepartment(department: StoredDepartment, stored: StoredTask): boolean {
  return department.flows === null || department.flows.includes(stored.flow);
}

/** The group a task counts in: its own, or no release. */
function groupOf(facts: StoredDelivery, stored: StoredTask): StoredGroup {
  return (stored.group === null ? undefined : facts.groups[stored.group]) ?? { group: "none", release: null };
}

/** A late task as served: its lists cut at their oldest 16. */
function lateOf(facts: StoredDelivery, stored: StoredTask, asOf: string): DeliveryLate {
  const due = stored.due ?? "";
  const end = stored.accepted_at === null ? asOf : stored.accepted_at;
  const { group, release } = groupOf(facts, stored);
  const cut = <T,>(list: readonly T[]) => list.slice(0, REASONS_MAX);
  const past = (list: readonly unknown[]) => Math.max(0, list.length - REASONS_MAX);
  return {
    id: stored.id,
    title: stored.title,
    flow: stored.flow,
    column: stored.column,
    parent: stored.parent,
    group,
    release,
    departments: facts.departments.filter((department) => inDepartment(department, stored)).map((department) => department.name),
    created_at: stored.created_at,
    taken_at: stored.taken_at,
    accepted_at: stored.accepted_at,
    due,
    days_late: (unixDays(end) ?? 0) - (unixDays(due) ?? 0),
    lead_days: leadOf(stored),
    estimate: stored.estimate,
    returns: cut(stored.returns),
    reestimates: cut(stored.reestimates),
    cancelled_subtasks: cut(stored.cancelled_subtasks),
    items: cut(stored.items),
    more: { returns: past(stored.returns), reestimates: past(stored.reestimates), cancelled_subtasks: past(stored.cancelled_subtasks), items: past(stored.items) },
  };
}

/** A card ID's number, as the late tasks are ordered after their days. */
function idNumber(id: string): number {
  return Number(id.replace(/^\D+/, ""));
}

/** A row's place among the groups: its group's in plan order, no release after them all. */
function groupOrder(facts: StoredDelivery, stored: StoredTask): number {
  return stored.group === null || facts.groups[stored.group] === undefined ? facts.groups.length : stored.group;
}

/** The least, or the greatest, of stored values of one form; null with none. */
function least(values: readonly (string | null)[]): string | null {
  return values.reduce<string | null>((low, value) => (value !== null && (low === null || value < low) ? value : low), null);
}

function greatest(values: readonly (string | null)[]): string | null {
  return values.reduce<string | null>((high, value) => (value !== null && (high === null || value > high) ? value : high), null);
}

/** Every task not cancelled on one axis at `asOf`, as the core's judge step 7 lays it out (DT1, DT2, DT5). */
function timelineOf(facts: StoredDelivery, asOf: string): DeliveryTimeline {
  const rows = facts.tasks
    .filter((stored) => !stored.cancelled && unixDays(stored.created_at) !== null)
    .sort(
      (a, b) =>
        groupOrder(facts, a) - groupOrder(facts, b) || (a.created_at < b.created_at ? -1 : a.created_at > b.created_at ? 1 : 0) || idNumber(a.id) - idNumber(b.id),
    );
  const markers: DeliveryTimeline["markers"] = [];
  for (const place of new Set(rows.map((stored) => groupOrder(facts, stored)))) {
    const held = rows.filter((stored) => groupOrder(facts, stored) === place);
    const first = held[0];
    if (first !== undefined) {
      const { group, release } = groupOf(facts, first);
      markers.push({ group, release, first_taken_at: least(held.map((stored) => stored.taken_at)), last_accepted_at: greatest(held.map((stored) => stored.accepted_at)) });
    }
  }
  const tasks = rows.map((stored) => {
    const { group, release } = groupOf(facts, stored);
    const late = deliveredLate(stored) || overdue(stored, asOf);
    return {
      id: stored.id,
      title: stored.title,
      flow: stored.flow,
      group,
      release,
      departments: facts.departments.filter((department) => inDepartment(department, stored)).map((department) => department.name),
      created_at: stored.created_at,
      taken_at: stored.taken_at,
      accepted_at: stored.accepted_at,
      due: stored.due,
      days_late: late ? lateOf(facts, stored, asOf).days_late : null,
    };
  });
  const from = least(rows.map((stored) => stored.created_at.slice(0, 10)));
  // Every stored day of a row, as the core's: the axis covers a card made or taken after `asOf` too.
  const days = rows.flatMap((row) => [row.created_at, row.taken_at, row.accepted_at, row.due].flatMap((value) => (value === null || unixDays(value) === null ? [] : [value.slice(0, 10)])));
  const to = rows.length === 0 ? null : greatest([asOf, ...days]);
  return { from, to, markers, tasks };
}

/** The document at `asOf`, as the core judges it; its timeline built only when asked. */
function judged(facts: StoredDelivery, asOf: string, timeline = false): DeliveryDocument {
  const releases: DeliveryRelease[] = [];
  facts.groups.forEach((stored, index) => {
    const held = facts.tasks.filter((candidate) => candidate.group === index);
    if (held.length > 0) {
      releases.push({ group: stored.group, release: stored.release, ...countsOf(held, asOf) });
    }
  });
  const placed = facts.tasks.filter((candidate) => candidate.group === null);
  if (placed.length > 0) {
    releases.push({ group: "none", release: null, ...countsOf(placed, asOf) });
  }
  const late = facts.tasks
    .filter((stored) => !stored.cancelled && (deliveredLate(stored) || overdue(stored, asOf)))
    .map((stored) => lateOf(facts, stored, asOf))
    .sort((a, b) => b.days_late - a.days_late || idNumber(a.id) - idNumber(b.id));
  return {
    as_of: asOf,
    notes: [...facts.notes],
    releases,
    departments: facts.departments.map((department) => ({
      name: department.name,
      title: department.title,
      ...countsOf(
        facts.tasks.filter((stored) => inDepartment(department, stored)),
        asOf,
      ),
    })),
    total: countsOf(facts.tasks, asOf),
    late,
    timeline: timeline ? timelineOf(facts, asOf) : null,
    truncated: false,
    omitted: null,
  };
}

/** The calendar day `days` from the clock's, as `--due` writes one. */
function day(at: number, days: number): string {
  return stamp(at, -days * MINUTE_DAY).slice(0, 10);
}

/** That day at `hour` UTC. */
function time(at: number, days: number, hour = 9): string {
  return `${day(at, days)}T${String(hour).padStart(2, "0")}:00:00Z`;
}

/** What a reviewer said, in turn, on C-0001's returns; one return says nothing. */
const RETURN_NOTES: readonly (string | null)[] = [
  "Too long for one page",
  "Wrong tone for skippers",
  "The datum is not named",
  "The surge paragraph reads as advice",
  "The second table's heading changed",
  null,
];

/** C-0001's seventeen returns from proof, two a day, the last one past the list's sixteen. */
function forewordReturns(at: number): DeliveryReturn[] {
  return Array.from({ length: 17 }, (_, index) => ({
    at: time(at, -11 + Math.floor(index / 2), index % 2 === 0 ? 9 : 15),
    from: "proof",
    note: RETURN_NOTES[index % RETURN_NOTES.length] ?? null,
  }));
}

/** harbor-sim's plan groups, in plan order. */
const HARBOR_GROUPS: readonly StoredGroup[] = [
  { group: "release", release: "keel" },
  { group: "release", release: "hull" },
  { group: "release", release: "sails" },
  { group: "pool", release: null },
  { group: "unplaced", release: null },
  { group: "milestone", release: null },
];

const [KEEL, HULL, SAILS, POOL, UNPLACED, MILESTONE] = [0, 1, 2, 3, 4, 5];

/** harbor-sim's departments, as src/mocks/workspaces.ts serves the first two, and one whose flow holds no task. */
const HARBOR_DEPARTMENTS: readonly StoredDepartment[] = [
  { name: "galley", title: "Galley", flows: ["copy"] },
  { name: "foundry", title: "Foundry", flows: ["plates"] },
  { name: "chandlery", title: "Chandlery", flows: ["rigging"] },
];

function harborTasks(at: number): StoredTask[] {
  return [
    task({
      id: "C-0001",
      title: "Write the tide table foreword",
      flow: "copy",
      column: "draft",
      group: KEEL,
      created_at: time(at, -14),
      taken_at: time(at, -12),
      due: day(at, -2),
      estimate: "5",
      returns: forewordReturns(at),
      reestimates: [{ at: time(at, -8), old: "3", new: "5", reason: "The tables doubled" }],
      cancelled_subtasks: [{ id: "C-0017", title: "Second tide plate", at: time(at, -1), reason: "One plate holds both tables." }],
      items: [
        { id: "PR-0041", kind: "discrepancy", status: "open" },
        { id: "PR-0055", kind: "question", status: "open" },
        { id: "PR-0056", kind: "decision", status: "applied" },
      ],
    }),
    task({ id: "C-0002", title: "Caption the berth plans", flow: "copy", column: "draft", group: POOL, created_at: time(at, -3), due: day(at, 6), estimate: "1-2" }),
    task({ id: "C-0003", title: "Night passage rules, plain words", flow: "copy", column: "edit", group: HULL, created_at: time(at, -6), taken_at: time(at, -5), due: day(at, 1), estimate: "2" }),
    task({ id: "C-0004", title: "Proof the lock opening hours", flow: "copy", column: "proof", group: SAILS, created_at: time(at, -5), taken_at: time(at, -4), due: day(at, 0), estimate: "1" }),
    task({
      id: "C-0005",
      title: "Mooring fees table",
      flow: "copy",
      column: "filed",
      group: KEEL,
      created_at: time(at, -16),
      taken_at: time(at, -15),
      accepted_at: time(at, -4),
      due: day(at, -9),
      estimate: "1",
      reestimates: [{ at: time(at, -12), old: null, new: "1", reason: "First estimate, after the harbour office's call" }],
    }),
    task({ id: "C-0006", title: "Advert for the old quays tour", flow: "copy", column: "draft", group: null, created_at: time(at, -25), cancelled: true, due: day(at, -20) }),
    task({ id: "C-0007", title: "Fairway chart, sheet 2", flow: "plates", column: "queued", group: UNPLACED, created_at: time(at, -2), due: day(at, 10), estimate: "4" }),
    task({
      id: "C-0008",
      title: "Crane reach diagram",
      flow: "plates",
      column: "blocked",
      group: HULL,
      created_at: time(at, -10),
      taken_at: time(at, -9),
      due: day(at, -5),
      returns: [{ at: time(at, -1), from: "queued", note: "The survey's crane reach numbers are not in MEC-CARGO-CRANES yet; the diagram waits on them." }],
      reestimates: [
        { at: time(at, -7), old: "2", new: "3", reason: "Two more cranes on the survey" },
        { at: time(at, -1, 10), old: "3", new: null, reason: "No estimate until the survey lands" },
      ],
    }),
    task({ id: "C-0009", title: "Harbour overview, colour plate", flow: "plates", column: "checked", group: SAILS, created_at: time(at, -4), taken_at: time(at, -3), due: day(at, 2), estimate: "3" }),
    task({ id: "C-0010", title: "Deep-draft entry notice", flow: "copy", column: "filed", group: MILESTONE, created_at: time(at, -7), taken_at: time(at, -6), accepted_at: time(at, -1), estimate: "1" }),
    task({ id: "C-0011", title: "High water note", flow: "copy", column: "filed", group: KEEL, created_at: time(at, -9), taken_at: time(at, -8), accepted_at: time(at, -5), estimate: "1" }),
    // Accepted from the column it was made in: the accept is its take.
    task({ id: "C-0012", title: "Spring window table", flow: "copy", column: "filed", group: HULL, created_at: time(at, -6), taken_at: time(at, -2), accepted_at: time(at, -2), estimate: "2" }),
    task({ id: "C-0013", title: "Tide table plate", flow: "plates", column: "queued", parent: "C-0001", group: KEEL, created_at: time(at, -10) }),
    task({ id: "C-0014", title: "Lock hours plate", flow: "plates", column: "struck", parent: "C-0004", group: SAILS, created_at: time(at, -3), taken_at: time(at, -2), accepted_at: time(at, -1, 21) }),
    task({ id: "C-0015", title: "Night passage chart", flow: "plates", column: "blocked", parent: "C-0003", group: HULL, created_at: time(at, -5), taken_at: time(at, -4) }),
    task({ id: "C-0016", title: "Tide table cover", flow: "plates", column: "struck", parent: "C-0001", group: KEEL, created_at: time(at, -7), taken_at: time(at, -6), accepted_at: time(at, -1) }),
    task({ id: "C-0017", title: "Second tide plate", flow: "plates", column: "queued", parent: "C-0001", group: KEEL, created_at: time(at, -10), cancelled: true }),
    task({ id: "C-0018", title: "Caption the harbour overview", flow: "copy", column: "draft", parent: "C-0009", group: SAILS, created_at: time(at, -1) }),
    task({ id: "C-0019", title: "Tide table glossary", flow: "copy", column: "edit", parent: "C-0001", group: KEEL, created_at: time(at, -10), taken_at: time(at, -8) }),
  ];
}

/** The note of a project with no `[roadmap]` table. */
export const NO_ROADMAP_NOTE = "delivery: no [roadmap] table: every task is in no release";

/** `large`'s five releases, in plan order. */
const LARGE_GROUPS: readonly StoredGroup[] = ["keel", "hull", "sails", "masts", "rigging"].map((release) => ({ group: "release", release }));

/** `large`'s four departments, a flow each. */
const LARGE_DEPARTMENTS: readonly StoredDepartment[] = [
  ...HARBOR_DEPARTMENTS,
  { name: "ropewalk", title: "Ropewalk", flows: ["lines"] },
];

const LARGE_FLOWS = ["copy", "plates", "rigging", "lines"];

/** How many tasks `large` holds (docs/features/delivery-timeline.md "Acceptance criteria", DT9). */
export const LARGE_TASKS = 2000;

/**
 * `large`: 2 000 tasks made over the last 120 days, spread over five releases and four departments by
 * their number: the older ones mostly accepted, every thirteenth left open, the last 25 days' open;
 * some recent ones untaken; two in three with a due 10 to 39 days after creation, so a few are late
 * or overdue. Arithmetic on the number alone, so every read gives the same rows.
 */
function largeTasks(at: number): StoredTask[] {
  return Array.from({ length: LARGE_TASKS }, (_, index) => {
    const made = 1 + ((index * 7919) % 120);
    const flow = LARGE_FLOWS[index % LARGE_FLOWS.length] ?? "copy";
    const taken = index % 10 === 0 && made < 40 ? null : Math.max(0, made - 1 - (index % 4));
    const open = made < 25 || index % 13 === 0;
    const accepted = taken === null || open ? null : Math.max(0, taken - 1 - (index % 8));
    const due = index % 3 === 2 ? null : made - 10 - (index % 30);
    return task({
      id: `C-${String(index + 1).padStart(4, "0")}`,
      title: `${flow} ${String(index + 1)}, berth ${String((index % 40) + 1)}`,
      flow,
      column: accepted === null ? (taken === null ? "draft" : "edit") : "filed",
      group: index % LARGE_GROUPS.length,
      created_at: time(at, -made),
      taken_at: taken === null ? null : time(at, -taken, 10),
      accepted_at: accepted === null ? null : time(at, -accepted, 11),
      due: due === null ? null : day(at, -due),
    });
  });
}

function harbor(at: number, scenario: Scenario): StoredDelivery {
  if (scenario === "large") {
    return { groups: LARGE_GROUPS, departments: LARGE_DEPARTMENTS, tasks: largeTasks(at), notes: [] };
  }
  return { groups: HARBOR_GROUPS, departments: HARBOR_DEPARTMENTS, tasks: scenario === "empty" ? [] : harborTasks(at), notes: [] };
}

/** ledger-api: no `[roadmap]`, the implicit department listing every flow. */
function ledger(at: number, scenario: Scenario): StoredDelivery {
  const tasks = [
    task({ id: "C-0101", title: "Write the refund runbook", flow: "plain", column: "backlog", group: null, created_at: time(at, -1), estimate: "2" }),
    task({ id: "C-0102", title: "Retry the failed payouts", flow: "plain", column: "done", group: null, created_at: time(at, -6), taken_at: time(at, -5), accepted_at: time(at, -2), due: day(at, -3) }),
  ];
  return { groups: [], departments: [{ name: "all", title: "All sections", flows: null }], tasks: scenario === "empty" ? [] : tasks, notes: [NO_ROADMAP_NOTE] };
}

/** The spec's scenario: its config and five tasks, every time at nine UTC. */
const PINNED: StoredDelivery = {
  groups: [
    { group: "release", release: "r1" },
    { group: "release", release: "r2" },
  ],
  departments: [
    { name: "words", title: "Words", flows: ["copy"] },
    { name: "print", title: "Print", flows: ["copy", "plates"] },
  ],
  tasks: [
    task({
      id: "C-0001",
      title: "Write the tide table foreword",
      flow: "copy",
      column: "done",
      group: 0,
      created_at: "2026-11-01T09:00:00Z",
      taken_at: "2026-11-03T09:00:00Z",
      accepted_at: "2026-11-15T09:00:00Z",
      due: "2026-11-10",
      estimate: "5",
      returns: [
        { at: "2026-11-06T09:00:00Z", from: "review", note: "Too long" },
        { at: "2026-11-08T09:00:00Z", from: "review", note: "Wrong tone" },
      ],
      reestimates: [{ at: "2026-11-07T09:00:00Z", old: "3", new: "5", reason: "The tables doubled" }],
      cancelled_subtasks: [{ id: "C-0002", title: "Draw the tide plates", at: "2026-11-09T09:00:00Z", reason: "Plates reused from last year" }],
      items: [{ id: "PR-0040", kind: "question", status: "applied" }],
    }),
    task({ id: "C-0002", title: "Draw the tide plates", flow: "plates", column: "todo", parent: "C-0001", group: 0, created_at: "2026-11-02T09:00:00Z", cancelled: true }),
    task({ id: "C-0003", title: "Engrave the harbor map", flow: "plates", column: "drawing", group: 1, created_at: "2026-11-04T09:00:00Z", taken_at: "2026-11-05T09:00:00Z", due: "2026-11-12" }),
    // Taken 2026-11-03 by its move to `review`: `copy` accepts from `review` alone (DT1).
    task({ id: "C-0004", title: "Fix the index", flow: "copy", column: "done", group: null, created_at: "2026-11-02T09:00:00Z", taken_at: "2026-11-03T09:00:00Z", accepted_at: "2026-11-05T09:00:00Z" }),
    // Made 2026-11-06, as docs/features/delivery-timeline.md "Acceptance criteria", DT1 settles it.
    task({ id: "C-0005", title: "Record the foghorn", flow: "sound", column: "todo", group: null, created_at: "2026-11-06T09:00:00Z" }),
  ],
  notes: [],
};

/** The day the spec's scenario is pinned at. */
export const PINNED_AS_OF = "2026-11-20";

/** The spec's scenario judged at `asOf`: at PINNED_AS_OF, its JSON; with `timeline`, the timeline's too. */
export function pinnedDelivery(asOf: string = PINNED_AS_OF, timeline = false): DeliveryDocument {
  return judged(PINNED, asOf, timeline);
}

/** A project's delivery at `asOf`, its timeline when asked; a day that is none is the daemon's 400 naming it, nothing read. */
export function deliveryOf(slug: string, scenario: Scenario, at: number, asOf: string, timeline = false): DeliveryDocument {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(asOf) || unixDays(asOf) === null) {
    throw new ClientError({ status: 400, message: `\`as_of\`: \`${asOf}\` is no calendar date, YYYY-MM-DD` });
  }
  return judged(slug === "harbor-sim" ? harbor(at, scenario) : ledger(at, scenario), asOf, timeline);
}

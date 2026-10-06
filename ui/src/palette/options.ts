import type { Project, Proposal, SearchHit, TaskListEntry } from "../api/types";
import { homeHash, navOf, sectionHash } from "../app/routes";
import { fold } from "../inbox/filter";
import { queueOrder } from "../inbox/order";
import { summaryOf } from "../inbox/summary";
import { groupsOf } from "../tasks/groups";

// The palette's options (docs/features/ui-home.md "Palette"): built from what the client holds and
// the text typed, nothing read here. A match is a substring of the NFC, lower-case text.

/** Options shown per group; the label says "<name>, 10 of <n>" over it. */
export const GROUP_LIMIT = 10;

export type PaletteOption =
  | { type: "section"; key: string; label: string; hash: string }
  | { type: "task"; key: string; entry: TaskListEntry; hash: string }
  | { type: "proposal"; key: string; proposal: Proposal; hash: string }
  | { type: "project"; key: string; project: Project; hash: string }
  | { type: "search"; key: string; query: string }
  | { type: "ref"; key: string; ref: string; hash: string }
  | { type: "hit"; key: string; hit: SearchHit; hash: string };

export type GroupName = "Sections" | "Tasks" | "Inbox" | "Projects" | "Spec tree";

export interface PaletteGroup {
  name: GroupName;
  /** At most GROUP_LIMIT of the `total` options matched. */
  options: PaletteOption[];
  total: number;
  /** The group's read is on its way: labelled "<name>, loading" and busy. */
  loading: boolean;
}

/** A read as the palette holds it: its answer, on its way, or failed (said under the list). */
export type Held<T> = { state: "ready"; value: T } | { state: "loading" } | { state: "failed" };

export interface PaletteInput {
  /** The project jumps go to; null when there is none. */
  project: string | null;
  /** The text as typed. */
  text: string;
  tasks: Held<readonly TaskListEntry[]>;
  inbox: Held<readonly Proposal[]>;
  projects: Held<readonly Project[]>;
  /** The node search activated for this text: its hits, on its way, or not run. */
  search: Held<readonly SearchHit[]> | null;
}

/** The text a jump is read for: trimmed; empty lists only Sections and Projects. */
export function needleOf(text: string): string {
  return fold(text.trim());
}

function holds(needle: string, fields: readonly (string | null)[]): boolean {
  return needle === "" || fields.some((field) => field !== null && fold(field).includes(needle));
}

/** An entry of a list as the palette matches it: its folded ID and the folded words it is matched on. */
interface Indexed<T> {
  item: T;
  id: string;
  fields: readonly string[];
}

/**
 * A list put in its screen's order and folded once per answer (the answer's array is the key):
 * a keystroke only filters what is held here.
 */
function indexedOnce<T>(
  memo: WeakMap<readonly T[], readonly Indexed<T>[]>,
  answer: readonly T[],
  order: (items: readonly T[]) => readonly T[],
  fieldsOf: (item: T) => readonly (string | null)[],
  idOf: (item: T) => string,
): readonly Indexed<T>[] {
  const known = memo.get(answer);
  if (known !== undefined) {
    return known;
  }
  const indexed = order(answer).map((item) => ({
    item,
    id: fold(idOf(item)),
    fields: fieldsOf(item)
      .filter((field): field is string => field !== null)
      .map(fold),
  }));
  memo.set(answer, indexed);
  return indexed;
}

const TASKS_IN_ORDER = new WeakMap<readonly TaskListEntry[], readonly Indexed<TaskListEntry>[]>();
const QUEUE_IN_ORDER = new WeakMap<readonly Proposal[], readonly Indexed<Proposal>[]>();

/** The tasks in the Tasks screen's order (`groupsOf`), matched on ID and title. */
function tasksInOrder(tasks: readonly TaskListEntry[]): readonly Indexed<TaskListEntry>[] {
  return indexedOnce(
    TASKS_IN_ORDER,
    tasks,
    (entries) => groupsOf(entries).flatMap((group) => group.entries),
    (entry) => [entry.id, entry.title],
    (entry) => entry.id,
  );
}

/** The proposals in the Inbox's order (`queueOrder`), matched on ID and summary. */
function queueInOrder(proposals: readonly Proposal[]): readonly Indexed<Proposal>[] {
  return indexedOnce(QUEUE_IN_ORDER, proposals, queueOrder, (proposal) => [proposal.id, summaryOf(proposal)], (proposal) => proposal.id);
}

/** The held entries whose folded words hold the needle (all of them for an empty one). */
function matching<T>(indexed: readonly Indexed<T>[], needle: string): readonly Indexed<T>[] {
  return needle === "" ? indexed : indexed.filter(({ fields }) => fields.some((field) => field.includes(needle)));
}

interface Built {
  group: PaletteGroup;
  /** An option's ID folds equal to the text: the group goes first, that option first in it. */
  exact: boolean;
}

function build(name: GroupName, options: PaletteOption[], exactKey: string | null = null, loading = false): Built {
  const exact = exactKey === null ? undefined : options.find((option) => option.key === exactKey);
  const ordered = exact === undefined ? options : [exact, ...options.filter((option) => option !== exact)];
  return {
    group: { name, options: ordered.slice(0, GROUP_LIMIT), total: ordered.length, loading },
    exact: exact !== undefined,
  };
}

function loadingGroup(name: GroupName): Built {
  return build(name, [], null, true);
}

/**
 * The groups in order (Sections, Tasks, Inbox, Projects, Spec tree), a group with an exact ID first;
 * an empty group left out unless its read is on its way. Empty text: Sections and Projects only.
 */
export function paletteGroups(input: PaletteInput): PaletteGroup[] {
  const needle = needleOf(input.text);
  const project = input.project;
  const built: Built[] = [];

  if (project !== null) {
    const sections = navOf(project)
      .filter((entry) => holds(needle, [entry.label]))
      .map((entry): PaletteOption => ({ type: "section", key: `section:${entry.key}`, label: entry.label, hash: entry.hash }));
    built.push(build("Sections", sections));
  }

  if (project !== null && needle !== "") {
    const { tasks, inbox } = input;
    if (tasks.state === "loading") {
      built.push(loadingGroup("Tasks"));
    } else if (tasks.state === "ready") {
      const entries = matching(tasksInOrder(tasks.value), needle);
      const exact = entries.find(({ id }) => id === needle);
      built.push(
        build(
          "Tasks",
          entries.map(({ item: entry }) => ({ type: "task", key: `task:${entry.id}`, entry, hash: sectionHash(project, "tasks", entry.id) })),
          exact === undefined ? null : `task:${exact.item.id}`,
        ),
      );
    }
    if (inbox.state === "loading") {
      built.push(loadingGroup("Inbox"));
    } else if (inbox.state === "ready") {
      const proposals = matching(queueInOrder(inbox.value), needle);
      const exact = proposals.find(({ id }) => id === needle);
      built.push(
        build(
          "Inbox",
          proposals.map(({ item: proposal }) => ({
            type: "proposal",
            key: `proposal:${proposal.id}`,
            proposal,
            hash: sectionHash(project, "inbox", proposal.id),
          })),
          exact === undefined ? null : `proposal:${exact.item.id}`,
        ),
      );
    }
  }

  if (input.projects.state === "loading") {
    built.push(loadingGroup("Projects"));
  } else if (input.projects.state === "ready") {
    const projects = input.projects.value
      .filter((candidate) => holds(needle, [candidate.slug, candidate.name]))
      .map((candidate): PaletteOption => ({ type: "project", key: `project:${candidate.slug}`, project: candidate, hash: homeHash(candidate.slug) }));
    built.push(build("Projects", projects));
  }

  if (project !== null && needle !== "") {
    const ref = input.text.trim();
    const hits =
      input.search?.state === "ready"
        ? input.search.value.map(
            (hit): PaletteOption => ({
              type: "hit",
              key: `hit:${hit.path}:${String(hit.line)}`,
              hit,
              hash: sectionHash(project, "tree", hit.id ?? hit.path),
            }),
          )
        : [];
    built.push(
      build(
        "Spec tree",
        [
          { type: "search", key: "search", query: input.text },
          { type: "ref", key: "ref", ref, hash: sectionHash(project, "tree", ref) },
          ...hits,
        ],
        null,
        input.search?.state === "loading",
      ),
    );
  }

  const shown = built.filter(({ group }) => group.options.length > 0 || group.loading);
  return [...shown.filter(({ exact }) => exact), ...shown.filter(({ exact }) => !exact)].map(({ group }) => group);
}

/** A group's label: its name, "<name>, loading", or "<name>, 10 of <n>". */
export function groupLabel(group: PaletteGroup): string {
  if (group.loading) {
    return `${group.name}, loading`;
  }
  return group.total > group.options.length ? `${group.name}, ${String(group.options.length)} of ${String(group.total)}` : group.name;
}

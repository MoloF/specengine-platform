import type { InboxEntry, ProposalStatus, TaskListEntry } from "../api/types";
import { statusRank } from "../inbox/labels";
import { isClosed, type TaskGroup } from "../tasks/groups";

// The home counts what the two reads list and groups it; nothing else is computed (ADR-0012).

/** How many entries of the home's lists are shown before "<k> of <n> shown". */
export const HOME_ROWS = 5;

const collator = new Intl.Collator("en");

/** What waits for the owner (`waitsForYou`), in the Tasks list's order. */
export function waitingOf(groups: readonly TaskGroup[]): TaskListEntry[] {
  return groups.find((group) => group.waiting)?.entries ?? [];
}

/** One line of "Other open tasks": a group's title and its size. */
export interface GroupCount {
  key: string;
  title: string;
  count: number;
}

/** Every other group of the Tasks list, in its order, but Closed (done and cancelled). */
export function otherOpenOf(groups: readonly TaskGroup[]): GroupCount[] {
  return groups
    .filter((group) => !group.waiting && !group.entries.every((entry) => isClosed(entry.status)))
    .map((group) => ({ key: group.key, title: group.title, count: group.entries.length }));
}

/** The entries counted by one raw value. */
function countBy(values: readonly string[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const value of values) {
    counts.set(value, (counts.get(value) ?? 0) + 1);
  }
  return counts;
}

export interface StatusCount {
  status: ProposalStatus;
  count: number;
}

/** Proposals per status: the table's order, unknown values after it (by text), each raw. */
export function statusCountsOf(proposals: readonly Pick<InboxEntry, "status">[]): StatusCount[] {
  return [...countBy(proposals.map(({ status }) => status))]
    .map(([status, count]) => ({ status, count }))
    .sort((a, b) => statusRank(a.status) - statusRank(b.status) || collator.compare(a.status, b.status));
}

/** One raw `kind` and its count: project vocabulary, shown as text, never branched on (ADR-0031). */
export interface KindCount {
  name: string;
  count: number;
}

/** Proposals per raw kind: the most first, then by text. */
export function kindCountsOf(proposals: readonly Pick<InboxEntry, "kind">[]): KindCount[] {
  return [...countBy(proposals.map(({ kind }) => kind))]
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count || collator.compare(a.name, b.name));
}

/** The noun after a count: "1 proposal", "7 proposals". */
export function proposalNoun(count: number): string {
  return count === 1 ? "proposal" : "proposals";
}

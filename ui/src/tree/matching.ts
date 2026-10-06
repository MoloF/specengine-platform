import type { InboxEntry } from "../api/types";
import { targetsOf } from "../inbox/targets";
import { stemOf } from "./rows";

// Which inbox items target a node (docs/features/ui-tree-node.md "Rules and edge cases"): a target
// (`target_ids`, else `target_id`) equal to the node's ID, to `<stem>/<ID>` (the file name without
// `.md`), or, for an ID-less node, to its path. Information only: nothing waits on it (ADR-0012).

/** The names a target may give a node by. */
export function namesOf(node: { id: string | null; path: string }): string[] {
  return node.id === null ? [node.path] : [node.id, `${stemOf(node.path)}/${node.id}`];
}

/** Whether a proposal targets any of the nodes. */
export function targetsAny(proposal: InboxEntry, nodes: readonly { id: string | null; path: string }[]): boolean {
  const names = new Set(nodes.flatMap(namesOf));
  return targetsOf(proposal).some((target) => names.has(target));
}

/** The proposals targeting any of the nodes, in the inbox's order. */
export function proposalsFor(proposals: readonly InboxEntry[], nodes: readonly { id: string | null; path: string }[]): InboxEntry[] {
  return proposals.filter((proposal) => targetsAny(proposal, nodes));
}

/** How many proposals target each name; a proposal naming one node twice counts once per node. */
export function targetIndex(proposals: readonly InboxEntry[]): Map<string, Set<string>> {
  const index = new Map<string, Set<string>>();
  for (const proposal of proposals) {
    for (const target of targetsOf(proposal)) {
      const ids = index.get(target) ?? new Set<string>();
      ids.add(proposal.id);
      index.set(target, ids);
    }
  }
  return index;
}

/** The number of distinct proposals targeting a node, from a target index. */
export function countFor(index: ReadonlyMap<string, ReadonlySet<string>>, node: { id: string | null; path: string }): number {
  const ids = new Set<string>();
  for (const name of namesOf(node)) {
    for (const id of index.get(name) ?? []) {
      ids.add(id);
    }
  }
  return ids.size;
}

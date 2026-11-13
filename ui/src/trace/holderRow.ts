import type { ShownNode, TraceDocument, TraceNode } from "../api/types";
import { rowName } from "../tree/rows";

/**
 * The trace read's row of the resolved holder, never another's: a read of a group holds what it holds
 * besides. A row names its node by its canonical ID, an ID-less one by its path, so the holder is
 * matched by those, never by the REF it was opened by: an alias finds its row as the canonical ID does
 * (docs/features/release-hardening.md "Acceptance criteria", RH-U5). An ambiguous bare ID can read
 * several holders, two rows sharing the ID: the row's file, root-relative as the holder's, picks this one.
 */
export function holderRow(trace: TraceDocument, holder: Pick<ShownNode, "id" | "path">): TraceNode | undefined {
  const name = rowName(holder);
  return trace.nodes.find((node) => node.id === name && node.path === holder.path);
}

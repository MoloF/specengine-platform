import type { TreeNode } from "../api/types";
import { driftBadgeLooks } from "../health/labels";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import type { NodeDrift } from "./drift";
import { markLook } from "./labels";
import { rowName } from "./rows";

/**
 * One tree line's content: its name, title, kind, status, rev, archived, mark, the inbox count,
 * and with "Show drift" on the node's own row 6 rows by class (docs/features/ui-tree-drift.md
 * "Badges"), each a badge with its icon and `<label> <count>`, never colour alone, never a 0. The
 * tree memoises it (SpecTree.tsx): an answer renders again only the lines whose values changed.
 */
export function RowContent({ node, count, drift }: { node: TreeNode; count: number | null; drift: NodeDrift | null }) {
  return (
    <span className="tree-row-content">
      <span className="tree-row-name mono">{rowName(node)}</span>
      {node.title !== null && <span className="tree-row-title">{node.title}</span>}
      <span className="tree-row-meta">
        <span className="kind-tag">
          <span className="sr-only">Kind: </span>
          {node.kind ?? "-"}
        </span>
        {node.status !== null && (
          <span className="status-tag">
            <span className="sr-only">Status: </span>
            {node.status}
          </span>
        )}
        {node.rev !== null && <span className="tree-row-rev">rev {node.rev}</span>}
        {node.archived && (
          <span className="plain-tag">
            <Icon name="archived" />
            archived
          </span>
        )}
        {node.mark !== null && <Badge look={markLook(node.mark)} />}
        {count !== null && count > 0 && (
          <span className="count-tag">
            <Icon name="inbox" />
            {count} in inbox
          </span>
        )}
        {drift !== null &&
          driftBadgeLooks(drift).map((look) => (
            <span key={look.tone} className="tree-drift-badge">
              <Badge name="Drift" look={look} />
            </span>
          ))}
      </span>
    </span>
  );
}

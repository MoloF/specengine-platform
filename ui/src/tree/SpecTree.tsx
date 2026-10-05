import { memo, useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type MouseEvent } from "react";
import type { TreeNode } from "../api/types";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { useFocusLater } from "../ui/useFocusLater";
import { markLook } from "./labels";
import { ancestorsOf, rowName, visibleRows, type TreeRow } from "./rows";

function rootsWithChildren(rows: readonly TreeRow[]): Set<string> {
  return new Set(rows.filter((row) => row.node.depth === 0 && row.children.length > 0).map((row) => row.key));
}

function RowContent({ node, count }: { node: TreeNode; count: number | null }) {
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
      </span>
    </span>
  );
}

/**
 * The containment tree as a WAI-ARIA tree: flat treeitems with level, set size and position,
 * `aria-expanded` on parents only, one tab stop that moves with the keys. Up and Down (k, j) move,
 * Right expands or enters, Left collapses or climbs, Home and End jump, Enter opens the row's node
 * (one history entry) and keeps focus here. Keys act only with focus on a row, never with Ctrl,
 * Alt or Cmd. The rows the route opens are marked current and revealed. Memoised: with stable
 * props from the view, a search or an inbox read renders no row again.
 */
export const SpecTree = memo(function SpecTree({
  rows,
  current,
  counts,
  onOpen,
  activeRef,
}: {
  rows: readonly TreeRow[];
  /** The rows the route's node is, by key. */
  current: readonly string[];
  /** Inbox items per row key; null while the inbox is unread or failed. */
  counts: ReadonlyMap<string, number> | null;
  onOpen: (row: TreeRow) => void;
  /** Receives the row that has the tab stop, so the view can focus it (Back to tree). */
  activeRef: (element: HTMLElement | null) => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const focusLater = useFocusLater();
  const [expanded, setExpanded] = useState<ReadonlySet<string> | null>(null);
  const [active, setActive] = useState<string | null>(null);
  const [revealed, setRevealed] = useState<string | null>(null);

  // Reveal the route's rows: once at the start (roots open) and whenever they change. Stored as
  // state from the previous render (react.dev "Storing information from previous renders").
  const signature = current.join("\n");
  let open = expanded;
  if (rows.length > 0 && (open === null || signature !== revealed)) {
    const next = new Set(open ?? rootsWithChildren(rows));
    const wanted = rows.filter((row) => current.includes(row.key));
    for (const row of wanted) {
      for (const ancestor of ancestorsOf(rows, row)) {
        next.add(ancestor.key);
      }
    }
    open = next;
    setExpanded(next);
    setRevealed(signature);
    const first = wanted[0];
    if (first !== undefined) {
      setActive(first.key);
    }
  }
  const shown = visibleRows(rows, open ?? new Set());
  const activeRow =
    shown.find((row) => row.key === active) ??
    (() => {
      // The active row went out of sight: its nearest shown ancestor takes the tab stop.
      const gone = rows.find((row) => row.key === active);
      const ancestor = gone === undefined ? undefined : ancestorsOf(rows, gone).find((row) => shown.includes(row));
      return ancestor ?? shown[0];
    })();

  // Bring a newly revealed row into view without taking focus; a re-read leaves the scroll alone.
  useEffect(() => {
    if (signature === "") {
      return;
    }
    const element = container.current?.querySelector<HTMLElement>('[role="treeitem"][aria-current="page"]');
    element?.scrollIntoView({ block: "nearest" });
  }, [signature]);

  function rowElement(key: string): HTMLElement | null {
    const items = container.current?.querySelectorAll<HTMLElement>('[role="treeitem"]') ?? [];
    return Array.from(items).find((item) => item.dataset.key === key) ?? null;
  }

  function moveTo(row: TreeRow | undefined) {
    if (row === undefined) {
      return;
    }
    setActive(row.key);
    focusLater(() => rowElement(row.key));
  }

  function setOpen(row: TreeRow, value: boolean) {
    setExpanded((currentSet) => {
      const next = new Set(currentSet ?? []);
      if (value) {
        next.add(row.key);
      } else {
        next.delete(row.key);
      }
      return next;
    });
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (hasModifier(event) || isTextField(event.target) || !(event.target instanceof HTMLElement)) {
      return;
    }
    // The row with focus is the one the keys act on (a click or a script may have moved it).
    const key = event.target.closest<HTMLElement>('[role="treeitem"]')?.dataset.key;
    const focusedRow = shown.find((candidate) => candidate.key === key);
    if (focusedRow === undefined) {
      return;
    }
    const at = shown.indexOf(focusedRow);
    const isOpen = open?.has(focusedRow.key) === true;
    const parent = focusedRow.parent === null ? undefined : rows[focusedRow.parent];
    switch (event.key) {
      case "ArrowDown":
      case "j":
        moveTo(shown[at + 1]);
        break;
      case "ArrowUp":
      case "k":
        moveTo(shown[at - 1]);
        break;
      case "Home":
        moveTo(shown[0]);
        break;
      case "End":
        moveTo(shown[shown.length - 1]);
        break;
      case "ArrowRight":
        if (focusedRow.children.length === 0) {
          break;
        }
        if (isOpen) {
          moveTo(rows[focusedRow.children[0] ?? -1]);
        } else {
          setOpen(focusedRow, true);
        }
        break;
      case "ArrowLeft":
        if (focusedRow.children.length > 0 && isOpen) {
          setOpen(focusedRow, false);
        } else {
          moveTo(parent);
        }
        break;
      case "Enter":
        onOpen(focusedRow);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  function onRowClick(row: TreeRow, event: MouseEvent<HTMLElement>) {
    const toggle = event.target instanceof Element && event.target.closest("[data-toggle]") !== null;
    setActive(row.key);
    if (toggle) {
      setOpen(row, open?.has(row.key) !== true);
      focusLater(() => rowElement(row.key));
      return;
    }
    onOpen(row);
  }

  return (
    <div ref={container} role="tree" aria-label="Spec tree" className="tree" onKeyDown={onKeyDown}>
      {shown.map((row) => {
        const node = row.node;
        const parent = row.children.length > 0;
        const isOpen = open?.has(row.key) === true;
        const isActive = row === activeRow;
        return (
          <div
            key={row.key}
            ref={isActive ? activeRef : undefined}
            role="treeitem"
            aria-level={node.depth + 1}
            aria-setsize={row.setSize}
            aria-posinset={row.posInSet}
            aria-expanded={parent ? isOpen : undefined}
            aria-current={current.includes(row.key) ? "page" : undefined}
            tabIndex={isActive ? 0 : -1}
            data-key={row.key}
            className="tree-row"
            style={{ "--depth": String(node.depth) } as CSSProperties}
            onFocus={() => {
              if (!isActive) {
                setActive(row.key);
              }
            }}
            onClick={(event) => {
              onRowClick(row, event);
            }}
          >
            <span className="tree-toggle" data-toggle={parent ? "" : undefined} aria-hidden="true">
              {parent && <Icon name={isOpen ? "chevronDown" : "chevronRight"} />}
            </span>
            <RowContent node={node} count={counts?.get(row.key) ?? null} />
          </div>
        );
      })}
    </div>
  );
});

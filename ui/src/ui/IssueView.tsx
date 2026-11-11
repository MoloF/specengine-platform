import type { ReactNode } from "react";

// The work-item layout (docs/features/design-foundation.md "Primitives (`ui/src/ui/`)"), a size container: a head of
// breadcrumbs and tools on one line, the caller's title (an `h1` on a page, a dialog's `h2` in a
// modal), a meta line under it (a document's work state and status), then the acts; a body of a main column and a side column at most 360 px wide. Under 44.5rem
// of its own width the body is one column, main first. The DOM order is main then side at every
// width, so reading and focus order never jump.

export function IssueView({
  breadcrumbs,
  tools,
  title,
  meta,
  actions,
  main,
  side,
}: {
  breadcrumbs: ReactNode;
  tools?: ReactNode;
  title: ReactNode;
  /** Under the title: what the item stands at, as lozenges (docs/features/doc-modal.md "Acceptance criteria", DM-12). */
  meta?: ReactNode;
  actions?: ReactNode;
  main: ReactNode;
  side: ReactNode;
}) {
  return (
    <div className="issue-view">
      <div className="issue-view-head">
        <div className="issue-view-bar">
          {breadcrumbs}
          {tools !== undefined && tools !== null && <div className="issue-view-tools">{tools}</div>}
        </div>
        {title}
        {meta !== undefined && meta !== null && <div className="issue-view-meta">{meta}</div>}
        {actions !== undefined && actions !== null && <div className="issue-view-actions">{actions}</div>}
      </div>
      <div className="issue-view-body">
        <div className="issue-view-main">{main}</div>
        <div className="issue-view-side">{side}</div>
      </div>
    </div>
  );
}

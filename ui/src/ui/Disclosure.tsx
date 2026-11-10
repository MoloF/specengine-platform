import { createElement, useId, useState, type ReactNode } from "react";
import { Icon } from "./Icon";

// A disclosure (docs/features/design-foundation.md "Primitives (`ui/src/ui/`)"), shared by a property list's "More
// fields" and the work item's Development: a button with `aria-expanded` and `aria-controls`, its
// region `hidden` while collapsed, so nothing inside is in the accessibility tree until it opens.
// Toggling leaves focus on the button; only the chevron turns, through the duration tokens. With
// `open` and `onToggle` the caller holds the choice (a document's Inside, kept per project); with
// `level` the button stands in a heading of that level, so a part that folds is still in the outline.

export function Disclosure({
  summary,
  regionLabel,
  className,
  open: held,
  onToggle,
  level,
  children,
}: {
  /** The button's words: they name it. */
  summary: ReactNode;
  /** Names the region a landmark when given (a `section`); else a plain block. */
  regionLabel?: string;
  /** Layout only. */
  className?: string;
  /** Held by the caller: expanded or not; uncontrolled (collapsed at first) when absent. */
  open?: boolean;
  /** The button was pressed: the state asked for. */
  onToggle?: (open: boolean) => void;
  /** The button inside a heading of this level. */
  level?: 2 | 3 | 4 | 5;
  children: ReactNode;
}) {
  const id = useId();
  const [own, setOwn] = useState(false);
  const open = held ?? own;
  const region = regionLabel === undefined ? (
    <div id={id} className="disclosure-region" hidden={!open}>
      {children}
    </div>
  ) : (
    <section id={id} className="disclosure-region" aria-label={regionLabel} hidden={!open}>
      {children}
    </section>
  );
  const button = (
    <button
      type="button"
      className="disclosure-button"
      aria-expanded={open}
      aria-controls={id}
      onClick={() => {
        setOwn(!open);
        onToggle?.(!open);
      }}
    >
      <Icon name="chevronRight" />
      <span className="disclosure-summary">{summary}</span>
    </button>
  );
  return (
    <div className={className === undefined ? "disclosure" : `disclosure ${className}`}>
      {level === undefined ? button : createElement(`h${String(level)}`, { className: "disclosure-heading" }, button)}
      {region}
    </div>
  );
}

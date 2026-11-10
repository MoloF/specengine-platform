import { useId, useState, type ReactNode } from "react";
import { Icon } from "./Icon";

// A disclosure (docs/features/design-foundation.md "Primitives (`ui/src/ui/`)"), shared by a property list's "More
// fields" and the work item's Development: a button with `aria-expanded` and `aria-controls`, its
// region `hidden` while collapsed, so nothing inside is in the accessibility tree until it opens.
// Toggling leaves focus on the button; only the chevron turns, through the duration tokens.

export function Disclosure({
  summary,
  regionLabel,
  className,
  children,
}: {
  /** The button's words: they name it. */
  summary: ReactNode;
  /** Names the region a landmark when given (a `section`); else a plain block. */
  regionLabel?: string;
  /** Layout only. */
  className?: string;
  children: ReactNode;
}) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const region = regionLabel === undefined ? (
    <div id={id} className="disclosure-region" hidden={!open}>
      {children}
    </div>
  ) : (
    <section id={id} className="disclosure-region" aria-label={regionLabel} hidden={!open}>
      {children}
    </section>
  );
  return (
    <div className={className === undefined ? "disclosure" : `disclosure ${className}`}>
      <button
        type="button"
        className="disclosure-button"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => {
          setOpen(!open);
        }}
      >
        <Icon name="chevronRight" />
        <span className="disclosure-summary">{summary}</span>
      </button>
      {region}
    </div>
  );
}

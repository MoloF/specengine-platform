import type { ReactNode } from "react";

// An empty state (docs/features/design-foundation.md "Primitives (`ui/src/ui/`)"): a heading at the level the caller
// picks, a short description, and up to two acts, at most one of them primary. At most 464 px wide,
// never worded as a failure, in sentence case: it says what is here and the next step.

export function EmptyState({
  title,
  titleId,
  titleLang,
  level,
  description,
  primary,
  secondary,
}: {
  title: string;
  /** The heading's id, for a field the empty state describes (Graph's REF). */
  titleId?: string;
  /** A title that is the daemon's words, tagged as served text is (docs/canon/ui-languages.md "Served text"). */
  titleLang?: "en";
  level: 2 | 3 | 4;
  description?: ReactNode;
  /** The one primary act, if any. */
  primary?: ReactNode;
  /** A second act, never primary. */
  secondary?: ReactNode;
}) {
  const Heading = level === 2 ? "h2" : level === 3 ? "h3" : "h4";
  const acts = primary !== undefined || secondary !== undefined;
  return (
    <div className="empty-state-box">
      <Heading id={titleId} className="empty-state-title" lang={titleLang}>
        {title}
      </Heading>
      {description !== undefined && <div className="empty-state-description">{description}</div>}
      {acts && (
        <div className="empty-state-acts">
          {primary}
          {secondary}
        </div>
      )}
    </div>
  );
}

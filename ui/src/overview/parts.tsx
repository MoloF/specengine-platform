import { useId, type ReactNode, type Ref } from "react";
import { RegionBoundary } from "../app/RegionBoundary";

/**
 * One region of the home: a `section` with its `h2`, a way to the full screen, and its own error
 * boundary, so a region that fails to render leaves the other standing. `busy` while its read is
 * first loading.
 */
export function Region({
  title,
  boundary,
  headingRef,
  busy,
  link,
  children,
}: {
  title: string;
  /** The region's name in its boundary's message ("The <name> could not be shown"). */
  boundary: string;
  headingRef: Ref<HTMLHeadingElement>;
  busy: boolean;
  /** The link to the region's screen, e.g. "Open Tasks". */
  link: ReactNode;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className="home-region" aria-labelledby={id} aria-busy={busy}>
      <header className="home-region-head">
        <h2 id={id} ref={headingRef} tabIndex={-1}>
          {title}
        </h2>
        {link}
      </header>
      <RegionBoundary name={boundary}>{children}</RegionBoundary>
    </section>
  );
}

/** The daemon's notes on a read, verbatim. */
export function Notes({ notes, label }: { notes: readonly string[]; label: string }) {
  if (notes.length === 0) {
    return null;
  }
  return (
    <ul className="note-list" aria-label={label}>
      {notes.map((note, index) => (
        <li key={`${String(index)}-${note}`} className="verbatim-note">
          {note}
        </li>
      ))}
    </ul>
  );
}

/** A count beside its label; read aloud "<label>: <count>". */
export function Count({ value }: { value: number }) {
  return (
    <>
      <span className="sr-only">:</span> <span className="home-count">{value}</span>
    </>
  );
}

/** A stored time exactly as stored (never relative, no clock read), with its meaning for screen readers. */
export function Stored({ label, at }: { label: string; at: string }) {
  return (
    <span className="home-row-time">
      <span className="sr-only">{label} </span>
      <time dateTime={at}>{at}</time>
    </span>
  );
}

// Served text as a screen shows it: the daemon's notes on a read, verbatim, and a stored time
// exactly as stored. Health and the Spec tree's panels share them (the Overview's old regions held
// them before docs/features/department-overview.md replaced those regions).

/** The daemon's notes on a read, verbatim. */
export function Notes({ notes, label }: { notes: readonly string[]; label: string }) {
  if (notes.length === 0) {
    return null;
  }
  return (
    <ul className="note-list" aria-label={label}>
      {notes.map((note, index) => (
        <li key={`${String(index)}-${note}`} className="verbatim-note" lang="en">
          {note}
        </li>
      ))}
    </ul>
  );
}

/** A stored time exactly as stored (never relative, no clock read), with its meaning for screen readers. */
export function Stored({ label, at }: { label: string; at: string }) {
  return (
    <span className="stored-time">
      <span className="sr-only">{label} </span>
      <time dateTime={at}>{at}</time>
    </span>
  );
}

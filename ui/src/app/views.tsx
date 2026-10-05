import { SECTIONS } from "./sections";

type BuiltLater = Exclude<(typeof SECTIONS)[number], { slice: null }>;

/** A section a later slice builds: its heading and that slice, no stand-in data. */
export function NotBuilt({ section }: { section: BuiltLater }) {
  return (
    <section className="view" aria-labelledby="view-title">
      <header className="view-head">
        <h1 id="view-title" tabIndex={-1}>
          {section.label}
        </h1>
        <p className="view-about">{section.about}</p>
      </header>
      <p className="not-built">
        Not built yet: arrives in slice <code>{section.slice}</code>.
      </p>
    </section>
  );
}

/** A hash that names nothing: says so and links the Inbox. */
export function NotFound({ reason, inboxHref }: { reason: string; inboxHref: string }) {
  return (
    <section className="view" aria-labelledby="view-title">
      <header className="view-head">
        <h1 id="view-title" tabIndex={-1}>
          Not found
        </h1>
      </header>
      <p>{reason}</p>
      <p>
        <a href={inboxHref}>Go to the Inbox</a>
      </p>
    </section>
  );
}

/** The fallback of one view: the error's message verbatim; the shell and its nav keep working. */
export function ViewFailure({ error, onRetry }: { error: Error; onRetry: () => void }) {
  return (
    <section className="view" aria-labelledby="view-title">
      <header className="view-head">
        <h1 id="view-title" tabIndex={-1}>
          This view failed
        </h1>
      </header>
      <div className="error-panel" role="alert">
        <p className="error-title">The rest of SpecEngine still works; pick another section or try again.</p>
        <p className="error-message">{error.message}</p>
        <button type="button" className="button" onClick={onRetry}>
          Try again
        </button>
      </div>
    </section>
  );
}

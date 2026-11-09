import { useTranslate } from "../i18n";

/** A hash that names nothing: says so and links the Inbox. */
export function NotFound({ reason, inboxHref }: { reason: string; inboxHref: string }) {
  const t = useTranslate();
  return (
    <section className="view" aria-labelledby="view-title">
      <header className="view-head">
        <h1 id="view-title" tabIndex={-1}>
          {t("app.notFound.title")}
        </h1>
      </header>
      <p>{reason}</p>
      <p>
        <a href={inboxHref}>{t("app.notFound.toInbox")}</a>
      </p>
    </section>
  );
}

/** The fallback of one view: the error's message verbatim; the shell and its nav keep working. */
export function ViewFailure({ error, onRetry }: { error: Error; onRetry: () => void }) {
  const t = useTranslate();
  return (
    <section className="view" aria-labelledby="view-title">
      <header className="view-head">
        <h1 id="view-title" tabIndex={-1}>
          {t("app.viewFailed.title")}
        </h1>
      </header>
      <div className="error-panel" role="alert">
        <p className="error-title">{t("app.viewFailed.text")}</p>
        <p className="error-message" lang="en">
          {error.message}
        </p>
        <button type="button" className="button" onClick={onRetry}>
          {t("app.tryAgain")}
        </button>
      </div>
    </section>
  );
}

import type { ReactNode } from "react";
import { useTranslate } from "../i18n";
import { Icon } from "../ui/Icon";
import { ErrorBoundary } from "./ErrorBoundary";

/** A region's fallback: its name in the catalog's sentence, the error's message verbatim, Try again. */
function RegionFailure({ name, heading, error, reset }: { name: string; heading?: ReactNode; error: Error; reset: () => void }) {
  const t = useTranslate();
  return (
    <>
      {heading !== undefined && (
        <h1 className="region-heading" tabIndex={-1}>
          {heading}
        </h1>
      )}
      <div className="error-panel">
        <div className="error-text" role="alert">
          <p className="error-title">
            <Icon name="alert" />
            <span>{t("app.regionFailed", { name })}</span>
          </p>
          <p className="error-message" lang="en">
            {error.message}
          </p>
        </div>
        <button type="button" className="button" onClick={reset}>
          <Icon name="retry" />
          <span>{t("app.tryAgain")}</span>
        </button>
      </div>
    </>
  );
}

/**
 * One region's own boundary (the tree, the node, each tab, the search, the task list, a task): a
 * render error there shows its message and "Try again" in place; the other regions and the shell
 * keep working. A region that holds the view's `h1` passes `heading`, kept above the message.
 */
export function RegionBoundary({ name, heading, children }: { name: string; heading?: ReactNode; children: ReactNode }) {
  return <ErrorBoundary fallback={(error, reset) => <RegionFailure name={name} heading={heading} error={error} reset={reset} />}>{children}</ErrorBoundary>;
}

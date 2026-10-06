import type { ReactNode } from "react";
import { Icon } from "../ui/Icon";
import { ErrorBoundary } from "./ErrorBoundary";

/**
 * One region's own boundary (the tree, the node, each tab, the search, the task list, a task): a
 * render error there shows its message and "Try again" in place; the other regions and the shell
 * keep working. A region that holds the view's `h1` passes `heading`, kept above the message.
 */
export function RegionBoundary({ name, heading, children }: { name: string; heading?: ReactNode; children: ReactNode }) {
  return (
    <ErrorBoundary
      fallback={(error, reset) => (
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
                <span>The {name} could not be shown; the rest of the page still works.</span>
              </p>
              <p className="error-message">{error.message}</p>
            </div>
            <button type="button" className="button" onClick={reset}>
              <Icon name="retry" />
              <span>Try again</span>
            </button>
          </div>
        </>
      )}
    >
      {children}
    </ErrorBoundary>
  );
}

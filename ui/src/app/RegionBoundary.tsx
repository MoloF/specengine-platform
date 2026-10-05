import type { ReactNode } from "react";
import { Icon } from "../ui/Icon";
import { ErrorBoundary } from "./ErrorBoundary";

/**
 * One region's own boundary (the tree, the node, each tab, the search): a render error there shows
 * its message and "Try again" in place; the other regions and the shell keep working.
 */
export function RegionBoundary({ name, children }: { name: string; children: ReactNode }) {
  return (
    <ErrorBoundary
      fallback={(error, reset) => (
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
      )}
    >
      {children}
    </ErrorBoundary>
  );
}

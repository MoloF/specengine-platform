import type { ReactNode } from "react";
import { apiErrorOf, isNotServed } from "../api/client";
import { Icon } from "./Icon";

/** A loading placeholder: grey bars, aria-busy, a label for screen readers. */
export function Skeleton({ label, lines = 3 }: { label: string; lines?: number }) {
  return (
    <div className="skeleton" aria-busy="true" aria-label={label} role="status">
      {Array.from({ length: lines }, (_, index) => (
        <span key={index} className="skeleton-bar" />
      ))}
    </div>
  );
}

/**
 * A failed read: what failed, the daemon's message verbatim, and Retry. Only the title and the
 * message are the alert; Retry sits outside it, so its label turning to Retrying never makes the
 * alert speak again. Each failure (`attempt`) is an alert of its own: a failed retry is spoken
 * again, a failure merely kept up while a retry runs is not. While retrying, Retry stays focusable
 * (aria-disabled) and does nothing, so a failed retry leaves focus on it.
 */
export function ErrorPanel({
  title,
  message,
  onRetry,
  retrying = false,
  attempt = 0,
}: {
  title: string;
  message: string;
  onRetry: () => void;
  retrying?: boolean;
  /** Counts the read's failures (the query's errorUpdateCount). */
  attempt?: number;
}) {
  return (
    <div className="error-panel">
      <div key={attempt} className="error-text" role="alert">
        <p className="error-title">
          <Icon name="alert" />
          <span>{title}</span>
        </p>
        <p className="error-message">{message}</p>
      </div>
      <button
        type="button"
        className="button"
        aria-disabled={retrying}
        onClick={() => {
          if (!retrying) {
            onRetry();
          }
        }}
      >
        <Icon name="retry" />
        <span>{retrying ? "Retrying" : "Retry"}</span>
      </button>
    </div>
  );
}

/**
 * A read that failed, as a screen shows it. A read the daemon does not serve yet (ClientError
 * `notServed`: nothing was requested) is not built, not broken: a note, the client's message
 * verbatim, no Retry, since asking again cannot change it. Anything else (the daemon down, no
 * response; a refusal in the daemon's words) is an ErrorPanel with Retry.
 */
export function ReadFailure({
  title,
  failure,
  onRetry,
  retrying = false,
  attempt = 0,
}: {
  title: string;
  failure: unknown;
  onRetry: () => void;
  retrying?: boolean;
  attempt?: number;
}) {
  const { message } = apiErrorOf(failure);
  if (isNotServed(failure)) {
    return (
      <div className="notice notice-not-served" role="status">
        <p className="notice-title">
          <Icon name="info" />
          <span>Not built yet: the daemon has no endpoint for this read</span>
        </p>
        <p className="error-message">{message}</p>
      </div>
    );
  }
  return <ErrorPanel title={title} message={message} onRetry={onRetry} retrying={retrying} attempt={attempt} />;
}

/** A section with a small heading inside a card. */
export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="card-section">
      <h3 className="card-section-title">{title}</h3>
      {children}
    </section>
  );
}

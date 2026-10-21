import { Fragment, useRef, useState, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import type { HealthQuery } from "../api/queries";
import type { HealthAssumes, HealthDrift, HealthReport, HealthUnbound } from "../api/types";
import { RegionBoundary } from "../app/RegionBoundary";
import { Icon } from "../ui/Icon";
import { ReadFailure, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetryFocus } from "../ui/useRetryFocus";
import { SHOWN_OPEN } from "./findings";
import { codeRowLook, MEASURED_WITHOUT_COUNTS } from "./labels";
import { oneLine } from "./oneLine";

// What rows 4 to 6 of What is left share (docs/features/health-code-ui.md "Data";
// docs/features/health-drift-ui.md "Description and interactions"): the part's frame and its four
// states, the fields as `spec health` prints them, and "Show all".

/** How many lines of a code row are rendered until "Show all": as many as a Findings group shows open. */
export const CODE_ROWS_SHOWN = SHOWN_OPEN;

/** "<n> <noun>", the noun in the singular for one. */
export function counted(count: number, one: string, many: string): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

/** A field on one line, as `spec health` prints it (`./oneLine.ts`, a leaf the label maps read too). */
export { oneLine };

/**
 * A code row the daemon did not measure (row 4, 5 or 6). Off or not configured, and any state word
 * not listed: "Not measured", the state's words and each cause, no digit, no Retry, since asking
 * again cannot change a configuration. A state of measured whose answer holds no counts or no rows
 * (outside the contract): the same, in fixed neutral words, never "Measured". Could not be
 * measured: the causes and a Retry reading the health read alone; no value at all, never 0.
 */
export function RowNotMeasured({ row, retrying, onRetry }: { row: HealthAssumes | HealthUnbound | HealthDrift; retrying: boolean; onRetry: () => void }) {
  const look = codeRowLook(row.state);
  const causes =
    row.causes.length === 0 ? null : (
      <ul className={look.retry ? "check-causes health-code-causes" : "note-list health-code-causes"} aria-label="Causes, as the daemon gives them">
        {row.causes.map((cause, index) => (
          <li key={`${String(index)}-${cause}`} className="verbatim" dir="auto">
            {oneLine(cause)}
          </li>
        ))}
      </ul>
    );
  if (look.retry) {
    return (
      <div className="health-code-cannot">
        <p className="health-code-cannot-title">
          <Icon name="cannotCheck" />
          <span>{oneLine(look.label)}</span>
        </p>
        {causes}
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
  return (
    <>
      <p className="health-left-total health-code-off">
        <span className="health-value">Not measured</span>: <span className="health-code-state">{oneLine(look.measured ? MEASURED_WITHOUT_COUNTS : look.label)}</span>
      </p>
      {causes}
    </>
  );
}

/**
 * A code row's lines in the daemon's order, never sorted here: the first CODE_ROWS_SHOWN rendered,
 * the rest not until "Show all <n>", which hands the focus to the first line it adds.
 */
export function CodeRowList<T>({ rows, label, order, render }: { rows: readonly T[]; label: string; order: string; render: (row: T) => ReactNode }) {
  const [all, setAll] = useState(false);
  const list = useRef<HTMLOListElement>(null);
  const focusLater = useFocusLater();
  const shown = all ? rows : rows.slice(0, CODE_ROWS_SHOWN);
  const hidden = rows.length - shown.length;
  return (
    <>
      <ol ref={list} className="health-left-list health-code-list" aria-label={label}>
        {shown.map((row, index) => (
          // The daemon's order is the lines' identity: a line of row 4 may repeat its ID and place (one per holder).
          <Fragment key={index}>{render(row)}</Fragment>
        ))}
      </ol>
      {hidden > 0 && (
        <div className="health-amendments-more">
          <p className="health-amendments-scope">
            The first {shown.length} of {rows.length}, {order}.
          </p>
          <button
            type="button"
            className="button"
            onClick={() => {
              setAll(true);
              focusLater(() => list.current?.querySelectorAll<HTMLElement>(":scope > li")[CODE_ROWS_SHOWN]);
            }}
          >
            Show all {rows.length}
          </button>
        </div>
      )}
    </>
  );
}

/** Draws a row from the report inside the part's boundary, so a report it cannot draw fails this part alone. */
function RowBody({ report, retry, draw }: { report: HealthReport; retry: () => void; draw: (report: HealthReport, retry: () => void) => ReactNode }) {
  return draw(report, retry);
}

/**
 * Row 4, 5 or 6 of What is left (docs/features/health-code-ui.md, docs/features/health-drift-ui.md
 * "Description and interactions"): its heading and its own boundary, over the view's health read.
 * Being measured: a skeleton, no digit. The read failed (a 503, an older daemon's 404, no
 * response): the daemon's words verbatim, shown but not spoken (row 3 speaks the same failure), and
 * a Retry reading `getHealth` alone. Answered: `children` draws the row from the report, its Retry
 * the same read's; a later failure is a note. `failure` and `busy` are What is left's, computed
 * once for the health read (LeftRegion).
 */
export function CodePart({
  health,
  failure,
  busy,
  row,
  title,
  what,
  children,
}: {
  health: HealthQuery;
  failure: Error | null;
  busy: boolean;
  /** The part's `data-row`: the report's key. */
  row: "assumes" | "unbound" | "drift";
  title: string;
  /** The row in a sentence, after "The": "code assuming a decision". */
  what: string;
  children: (report: HealthReport, retry: () => void) => ReactNode;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const retried = useRetryFocus(health.data, () => heading.current);
  const retry = () => {
    retried();
    void health.refetch({ cancelRefetch: false });
  };

  let body;
  if (health.data === undefined) {
    body =
      failure === null ? (
        <Skeleton label={`Measuring the ${what}`} lines={2} />
      ) : (
        <ReadFailure title={`The ${what} could not be read`} failure={failure} retrying={health.isFetching} attempt={health.errorUpdateCount} announce={false} onRetry={retry} />
      );
  } else {
    body = (
      <>
        <RowBody report={health.data} retry={retry} draw={children} />
        {health.error !== null && !health.isFetching && (
          <p className="note">
            The {what} could not be read again: {apiErrorOf(health.error).message}
          </p>
        )}
      </>
    );
  }

  return (
    <div className="health-part health-code-part" data-row={row} aria-busy={busy}>
      <h3 className="health-part-title" ref={heading} tabIndex={-1}>
        {title}
      </h3>
      <RegionBoundary name={what}>{body}</RegionBoundary>
    </div>
  );
}

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
import { codeRowLook, measuredWithoutCounts } from "./labels";
import { oneLine } from "./oneLine";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";

// What rows 4 to 6 of What is left share (docs/features/health-code-ui.md "Data";
// docs/features/health-drift-ui.md "Description and interactions"): the part's frame and its four
// states, the fields as `spec health` prints them, and "Show all".

/** How many lines of a code row are rendered until "Show all": as many as a Findings group shows open. */
export const CODE_ROWS_SHOWN = SHOWN_OPEN;

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
  const t = useTranslate();
  const look = codeRowLook(row.state, t);
  const causes =
    row.causes.length === 0 ? null : (
      <ul className={look.retry ? "check-causes health-code-causes" : "note-list health-code-causes"} aria-label={t("health.code.causes")}>
        {row.causes.map((cause, index) => (
          <li key={`${String(index)}-${cause}`} className="verbatim" dir="auto" lang="en">
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
          <span>{retrying ? t("ui.retrying") : t("ui.retry")}</span>
        </button>
      </div>
    );
  }
  return (
    <>
      <p className="health-left-total health-code-off">
        {fill(t("health.code.notMeasured", { word: slot("word"), state: slot("state") }), {
          word: <span className="health-value">{t("health.code.notMeasuredWord")}</span>,
          state: <span className="health-code-state">{oneLine(look.measured ? measuredWithoutCounts(t) : look.label)}</span>,
        })}
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
  const t = useTranslate();
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
          <p className="health-amendments-scope">{t("health.code.firstOf", { shown: shown.length, count: rows.length, order })}</p>
          <button
            type="button"
            className="button"
            onClick={() => {
              setAll(true);
              focusLater(() => list.current?.querySelectorAll<HTMLElement>(":scope > li")[CODE_ROWS_SHOWN]);
            }}
          >
            {t("health.code.showAll", { count: rows.length })}
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

/** Each code row's words: its heading, its states' lines, its boundary's name. */
const PART_WORDS = {
  assumes: {
    title: "health.assumes.title",
    measuring: "health.assumes.measuring",
    failed: "health.assumes.failed",
    failedAgain: "health.assumes.failedAgain",
    boundary: "health.assumes.boundary",
  },
  unbound: {
    title: "health.unboundRow.title",
    measuring: "health.unboundRow.measuring",
    failed: "health.unboundRow.failed",
    failedAgain: "health.unboundRow.failedAgain",
    boundary: "health.unboundRow.boundary",
  },
  drift: {
    title: "health.drift.title",
    measuring: "health.drift.measuring",
    failed: "health.drift.failed",
    failedAgain: "health.drift.failedAgain",
    boundary: "health.drift.boundary",
  },
} as const;

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
  children,
}: {
  health: HealthQuery;
  failure: Error | null;
  busy: boolean;
  /** The part's `data-row`: the report's key, and its words. */
  row: "assumes" | "unbound" | "drift";
  children: (report: HealthReport, retry: () => void) => ReactNode;
}) {
  const t = useTranslate();
  const words = PART_WORDS[row];
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
        <Skeleton label={t(words.measuring)} lines={2} />
      ) : (
        <ReadFailure title={t(words.failed)} failure={failure} retrying={health.isFetching} attempt={health.errorUpdateCount} announce={false} onRetry={retry} />
      );
  } else {
    body = (
      <>
        <RowBody report={health.data} retry={retry} draw={children} />
        {health.error !== null && !health.isFetching && (
          <p className="note">
            {fill(t(words.failedAgain, { message: slot("message") }), { message: <span lang="en">{apiErrorOf(health.error).message}</span> })}
          </p>
        )}
      </>
    );
  }

  return (
    <div className="health-part health-code-part" data-row={row} aria-busy={busy}>
      <h3 className="health-part-title" ref={heading} tabIndex={-1}>
        {t(words.title)}
      </h3>
      <RegionBoundary name={t(words.boundary)}>{body}</RegionBoundary>
    </div>
  );
}

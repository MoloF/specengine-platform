import { useId, useMemo } from "react";
import { apiErrorOf } from "../api/client";
import { useCard, type ChildrenQuery } from "../api/queries";
import type { Card, CardFlow, CardRow } from "../api/types";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Badge } from "../ui/Badge";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Lozenge } from "../ui/Lozenge";
import { Skeleton } from "../ui/states";
import { shownSlot, shownText } from "../ui/text";
import { formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { statusLook } from "./category";
import { CardLink, type CardLinks } from "./CardLink";
import { flowTitleOf } from "./filter";
import { cancelledLook, handOffLook, progressMark, progressWords, readyLook, unassigned } from "./labels";
import { progressOf, type HandOff, type ProgressSegment } from "./rollup";

// A work item's Subtasks (docs/features/handoff-progress.md "Acceptance criteria", HP-1 to HP-6): its
// children read beside the card, cancelled ones too, as the progress model in rollup.ts counts them.
// In order: Ready to accept on an open parent whose every child is accepted or cancelled, a status and
// never a control; the bar and its words line; what the task relies on; the subtasks still standing;
// the cancelled ones with their reasons, one card read each. Nothing here moves, closes or waits on a
// card, and Accept stands where the acts put it (ADR-0074, ADR-0012); every word is looked up.

/** A stored UTC time, as stored to the minute. */
function At({ at }: { at: string }) {
  const t = useTranslate();
  return <time dateTime={at}>{formatUtc(at, t)}</time>;
}

/**
 * A read's failure in the daemon's words, under `lang="en"`, with Retry. The children read's is an
 * alert, each failure spoken once; a reason's is not (`live` false), so N cancelled subtasks failing
 * never make N assertive announcements.
 */
function ReadFailed({ words, message, retrying, attempt, live = true, onRetry }: { words: string; message: string; retrying: boolean; attempt: number; live?: boolean; onRetry: () => void }) {
  const t = useTranslate();
  return (
    <div className="board-progress-failed">
      <p key={attempt} className="note" role={live ? "alert" : undefined}>
        <Icon name="alert" />
        <span>{fill(words, { message: <span lang="en">{message}</span> })}</span>
      </p>
      <Button
        icon="retry"
        spacing="compact"
        aria-disabled={retrying}
        onClick={() => {
          if (!retrying) {
            onRetry();
          }
        }}
      >
        {retrying ? t("ui.retrying") : t("ui.retry")}
      </Button>
    </div>
  );
}

/** The bar, hidden from assistive technology, and its words line, which says every count with its icon: never colour alone. */
function ProgressBar({ segments }: { segments: readonly ProgressSegment[] }) {
  const t = useTranslate();
  return (
    <div className="board-progress">
      <div className="progress-bar" aria-hidden="true">
        {segments.map((segment) => (
          <span key={segment.word} className="progress-segment" data-appearance={progressMark(segment.word).appearance} style={{ flexGrow: segment.count }} />
        ))}
      </div>
      <p className="progress-words">
        {segments.map((segment, index) => (
          <span key={segment.word} className="progress-word" data-word={segment.word}>
            {index > 0 && (
              <span className="progress-separator" aria-hidden="true">
                {"\u00b7"}
              </span>
            )}
            <Icon name={progressMark(segment.word).icon} />
            <span className="progress-count">{progressWords(segment.word, segment.count, t)}</span>
          </span>
        ))}
      </p>
    </div>
  );
}

/** What an open task relies on: its children neither accepted nor cancelled, each its ID (a link), its title and its flow's title, as Subtasks' rows; never an item, never an agent task. */
function ReliesOn({ rows, flowOf, links }: { rows: readonly CardRow[]; flowOf: (name: string) => CardFlow | undefined; links: CardLinks }) {
  const t = useTranslate();
  const labelId = useId();
  return (
    <div className="board-progress-relies">
      <p id={labelId} className="board-progress-label">
        {t("board.progress.reliesOn")}
      </p>
      <ul className="board-panel-list" aria-labelledby={labelId}>
        {rows.map((row) => (
          <li key={row.id} className="board-handoff">
            <CardLink id={row.id} links={links} />
            <bdi className="board-handoff-title">{shownSlot(row.title)}</bdi>
            <bdi className="muted">{shownSlot(flowTitleOf(flowOf(row.flow), row.flow))}</bdi>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The subtasks still standing: ID (a link), title, the flow's title, its status lozenge, assignee and word; a child of a flow not served has no word. */
function SubtaskRows({ rows, flowOf, links }: { rows: readonly HandOff[]; flowOf: (name: string) => CardFlow | undefined; links: CardLinks }) {
  const t = useTranslate();
  return (
    <ul className="board-panel-list">
      {rows.map(({ row, word }) => (
        <li key={row.id} className="board-handoff">
          <CardLink id={row.id} links={links} />
          <bdi className="board-handoff-title">{shownSlot(row.title)}</bdi>
          <bdi className="muted">{shownSlot(flowTitleOf(flowOf(row.flow), row.flow))}</bdi>
          <Lozenge {...statusLook(flowOf(row.flow), row.column)} />
          <span className={row.assignee === null ? "muted" : undefined}>
            <bdi>{row.assignee === null ? unassigned(t) : shownSlot(row.assignee)}</bdi>
          </span>
          {word !== null && <Badge look={handOffLook(word, t)} />}
        </li>
      ))}
    </ul>
  );
}

/** Why a cancelled child was cancelled: its own card read (shared with its modal), pending, failed or answered. */
function CancelReason({ project, id }: { project: string; id: string }) {
  const t = useTranslate();
  const query = useCard(project, id);
  const failure = useRetainedFailure(query.error, query.isFetching);
  const answer = query.data;
  if (answer === undefined) {
    return failure === null ? (
      <Skeleton label={t("board.progress.reasonReading")} lines={1} />
    ) : (
      <ReadFailed
        words={t("board.progress.reasonFailed", { message: slot("message") })}
        message={apiErrorOf(failure).message}
        retrying={query.isFetching}
        attempt={query.errorUpdateCount}
        live={false}
        onRetry={() => {
          void query.refetch({ cancelRefetch: false });
        }}
      />
    );
  }
  if (!("title" in answer)) {
    // No such card now: its `{id, reason}` in the daemon's words (a card read's answer, as CardPanel.tsx `isCard` tells it).
    return (
      <p className="note">
        <Icon name="alert" />
        <span>{fill(t("board.progress.reasonFailed", { message: slot("message") }), { message: <span lang="en">{answer.reason}</span> })}</span>
      </p>
    );
  }
  return answer.cancellation === null ? null : <Cancellation by={answer.cancellation.by} at={answer.cancellation.at} reason={answer.cancellation.reason} />;
}

function Cancellation({ by, at, reason }: { by: string; at: string; reason: string }) {
  const t = useTranslate();
  return (
    <>
      <p className="board-text">
        <bdi>{shownText(reason)}</bdi>
      </p>
      <p className="muted">{fill(t("board.panel.byAt", { by: slot("by"), at: slot("at") }), { by: <bdi>{shownSlot(by)}</bdi>, at: <At at={at} /> })}</p>
    </>
  );
}

/** The cancelled subtasks, a labelled list: ID, title, flow, status, Cancelled, then the reason and who cancelled it when. */
function CancelledList({ project, rows, flowOf, links }: { project: string; rows: readonly CardRow[]; flowOf: (name: string) => CardFlow | undefined; links: CardLinks }) {
  const t = useTranslate();
  const labelId = useId();
  return (
    <div className="board-progress-cancelled">
      <p id={labelId} className="board-progress-label">
        {t("board.progress.cancelledList")}
      </p>
      <ul className="board-panel-list" aria-labelledby={labelId}>
        {rows.map((row) => (
          <li key={row.id} className="board-progress-cancel">
            <span className="board-handoff">
              <CardLink id={row.id} links={links} />
              <bdi className="board-handoff-title">{shownSlot(row.title)}</bdi>
              <bdi className="muted">{shownSlot(flowTitleOf(flowOf(row.flow), row.flow))}</bdi>
              <Lozenge {...statusLook(flowOf(row.flow), row.column)} />
              <Badge look={cancelledLook(t)} />
            </span>
            <CancelReason project={project} id={row.id} />
          </li>
        ))}
      </ul>
    </div>
  );
}

/**
 * The Subtasks part's body: a skeleton while the children read is out; its failure in the daemon's
 * words with Retry; else Ready, the bar, Relies on, the rows and the cancelled ones. A read failing
 * again keeps the last parts, saying so. No count is ever taken from the Board's list in its place.
 */
export function CardProgress({ card, query, links }: { card: Card; query: ChildrenQuery; links: CardLinks }) {
  const t = useTranslate();
  const failure = useRetainedFailure(query.error, query.isFetching);
  const list = query.data;
  const open = card.acceptance === null && card.cancellation === null;
  const progress = useMemo(() => (list === undefined ? null : progressOf(list.cards, list.flows, { id: card.id, open })), [list, card.id, open]);
  const retry = () => {
    void query.refetch({ cancelRefetch: false });
  };
  const failed =
    failure === null ? null : (
      <ReadFailed words={t("board.progress.failed", { message: slot("message") })} message={apiErrorOf(failure).message} retrying={query.isFetching} attempt={query.errorUpdateCount} onRetry={retry} />
    );
  if (list === undefined || progress === null) {
    return failed ?? <Skeleton label={t("board.progress.reading")} lines={3} />;
  }
  const flowOf = (name: string) => list.flows.find((flow) => flow.name === name);
  const total = progress.children.length + progress.cancelled.length;
  return (
    <>
      {failed}
      {progress.ready && (
        <p className="board-progress-ready">
          <Lozenge {...readyLook(t)} />
          <span>{t("board.progress.readyAbout")}</span>
        </p>
      )}
      {total > 0 && <ProgressBar segments={progress.segments} />}
      {progress.reliesOn.length > 0 && <ReliesOn rows={progress.reliesOn} flowOf={flowOf} links={links} />}
      {total === 0 ? <p className="muted">{t("board.panel.noneYet")}</p> : progress.children.length > 0 && <SubtaskRows rows={progress.children} flowOf={flowOf} links={links} />}
      {progress.cancelled.length > 0 && <CancelledList project={links.project} rows={progress.cancelled} flowOf={flowOf} links={links} />}
    </>
  );
}

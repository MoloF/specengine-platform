import { useEffect, useMemo, useRef } from "react";
import { apiErrorOf } from "../api/client";
import { useTraceLanes, type TraceQuery } from "../api/queries";
import type { TraceNode, TraceStage } from "../api/types";
import { sectionHash } from "../app/routes";
import { TraceBar } from "../trace/TraceBar";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { shownSlot } from "../ui/text";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { useTranslate } from "../i18n";
import { confirmedLine, countParts, isNoCard, lastReadWords, noCardsYet, noStagesYet, rollupLook, stageWords, stepWords } from "./labels";
import { laneTotal, noCardAnywhere, noStageAnywhere, stageAsked, stageKey, stepsInOrder } from "./timeline";

type LaneRead = ReturnType<typeof useTraceLanes>[number];

/** A step of an open lane: its name, what it reads, how far it is along, its rollup. A step with no card keeps its place, hollow. */
function Step({ project, step, selected }: { project: string; step: TraceNode; selected: boolean }) {
  const t = useTranslate();
  const classes = ["timeline-step", isNoCard(step.rollup) && "is-hollow"].filter(Boolean).join(" ");
  return (
    <li className={classes} data-flow-row={step.id} aria-current={selected ? "true" : undefined}>
      <a className="mono timeline-step-name" href={sectionHash(project, "flow", step.id)}>
        <bdi>{shownSlot(step.id)}</bdi>
      </a>
      {step.title !== null && (
        <span className="timeline-step-title">
          <bdi>{shownSlot(step.title)}</bdi>
        </span>
      )}
      <span className="muted timeline-step-place">{stepWords(step.distance, t)}</span>
      <Badge look={rollupLook(step.rollup, t)} />
    </li>
  );
}

/** A lane's steps by `distance`: a skeleton while they are read, a line and Retry when they fail, the picture kept. */
function LaneSteps({ project, read, label, selected, afterRetry }: { project: string; read: LaneRead | undefined; label: string; selected: string | null; afterRetry: () => HTMLElement | null }) {
  const t = useTranslate();
  const failure = useRetainedFailure(read?.error ?? null, read?.fetching ?? false);
  const steps = useMemo(() => stepsInOrder(read?.data?.nodes ?? []), [read?.data]);
  const retried = useRetryFocus(read === undefined || read.updatedAt === 0 ? undefined : read.updatedAt, afterRetry);
  const retry = () => {
    retried();
    void read?.retry({ cancelRefetch: false });
  };
  if (read?.data === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={`Reading the steps of ${label}`} lines={2} />
      </div>
    ) : (
      <ErrorPanel title={`The steps of ${label} could not be read`} message={apiErrorOf(failure).message} retrying={read?.fetching} attempt={read?.errorCount} onRetry={retry} />
    );
  }
  return (
    <>
      {read.error !== null && (
        <div className="flow-failed">
          <p className="note" role="alert">
            <Icon name="alert" />
            <span>
              The steps of {label} could not be read again: {apiErrorOf(read.error).message}
              <br />
              {lastReadWords(t)}
            </span>
          </p>
          <button type="button" className="button" aria-disabled={read.fetching} onClick={read.fetching ? undefined : retry}>
            <Icon name="retry" />
            <span>{read.fetching ? "Retrying" : "Retry"}</span>
          </button>
        </div>
      )}
      {steps.length === 0 ? (
        <p className="muted">No step stands in this stage now.</p>
      ) : (
        <ol className="timeline-steps" aria-label={`Steps of ${label}`}>
          {steps.map((step) => (
            <Step key={`${step.path}:${step.id}`} project={project} step={step} selected={step.id === selected} />
          ))}
        </ol>
      )}
    </>
  );
}

/** One lane: the stage, how much of it is confirmed, the bar with each part in words; open, its steps. */
function Lane({
  project,
  lane,
  open,
  read,
  selected,
  afterRetry,
  onToggle,
}: {
  project: string;
  lane: TraceStage;
  open: boolean;
  read: LaneRead | undefined;
  selected: string | null;
  /** Where focus goes once a Retry took its button away. */
  afterRetry: () => HTMLElement | null;
  onToggle: (key: string) => void;
}) {
  const t = useTranslate();
  const label = stageWords(lane.stage, t);
  return (
    <li className="timeline-lane">
      <h2 className="timeline-lane-title">
        <button
          type="button"
          className="timeline-lane-head"
          aria-expanded={open}
          onClick={() => {
            onToggle(stageKey(lane.stage));
          }}
        >
          <Icon name={open ? "chevronDown" : "chevronRight"} />
          <bdi className="timeline-stage">{label}</bdi>
          <span className="sr-only">: </span>
          <span className="timeline-score">{confirmedLine(lane.counts.confirmed, laneTotal(lane.counts), t)}</span>
        </button>
      </h2>
      <TraceBar counts={lane.counts} />
      <ul className="timeline-parts" aria-label={`What ${label} holds`}>
        {countParts(lane.counts, t).map((part) => (
          <li key={part.key}>
            <Badge look={{ ...part.look, label: part.words }} />
          </li>
        ))}
      </ul>
      {open && <LaneSteps project={project} read={read} label={label} selected={selected} afterRetry={afterRetry} />}
    </li>
  );
}

/**
 * The Timeline (docs/features/trace-views.md "Timeline"): lanes by stage, as served, "No stage" last,
 * each from its served counts. On load the summary alone, then one read per open lane, never a whole
 * read; `card.*` events read the lanes and what is open again, so it fills as cards are accepted. A
 * person sees how far each stage has come; nothing here changes a status or a control (ADR-0012).
 */
export function TimelineView({
  project,
  read,
  openLanes,
  selected,
  noted,
  onToggle,
}: {
  project: string;
  /** The summary read, shared with the stage filter. */
  read: TraceQuery;
  /** The lanes opened, by `stageKey`. */
  openLanes: readonly string[];
  selected: string | null;
  /** The notes the view already shows: a note is said once. */
  noted: readonly string[];
  onToggle: (key: string) => void;
}) {
  const t = useTranslate();
  // The open lanes read again as this mounts; the score beside their steps goes with them.
  const { refetch } = read;
  useEffect(() => {
    void refetch({ cancelRefetch: false });
  }, [refetch]);
  const failure = useRetainedFailure(read.error, read.isFetching);
  const summary = read.data;
  const stages = useMemo(() => summary?.stages ?? [], [summary]);
  const asked = useMemo(() => stages.map((lane) => stageAsked(lane.stage)), [stages]);
  const opened = useMemo(() => new Set(stages.filter((lane) => openLanes.includes(stageKey(lane.stage))).map((lane) => stageAsked(lane.stage))), [stages, openLanes]);
  const lanes = useTraceLanes(project, asked, opened);
  const laneList = useRef<HTMLOListElement>(null);
  const retried = useRetryFocus(read.dataUpdatedAt === 0 ? undefined : read.dataUpdatedAt, () => laneList.current);
  const retry = () => {
    retried();
    void refetch({ cancelRefetch: false });
  };

  if (summary === undefined) {
    return failure === null ? (
      <div aria-busy="true">
        <Skeleton label={`Reading the stages of ${project}`} lines={4} />
      </div>
    ) : (
      <ErrorPanel title="The stages could not be read" message={apiErrorOf(failure).message} retrying={read.isFetching} attempt={read.errorUpdateCount} onRetry={retry} />
    );
  }

  const notes = summary.notes.filter((note) => !noted.includes(note));
  return (
    <div className="timeline">
      {notes.length > 0 && (
        <ul className="note-list" aria-label="Notes from the daemon on the stages">
          {notes.map((note, index) => (
            <li key={`${String(index)}-${note}`}>{note}</li>
          ))}
        </ul>
      )}
      {read.error !== null && (
        <div className="flow-failed">
          <p className="note" role="alert">
            <Icon name="alert" />
            <span>
              The stages could not be read again: {apiErrorOf(read.error).message}
              <br />
              {lastReadWords(t)}
            </span>
          </p>
          <button type="button" className="button" aria-disabled={read.isFetching} onClick={read.isFetching ? undefined : retry}>
            <Icon name="retry" />
            <span>{read.isFetching ? "Retrying" : "Retry"}</span>
          </button>
        </div>
      )}
      {noCardAnywhere(summary.counts) && (
        <div className="empty-state">
          <p>{noCardsYet(t)}</p>
        </div>
      )}
      {noStageAnywhere(stages) && (
        <div className="empty-state">
          <p>{noStagesYet(t)}</p>
        </div>
      )}
      <ol ref={laneList} tabIndex={-1} className="timeline-lanes">
        {stages.map((lane, index) => (
          <Lane
            key={stageKey(lane.stage)}
            project={project}
            lane={lane}
            open={opened.has(stageAsked(lane.stage))}
            read={lanes[index]}
            selected={selected}
            afterRetry={() => laneList.current}
            onToggle={onToggle}
          />
        ))}
      </ol>
    </div>
  );
}

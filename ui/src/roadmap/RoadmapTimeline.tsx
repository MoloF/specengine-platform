import { memo, useId, useMemo, useRef, useState, type ChangeEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useDelivery } from "../api/queries";
import type { DeliveryDepartment, DeliveryDocument, DeliveryMarker, DeliveryTask } from "../api/types";
import { sectionHash } from "../app/routes";
import { useLanguage, useTranslate, type Translate } from "../i18n";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { Lozenge, type LozengeLook } from "../ui/Lozenge";
import { ErrorPanel, Skeleton } from "../ui/states";
import { formatUtc } from "../ui/time";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { groupKey, groupLabel, lateMark } from "./delivery";
import { dayOf, dayWords, percent, pointOf, stretchOf, tickLabel, ticksOf, TIMELINE_PAGE } from "./timeline";

// The Delivery tab's Timeline view (docs/features/delivery-timeline.md "Acceptance criteria", DT8,
// DT9, DT10), its own chunk: every task the daemon serves on one axis, as served. A row per task in
// served order under its group's heading and markers in words; on a track hidden from assistive
// tech, waiting from creation to its take (else the day asked), work from the take to its accept
// (else the day asked), an accept mark and a due tick, each placed between the served ends by
// percentage alone. Each ID links the Board and is described by its title and its dates in words, so
// a screen reader hears the row whole; "Show as table" is the same rows and dates as a table. The first 200
// rows, then 200 more a press. Nothing here judges lateness, a release, a department or a take: the
// late mark is the served days', a department keeps the rows that name it. Read only (ADR-0012).

/** The axis' served ends and the day asked, as day numbers. */
interface Axis {
  from: number;
  to: number;
  asOf: number | null;
}

/** A run of shown rows of one group, in served order: its first row, that group's marker, where it starts among the shown rows. */
interface Section {
  key: string;
  head: DeliveryTask;
  marker: DeliveryMarker | undefined;
  offset: number;
  tasks: DeliveryTask[];
}

/** The shown rows cut into runs of one group each, served order kept; each run's marker looked up by its group. */
function sectionsOf(tasks: readonly DeliveryTask[], markers: readonly DeliveryMarker[]): Section[] {
  const byGroup = new Map(markers.map((marker) => [groupKey(marker), marker]));
  const sections: Section[] = [];
  tasks.forEach((task, offset) => {
    const key = groupKey(task);
    const last = sections.at(-1);
    if (last?.key === key) {
      last.tasks.push(task);
    } else {
      sections.push({ key, head: task, marker: byGroup.get(key), offset, tasks: [task] });
    }
  });
  return sections;
}

/** A group's markers in words: its first take and its last accept, or that there is none yet. */
function markerWords(marker: DeliveryMarker, language: string, t: Translate): string {
  const taken = marker.first_taken_at === null ? t("roadmap.delivery.timeline.noneTaken") : t("roadmap.delivery.timeline.firstTaken", { day: dayWords(marker.first_taken_at, language, t) });
  const accepted =
    marker.last_accepted_at === null ? t("roadmap.delivery.timeline.noneAccepted") : t("roadmap.delivery.timeline.lastAccepted", { day: dayWords(marker.last_accepted_at, language, t) });
  return t("roadmap.delivery.timeline.markers", { taken, accepted });
}

/** A row's dates in words, the missing ones left out, then its served lateness: what its link's description says. */
function datesInWords(task: DeliveryTask, late: LozengeLook | null, language: string, t: Translate): string {
  const words = (stored: string) => ({ day: dayWords(stored, language, t) });
  return [
    t("roadmap.delivery.timeline.created", words(task.created_at)),
    task.taken_at === null ? null : t("roadmap.delivery.timeline.taken", words(task.taken_at)),
    task.accepted_at === null ? null : t("roadmap.delivery.timeline.accepted", words(task.accepted_at)),
    task.due === null ? null : t("roadmap.delivery.timeline.due", words(task.due)),
    late === null ? null : `${late.label}.`,
  ]
    .filter((sentence) => sentence !== null)
    .join(" ");
}

/** A stretch's inline place, the one inline style a mark takes. */
function placed(start: number, width?: number) {
  return width === undefined ? { insetInlineStart: percent(start) } : { insetInlineStart: percent(start), width: percent(width) };
}

/** A row's track, hidden from assistive tech: waiting, work with its accept mark, the due tick. */
function Track({ task, axis }: { task: DeliveryTask; axis: Axis }) {
  const { from, to, asOf } = axis;
  const created = dayOf(task.created_at);
  const taken = task.taken_at === null ? null : dayOf(task.taken_at);
  const accepted = task.accepted_at === null ? null : dayOf(task.accepted_at);
  const due = task.due === null ? null : dayOf(task.due);
  const waitingEnd = taken ?? asOf;
  const workEnd = accepted ?? asOf;
  const waiting = created === null || waitingEnd === null ? null : stretchOf(created, waitingEnd, from, to);
  const work = taken === null || workEnd === null ? null : stretchOf(taken, workEnd, from, to);
  return (
    <span className="delivery-timeline-track" aria-hidden="true">
      {waiting !== null && <span className="delivery-timeline-waiting" style={placed(waiting.start, waiting.width)} />}
      {work !== null && <span className="delivery-timeline-work" data-accepted={accepted !== null} style={placed(work.start, work.width)} />}
      {due !== null && <span className="delivery-timeline-due" style={placed(pointOf(due, from, to))} />}
    </span>
  );
}

/**
 * One row: the ID to the Board, described by its title and then its dates; the title, the served late
 * mark, the track. The dates line is `hidden`: a description alone, so browse mode reads the lateness
 * once, from the mark. Drawn again only when its own props change, so 200 more rows leave the shown
 * ones be.
 */
const Row = memo(function Row({ project, task, axis, rowId }: { project: string; task: DeliveryTask; axis: Axis; rowId: string }) {
  const t = useTranslate();
  const { language } = useLanguage();
  const late = lateMark(task, t);
  const titleId = `${rowId}-title`;
  const datesId = `${rowId}-dates`;
  return (
    <li className="delivery-timeline-row" data-task={task.id}>
      <a href={sectionHash(project, "board", task.id)} aria-describedby={`${titleId} ${datesId}`}>
        {task.id}
      </a>
      <bdi id={titleId} className="delivery-timeline-title">
        {task.title}
      </bdi>
      {late !== null && <Lozenge {...late} />}
      <span id={datesId} hidden>
        {datesInWords(task, late, language, t)}
      </span>
      <Track task={task} axis={axis} />
    </li>
  );
});

/** A group's heading, its markers in words and lines, then its rows. */
function Group({ project, section, axis, idBase }: { project: string; section: Section; axis: Axis; idBase: string }) {
  const t = useTranslate();
  const { language } = useLanguage();
  const headingId = useId();
  const { marker } = section;
  const label = groupLabel(marker ?? section.head, t);
  const firstTaken = marker?.first_taken_at == null ? null : dayOf(marker.first_taken_at);
  const lastAccepted = marker?.last_accepted_at == null ? null : dayOf(marker.last_accepted_at);
  return (
    <section className="delivery-timeline-group" aria-labelledby={headingId}>
      <h2 id={headingId} className="delivery-timeline-heading">
        <bdi lang={label.served ? "en" : undefined}>{label.text}</bdi>
      </h2>
      {marker !== undefined && <p className="delivery-timeline-markers">{markerWords(marker, language, t)}</p>}
      <div className="delivery-timeline-body">
        <div className="delivery-timeline-lines" aria-hidden="true">
          {firstTaken !== null && <span className="delivery-timeline-first" style={placed(pointOf(firstTaken, axis.from, axis.to))} />}
          {lastAccepted !== null && <span className="delivery-timeline-last" style={placed(pointOf(lastAccepted, axis.from, axis.to))} />}
          {axis.asOf !== null && <span className="delivery-timeline-as-of" style={placed(pointOf(axis.asOf, axis.from, axis.to))} />}
        </div>
        <ol className="delivery-timeline-rows">
          {section.tasks.map((task, index) => (
            <Row key={task.id} project={project} task={task} axis={axis} rowId={`${idBase}-${String(section.offset + index)}`} />
          ))}
        </ol>
      </div>
    </section>
  );
}

/** The axis above the groups, hidden from assistive tech: the ticks, a tenth of the axis apart at least, a Monday's light; the As of mark. */
function AxisScale({ axis }: { axis: Axis }) {
  const t = useTranslate();
  const { language } = useLanguage();
  const ticks = useMemo(() => ticksOf(axis.from, axis.to), [axis.from, axis.to]);
  return (
    <div className="delivery-timeline-axis" aria-hidden="true">
      {ticks.map((tick) => (
        <span key={tick.day} className="delivery-timeline-tick" data-month={tick.label !== "monday"} style={placed(pointOf(tick.day, axis.from, axis.to))}>
          {tickLabel(tick, language)}
        </span>
      ))}
      {axis.asOf !== null && (
        <span className="delivery-timeline-as-of-mark" style={placed(pointOf(axis.asOf, axis.from, axis.to))}>
          {t("roadmap.delivery.asOf")}
        </span>
      )}
    </div>
  );
}

/** The same rows as a table: a row per task, its dates in UTC as stored, its served lateness. */
function TimelineTable({ project, tasks }: { project: string; tasks: readonly DeliveryTask[] }) {
  const t = useTranslate();
  const captionId = useId();
  const none = t("ui.emptyValue");
  const stamp = (stored: string | null) => (stored === null ? none : <time dateTime={stored}>{formatUtc(stored, t)}</time>);
  return (
    <div className="delivery-table-wrap" role="region" aria-labelledby={captionId} tabIndex={0}>
      <table className="delivery-table delivery-timeline-table">
        <caption id={captionId}>{t("roadmap.delivery.views.timeline")}</caption>
        <thead>
          <tr>
            <th scope="col">{t("roadmap.delivery.timeline.task")}</th>
            <th scope="col">{t("roadmap.delivery.column.release")}</th>
            <th scope="col">{t("roadmap.delivery.property.created")}</th>
            <th scope="col">{t("roadmap.delivery.property.taken")}</th>
            <th scope="col">{t("roadmap.delivery.property.accepted")}</th>
            <th scope="col">{t("roadmap.delivery.property.due")}</th>
            <th scope="col">{t("roadmap.delivery.timeline.late")}</th>
          </tr>
        </thead>
        <tbody>
          {tasks.map((task) => {
            const label = groupLabel(task, t);
            const late = lateMark(task, t);
            return (
              <tr key={task.id} data-task={task.id}>
                <th scope="row">
                  <a href={sectionHash(project, "board", task.id)}>{task.id}</a> <bdi>{task.title}</bdi>
                </th>
                <td>
                  <bdi lang={label.served ? "en" : undefined}>{label.text}</bdi>
                </td>
                <td>{stamp(task.created_at)}</td>
                <td>{stamp(task.taken_at)}</td>
                <td>{stamp(task.accepted_at)}</td>
                <td>{task.due ?? none}</td>
                <td>{late === null ? none : late.label}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** The department filter's choices: All, then each served department by its title, in served order. */
function DepartmentField({ departments, value, onChange }: { departments: readonly DeliveryDepartment[]; value: string; onChange: (value: string) => void }) {
  const t = useTranslate();
  const id = useId();
  return (
    <div className="delivery-timeline-field">
      <label className="field-label" htmlFor={id}>
        {t("roadmap.delivery.timeline.department")}
      </label>
      <select
        id={id}
        className="field"
        value={value}
        onChange={(event: ChangeEvent<HTMLSelectElement>) => {
          onChange(event.target.value);
        }}
      >
        <option value="">{t("roadmap.delivery.timeline.all")}</option>
        {departments.map((department) => (
          <option key={department.name} value={department.name}>
            {department.title}
          </option>
        ))}
      </select>
    </div>
  );
}

/** A served document's timeline: the filter and the switch, the span in words, the axis and groups or the table, then Show more. */
function TimelineBody({ project, document }: { project: string; document: DeliveryDocument }) {
  const t = useTranslate();
  const { language } = useLanguage();
  const focusLater = useFocusLater();
  const idBase = useId();
  const shownRows = useRef<HTMLDivElement>(null);
  const [department, setDepartment] = useState("");
  const [asTable, setAsTable] = useState(false);
  const [shown, setShown] = useState(TIMELINE_PAGE);
  const timeline = document.timeline;
  const tasks = useMemo(() => {
    const all = timeline?.tasks ?? [];
    return department === "" ? all : all.filter((task) => task.departments.includes(department));
  }, [timeline, department]);
  const visible = useMemo(() => tasks.slice(0, shown), [tasks, shown]);
  const left = tasks.length - visible.length;
  const sections = useMemo(() => sectionsOf(visible, timeline?.markers ?? []), [visible, timeline]);
  const from = timeline?.from == null ? null : dayOf(timeline.from);
  const to = timeline?.to == null ? null : dayOf(timeline.to);
  const asOf = dayOf(document.as_of);
  const axis = useMemo<Axis | null>(() => (from === null || to === null ? null : { from, to, asOf }), [from, to, asOf]);

  if (timeline === null || axis === null || timeline.tasks.length === 0) {
    return <EmptyState level={3} title={t("roadmap.delivery.timeline.empty")} />;
  }

  function onDepartment(value: string) {
    setDepartment(value);
    setShown(TIMELINE_PAGE);
  }

  function more() {
    const first = visible.length;
    setShown(first + TIMELINE_PAGE);
    focusLater(() => shownRows.current?.querySelectorAll("[data-task]")[first]?.querySelector("a"));
  }

  return (
    <>
      <div className="delivery-timeline-controls">
        <DepartmentField departments={document.departments} value={department} onChange={onDepartment} />
        <Button
          appearance="subtle"
          onClick={() => {
            setAsTable(!asTable);
          }}
        >
          {asTable ? t("roadmap.delivery.timeline.showTimeline") : t("roadmap.delivery.timeline.showTable")}
        </Button>
      </div>
      <p className="delivery-timeline-span">
        {t("roadmap.delivery.timeline.span", { from: dayWords(timeline.from ?? "", language, t), to: dayWords(timeline.to ?? "", language, t), asOf: dayWords(document.as_of, language, t) })}
      </p>
      <div ref={shownRows} className="delivery-timeline-shown">
        {visible.length === 0 ? (
          // A department whose rows the timeline holds none of: said, no empty chart.
          <p className="delivery-none">{t("roadmap.delivery.timeline.empty")}</p>
        ) : asTable ? (
          <TimelineTable project={project} tasks={visible} />
        ) : (
          <div className="delivery-timeline-region" role="region" aria-label={t("roadmap.delivery.timeline.region")} tabIndex={0}>
            <div className="delivery-timeline-chart">
              <AxisScale axis={axis} />
              {sections.map((section) => (
                <Group key={`${String(section.offset)}-${section.key}`} project={project} section={section} axis={axis} idBase={idBase} />
              ))}
            </div>
          </div>
        )}
      </div>
      {left > 0 && (
        <Button className="delivery-timeline-more" onClick={more}>
          {t("roadmap.delivery.timeline.more", { count: Math.min(TIMELINE_PAGE, left), left: String(left) })}
        </Button>
      )}
    </>
  );
}

/** The Timeline view: its own read at the day asked, `timeline` true, its states, then the body. */
export function RoadmapTimeline({ project, asOf }: { project: string; asOf: string }) {
  const t = useTranslate();
  const delivery = useDelivery(project, asOf, true);
  const failure = useRetainedFailure(delivery.error, delivery.isFetching);

  function retry() {
    void delivery.refetch({ cancelRefetch: false });
  }

  const data = delivery.data;
  const failed =
    failure === null ? null : (
      <ErrorPanel title={t("roadmap.delivery.failed")} message={apiErrorOf(failure).message} retrying={delivery.isFetching} attempt={delivery.errorUpdateCount} onRetry={retry} />
    );
  return (
    <div className="delivery-timeline" aria-busy={delivery.isFetching}>
      {failed}
      {data === undefined ? failed === null && <Skeleton label={t("roadmap.delivery.loading", { project })} lines={6} /> : <TimelineBody project={project} document={data} />}
    </div>
  );
}

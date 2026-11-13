import { lazy, Suspense, useId, useState, type ChangeEvent, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { useDelivery } from "../api/queries";
import type { DeliveryCounts, DeliveryDepartment, DeliveryDocument, DeliveryLate, DeliveryRelease } from "../api/types";
import { sectionHash } from "../app/routes";
import { useToday } from "../board/due";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { Lozenge } from "../ui/Lozenge";
import { PropertyList } from "../ui/PropertyList";
import { ErrorPanel, Skeleton } from "../ui/states";
import { formatUtc } from "../ui/time";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { departmentTitles, groupLabel, itemKindLabel, lateLook, leadText } from "./delivery";

// The Roadmap's Delivery tab (docs/features/delivery-analytics.md "Acceptance criteria", DA-12), its
// own chunk, loaded when the tab is first shown (DA-7): what was delivered out of how many, what is
// open, overdue or delivered late, the lead time, by release and by department, then each late task
// with its reasons. One read per day asked, the viewer's own calendar day unless the field moves it,
// as the Board judges its overdue badge. Every count is the daemon's, laid out as served: nothing
// here sums, compares or recounts one. Read only; it moves, refuses and gates nothing (ADR-0012).
// Two views of the day asked, chosen by a View group and kept while the tab stays open: Tables
// first, then Timeline, a chunk of its own with a read of its own, `timeline` asked only there
// (docs/features/delivery-timeline.md "Acceptance criteria", DT8, DT10).

/** The timeline loads when its view is first chosen, a chunk of its own (docs/features/delivery-timeline.md "Acceptance criteria", DT10). */
const RoadmapTimeline = lazy(() => import("./RoadmapTimeline").then((module) => ({ default: module.RoadmapTimeline })));

/** The tab's two views, Tables first. */
type DeliveryView = "tables" | "timeline";

/** A stored time as the page shows one, its stored value the machine's. */
function stamped(at: string, t: Translate): ReactNode {
  return <time dateTime={at}>{formatUtc(at, t)}</time>;
}

/** The six count headers after a row's label, in the tables' order. */
function CountHeads() {
  const t = useTranslate();
  return (
    <>
      <th scope="col">{t("roadmap.delivery.column.delivered")}</th>
      <th scope="col">{t("roadmap.delivery.column.open")}</th>
      <th scope="col">{t("roadmap.delivery.column.overdue")}</th>
      <th scope="col">{t("roadmap.delivery.column.deliveredLate")}</th>
      <th scope="col">{t("roadmap.delivery.column.cancelled")}</th>
      <th scope="col">{t("roadmap.delivery.column.lead")}</th>
    </>
  );
}

/** A row's six counts as served. */
function CountCells({ counts }: { counts: DeliveryCounts }) {
  const t = useTranslate();
  return (
    <>
      <td>{t("roadmap.delivery.deliveredOf", { delivered: String(counts.delivered), tasks: String(counts.tasks) })}</td>
      <td>{counts.open}</td>
      <td>{counts.overdue}</td>
      <td>{counts.delivered_late}</td>
      <td>{counts.cancelled}</td>
      <td>{leadText(counts.lead_days, t)}</td>
    </>
  );
}

/** A table in its own scrolling region, named by its caption. */
function Table({ caption, children }: { caption: string; children: ReactNode }) {
  const captionId = useId();
  return (
    <div className="delivery-table-wrap" role="region" aria-labelledby={captionId} tabIndex={0}>
      <table className="delivery-table">
        <caption id={captionId}>{caption}</caption>
        {children}
      </table>
    </div>
  );
}

/** By release: each group holding a task, in served order. */
function ReleaseTable({ rows }: { rows: readonly DeliveryRelease[] }) {
  const t = useTranslate();
  return (
    <Table caption={t("roadmap.delivery.byRelease")}>
      <thead>
        <tr>
          <th scope="col">{t("roadmap.delivery.column.release")}</th>
          <CountHeads />
        </tr>
      </thead>
      <tbody>
        {rows.map((row, index) => {
          const label = groupLabel(row, t);
          return (
            <tr key={`${String(index)}-${row.group}`} data-group={row.group}>
              <th scope="row">
                <bdi lang={label.served ? "en" : undefined}>{label.text}</bdi>
              </th>
              <CountCells counts={row} />
            </tr>
          );
        })}
      </tbody>
    </Table>
  );
}

/** By department: every served department, zeros too, then every task once. */
function DepartmentTable({ rows, total }: { rows: readonly DeliveryDepartment[]; total: DeliveryCounts }) {
  const t = useTranslate();
  return (
    <Table caption={t("roadmap.delivery.byDepartment")}>
      <thead>
        <tr>
          <th scope="col">{t("roadmap.delivery.column.department")}</th>
          <CountHeads />
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.name} data-department={row.name}>
            <th scope="row">
              <bdi>{row.title}</bdi>
            </th>
            <CountCells counts={row} />
          </tr>
        ))}
      </tbody>
      <tfoot>
        <tr>
          <th scope="row">{t("roadmap.delivery.total")}</th>
          <CountCells counts={total} />
        </tr>
      </tfoot>
    </Table>
  );
}

/** "N more not listed" after a list the daemon cut, never when it cut nothing. */
function More({ count }: { count: number }) {
  const t = useTranslate();
  return count > 0 ? <li className="delivery-more">{t("roadmap.delivery.more", { count })}</li> : null;
}

/** A late task's reasons, each list in served order, oldest first, its cut said after it. */
function Reasons({ project, row }: { project: string; row: DeliveryLate }) {
  const t = useTranslate();
  const none = t("ui.emptyValue");
  const { more } = row;
  const listed = [row.returns, row.reestimates, row.cancelled_subtasks, row.items].some((list) => list.length > 0);
  if (!listed && !Object.values(more).some((count) => count > 0)) {
    return null;
  }
  return (
    <ul className="delivery-reasons">
      {row.returns.map((entry, index) => (
        <li key={`return-${String(index)}`} className="delivery-reason" data-reason="return">
          <span>{fill(t("roadmap.delivery.returned", { from: slot("from"), at: slot("at") }), { from: <bdi>{entry.from}</bdi>, at: stamped(entry.at, t) })}</span>
          {entry.note !== null && <bdi className="delivery-reason-note">{entry.note}</bdi>}
        </li>
      ))}
      <More count={more.returns} />
      {row.reestimates.map((entry, index) => (
        <li key={`reestimate-${String(index)}`} className="delivery-reason" data-reason="reestimate">
          <span>
            {entry.new === null
              ? fill(t("roadmap.delivery.estimateCleared", { old: slot("old"), at: slot("at") }), { old: <bdi>{entry.old ?? none}</bdi>, at: stamped(entry.at, t) })
              : fill(t("roadmap.delivery.reestimated", { old: slot("old"), new: slot("new"), at: slot("at") }), {
                  old: <bdi>{entry.old ?? none}</bdi>,
                  new: <bdi>{entry.new}</bdi>,
                  at: stamped(entry.at, t),
                })}
          </span>
          <bdi className="delivery-reason-note">{entry.reason}</bdi>
        </li>
      ))}
      <More count={more.reestimates} />
      {row.cancelled_subtasks.map((entry) => (
        <li key={`cancelled-${entry.id}`} className="delivery-reason" data-reason="cancelled">
          <span>
            {fill(t("roadmap.delivery.subtaskCancelled", { id: slot("id"), at: slot("at") }), {
              id: <a href={sectionHash(project, "board", entry.id)}>{entry.id}</a>,
              at: stamped(entry.at, t),
            })}
          </span>
          <bdi>{entry.title}</bdi>
          <bdi className="delivery-reason-note">{entry.reason}</bdi>
        </li>
      ))}
      <More count={more.cancelled_subtasks} />
      {row.items.map((entry) => {
        const kind = itemKindLabel(entry, t);
        return (
          <li key={`item-${entry.id}`} className="delivery-reason" data-reason="item">
            <span lang={kind.served ? "en" : undefined}>{kind.text}</span>
            <a href={sectionHash(project, "inbox", entry.id)}>{entry.id}</a>
            <span className="delivery-reason-note" lang="en">
              {entry.status}
            </span>
          </li>
        );
      })}
      <More count={more.items} />
    </ul>
  );
}

/** One late task: its ID to the Board, its title, how late, its facts, its reasons. */
function LateTask({ project, row, departments }: { project: string; row: DeliveryLate; departments: readonly DeliveryDepartment[] }) {
  const t = useTranslate();
  const titles = departmentTitles(row.departments, departments);
  return (
    <li className="delivery-late-task" data-late={row.id}>
      <p className="delivery-late-head">
        <a href={sectionHash(project, "board", row.id)}>{row.id}</a>
        <bdi className="delivery-late-title">{row.title}</bdi>
        <Lozenge {...lateLook(row, t)} />
      </p>
      <PropertyList
        label={row.id}
        items={[
          { key: "due", label: t("roadmap.delivery.property.due"), value: row.due },
          { key: "created", label: t("roadmap.delivery.property.created"), value: stamped(row.created_at, t) },
          { key: "taken", label: t("roadmap.delivery.property.taken"), value: row.taken_at === null ? null : stamped(row.taken_at, t) },
          { key: "accepted_at", label: t("roadmap.delivery.property.accepted"), value: row.accepted_at === null ? null : stamped(row.accepted_at, t) },
          { key: "estimate", label: t("roadmap.delivery.property.estimate"), value: row.estimate === null ? null : <bdi>{row.estimate}</bdi> },
          { key: "departments", label: t("roadmap.delivery.property.departments"), value: titles.length === 0 ? null : <bdi>{titles.join(", ")}</bdi> },
        ]}
      />
      <Reasons project={project} row={row} />
    </li>
  );
}

/** The late tasks in served order, most days late first. */
function LateTasks({ project, document }: { project: string; document: DeliveryDocument }) {
  const t = useTranslate();
  const headingId = useId();
  return (
    <section className="delivery-late" aria-labelledby={headingId}>
      <h2 id={headingId}>{t("roadmap.delivery.late")}</h2>
      {document.late.length === 0 ? (
        <p className="delivery-none">{t("roadmap.delivery.noLate")}</p>
      ) : (
        <ol className="delivery-late-list">
          {document.late.map((row) => (
            <LateTask key={row.id} project={project} row={row} departments={document.departments} />
          ))}
        </ol>
      )}
    </section>
  );
}

/** The daemon's notes, verbatim. */
function Notes({ notes }: { notes: readonly string[] }) {
  const t = useTranslate();
  const headingId = useId();
  return (
    <section className="delivery-notes" aria-labelledby={headingId}>
      <h2 id={headingId}>{t("roadmap.delivery.notes")}</h2>
      <ul>
        {notes.map((note, index) => (
          <li key={`${String(index)}-${note}`} lang="en">
            {note}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** A served document: no task at all an empty state, else the two tables and the late tasks; the notes last. */
function DeliveryBody({ project, document }: { project: string; document: DeliveryDocument }) {
  const t = useTranslate();
  const noTask = document.total.tasks === 0 && document.total.cancelled === 0;
  return (
    <>
      {noTask ? (
        <EmptyState level={3} title={t("roadmap.delivery.empty")} description={<p>{t("roadmap.delivery.emptyText")}</p>} />
      ) : (
        <>
          <ReleaseTable rows={document.releases} />
          <DepartmentTable rows={document.departments} total={document.total} />
          <LateTasks project={project} document={document} />
        </>
      )}
      {document.notes.length > 0 && <Notes notes={document.notes} />}
    </>
  );
}

/** The tables' read at the day asked, its states, then the tables. */
function DeliveryTables({ project, asOf }: { project: string; asOf: string }) {
  const t = useTranslate();
  const delivery = useDelivery(project, asOf);
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
    <div className="delivery-view-body" aria-busy={delivery.isFetching}>
      {failed}
      {data === undefined ? failed === null && <Skeleton label={t("roadmap.delivery.loading", { project })} lines={5} /> : <DeliveryBody project={project} document={data} />}
    </div>
  );
}

/** The View group: two pressed-or-not buttons, Tables first. */
function ViewSwitch({ view, onChange }: { view: DeliveryView; onChange: (view: DeliveryView) => void }) {
  const t = useTranslate();
  const labelId = useId();
  return (
    <div className="delivery-views" role="group" aria-labelledby={labelId}>
      <span id={labelId} className="field-label">
        {t("roadmap.delivery.views.label")}
      </span>
      <Button
        aria-pressed={view === "tables"}
        onClick={() => {
          onChange("tables");
        }}
      >
        {t("roadmap.delivery.views.tables")}
      </Button>
      <Button
        aria-pressed={view === "timeline"}
        onClick={() => {
          onChange("timeline");
        }}
      >
        {t("roadmap.delivery.views.timeline")}
      </Button>
    </div>
  );
}

export function RoadmapDelivery({ project }: { project: string }) {
  const t = useTranslate();
  const today = useToday();
  // The day the field holds: null follows the viewer's own day, across midnight too.
  const [chosen, setChosen] = useState<string | null>(null);
  const [view, setView] = useState<DeliveryView>("tables");
  const asOf = chosen ?? today;
  const fieldId = useId();
  const hintId = useId();

  function onDate(event: ChangeEvent<HTMLInputElement>) {
    // A date field's value is a whole day or empty; emptied, it keeps the day asked.
    const value = event.target.value;
    if (value !== "") {
      setChosen(value === today ? null : value);
    }
  }

  return (
    <div className="delivery">
      <div className="delivery-as-of">
        <label className="field-label" htmlFor={fieldId}>
          {t("roadmap.delivery.asOf")}
        </label>
        <div className="delivery-as-of-field">
          <input id={fieldId} type="date" className="field delivery-date" value={asOf} onChange={onDate} aria-describedby={hintId} />
          <Button
            appearance="subtle"
            onClick={() => {
              setChosen(null);
            }}
          >
            {t("roadmap.delivery.today")}
          </Button>
        </div>
        <p id={hintId} className="delivery-hint">
          {t("roadmap.delivery.asOfHint")}
        </p>
      </div>
      <ViewSwitch view={view} onChange={setView} />
      {view === "tables" ? (
        <DeliveryTables project={project} asOf={asOf} />
      ) : (
        <Suspense fallback={<Skeleton label={t("roadmap.delivery.loading", { project })} lines={6} />}>
          <RoadmapTimeline project={project} asOf={asOf} />
        </Suspense>
      )}
    </div>
  );
}

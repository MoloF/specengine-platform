import { useId, useRef, type ReactNode } from "react";
import { apiErrorOf } from "../api/client";
import { flowForm, useCardCounts, useFlowRead } from "../api/queries";
import type { CardCounts, Workspace, WorkspaceModule } from "../api/types";
import { sectionHash } from "../app/routes";
import { sectionLabel, type SectionId } from "../app/sections";
import { useToday } from "../board/due";
import { categoryMark } from "../board/labels";
import { gapLook, gapMeaning, gapsInOrder, isFound, isMeasured } from "../flow/labels";
import { useTranslate, type Translate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { Badge } from "../ui/Badge";
import { EmptyState } from "../ui/EmptyState";
import { Icon, type IconName } from "../ui/Icon";
import { PropertyList } from "../ui/PropertyList";
import { Notes } from "../ui/served";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { isRooted } from "../workspace/choice";
import { ageDays, agentTasksShown, blocksOf, departmentFlows, isEmpty, servedFlow, tallyOf, type DepartmentTally } from "./department";

// The Overview's blocks (docs/features/department-overview.md "Acceptance criteria", DO2 to DO6,
// DO12), their own chunk (DO7): Work, what waits on this department, the flow's gaps where the
// department's sections list Flow, and every department side by side where its `flows` is null.
// The numbers are the daemon's counts, folded by `./department.ts`; an age is counted from a stored
// time to the viewer's day (`useToday`), never read from a clock here. Read only: each count points
// to its page, and nothing here moves, refuses or gates a task (ADR-0012).

/** The five status columns of a table, in its order, each with its category's icon. */
const STATUS_COLUMNS: readonly { key: "todo" | "doing" | "review" | "returned" | "done"; icon: IconName }[] = [
  { key: "todo", icon: categoryMark("todo").icon },
  { key: "doing", icon: categoryMark("doing").icon },
  { key: "review", icon: "waiting" },
  { key: "returned", icon: categoryMark("returned").icon },
  { key: "done", icon: categoryMark("done").icon },
];

/** The oldest open question's age in words, against the viewer's day; a time that does not read, as stored. */
function oldestText(oldest: string, today: string, t: Translate): ReactNode {
  const days = ageDays(oldest, today);
  if (days === null) {
    return <time dateTime={oldest}>{oldest}</time>;
  }
  return days <= 0 ? t("home.department.oldestToday") : t("home.department.oldest", { count: days });
}

/** A table in its own scrolling region, named by its caption. */
function Table({ caption, children }: { caption: string; children: ReactNode }) {
  const captionId = useId();
  return (
    <div className="department-table-wrap" role="region" aria-labelledby={captionId} tabIndex={0}>
      <table className="department-table">
        <caption id={captionId}>{caption}</caption>
        {children}
      </table>
    </div>
  );
}

/** The five status heads after a row's label. */
function StatusHeads() {
  const t = useTranslate();
  return (
    <>
      {STATUS_COLUMNS.map(({ key, icon }) => (
        <th key={key} scope="col">
          <span className="department-head">
            <Icon name={icon} />
            <span>{t(`home.department.column.${key}`)}</span>
          </span>
        </th>
      ))}
    </>
  );
}

/** A tally's five status counts. */
function StatusCells({ tally }: { tally: DepartmentTally }) {
  return (
    <>
      {STATUS_COLUMNS.map(({ key }) => (
        <td key={key}>{tally[key]}</td>
      ))}
    </>
  );
}

/** A link to a section of the project, its words and an arrow. */
function Open({ project, section, children }: { project: string; section: SectionId; children: string }) {
  return (
    <a className="department-open" href={sectionHash(project, section)}>
      {children}
      <Icon name="arrowRight" />
    </a>
  );
}

/**
 * One block: a section named by its `h2`. `region` false where the body is a region of its own under
 * the same name (a PropertyList), so none is named twice.
 */
function Block({ title, busy, focusable = false, region = true, children }: { title: string; busy?: boolean; focusable?: boolean; region?: boolean; children: ReactNode }) {
  const headingId = useId();
  const head = (
    <h2 id={headingId} tabIndex={focusable ? -1 : undefined}>
      {title}
    </h2>
  );
  return region ? (
    <section className="department-block" aria-labelledby={headingId} aria-busy={busy}>
      {head}
      {children}
    </section>
  ) : (
    <div className="department-block">
      {head}
      {children}
    </div>
  );
}

/** Work: one row per department flow, its title as served, a flow no `[card]` lists by its name. */
function WorkBlock({ counts, flows, tally }: { counts: CardCounts; flows: readonly string[]; tally: DepartmentTally }) {
  const t = useTranslate();
  return (
    <Block title={t("home.department.work")} focusable>
      <Table caption={t("home.department.workCaption")}>
        <thead>
          <tr>
            <th scope="col">{t("home.department.column.flow")}</th>
            <StatusHeads />
          </tr>
        </thead>
        <tbody>
          {flows.map((name) => {
            const flow = servedFlow(counts, name);
            return (
              <tr key={name} data-flow={name}>
                <th scope="row">{flow === undefined ? <bdi lang="en">{name}</bdi> : <bdi>{flow.title}</bdi>}</th>
                <StatusCells tally={tallyOf(counts, [name])} />
              </tr>
            );
          })}
        </tbody>
        {flows.length >= 2 && (
          <tfoot>
            <tr>
              <th scope="row">{t("home.department.allFlows")}</th>
              <StatusCells tally={tally} />
            </tr>
          </tfoot>
        )}
      </Table>
      {(tally.stranded > 0 || tally.cancelled > 0) && (
        <ul className="department-aside">
          {tally.stranded > 0 && <li>{t("home.department.stranded", { count: tally.stranded })}</li>}
          {tally.cancelled > 0 && <li>{t("home.department.cancelled", { count: tally.cancelled })}</li>}
        </ul>
      )}
    </Block>
  );
}

/** A count and the link to the section that lists it. */
function Pointer({ count, extra, project, section, link }: { count: number; extra?: ReactNode; project: string; section: SectionId; link: string }) {
  return (
    <span className="department-value">
      <span className="department-count">{count}</span>
      {extra}
      <Open project={project} section={section}>
        {link}
      </Open>
    </span>
  );
}

/** What waits on this department, each row a count and the page that lists it. */
function WaitingBlock({ project, counts, tally, today }: { project: string; counts: CardCounts; tally: DepartmentTally; today: string }) {
  const t = useTranslate();
  const toBoard = t("home.department.openBoard");
  const agents = agentTasksShown(counts, tally);
  let agentValue: ReactNode;
  if (agents.shown === "count") {
    agentValue = <Pointer count={agents.count} project={project} section="tasks" link={t("home.department.openTasks")} />;
  } else if (agents.shown === "local") {
    // A hub reads no agent task (ADR-0075): said in words, never a 0 it did not count.
    agentValue = (
      <span className="department-value">
        <span>{t("home.department.agentTasksLocal")}</span>
        <Open project={project} section="tasks">
          {t("home.department.openTasks")}
        </Open>
      </span>
    );
  } else {
    agentValue = <span lang="en">{agents.word}</span>;
  }
  const oldest = tally.oldest === null ? undefined : <span className="department-oldest">{oldestText(tally.oldest, today, t)}</span>;
  return (
    <Block title={t("home.department.waiting")} region={false}>
      <PropertyList
        label={t("home.department.waiting")}
        items={[
          { key: "reviews", label: t("home.department.reviews"), value: <Pointer count={tally.review} project={project} section="board" link={toBoard} /> },
          { key: "returned", label: t("home.department.returnedSubtasks"), value: <Pointer count={tally.returnedSubtasks} project={project} section="board" link={toBoard} /> },
          // The owner's word for a parent whose every subtask is closed: a status line, never a control (ADR-0074).
          { key: "ready", label: t("board.progress.ready"), value: <Pointer count={tally.ready} project={project} section="board" link={toBoard} /> },
          {
            key: "questions",
            label: t("home.department.questions"),
            value: <Pointer count={tally.questions} extra={oldest} project={project} section="inbox" link={t("home.department.openInbox")} />,
          },
          { key: "agentTasks", label: t("home.department.agentTasks"), value: agentValue },
        ]}
      />
    </Block>
  );
}

/**
 * Gaps: the drawing read Flow makes for the chosen module's root, or the top, its cache shared;
 * each row that found something in its words, then what found nothing or was not measured, then
 * the way to Flow. Its own skeleton and its own failure.
 */
function GapsBlock({ project, module }: { project: string; module: WorkspaceModule | null }) {
  const t = useTranslate();
  const root = module !== null && isRooted(module) ? module.root : null;
  const read = useFlowRead(project, flowForm(root, null));
  const failure = useRetainedFailure(read.error, read.isFetching);
  const view = read.data;
  const failed =
    failure === null ? null : (
      <ErrorPanel
        title={t("flow.readFailed")}
        message={apiErrorOf(failure).message}
        retrying={read.isFetching}
        attempt={read.errorUpdateCount}
        onRetry={() => {
          void read.refetch({ cancelRefetch: false });
        }}
      />
    );
  let body: ReactNode;
  if (view === undefined) {
    body = failed ?? <Skeleton label={t("flow.reading", { project })} lines={3} />;
  } else {
    const ordered = gapsInOrder(view.gaps);
    const found = ordered.filter(isFound);
    const notMeasured = ordered.filter((gap) => !isMeasured(gap)).length;
    body = (
      <>
        {failed}
        {view.reason !== null && (
          <p className="note" lang="en">
            {view.reason}
          </p>
        )}
        {found.length > 0 && (
          <ul className="department-gaps-list">
            {found.map((gap) => {
              const meaning = gapMeaning(gap.code, t);
              return (
                <li key={gap.code} className="department-gap">
                  <Badge look={gapLook(gap, t)} />
                  {meaning === null ? <span lang="en">{gap.code}</span> : <span>{meaning}</span>}
                </li>
              );
            })}
          </ul>
        )}
        {found.length === 0 && <p className="department-gaps-line">{t("home.department.gapsNone")}</p>}
        {notMeasured > 0 && <p className="department-gaps-line">{t("home.department.gapsNotMeasured", { count: notMeasured })}</p>}
      </>
    );
  }
  return (
    <Block title={t("home.department.gaps")} busy={view === undefined && failure === null}>
      {body}
      <p>
        <Open project={project} section="flow">
          {t("home.department.openFlow")}
        </Open>
      </p>
    </Block>
  );
}

/** Every served department side by side, in served order, then every flow once. */
function SideBlock({ counts, workspaces, today }: { counts: CardCounts; workspaces: readonly Workspace[]; today: string }) {
  const t = useTranslate();
  const none = t("ui.emptyValue");
  const cells = (tally: DepartmentTally) => (
    <>
      <StatusCells tally={tally} />
      <td>{tally.questions}</td>
      <td>{tally.oldest === null ? none : oldestText(tally.oldest, today, t)}</td>
      <td>{tally.ready}</td>
    </>
  );
  return (
    <Block title={t("home.department.side")}>
      <Table caption={t("home.department.sideCaption")}>
        <thead>
          <tr>
            <th scope="col">{t("home.department.column.department")}</th>
            <StatusHeads />
            <th scope="col">{t("home.department.column.questions")}</th>
            <th scope="col">{t("home.department.column.oldest")}</th>
            <th scope="col">{t("home.department.column.ready")}</th>
          </tr>
        </thead>
        <tbody>
          {workspaces.map((workspace) => (
            <tr key={workspace.name} data-department={workspace.name}>
              <th scope="row">
                <bdi>{workspace.title}</bdi>
              </th>
              {cells(tallyOf(counts, departmentFlows(counts, workspace)))}
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <th scope="row">{t("home.department.allFlows")}</th>
            {cells(tallyOf(counts, departmentFlows(counts, null)))}
          </tr>
        </tfoot>
      </Table>
    </Block>
  );
}

/** No flow and no column served: what that means, and where what waits for you is meanwhile. */
function NoFlow({ project }: { project: string }) {
  const t = useTranslate();
  return (
    <EmptyState
      level={2}
      title={t("home.department.empty")}
      description={
        <p>
          {fill(t("home.department.emptyText", { tasks: slot("tasks"), inbox: slot("inbox") }), {
            tasks: <a href={sectionHash(project, "tasks")}>{sectionLabel("tasks", t)}</a>,
            inbox: <a href={sectionHash(project, "inbox")}>{sectionLabel("inbox", t)}</a>,
          })}
        </p>
      }
    />
  );
}

/**
 * The chosen department's Overview over one counts read. `workspaces` null: the departments' read
 * is out, and the blocks wait; empty: it failed, so every flow counts, Gaps shows and no side table
 * does. A failed read is one ErrorPanel in the daemon's words; a failed read again keeps the last
 * numbers under it.
 */
export function DepartmentOverview({
  project,
  workspace,
  module,
  workspaces,
}: {
  project: string;
  workspace: Workspace | null;
  module: WorkspaceModule | null;
  workspaces: readonly Workspace[] | null;
}) {
  const t = useTranslate();
  const today = useToday();
  const counts = useCardCounts(project);
  const failure = useRetainedFailure(counts.error, counts.isFetching);
  const root = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(counts.data, () => root.current?.querySelector<HTMLElement>("h2[tabindex]"));
  const data = counts.data;
  if (workspaces === null) {
    return (
      <div className="department-overview" aria-busy="true">
        <Skeleton label={t("home.department.loading", { project })} lines={6} />
      </div>
    );
  }
  const blocks = blocksOf(workspace, workspaces);
  const gaps = blocks.gaps ? <GapsBlock project={project} module={module} /> : null;
  const failed =
    failure === null ? null : (
      <ErrorPanel
        title={t("home.department.failed")}
        message={apiErrorOf(failure).message}
        retrying={counts.isFetching}
        attempt={counts.errorUpdateCount}
        onRetry={() => {
          retried();
          void counts.refetch({ cancelRefetch: false });
        }}
      />
    );
  let body: ReactNode;
  if (data === undefined) {
    body = (
      <>
        {failed ?? <Skeleton label={t("home.department.loading", { project })} lines={6} />}
        {gaps}
      </>
    );
  } else if (isEmpty(data)) {
    body = (
      <>
        {failed}
        <NoFlow project={project} />
        {gaps}
      </>
    );
  } else {
    const flows = departmentFlows(data, workspace);
    const tally = tallyOf(data, flows);
    body = (
      <>
        {failed}
        <WorkBlock counts={data} flows={flows} tally={tally} />
        <WaitingBlock project={project} counts={data} tally={tally} today={today} />
        {gaps}
        {blocks.side && <SideBlock counts={data} workspaces={workspaces} today={today} />}
      </>
    );
  }
  return (
    <div ref={root} className="department-overview" aria-busy={data === undefined && failure === null}>
      {body}
      {data !== undefined && <Notes notes={data.notes} label={t("home.department.notes")} />}
    </div>
  );
}

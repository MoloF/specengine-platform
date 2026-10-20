import { Fragment, memo, useId, useRef } from "react";
import type { RoadmapDocument, RoadmapGroup, RoadmapItem, RoadmapPoolTask, RoadmapSum } from "../api/types";
import { sectionHash } from "../app/routes";
import { taskStatusLook } from "../tasks/labels";
import { Badge } from "../ui/Badge";
import { Icon } from "../ui/Icon";
import {
  countsText,
  CURRENT,
  cutRuleText,
  estimateText,
  IN_A_CYCLE,
  isPool,
  isRelease,
  isUnplaced,
  itemStatusLook,
  laneName,
  MVP_CUT,
  oneLine,
  priorityText,
  sumText,
  UNESTIMATED,
  unplacedReleaseText,
  WAITS_ON,
} from "./text";

// The Lanes tab (docs/features/roadmap-screen.md): the canvas's text twin and the keyboard's path.
// Every group in the served order, each open item a card, the pool's tasks after its items, the
// plan's notes last; nothing sorted, filtered or recomputed. A mark is a label and an icon, never a
// colour alone; vocabulary (release, priority, unit, item status) comes from the document only.

/** A release's range on the plan's scale: a track, solid to `min`, lighter to `max`; the text beside it says it. */
function RangeBar({ sum, scale }: { sum: RoadmapSum; scale: number }) {
  const solid = Math.min(100, (sum.min / scale) * 100);
  const range = Math.min(100 - solid, ((sum.max - sum.min) / scale) * 100);
  return (
    <span className="roadmap-bar" aria-hidden="true">
      <span className="roadmap-bar-solid" style={{ width: `${String(solid)}%` }} />
      <span className="roadmap-bar-range" style={{ width: `${String(range)}%` }} />
    </span>
  );
}

/** One open item: where it stands, what it waits on, the tasks executing it; its ID opens the Tree. */
function ItemCard({
  project,
  group,
  item,
  unit,
  selected,
}: {
  project: string;
  group: RoadmapGroup;
  item: RoadmapItem;
  unit: string | null;
  selected: boolean;
}) {
  const headingId = useId();
  return (
    <article
      className="roadmap-card"
      data-item={item.id}
      tabIndex={-1}
      aria-labelledby={headingId}
      aria-current={selected ? "true" : undefined}
    >
      <h3 id={headingId} className="roadmap-card-title">
        {isRelease(group) && item.position !== null && <span className="roadmap-position">{item.position}.</span>}
        <a href={sectionHash(project, "tree", item.id)} className="mono roadmap-card-id">
          {item.id}
        </a>
        <span className="roadmap-card-name">{item.title === null ? "-" : oneLine(item.title)}</span>
      </h3>
      <p className="roadmap-card-facts">
        <Badge look={itemStatusLook(item.status)} />
        <span className="roadmap-priority">{priorityText(item.priority)}</span>
        {item.estimate === null ? <Badge look={UNESTIMATED} /> : <span className="roadmap-estimate">{estimateText(item.estimate, unit)}</span>}
        {isUnplaced(group) && <span className="roadmap-release">{unplacedReleaseText(item.release)}</span>}
      </p>
      {item.waits_on.length > 0 && (
        <p className="roadmap-waits">
          <Badge look={WAITS_ON} />
          <span className="roadmap-waits-list">
            {item.waits_on.map((id, index) => (
              <Fragment key={`${String(index)}-${id}`}>
                {index > 0 && ", "}
                <a href={sectionHash(project, "roadmap", id)} className="mono" data-waits-on={id}>
                  {id}
                </a>
              </Fragment>
            ))}
          </span>
        </p>
      )}
      {item.cycle && (
        <p className="roadmap-cycle" data-cycle="">
          <Badge look={IN_A_CYCLE} />
        </p>
      )}
      {item.tasks.length > 0 && (
        <ul className="roadmap-tasks" aria-label={`Tasks executing ${item.id}`}>
          {item.tasks.map((task) => (
            <li key={task.id} className="roadmap-task" data-task={task.id}>
              <a href={sectionHash(project, "tasks", task.id)} className="mono">
                {task.id}
              </a>
              <Badge name="State" look={taskStatusLook(task.status)} />
            </li>
          ))}
        </ul>
      )}
      <p className="roadmap-path mono">{item.path}</p>
    </article>
  );
}

/** The pool's tasks: open tasks executing no open item, after the pool's items. */
function PoolTasks({ project, tasks }: { project: string; tasks: RoadmapPoolTask[] }) {
  const headingId = useId();
  return (
    <section className="roadmap-pool-tasks" aria-labelledby={headingId}>
      <h3 id={headingId} className="card-section-title">
        Pool tasks
      </h3>
      <ul className="roadmap-pool-task-list">
        {tasks.map((task) => (
          <li key={task.id} className="roadmap-pool-task" data-pool-task={task.id}>
            <a href={sectionHash(project, "tasks", task.id)} className="mono">
              {task.id}
            </a>
            <Badge name="State" look={taskStatusLook(task.status)} />
            <span className="roadmap-pool-task-title">{task.title === null ? "-" : oneLine(task.title)}</span>
            <span className="roadmap-pool-task-targets mono">{task.targets.length === 0 ? "-" : task.targets.join(", ")}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** One group: its name and marks, counts, bar and sum, then its items (numbered in a release). */
function Lane({
  project,
  plan,
  group,
  scale,
  selectedId,
  headingRef,
}: {
  project: string;
  plan: RoadmapDocument;
  group: RoadmapGroup;
  scale: number;
  selectedId: string | null;
  headingRef: (element: HTMLHeadingElement | null) => void;
}) {
  const headingId = useId();
  const release = isRelease(group);
  const cards = group.items.map((item) => (
    <li key={item.id}>
      <ItemCard project={project} group={group} item={item} unit={plan.unit} selected={item.id === selectedId} />
    </li>
  ));
  return (
    <section className="roadmap-lane" aria-labelledby={headingId} data-group={group.group}>
      <header className="roadmap-lane-head">
        <h2 id={headingId} ref={headingRef} tabIndex={-1} className="roadmap-lane-name">
          {laneName(group)}
        </h2>
        {group.current && <Badge look={CURRENT} />}
        {group.mvp_cut && <Badge look={MVP_CUT} />}
        <p className="roadmap-lane-counts">{countsText(group)}</p>
        <p className="roadmap-lane-sum">
          {release && scale > 0 && <RangeBar sum={group.sum} scale={scale} />}
          <span className="roadmap-sum-text">{sumText(group.sum, plan.unit)}</span>
        </p>
      </header>
      {cards.length > 0 && (release ? <ol className="roadmap-items">{cards}</ol> : <ul className="roadmap-items">{cards}</ul>)}
      {isPool(group) && plan.pool_tasks.length > 0 && <PoolTasks project={project} tasks={plan.pool_tasks} />}
    </section>
  );
}

/**
 * Every group of the plan as a lane, in served order, with the cut's rule after the `mvp_cut`
 * release and the plan's notes after the lanes; jump buttons above move focus to a lane's heading
 * (the hash is the route: no in-page `#` link). Memoised: its props stay the same objects while
 * a read is in flight (TanStack's structural sharing keeps an unchanged plan), so a read's
 * `isFetching` never re-renders hundreds of cards.
 */
export const RoadmapLanes = memo(function RoadmapLanes({ project, plan, selectedId }: { project: string; plan: RoadmapDocument; selectedId: string | null }) {
  const headings = useRef(new Map<number, HTMLHeadingElement>());
  const notesHeading = useRef<HTMLHeadingElement>(null);
  const notesId = useId();
  const scale = plan.groups.reduce((largest, group) => (isRelease(group) ? Math.max(largest, group.sum.max) : largest), 0);
  const cutRule = cutRuleText(plan);

  function jumpTo(heading: HTMLElement | null | undefined) {
    heading?.focus();
    heading?.scrollIntoView({ block: "start" });
  }

  return (
    <div className="roadmap-lanes">
      <nav className="roadmap-jumps" aria-label="Lanes of the plan">
        <ul>
          {plan.groups.map((group, index) => (
            <li key={`${String(index)}-${group.group}`}>
              <button
                type="button"
                className="link-button"
                onClick={() => {
                  jumpTo(headings.current.get(index));
                }}
              >
                {laneName(group)}
              </button>
            </li>
          ))}
          {plan.notes.length > 0 && (
            <li>
              <button
                type="button"
                className="link-button"
                onClick={() => {
                  jumpTo(notesHeading.current);
                }}
              >
                Notes
              </button>
            </li>
          )}
        </ul>
      </nav>
      {plan.groups.map((group, index) => (
        <Fragment key={`${String(index)}-${group.group}`}>
          <Lane
            project={project}
            plan={plan}
            group={group}
            scale={scale}
            selectedId={selectedId}
            headingRef={(element) => {
              if (element === null) {
                headings.current.delete(index);
              } else {
                headings.current.set(index, element);
              }
            }}
          />
          {isRelease(group) && group.mvp_cut && cutRule !== null && (
            <p className="roadmap-cut-rule" data-cut="">
              <Icon name={MVP_CUT.icon} />
              <span>{cutRule}</span>
            </p>
          )}
        </Fragment>
      ))}
      {plan.notes.length > 0 && (
        <section className="roadmap-notes" aria-labelledby={notesId}>
          <h2 id={notesId} ref={notesHeading} tabIndex={-1}>
            Notes
          </h2>
          <ul className="note-list">
            {plan.notes.map((note, index) => (
              <li key={`${String(index)}-${note}`}>{note}</li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
});

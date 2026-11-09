import { useId, type ReactNode } from "react";
import type { TraceCounts, WorkspaceModule } from "../api/types";
import { replaceHash } from "../app/location";
import { flowHash, type FlowPlace } from "../app/routes";
import { useTranslate } from "../i18n";
import { TraceBar } from "../trace/TraceBar";
import { Icon } from "../ui/Icon";
import { Skeleton } from "../ui/states";
import { isPlainClick } from "./clicks";
import { confirmedLine } from "./labels";
import { laneTotal } from "./timeline";

// The places pane (docs/features/writer-studio.md "Description and interactions"): a `<nav>` of links, each a
// `flowHash`, so a click pushes one history entry and Back returns. The module bar, the scope with
// Up, the stages with their scores, and under the current one where to go next. Nothing here reads:
// the view hands it what its reads hold.

/** A place to go and its words. */
export interface PlaceLink {
  place: FlowPlace;
  label: string;
}

/** A group to enter: its name and words. */
export interface GroupLink extends PlaceLink {
  name: string;
}

/** A stage to choose, with its served score. */
export interface LaneLink extends PlaceLink {
  key: string;
  current: boolean;
  counts: TraceCounts;
}

/** What the stages say: none at the top, a skeleton, a failure, or the lanes. */
export type StagesShown =
  | { show: "top" }
  | { show: "reading" }
  | { show: "failed"; message: string; retrying: boolean; onRetry: () => void }
  | { show: "lanes"; all: PlaceLink & { current: boolean }; lanes: LaneLink[] };

/** The groups under the current item. */
function Groups({ project, groups }: { project: string; groups: readonly GroupLink[] }) {
  const t = useTranslate();
  if (groups.length === 0) {
    return null;
  }
  return (
    <ul className="flow-places-groups" aria-label={t("flow.groupsLabel")}>
      {groups.map((group) => (
        <li key={group.name}>
          <a href={flowHash(project, group.place)}>
            <Icon name="chevronRight" />
            <span className="flow-places-words">
              <bdi>{group.label}</bdi>
              {group.label !== group.name && <span className="mono flow-places-name">{group.name}</span>}
            </span>
          </a>
        </li>
      ))}
    </ul>
  );
}

/** A section of the pane, its heading and its list. */
function Part({ heading, children }: { heading: string; children: ReactNode }) {
  const id = useId();
  return (
    <section className="flow-places-part" aria-labelledby={id}>
      <h2 id={id} className="flow-places-heading">
        {heading}
      </h2>
      {children}
    </section>
  );
}

export function FlowPlaces({
  project,
  here,
  open,
  onToggle,
  modules,
  module,
  onModule,
  up,
  scope,
  stages,
  groups,
}: {
  project: string;
  /** The hash of the place shown with nothing selected: a link to it only clears the selection. */
  here: string;
  /** The disclosure, under 80rem: open or closed; above it the pane always shows. */
  open: boolean;
  onToggle: () => void;
  modules: readonly WorkspaceModule[];
  /** The module in force: a served name, `""` for All modules. */
  module: string;
  /** A module chosen here is the viewer's default from then on (1a's key). */
  onModule: (name: string) => void;
  /** Up, its words whole. */
  up: PlaceLink | null;
  /** The scope in force; null while it is not known. */
  scope: PlaceLink | null;
  stages: StagesShown;
  groups: readonly GroupLink[];
}) {
  const t = useTranslate();
  const bodyId = useId();
  const modulesId = useId();
  const allCurrent = stages.show === "lanes" ? stages.all.current : stages.show === "top";
  return (
    <div className={`flow-places${open ? " is-open" : ""}`}>
      <button type="button" className="button button-quiet flow-places-toggle" aria-expanded={open} aria-controls={bodyId} onClick={onToggle}>
        <Icon name={open ? "chevronDown" : "chevronRight"} />
        <span>{t("flow.placesToggle")}</span>
      </button>
      <nav
        id={bodyId}
        className="flow-places-body"
        aria-label={t("flow.placesLabel")}
        onClick={(event) => {
          const link = event.target instanceof Element ? event.target.closest("a") : null;
          if (link === null) {
            return;
          }
          // The place shown, only its selection apart: clearing a selection replaces, never pushes (D5).
          if (isPlainClick(event) && link.getAttribute("href") === here && window.location.hash !== here) {
            event.preventDefault();
            replaceHash(here);
          }
          // A place followed folds the disclosure back where it is one; the link navigates as links do.
          if (open) {
            onToggle();
          }
        }}
      >
        {modules.length > 0 && (
          <div className="flow-places-modules" role="group" aria-labelledby={modulesId}>
            <span id={modulesId} className="flow-places-heading">
              {t("workspace.modulesLabel")}
            </span>
            <ul>
              {[{ name: "", label: t("workspace.allModules") }, ...modules.map((served) => ({ name: served.name, label: served.title }))].map((item) => (
                <li key={item.name}>
                  <a
                    className="module-option"
                    href={flowHash(project, { module: item.name, scope: null, stage: null, select: null })}
                    aria-current={item.name === module ? "true" : undefined}
                    onClick={(event) => {
                      if (isPlainClick(event)) {
                        onModule(item.name);
                      }
                    }}
                  >
                    <bdi>{item.label}</bdi>
                  </a>
                </li>
              ))}
            </ul>
          </div>
        )}
        <Part heading={t("flow.scopeLabel")}>
          <ul className="flow-places-scope">
            {up !== null && (
              <li>
                <a href={flowHash(project, up.place)}>
                  <Icon name="back" />
                  <span>{up.label}</span>
                </a>
              </li>
            )}
            {scope !== null && (
              <li>
                <a href={flowHash(project, scope.place)} aria-current="location" className="flow-places-current">
                  <bdi>{scope.label}</bdi>
                </a>
              </li>
            )}
          </ul>
        </Part>
        <Part heading={t("flow.stagesHeading")}>
          {stages.show === "top" && (
            <>
              <p className="muted">{t("flow.stagesAtTop")}</p>
              <Groups project={project} groups={groups} />
            </>
          )}
          {stages.show === "reading" && (
            <div aria-busy="true">
              <Skeleton label={t("flow.stagesReading")} lines={3} />
            </div>
          )}
          {stages.show === "failed" && (
            <div className="flow-failed">
              <p className="note" role="alert">
                <Icon name="alert" />
                <span>
                  {t("flow.stagesFailed")}{" "}
                  <span className="verbatim" lang="en">
                    {stages.message}
                  </span>
                </span>
              </p>
              <button type="button" className="button" aria-disabled={stages.retrying} onClick={stages.retrying ? undefined : stages.onRetry}>
                <Icon name="retry" />
                <span>{stages.retrying ? t("ui.retrying") : t("ui.retry")}</span>
              </button>
            </div>
          )}
          {stages.show === "lanes" && (
            <ul className="flow-places-stages">
              <li>
                <a href={flowHash(project, stages.all.place)} aria-current={allCurrent ? "true" : undefined}>
                  {stages.all.label}
                </a>
                {allCurrent && <Groups project={project} groups={groups} />}
              </li>
              {stages.lanes.map((lane) => (
                <li key={lane.key}>
                  <a href={flowHash(project, lane.place)} aria-current={lane.current ? "true" : undefined} className="flow-places-lane">
                    <bdi className="flow-places-stage">{lane.label}</bdi>
                    <span className="flow-places-score">{confirmedLine(lane.counts.confirmed, laneTotal(lane.counts), t)}</span>
                    <TraceBar counts={lane.counts} />
                  </a>
                  {lane.current && <Groups project={project} groups={groups} />}
                </li>
              ))}
            </ul>
          )}
          {stages.show !== "lanes" && stages.show !== "top" && <Groups project={project} groups={groups} />}
        </Part>
      </nav>
    </div>
  );
}

import { useId, type ReactNode } from "react";
import type { TraceCounts, WorkspaceModule } from "../api/types";
import { replaceHash } from "../app/location";
import { flowHash, type FlowPlace } from "../app/routes";
import { useTranslate } from "../i18n";
import { TraceBar } from "../trace/TraceBar";
import { Button } from "../ui/Button";
import { Icon } from "../ui/Icon";
import { Skeleton } from "../ui/states";
import { isPlainClick } from "./clicks";
import { confirmedLine } from "./labels";
import { laneTotal } from "./timeline";

// The places pane (docs/features/writer-studio.md "Description and interactions";
// docs/features/flow-scenario.md D11): a `<nav>` of links, each a `flowHash`, so a
// click pushes one history entry and Back returns. The module bar; the path, top to bottom, from the
// scope's served `ancestors` to the scope, the groups under it; the stages with their scores; Records
// for the AI at its foot. Nothing here reads: the view hands it what its reads hold.

/** A place to go and its words. */
export interface PlaceLink {
  place: FlowPlace;
  label: string;
}

/** A group to enter: its name and words. */
export interface GroupLink extends PlaceLink {
  name: string;
}

/** One entry of the path: its title, its depth from the top shown, whether it is where you are. */
export interface PathEntry extends PlaceLink {
  key: string;
  depth: number;
  current: boolean;
}

/** The path as the scope's node read gives it: the entries; a skeleton line while it reads; its failure. */
export interface PathShown {
  entries: PathEntry[];
  pending: boolean;
  failed: { message: string; retrying: boolean; onRetry: () => void } | null;
}

/** Indentation stops at six levels: a deeper path keeps the sixth. */
const DEEPEST = 6;

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

/** The groups under the path's current entry, one level deeper, by title: the chevron stays on the title's first line. */
function Groups({ project, groups, depth }: { project: string; groups: readonly GroupLink[]; depth: number }) {
  const t = useTranslate();
  if (groups.length === 0) {
    return null;
  }
  return (
    <ul className="flow-places-groups" aria-label={t("flow.groupsLabel")} data-depth={Math.min(depth, DEEPEST)}>
      {groups.map((group) => (
        <li key={group.name}>
          <a className="flow-places-group" href={flowHash(project, group.place)}>
            <span className="flow-places-words">
              <bdi>{group.label}</bdi>
            </span>
            <Icon name="chevronRight" />
          </a>
        </li>
      ))}
    </ul>
  );
}

/** The path: an ordered list, top first, each entry a place link, the last where you are. */
function Path({ project, path }: { project: string; path: PathShown }) {
  const t = useTranslate();
  const id = useId();
  return (
    <section className="flow-places-part" aria-labelledby={id}>
      <h2 id={id} className="flow-places-heading">
        {t("flow.path")}
      </h2>
      {path.pending && (
        <div aria-busy="true">
          <Skeleton label={t("flow.path")} lines={1} />
        </div>
      )}
      <ol className="flow-path" aria-labelledby={id}>
        {path.entries.map((entry) => (
          <li key={entry.key} data-depth={Math.min(entry.depth, DEEPEST)}>
            <a href={flowHash(project, entry.place)} aria-current={entry.current ? "location" : undefined} className={entry.current ? "flow-places-current" : undefined}>
              <bdi>{entry.label}</bdi>
            </a>
          </li>
        ))}
      </ol>
      {path.failed !== null && (
        <div className="flow-failed">
          <p className="note" role="alert">
            <Icon name="alert" />
            <span>
              {t("flow.pathFailed")}{" "}
              <span className="verbatim" lang="en">
                {path.failed.message}
              </span>
            </span>
          </p>
          <Button icon="retry" aria-disabled={path.failed.retrying} onClick={path.failed.retrying ? undefined : path.failed.onRetry}>
            {path.failed.retrying ? t("ui.retrying") : t("ui.retry")}
          </Button>
        </div>
      )}
    </section>
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
  path,
  stages,
  groups,
  records,
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
  /** The path to the scope in force; null while the scope is not known. */
  path: PathShown | null;
  stages: StagesShown;
  /** The groups under the path's current entry. */
  groups: readonly GroupLink[];
  /** Records for the AI, at the pane's foot; null where there is none. */
  records: ReactNode;
}) {
  const t = useTranslate();
  const bodyId = useId();
  const modulesId = useId();
  const depth = path === null ? 0 : (path.entries[path.entries.length - 1]?.depth ?? 0) + 1;
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
        {path !== null && <Path project={project} path={path} />}
        <Groups project={project} groups={groups} depth={depth} />
        <Part heading={t("flow.stagesHeading")}>
          {stages.show === "top" && <p className="muted">{t("flow.stagesAtTop")}</p>}
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
                <a href={flowHash(project, stages.all.place)} aria-current={stages.all.current ? "true" : undefined}>
                  {stages.all.label}
                </a>
              </li>
              {stages.lanes.map((lane) => (
                <li key={lane.key}>
                  <a href={flowHash(project, lane.place)} aria-current={lane.current ? "true" : undefined} className="flow-places-lane">
                    <bdi className="flow-places-stage">{lane.label}</bdi>
                    <span className="flow-places-score">{confirmedLine(lane.counts.confirmed, laneTotal(lane.counts), t)}</span>
                    <TraceBar counts={lane.counts} />
                  </a>
                </li>
              ))}
            </ul>
          )}
        </Part>
        {records}
      </nav>
    </div>
  );
}

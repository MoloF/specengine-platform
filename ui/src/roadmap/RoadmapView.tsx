import { lazy, Suspense, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useRoadmap } from "../api/queries";
import { pushHash } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { SECTIONS } from "../app/sections";
import { useOpenShortcuts } from "../app/shortcuts";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { RoadmapLanes } from "./RoadmapLanes";
import { emptyPlanText, headerLine, NO_PLAN_LINE, unlistedText } from "./text";

/** The canvas and its drawing library load when the Chain tab is first shown, not with the app. */
const RoadmapChain = lazy(() => import("./RoadmapChain").then((module) => ({ default: module.RoadmapChain })));

const ABOUT = SECTIONS.find((section) => section.id === "roadmap")?.about ?? "";

/** The Roadmap's two tabs: Lanes, the text twin and the keyboard's path, and Chain, the canvas. */
export type RoadmapTab = "lanes" | "chain";

/** The chosen tab per project, kept by the Shell for this page's life (as the Graph's options). */
export interface RoadmapMemory {
  recall: (project: string) => RoadmapTab | undefined;
  remember: (project: string, tab: RoadmapTab) => void;
}

/** No memory: each Roadmap opens on Lanes. */
export const NO_ROADMAP_MEMORY: RoadmapMemory = {
  recall: () => undefined,
  remember: () => undefined,
};

/**
 * The Roadmap (`#/<project>/roadmap[/<ID>]`, docs/features/roadmap-screen.md): the daemon's whole
 * plan, one read, drawn as lanes (the default tab) and as a read-only chain canvas. The ID in the
 * route only selects: its card is marked current and takes focus; an ID no group lists is said so,
 * with a link to its Tree node. Read only: an item moves when its file's front-matter is edited;
 * "Read again" reads the plan once more (a hand edit raises no event).
 */
export function RoadmapView({ project, itemId, memory = NO_ROADMAP_MEMORY }: { project: string; itemId: string | null; memory?: RoadmapMemory }) {
  const openShortcuts = useOpenShortcuts();
  const roadmap = useRoadmap(project);
  const failure = useRetainedFailure(roadmap.error, roadmap.isFetching);
  const titleId = useId();
  const tabsBase = useId();
  const [tab, setTab] = useState<RoadmapTab>(() => memory.recall(project) ?? "lanes");
  const results = useRef<HTMLDivElement>(null);
  const retried = useRetryFocus(roadmap.data, () => results.current);
  const plan = roadmap.data;
  const listed = plan !== undefined && itemId !== null && plan.groups.some((group) => group.items.some((item) => item.id === itemId));
  const hasPlan = plan !== undefined;

  useEffect(() => {
    memory.remember(project, tab);
  }, [memory, project, tab]);

  // The selected card takes focus once it is drawn, after the shell has focused the view's heading
  // on entering; a re-read of the same plan moves nothing.
  useEffect(() => {
    if (!hasPlan || itemId === null) {
      return;
    }
    queueMicrotask(() => {
      const card = Array.from(results.current?.querySelectorAll<HTMLElement>("[data-item]") ?? []).find((element) => element.dataset.item === itemId);
      card?.focus();
      card?.scrollIntoView({ block: "nearest" });
    });
  }, [hasPlan, itemId]);

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  function retry() {
    retried();
    void roadmap.refetch({ cancelRefetch: false });
  }

  let body;
  if (plan === undefined) {
    body =
      failure === null ? (
        <Skeleton label={`Reading the plan of ${project}`} lines={6} />
      ) : (
        <div className="roadmap-failure">
          <ErrorPanel
            title="The plan could not be read"
            message={apiErrorOf(failure).message}
            retrying={roadmap.isFetching}
            attempt={roadmap.errorUpdateCount}
            onRetry={retry}
          />
          <p className="note roadmap-no-plan">
            <Icon name="info" />
            <span>{NO_PLAN_LINE}</span>
          </p>
        </div>
      );
  } else if (plan.reason !== null) {
    // The exit-1 document: only for a release the screen never sends; shown as the daemon says it.
    body = (
      <div className="roadmap-failure">
        <ErrorPanel title="The plan could not be read" message={plan.reason} retrying={roadmap.isFetching} attempt={roadmap.dataUpdatedAt} onRetry={retry} />
        <p className="note roadmap-no-plan">
          <Icon name="info" />
          <span>{NO_PLAN_LINE}</span>
        </p>
      </div>
    );
  } else {
    const empty = emptyPlanText(plan);
    body = (
      <>
        <div className="roadmap-head">
          <p className="roadmap-line">{headerLine(plan)}</p>
          <button
            type="button"
            className="button"
            onClick={() => {
              void roadmap.refetch();
            }}
          >
            <Icon name="retry" />
            <span>Read again</span>
          </button>
        </div>
        {roadmap.error !== null && (
          <div className="roadmap-reread-failed">
            <p className="note" role="alert">
              <Icon name="alert" />
              <span>The plan could not be read again: {apiErrorOf(roadmap.error).message}</span>
            </p>
            <button type="button" className="button" aria-disabled={roadmap.isFetching} onClick={roadmap.isFetching ? undefined : retry}>
              <Icon name="retry" />
              <span>{roadmap.isFetching ? "Retrying" : "Retry"}</span>
            </button>
          </div>
        )}
        {empty !== null && <p className="roadmap-empty">{empty}</p>}
        {itemId !== null && !listed && (
          <p className="note roadmap-unlisted">
            <Icon name="info" />
            <span>{unlistedText(itemId)}</span>
            <a href={sectionHash(project, "tree", itemId)}>Open {itemId} in the Spec tree</a>
          </p>
        )}
        <Tabs
          label="Views of the plan"
          base={tabsBase}
          selected={tab}
          onSelect={setTab}
          tabs={[
            { id: "lanes", label: "Lanes" },
            { id: "chain", label: "Chain" },
          ]}
        />
        <TabPanel base={tabsBase} id="lanes" selected={tab === "lanes"}>
          {tab === "lanes" && (
            <RegionBoundary name="lanes">
              <RoadmapLanes project={project} plan={plan} selectedId={listed ? itemId : null} />
            </RegionBoundary>
          )}
        </TabPanel>
        <TabPanel base={tabsBase} id="chain" selected={tab === "chain"}>
          {tab === "chain" && (
            <RegionBoundary name="canvas">
              <Suspense fallback={<Skeleton label="Loading the canvas" lines={4} />}>
                <RoadmapChain
                  plan={plan}
                  selectedId={listed ? itemId : null}
                  onSelect={(id) => {
                    pushHash(sectionHash(project, "roadmap", id));
                  }}
                />
              </Suspense>
            </RegionBoundary>
          )}
        </TabPanel>
      </>
    );
  }

  return (
    <section className="view roadmap-view" aria-labelledby={titleId} onKeyDown={onKeyDown}>
      <header className="view-head">
        <h1 id={titleId} tabIndex={-1}>
          Roadmap
        </h1>
        <p className="view-about">{ABOUT} Read only.</p>
      </header>
      <div ref={results} tabIndex={-1} className="roadmap-results" aria-busy={roadmap.isFetching}>
        <RegionBoundary name="plan">{body}</RegionBoundary>
      </div>
    </section>
  );
}

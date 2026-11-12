import { lazy, Suspense, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useRoadmap } from "../api/queries";
import { pushHash } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { aboutOf } from "../app/sections";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { useOpenShortcuts } from "../app/shortcuts";
import { Button } from "../ui/Button";
import { EmptyState } from "../ui/EmptyState";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { ErrorPanel, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { RoadmapLanes } from "./RoadmapLanes";
import { emptyPlanText, headerLine, unlistedText } from "./text";

/** The canvas and its drawing library load when the Chain tab is first shown, not with the app. */
const RoadmapChain = lazy(() => import("./RoadmapChain").then((module) => ({ default: module.RoadmapChain })));

/** The delivery tables load when the Delivery tab is first shown, a chunk of their own (docs/features/delivery-analytics.md "Acceptance criteria", DA-7). */
const RoadmapDelivery = lazy(() => import("./RoadmapDelivery").then((module) => ({ default: module.RoadmapDelivery })));

/**
 * The Roadmap's three tabs: Lanes, the text twin and the keyboard's path; Chain, the canvas; and
 * Delivery, what was delivered and what is overdue (docs/features/delivery-analytics.md
 * "Acceptance criteria", DA-12).
 */
export type RoadmapTab = "lanes" | "chain" | "delivery";

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
 * plan, one read, drawn as lanes (the default tab) and as a read-only chain canvas, beside the
 * delivery of its tasks, a read of its own at a day (the Delivery tab). The ID in the
 * route only selects: its card is marked current and takes focus; an ID no group lists is said so,
 * with a link to its Tree node. Read only: an item moves when its file's front-matter is edited;
 * "Read again" reads the plan once more (a hand edit raises no event).
 */
export function RoadmapView({ project, itemId, memory = NO_ROADMAP_MEMORY }: { project: string; itemId: string | null; memory?: RoadmapMemory }) {
  const t = useTranslate();
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
        <Skeleton label={t("roadmap.reading", { project })} lines={6} />
      ) : (
        <div className="roadmap-failure">
          <ErrorPanel
            title={t("roadmap.failed")}
            message={apiErrorOf(failure).message}
            retrying={roadmap.isFetching}
            attempt={roadmap.errorUpdateCount}
            onRetry={retry}
          />
          <p className="note roadmap-no-plan">
            <Icon name="info" />
            <span>{t("roadmap.noPlan")}</span>
          </p>
        </div>
      );
  } else if (plan.reason !== null) {
    // The exit-1 document: only for a release the screen never sends; shown as the daemon says it.
    body = (
      <div className="roadmap-failure">
        <ErrorPanel title={t("roadmap.failed")} message={plan.reason} retrying={roadmap.isFetching} attempt={roadmap.dataUpdatedAt} onRetry={retry} />
        <p className="note roadmap-no-plan">
          <Icon name="info" />
          <span>{t("roadmap.noPlan")}</span>
        </p>
      </div>
    );
  } else {
    const empty = emptyPlanText(plan, t);
    body = (
      <>
        <div className="roadmap-head">
          <p className="roadmap-line">{headerLine(plan, t)}</p>
          <Button
            icon="retry"
            onClick={() => {
              void roadmap.refetch();
            }}
          >
            <span>{t("roadmap.readAgain")}</span>
          </Button>
        </div>
        {roadmap.error !== null && (
          <div className="roadmap-reread-failed">
            <p className="note" role="alert">
              <Icon name="alert" />
              <span>{fill(t("roadmap.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(roadmap.error).message}</span> })}</span>
            </p>
            <Button icon="retry" aria-disabled={roadmap.isFetching} onClick={roadmap.isFetching ? undefined : retry}>
              <span>{roadmap.isFetching ? t("ui.retrying") : t("ui.retry")}</span>
            </Button>
          </div>
        )}
        {/* No item is an EmptyState where the line stood, the tabs below it (docs/features/design-sweep-b.md "Acceptance criteria", DB4). */}
        {empty !== null &&
          (empty.noItem ? <EmptyState level={2} title={empty.text} description={<p>{t("roadmap.emptyNext")}</p>} /> : <p className="roadmap-empty">{empty.text}</p>)}
        {itemId !== null && !listed && (
          <p className="note roadmap-unlisted">
            <Icon name="info" />
            <span>{unlistedText(itemId, t)}</span>
            <a href={sectionHash(project, "tree", itemId)}>{t("roadmap.openInTree", { id: itemId })}</a>
          </p>
        )}
        <Tabs
          label={t("roadmap.tabsLabel")}
          base={tabsBase}
          selected={tab}
          onSelect={setTab}
          tabs={[
            { id: "lanes", label: t("roadmap.lanes") },
            { id: "chain", label: t("roadmap.chain") },
            { id: "delivery", label: t("roadmap.delivery.tab") },
          ]}
        />
        <TabPanel base={tabsBase} id="lanes" selected={tab === "lanes"}>
          {tab === "lanes" && (
            <RegionBoundary name={t("roadmap.boundary.lanes")}>
              <RoadmapLanes project={project} plan={plan} selectedId={listed ? itemId : null} />
            </RegionBoundary>
          )}
        </TabPanel>
        <TabPanel base={tabsBase} id="chain" selected={tab === "chain"}>
          {tab === "chain" && (
            <RegionBoundary name={t("roadmap.boundary.canvas")}>
              <Suspense fallback={<Skeleton label={t("roadmap.canvasLoading")} lines={4} />}>
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
        <TabPanel base={tabsBase} id="delivery" selected={tab === "delivery"}>
          {tab === "delivery" && (
            <RegionBoundary name={t("roadmap.boundary.delivery")}>
              <Suspense fallback={<Skeleton label={t("roadmap.delivery.loading", { project })} lines={4} />}>
                <RoadmapDelivery project={project} />
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
          {t("roadmap.title")}
        </h1>
        <p className="view-about">{t("roadmap.about", { about: aboutOf("roadmap", t) })}</p>
      </header>
      <div ref={results} tabIndex={-1} className="roadmap-results" aria-busy={roadmap.isFetching}>
        <RegionBoundary name={t("roadmap.boundary.plan")}>{body}</RegionBoundary>
      </div>
    </section>
  );
}

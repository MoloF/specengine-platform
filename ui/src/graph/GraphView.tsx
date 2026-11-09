import { lazy, Suspense, useCallback, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useGraph } from "../api/queries";
import type { GraphView as GraphAnswer } from "../api/types";
import { pushHash } from "../app/location";
import { RegionBoundary } from "../app/RegionBoundary";
import { sectionHash } from "../app/routes";
import { useOpenShortcuts } from "../app/shortcuts";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { useAnnounce } from "../ui/announcer";
import { Icon } from "../ui/Icon";
import { hasModifier, isTextField } from "../ui/keys";
import { ErrorPanel, ReadFailure, Skeleton } from "../ui/states";
import { TabPanel, Tabs } from "../ui/Tabs";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { useRetryFocus } from "../ui/useRetryFocus";
import { WEAK_LINK_TYPE } from "../tree/labels";
import { leftOutText } from "../tree/LinksPanel";
import type { BoxCommand } from "./GraphCanvas";
import { GraphControls } from "./GraphControls";
import { GraphDetails } from "./GraphDetails";
import { GraphLegend } from "./GraphLegend";
import { GraphList } from "./GraphList";
import { layoutGraph, type BoxContent, type Layout } from "./layout";
import { countText } from "./lines";
import {
  chipsOf,
  DEFAULT_SETTINGS,
  graphOptions,
  NO_MEMORY,
  patternOrder,
  toggleChip,
  withBase,
  withMode,
  type GraphMemory,
  type GraphSettings,
  type GraphTab,
} from "./settings";

/** The forms a REF takes, written as typed: code in every language. */
const REF_FORMS = { slug: "slug/ID", section: "ID#SECTION", md: ".md" } as const;

/** The canvas and its drawing library load with the first answer drawn, not with the app. */
const GraphCanvas = lazy(() => import("./GraphCanvas").then((module) => ({ default: module.GraphCanvas })));

/** What the answer says besides its nodes and edges: notes, what was left out, a cut. */
function AnswerNotes({ answer, onArchive }: { answer: GraphAnswer; onArchive: (() => void) | null }) {
  const t = useTranslate();
  const leftOut = leftOutText(answer.left_out, t);
  return (
    <>
      {answer.notes.length > 0 && (
        <ul className="note-list" aria-label={t("graph.notes")}>
          {answer.notes.map((note, index) => (
            <li key={`${String(index)}-${note}`} lang="en">
              {note}
            </li>
          ))}
        </ul>
      )}
      {leftOut !== null && (
        <p className="note">
          <Icon name="info" />
          <span>
            {leftOut}
            {onArchive !== null && answer.left_out.tier3 > 0 && (
              <>
                {" ("}
                <button type="button" className="link-button" onClick={onArchive}>
                  {t("graph.includeArchive")}
                </button>
                {")"}
              </>
            )}
          </span>
        </p>
      )}
      {answer.truncated && (
        <div className="notice cut-notice" role="note">
          <p className="notice-title">
            <Icon name="info" />
            <span>{t("graph.cut")}</span>
          </p>
        </div>
      )}
    </>
  );
}

/** An answer with no edge: what that means here, and what to try next. */
function NoEdge({
  answer,
  nodeRef,
  settings,
  mentionsChip,
  onImpact,
  onMentions,
  onArchive,
}: {
  answer: GraphAnswer;
  nodeRef: string;
  settings: GraphSettings;
  mentionsChip: boolean;
  onImpact: () => void;
  onMentions: () => void;
  onArchive: () => void;
}) {
  const t = useTranslate();
  return (
    <div className="empty-state">
      <h2>{t("graph.noEdge")}</h2>
      <p>
        {settings.impact ? t("graph.noImpact", { ref: nodeRef }) : t("graph.noOutgoing", { ref: nodeRef })} {t("graph.countLine", { counts: countText(answer, t) })}
      </p>
      <p className="muted">{t("graph.noEdgeNext")}</p>
      <p className="graph-next">
        {!settings.impact && (
          <button type="button" className="button" onClick={onImpact}>
            {t("graph.showImpact")}
          </button>
        )}
        {mentionsChip && (
          <button type="button" className="button" onClick={onMentions}>
            {t("graph.followMentions")}
          </button>
        )}
        {!settings.archive && (
          <button type="button" className="button" onClick={onArchive}>
            {t("graph.includeArchive")}
          </button>
        )}
      </p>
    </div>
  );
}

/**
 * The Graph view (`#/<project>/graph[/<REF>]`, docs/features/ui-graph.md): one answer of `spec
 * graph` per view, drawn on a read-only canvas and listed whole as text. The walk, its directions
 * and its cut stay in core; the options live in memory per project, the REF in the hash.
 */
export function GraphView({ project, nodeRef, memory = NO_MEMORY }: { project: string; nodeRef: string | null; memory?: GraphMemory }) {
  const t = useTranslate();
  const openShortcuts = useOpenShortcuts();
  const announce = useAnnounce();
  const focusLater = useFocusLater();
  const titleId = useId();
  const tabsBase = useId();
  const emptyId = useId();
  const [settings, setSettings] = useState<GraphSettings>(() => memory.recall(project) ?? DEFAULT_SETTINGS);
  const [roving, setRoving] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const field = useRef<HTMLInputElement>(null);
  const results = useRef<HTMLDivElement>(null);
  const detailsHeading = useRef<HTMLHeadingElement>(null);
  const focusBox = useRef<(id: string) => void>(() => undefined);
  const spoken = useRef(0);

  const options = useMemo(() => (nodeRef === null ? null : graphOptions(nodeRef, settings)), [nodeRef, settings]);
  const graph = useGraph(project, options);
  const failure = useRetainedFailure(graph.error, graph.isFetching);
  const retried = useRetryFocus(graph.data, () => results.current);

  useEffect(() => {
    memory.remember(project, settings);
  }, [memory, project, settings]);

  // An answer read without `types` names its mode's chips, in its order.
  const fresh = nodeRef !== null && !graph.isPlaceholderData ? graph.data : undefined;
  if (fresh !== undefined && fresh.reason === null && options?.types === undefined && fresh.impact === settings.impact) {
    const learned = withBase(settings, settings.impact, fresh.types);
    if (learned !== settings) {
      setSettings(learned);
    }
  }

  const answer = nodeRef === null ? undefined : graph.data;
  // Each type's line keeps its pattern while chips are pressed or released: the order is the
  // answer's mode's chips, learned once per mode.
  const bases = settings.bases;
  const order = useMemo(() => (answer === undefined ? [] : patternOrder(bases, answer.impact, answer.types)), [bases, answer]);
  const layout = useMemo<Layout | null>(
    () => (answer !== undefined && answer.reason === null && answer.edges.length > 0 ? layoutGraph(answer, order) : null),
    [answer, order],
  );
  const drawn = useMemo(() => new Set(layout?.boxes.map((box) => box.id) ?? []), [layout]);
  const focusId = layout?.columns[0]?.boxes[0] ?? null;
  const rovingId = roving !== null && drawn.has(roving) ? roving : focusId;
  const selectedBox = selected === null ? undefined : layout?.boxes.find((box) => box.id === selected);
  // `mentions` takes its direction only from an answer of this mode read with these options.
  const current = answer !== undefined && !graph.isPlaceholderData && answer.impact === settings.impact ? answer : undefined;
  const chips = chipsOf(settings, current?.types ?? []);
  const busy = nodeRef !== null && graph.isFetching;

  // Each answer is counted once in the polite live region.
  const answered = graph.dataUpdatedAt;
  useEffect(() => {
    if (fresh === undefined || fresh.reason !== null || answered === 0 || spoken.current === answered) {
      return;
    }
    spoken.current = answered;
    announce(countText(fresh, t));
  }, [fresh, answered, announce, t]);

  // No REF: the field takes focus, after the shell has focused the view's heading.
  useEffect(() => {
    if (nodeRef !== null) {
      return;
    }
    queueMicrotask(() => {
      field.current?.focus();
    });
  }, [nodeRef]);

  const change = useCallback((next: (current: GraphSettings) => GraphSettings) => {
    setSettings(next);
  }, []);
  const onMode = useCallback((impact: boolean) => {
    setSettings((current) => withMode(current, impact));
  }, []);
  const onChip = useCallback((type: string) => {
    setSettings((current) => toggleChip(current, type));
  }, []);
  const includeArchive = useCallback(() => {
    setSettings((current) => ({ ...current, archive: true }));
  }, []);
  const setFocusBox = useCallback((focus: (id: string) => void) => {
    focusBox.current = focus;
  }, []);

  function setTab(tab: GraphTab) {
    setSettings((current) => ({ ...current, tab }));
  }

  function closeDetails() {
    const back = selected;
    setSelected(null);
    if (back !== null) {
      setRoving(back);
      focusBox.current(back);
    }
  }

  /**
   * A more-box's "Show in List": the List, focus on its column's distance heading when it hides
   * walked nodes; on "Not walked" when it hides only stubs or the column has no distance (stubs
   * past the last one); else on the List itself.
   */
  function showInList(more: Extract<BoxContent, { type: "more" }>) {
    setTab("list");
    focusLater(
      () =>
        (more.hiddenWalked > 0 ? document.getElementById(`graph-distance-${String(more.column)}`) : null) ??
        document.getElementById("graph-stubs") ??
        document.getElementById(`${tabsBase}-panel-list`),
    );
  }

  /** Enter opens the details (focus goes there), `o` the spec tree, `c` centres here, Esc closes. */
  function onCommand(id: string, command: BoxCommand) {
    const box = layout?.boxes.find((candidate) => candidate.id === id);
    if (box === undefined) {
      return;
    }
    const name = box.content.type === "walked" ? box.content.walked.name : null;
    switch (command) {
      case "details":
        if (selected === id) {
          detailsHeading.current?.focus();
          break;
        }
        setRoving(id);
        setSelected(id);
        focusLater(() => detailsHeading.current);
        break;
      case "tree":
        if (name !== null) {
          pushHash(sectionHash(project, "tree", name));
        }
        break;
      case "centre":
        if (name !== null) {
          pushHash(sectionHash(project, "graph", name));
        }
        break;
      case "close":
        closeDetails();
        break;
    }
  }

  function onViewKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  let body;
  if (nodeRef === null) {
    body = (
      <div className="empty-state">
        <h2 id={emptyId}>{t("graph.empty")}</h2>
        <p>
          {fill(t("graph.emptyRef", { slug: slot("slug"), section: slot("section"), md: slot("md") }), {
            slug: <span className="mono">{REF_FORMS.slug}</span>,
            section: <span className="mono">{REF_FORMS.section}</span>,
            md: <span className="mono">{REF_FORMS.md}</span>,
          })}
        </p>
        <p className="muted">{t("graph.emptyNext")}</p>
      </div>
    );
  } else if (answer === undefined) {
    body =
      failure === null ? (
        <div aria-busy="true">
          <Skeleton label={t("graph.drawing", { ref: nodeRef })} lines={6} />
        </div>
      ) : (
        <ReadFailure
          title={t("graph.failed", { ref: nodeRef })}
          failure={failure}
          retrying={graph.isFetching}
          attempt={graph.errorUpdateCount}
          onRetry={() => {
            retried();
            void graph.refetch({ cancelRefetch: false });
          }}
        />
      );
  } else if (answer.reason !== null) {
    body = (
      <>
        <ErrorPanel
          title={t("graph.namesNothing", { ref: nodeRef })}
          message={answer.reason}
          retrying={graph.isFetching}
          attempt={graph.dataUpdatedAt}
          onRetry={() => {
            retried();
            void graph.refetch({ cancelRefetch: false });
          }}
        />
        <AnswerNotes answer={answer} onArchive={null} />
      </>
    );
  } else if (answer.edges.length === 0 || layout === null) {
    body = (
      <>
        <NoEdge
          answer={answer}
          nodeRef={nodeRef}
          settings={settings}
          mentionsChip={chips.some((chip) => chip.type === WEAK_LINK_TYPE && !chip.pressed)}
          onImpact={() => {
            onMode(true);
          }}
          onMentions={() => {
            onChip(WEAK_LINK_TYPE);
          }}
          onArchive={includeArchive}
        />
        <AnswerNotes answer={answer} onArchive={settings.archive ? null : includeArchive} />
      </>
    );
  } else {
    body = (
      <>
        <div className="graph-summary">
          <p className="graph-count">{countText(answer, t)}</p>
          <AnswerNotes answer={answer} onArchive={settings.archive ? null : includeArchive} />
        </div>
        <Tabs
          label={t("graph.tabsLabel")}
          base={tabsBase}
          selected={settings.tab}
          onSelect={setTab}
          tabs={[
            { id: "canvas", label: t("graph.tabCanvas") },
            { id: "list", label: t("graph.tabList") },
          ]}
        />
        <TabPanel base={tabsBase} id="canvas" selected={settings.tab === "canvas"}>
          {settings.tab === "canvas" && (
            <div className="graph-canvas-panel">
              {layout.limit !== null && (
                <p className="notice cut-notice graph-limit" role="note">
                  <span className="notice-title">
                    <Icon name="info" />
                    <span>{t("graph.limit", { last: layout.limit.lastColumn, shown: layout.limit.shown, count: layout.limit.total })}</span>
                  </span>
                </p>
              )}
              <div className={`graph-stage${selectedBox === undefined ? "" : " has-details"}`}>
                <RegionBoundary name={t("graph.boundary.canvas")}>
                  <Suspense fallback={<Skeleton label={t("graph.canvasLoading")} lines={4} />}>
                    <GraphCanvas
                      layout={layout}
                      rovingId={rovingId}
                      selectedId={selectedBox?.id ?? null}
                      onRove={setRoving}
                      onSelect={setSelected}
                      onCommand={onCommand}
                      focusRef={setFocusBox}
                    />
                  </Suspense>
                </RegionBoundary>
                {selectedBox !== undefined && (
                  <GraphDetails
                    project={project}
                    answer={answer}
                    box={selectedBox}
                    headingRef={detailsHeading}
                    onClose={closeDetails}
                    onShowInList={showInList}
                  />
                )}
              </div>
              <GraphLegend types={answer.types} order={order} />
            </div>
          )}
        </TabPanel>
        <TabPanel base={tabsBase} id="list" selected={settings.tab === "list"}>
          {settings.tab === "list" && (
            <RegionBoundary name={t("graph.boundary.list")}>
              <GraphList project={project} answer={answer} />
            </RegionBoundary>
          )}
        </TabPanel>
      </>
    );
  }

  return (
    <section className="view graph-view" aria-labelledby={titleId} onKeyDown={onViewKeyDown}>
      <header className="view-head">
        <h1 id={titleId} tabIndex={-1}>
          {nodeRef === null ? t("graph.title") : t("graph.titleOf", { ref: nodeRef })}
        </h1>
        <p className="view-about">{t("graph.about")}</p>
      </header>
      <GraphControls
        project={project}
        nodeRef={nodeRef}
        settings={settings}
        chips={chips}
        fieldRef={field}
        emptyHintId={nodeRef === null ? emptyId : null}
        onChange={change}
        onMode={onMode}
        onChip={onChip}
      />
      <div ref={results} tabIndex={-1} className="graph-results" aria-busy={busy}>
        <RegionBoundary name={t("graph.boundary.graph")}>{body}</RegionBoundary>
        {answer !== undefined && graph.error !== null && (
          <p className="note" role="alert">
            {fill(t("graph.failedAgain", { message: slot("message") }), { message: <span lang="en">{apiErrorOf(graph.error).message}</span> })}
          </p>
        )}
      </div>
    </section>
  );
}

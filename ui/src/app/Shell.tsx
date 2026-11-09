import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useDataSource } from "../api/provider";
import { useLiveQueue, useProjects, useWorkspaces } from "../api/queries";
import type { Project } from "../api/types";
import { BoardView, type BoardMemory, type BoardState } from "../board/BoardView";
import { CardPage } from "../board/CardPage";
import { FlowView, type FlowMemory, type FlowState } from "../flow/FlowView";
import { GraphView } from "../graph/GraphView";
import { HealthView } from "../health/HealthView";
import type { GraphMemory, GraphSettings } from "../graph/settings";
import { HomeView } from "../overview/HomeView";
import { InboxView } from "../inbox/InboxView";
import { OutsideStageAlert } from "../inbox/OutsideStageAlert";
import { Palette } from "../palette/Palette";
import { createRoundMemory } from "../round/memory";
import { RoadmapView, type RoadmapMemory, type RoadmapTab } from "../roadmap/RoadmapView";
import { QuestionsView } from "../round/QuestionsView";
import { TasksView } from "../tasks/TasksView";
import { useTranslate } from "../i18n";
import { TreeView } from "../tree/TreeView";
import { Announcer } from "../ui/announcer";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { landingHash, useWorkspaceChoice } from "../workspace/choice";
import { ModuleBar } from "../workspace/ModuleBar";
import { ConfigNotes, WorkspaceSwitcher } from "../workspace/WorkspaceControls";
import { isJumpChord, JUMP_KEYSHORTCUTS } from "./chord";
import { DataSource } from "./DataSource";
import { ErrorBoundary } from "./ErrorBoundary";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { pushHash, replaceHash, useHash } from "./location";
import { ALL_SECTIONS, homeHash, navOf, parseHash, sectionHash, type Route } from "./routes";
import { ShortcutsDialog, ShortcutsOpener } from "./shortcuts";
import { SyncLine } from "./SyncLine";
import { NotFound, ViewFailure } from "./views";

/** The chord's hint on the "Jump to" button: Cmd on an Apple keyboard, else Ctrl; the cap is the catalog's. */
const CHORD_HINT = /Mac|iPhone|iPad/.test(navigator.userAgent) ? "app.chordApple" : "app.chordOther";

/** One key per view: per section of a project, per card's page, per project's home; `#/` is "home". */
function viewKeyOf(route: Route): string {
  if (route.type === "section") {
    return route.view === undefined ? `${route.project}/${route.section}` : `${route.project}/${route.section}/${route.id ?? ""}/${route.view}`;
  }
  return route.type === "project" ? `project:${route.project}` : route.type;
}

/** The project a route names, if any. */
function projectOf(route: Route): string | null {
  return route.type === "section" || route.type === "project" ? route.project : null;
}

/**
 * The labelled project select. When the projects cannot be read it says so with the daemon's
 * message and a Retry, instead of waiting forever; after a successful retry focus lands on the
 * select rather than nowhere.
 */
function ProjectSwitcher({
  route,
  projects,
  failure,
  retrying,
  onRetry,
}: {
  route: Route;
  projects: Project[] | undefined;
  /** The daemon's message when the projects could not be read and none are known. */
  failure: string | null;
  retrying: boolean;
  onRetry: () => void;
}) {
  const t = useTranslate();
  const select = useRef<HTMLSelectElement>(null);
  const retried = useRef(false);
  const current = projectOf(route) ?? "";
  const known = projects ?? [];
  const listed = known.some((project) => project.slug === current);

  useEffect(() => {
    if (!retried.current || projects === undefined) {
      return;
    }
    retried.current = false;
    if (focusIsLost()) {
      select.current?.focus();
    }
  }, [projects]);

  if (projects === undefined && failure !== null) {
    return (
      <div className="project-switcher project-switcher-failed">
        <span className="project-switcher-error">
          <Icon name="alert" />
          <span>{t("app.project.failed")}</span>
        </span>
        <span className="verbatim" lang="en">
          {failure}
        </span>
        {/* The accessible name starts with the visible label (WCAG 2.5.3); the rest is for screen readers. */}
        <button
          type="button"
          className="button"
          aria-disabled={retrying}
          onClick={() => {
            if (!retrying) {
              retried.current = true;
              onRetry();
            }
          }}
        >
          <Icon name="retry" />
          <span>
            {retrying ? t("ui.retrying") : t("ui.retry")} <span className="sr-only">{t("app.project.retryWhat")}</span>
          </span>
        </button>
      </div>
    );
  }

  return (
    <div className="project-switcher">
      <label htmlFor="project-switcher">{t("app.project.label")}</label>
      <select
        id="project-switcher"
        ref={select}
        className="field"
        value={current}
        disabled={projects === undefined}
        onChange={(event) => {
          // A section keeps its section; the home and any other route land on the other project's home.
          const other = event.target.value;
          pushHash(route.type === "section" ? sectionHash(other, route.section) : homeHash(other));
        }}
      >
        {!listed && (
          <option value={current} disabled>
            {projects === undefined ? t("app.project.loading") : current === "" ? t("app.project.choose") : current}
          </option>
        )}
        {known.map((project) => (
          <option key={project.slug} value={project.slug}>
            {project.name ?? project.slug}
          </option>
        ))}
      </select>
    </div>
  );
}

/**
 * Header, the home and the chosen workspace's sections, the current view inside its own error
 * boundary, the jump palette. A workspace orders the nav and hides no route: every section renders
 * at its hash, from the palette and from any link (docs/features/workspaces.md "The UI", W4).
 */
export function Shell({ scenario }: { scenario: string | null }) {
  const t = useTranslate();
  const hash = useHash();
  const route = parseHash(hash);
  const projects = useProjects();
  const projectsFailure = useRetainedFailure(projects.error, projects.isFetching);
  const dataSource = useDataSource();
  const focusLater = useFocusLater();
  const main = useRef<HTMLElement>(null);
  const shortcutsTrigger = useRef<HTMLElement | null>(null);
  const shownKey = useRef(viewKeyOf(route));
  /** Retry was pressed on the home view's error panel; the answer takes away the Retry that had focus. */
  const homeRetried = useRef(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  /** The palette is open, as the document's key listener sees it, ahead of the next render. */
  const paletteShown = useRef(false);
  const paletteTrigger = useRef<HTMLElement | null>(null);
  const paletteInput = useRef<HTMLInputElement>(null);
  /** A jump's hash: once the route shows it, the view's `h1` takes the focus (once). */
  const jumpedTo = useRef<string | null>(null);
  /** The Graph's options per project for this page's life; never in the hash (docs/features/ui-graph.md). */
  const graphSettings = useRef(new Map<string, GraphSettings>());
  const graphMemory = useMemo<GraphMemory>(
    () => ({
      recall: (project) => graphSettings.current.get(project),
      remember: (project, settings) => {
        graphSettings.current.set(project, settings);
      },
    }),
    [],
  );
  /** The Roadmap's tab per project for this page's life; never in the hash (docs/features/roadmap-screen.md). */
  const roadmapTabs = useRef(new Map<string, RoadmapTab>());
  const roadmapMemory = useMemo<RoadmapMemory>(
    () => ({
      recall: (project) => roadmapTabs.current.get(project),
      remember: (project, tab) => {
        roadmapTabs.current.set(project, tab);
      },
    }),
    [],
  );
  /** The Flow's tab and last place per project for this page's life: a bare `#/<p>/flow` takes the last place (docs/features/writer-studio.md D5). */
  const flowStates = useRef(new Map<string, FlowState>());
  const flowMemory = useMemo<FlowMemory>(
    () => ({
      recall: (project) => flowStates.current.get(project),
      remember: (project, state) => {
        flowStates.current.set(project, state);
      },
    }),
    [],
  );
  /** The Board's tab, filters, Show cancelled and folded lanes per project for this page's life; never in the hash (`ui/README.md` "Screen rules", Shell). */
  const boardStates = useRef(new Map<string, BoardState>());
  const boardMemory = useMemo<BoardMemory>(
    () => ({
      recall: (project) => boardStates.current.get(project),
      remember: (project, state) => {
        boardStates.current.set(project, state);
      },
    }),
    [],
  );
  /** The Questions screen's drafts, limit and last outcome per project for this page's life; a reload drops them (docs/features/ui-round.md). */
  const [roundMemory] = useState(createRoundMemory);
  const firstProject = projects.data?.[0]?.slug ?? null;
  const viewKey = viewKeyOf(route);
  const routeProject = projectOf(route);
  /** The route's project once the daemon has named it among its projects. */
  const shownProject = routeProject !== null && projects.data?.some((project) => project.slug === routeProject) === true ? routeProject : null;
  // The shown project's live queue; a stage it says changed outside this tab is an alert above the
  // view (docs/canon/decision-staging.md "UI").
  const live = useLiveQueue(shownProject);
  const navProject = routeProject ?? firstProject;
  // The nav's project's workspaces and modules (docs/features/workspaces.md "The UI"): a config
  // read, no turn; while it is out the nav is a skeleton, and failed it lists every section.
  const workspaces = useWorkspaces(navProject ?? "", navProject !== null);
  const workspacesFailure = useRetainedFailure(workspaces.error, workspaces.isFetching);
  const choice = useWorkspaceChoice(navProject, workspaces.data);
  /** The workspaces read failed with no answer to keep: every section is shown. */
  const workspacesFailed = workspaces.data === undefined && workspacesFailure !== null;
  const navRef = useRef<HTMLElement>(null);
  /** Retry was pressed in the config notes; its answer may take the button that had focus. */
  const workspacesRetried = useRef(false);
  /**
   * The root the Spec tree and the Flow read from: the chosen module's, null for the whole project
   * (no module, or the read failed), undefined while the read is out, so neither view reads before
   * its module is known (WS-9).
   */
  const moduleRoot = workspaces.data !== undefined ? (choice.module?.root ?? null) : workspacesFailed ? null : undefined;
  const landingProject = route.type === "home" ? firstProject : null;
  const landing = landingProject === null ? null : workspaces.data !== undefined ? landingHash(landingProject, choice.workspace) : workspacesFailed ? homeHash(landingProject) : null;

  // `#/` is the first project's landing in its chosen workspace, once its workspaces are read, or its
  // home if they cannot be; without a history entry of its own. A typed `#/<p>` is never redirected.
  useEffect(() => {
    if (landing !== null) {
      replaceHash(landing);
    }
  }, [landing]);

  // A Retry in the config notes answered: the nav changed under it; focus lost with the button goes there.
  useEffect(() => {
    if (!workspacesRetried.current || workspaces.data === undefined) {
      return;
    }
    workspacesRetried.current = false;
    if (focusIsLost()) {
      navRef.current?.querySelector<HTMLElement>("a")?.focus();
    }
  }, [workspaces.data]);

  // A palette jump: focus went to main as the palette closed; the destination's heading takes it
  // once rendered, also when the view stays the same (one task to another).
  useEffect(() => {
    const wanted = jumpedTo.current;
    if (wanted === null || hash !== wanted) {
      return;
    }
    const heading = main.current?.querySelector<HTMLElement>("h1");
    if (heading !== null && heading !== undefined) {
      jumpedTo.current = null;
      heading.focus();
    }
  });

  // The chord, Cmd-K or Ctrl-K: the shell's one document key listener (capture), for the palette
  // only, from anywhere, a text field included. Every other key passes untouched.
  const chordAction = useRef<() => void>(() => undefined);
  useEffect(() => {
    chordAction.current = () => {
      if (paletteShown.current) {
        paletteInput.current?.focus();
      } else if (document.querySelector('[role="dialog"]') === null) {
        openPalette(document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : null);
      }
    };
  });
  useEffect(() => {
    function onChord(event: KeyboardEvent) {
      if (!isJumpChord(event)) {
        return;
      }
      event.preventDefault();
      chordAction.current();
    }
    document.addEventListener("keydown", onChord, true);
    return () => {
      document.removeEventListener("keydown", onChord, true);
    };
  }, []);

  // A new view focuses its heading. Leaving home does so only after a Retry there whose button has
  // gone with focus on it; on first load focus stays at the top of the page. A dialog the new view
  // opened (the Board's card) keeps the focus it took.
  useEffect(() => {
    const previous = shownKey.current;
    shownKey.current = viewKey;
    if (previous === viewKey) {
      return;
    }
    const afterRetry = previous === "home" && homeRetried.current && focusIsLost();
    if (previous === "home") {
      homeRetried.current = false;
    }
    if ((previous !== "home" || afterRetry) && (document.activeElement?.closest('[role="dialog"]') ?? null) === null) {
      main.current?.querySelector<HTMLElement>("h1")?.focus();
    }
  }, [viewKey]);

  // A Retry on home answered with no projects stays on home: its heading takes the focus.
  useEffect(() => {
    if (!homeRetried.current || projects.data?.length !== 0) {
      return;
    }
    homeRetried.current = false;
    if (focusIsLost()) {
      main.current?.querySelector<HTMLElement>("h1")?.focus();
    }
  }, [projects.data]);

  function retryProjects() {
    void projects.refetch({ cancelRefetch: false });
  }

  function openShortcuts() {
    shortcutsTrigger.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setShortcutsOpen(true);
  }

  function closeShortcuts() {
    setShortcutsOpen(false);
    const back = shortcutsTrigger.current;
    focusLater(() => (back?.isConnected === true ? back : null));
  }

  function openPalette(trigger: HTMLElement | null) {
    paletteTrigger.current = trigger;
    paletteShown.current = true;
    jumpedTo.current = null;
    setPaletteOpen(true);
  }

  function closePalette() {
    paletteShown.current = false;
    setPaletteOpen(false);
    const back = paletteTrigger.current;
    focusLater(() => (back?.isConnected === true ? back : main.current));
  }

  /** One history entry, none to the hash shown; focus to main, then the destination's heading. */
  function jump(target: string) {
    paletteShown.current = false;
    setPaletteOpen(false);
    jumpedTo.current = target;
    if (target !== window.location.hash) {
      pushHash(target);
    }
    focusLater(() => main.current);
  }

  const inboxHref = navProject === null ? "#/" : sectionHash(navProject, "inbox");

  function chooseWorkspace(name: string) {
    if (navProject === null || workspaces.data === undefined) {
      return;
    }
    choice.chooseWorkspace(name);
    // Its landing, one history entry (none when it is the hash shown).
    pushHash(landingHash(navProject, workspaces.data.workspaces.find((workspace) => workspace.name === name) ?? null));
  }

  function retryWorkspaces() {
    workspacesRetried.current = true;
    void workspaces.refetch({ cancelRefetch: false });
  }

  // The nav: the chosen workspace's sections; every section when the read failed, or with no project
  // to read them for (the projects could not be read, or there are none); a skeleton while it is out.
  const navPending = navProject === null ? projects.data === undefined && projectsFailure === null : workspaces.data === undefined && !workspacesFailed;
  const navSections = choice.workspace?.sections ?? ALL_SECTIONS;
  const routeModules = route.type === "section" && route.section === "tree" ? (workspaces.data?.modules ?? []) : [];

  let view;
  if (route.type === "not_found") {
    view = <NotFound reason={t("app.notFound.nothingAt", { hash })} inboxHref={inboxHref} />;
  } else if (route.type === "home") {
    if (projects.data === undefined && projectsFailure !== null) {
      view = (
        <section className="view" aria-labelledby="view-title">
          <header className="view-head">
            <h1 id="view-title" tabIndex={-1}>
              {t("app.start.projects")}
            </h1>
          </header>
          <ErrorPanel
            title={t("app.start.failed")}
            message={apiErrorOf(projectsFailure).message}
            retrying={projects.isFetching}
            attempt={projects.errorUpdateCount}
            onRetry={() => {
              homeRetried.current = true;
              retryProjects();
            }}
          />
        </section>
      );
    } else if (projects.data?.length === 0) {
      view = (
        <section className="view" aria-labelledby="view-title">
          <header className="view-head">
            <h1 id="view-title" tabIndex={-1}>
              {t("app.start.none")}
            </h1>
          </header>
          <p>{t("app.start.noneNext")}</p>
        </section>
      );
    } else {
      view = <Skeleton label={t("app.start.opening")} lines={3} />;
    }
  } else if (projects.data !== undefined && !projects.data.some((project) => project.slug === route.project)) {
    view = <NotFound reason={t("app.notFound.noProject", { project: route.project })} inboxHref={inboxHref} />;
  } else if (route.type === "project") {
    view = <HomeView key={route.project} project={route.project} />;
  } else if (route.section === "inbox") {
    view = <InboxView key={route.project} project={route.project} selectedId={route.id} />;
  } else if (route.section === "tasks") {
    view = <TasksView key={route.project} project={route.project} taskId={route.id} />;
  } else if (route.section === "board" && route.view !== undefined && route.id !== null) {
    view = <CardPage key={`${route.project}/${route.id}`} project={route.project} id={route.id} />;
  } else if (route.section === "board") {
    view = (
      <BoardView
        key={route.project}
        project={route.project}
        cardId={route.id}
        memory={boardMemory}
        flows={choice.workspace?.flows ?? null}
        flowsKnown={workspaces.data !== undefined || workspacesFailed}
        lens={choice.workspace?.name ?? null}
      />
    );
  } else if (route.section === "roadmap") {
    view = <RoadmapView key={route.project} project={route.project} itemId={route.id} memory={roadmapMemory} />;
  } else if (route.section === "flow") {
    // The module is in Flow's hash, the stored one its default; its bar is in the places pane (docs/features/writer-studio.md D4).
    view = (
      <FlowView
        key={route.project}
        project={route.project}
        nodeRef={route.id}
        query={route.query}
        memory={flowMemory}
        modules={workspaces.data?.modules ?? []}
        storedModule={choice.module}
        onChooseModule={choice.chooseModule}
        moduleKnown={moduleRoot !== undefined}
      />
    );
  } else if (route.section === "tree") {
    view = <TreeView key={route.project} project={route.project} nodeRef={route.id} moduleRoot={moduleRoot ?? null} moduleKnown={moduleRoot !== undefined} />;
  } else if (route.section === "graph") {
    view = <GraphView key={route.project} project={route.project} nodeRef={route.id} memory={graphMemory} />;
  } else if (route.section === "health") {
    view = <HealthView key={route.project} project={route.project} />;
  } else {
    view = <QuestionsView key={route.project} project={route.project} memory={roundMemory} />;
  }

  return (
    <ShortcutsOpener value={openShortcuts}>
      <Announcer>
        <a
          className="skip-link"
          href="#main"
          onClick={(event) => {
            event.preventDefault();
            main.current?.focus();
          }}
        >
          {t("app.skipToContent")}
        </a>
        <header className="app-header">
          <p className="brand">{t("app.brand")}</p>
          <ProjectSwitcher
            route={route}
            projects={projects.data}
            failure={projectsFailure === null ? null : apiErrorOf(projectsFailure).message}
            retrying={projects.isFetching}
            onRetry={retryProjects}
          />
          {navProject !== null && choice.workspace !== null && workspaces.data !== undefined && (
            <WorkspaceSwitcher workspaces={workspaces.data.workspaces} chosen={choice.workspace} onChoose={chooseWorkspace} />
          )}
          {navProject !== null && (
            <ConfigNotes
              key={navProject}
              notes={workspaces.data?.notes ?? []}
              failure={workspacesFailure === null ? null : apiErrorOf(workspacesFailure).message}
              kept={workspaces.data !== undefined}
              retrying={workspaces.isFetching}
              attempt={workspaces.errorUpdateCount}
              onRetry={retryWorkspaces}
            />
          )}
          <LanguageSwitcher />
          <DataSource dataSource={dataSource} scenario={scenario} />
          <button
            type="button"
            className="button button-quiet jump-button"
            aria-haspopup="dialog"
            aria-keyshortcuts={JUMP_KEYSHORTCUTS}
            onClick={(event: MouseEvent<HTMLButtonElement>) => {
              openPalette(event.currentTarget);
            }}
          >
            <Icon name="search" />
            <span>{t("app.jumpTo")}</span>
            <kbd className="jump-hint" aria-hidden="true">
              {t(CHORD_HINT)}
            </kbd>
          </button>
          <button type="button" className="button button-quiet" onClick={openShortcuts}>
            <Icon name="keyboard" />
            <span>{t("app.shortcutsButton")}</span>
          </button>
        </header>
        <div className="app-body">
          <nav ref={navRef} className="app-nav" aria-label={t("app.navLabel")} aria-busy={navPending ? true : undefined}>
            {navPending ? (
              <Skeleton label={t("workspace.navPending")} lines={6} />
            ) : (
              <ul>
                {navOf(navProject ?? "", navSections, t).map((entry) => {
                  // A section the nav does not list still renders at its hash; then no item is current.
                  const active = entry.key === "overview" ? route.type === "project" : route.type === "section" && route.section === entry.key;
                  return (
                    <li key={entry.key}>
                      <a href={navProject === null ? "#/" : entry.hash} aria-current={active ? "page" : undefined}>
                        {entry.label}
                      </a>
                    </li>
                  );
                })}
              </ul>
            )}
          </nav>
          <main id="main" ref={main} tabIndex={-1} className="app-main">
            {/* A hub's push status, above the view, no control (docs/features/hub-git.md "Status and the screen"). */}
            {shownProject !== null && <SyncLine key={shownProject} project={shownProject} />}
            {live.outside !== null && (
              // A new alert per change, so each is spoken.
              <OutsideStageAlert
                key={live.outside.seq}
                change={live.outside}
                onDismiss={() => {
                  live.dismiss();
                  focusLater(() => main.current);
                }}
              />
            )}
            {/* The module bar on the Spec tree alone, outside the view, so a root naming nothing leaves it (docs/features/workspaces.md "The UI"); Flow's is in its places pane. */}
            {shownProject !== null && <ModuleBar modules={routeModules} chosen={choice.module} onChoose={choice.chooseModule} />}
            <ErrorBoundary key={viewKey} fallback={(error, reset) => <ViewFailure error={error} onRetry={reset} />}>
              {view}
            </ErrorBoundary>
          </main>
        </div>
        {shortcutsOpen && <ShortcutsDialog section={route.type === "section" ? route.section : null} onClose={closeShortcuts} />}
        {paletteOpen && (
          <Palette
            routeProject={projectOf(route)}
            projects={projects.data}
            projectsFailure={projectsFailure === null ? null : apiErrorOf(projectsFailure).message}
            inputRef={paletteInput}
            onJump={jump}
            onClose={closePalette}
          />
        )}
      </Announcer>
    </ShortcutsOpener>
  );
}

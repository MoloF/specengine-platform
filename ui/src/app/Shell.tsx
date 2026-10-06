import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { apiErrorOf } from "../api/client";
import { useDataSource } from "../api/provider";
import { useProjects } from "../api/queries";
import type { Project } from "../api/types";
import { GraphView } from "../graph/GraphView";
import type { GraphMemory, GraphSettings } from "../graph/settings";
import { HomeView } from "../overview/HomeView";
import { InboxView } from "../inbox/InboxView";
import { Palette } from "../palette/Palette";
import { TasksView } from "../tasks/TasksView";
import { TreeView } from "../tree/TreeView";
import { Announcer } from "../ui/announcer";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { isJumpChord, JUMP_KEYSHORTCUTS } from "./chord";
import { DataSource } from "./DataSource";
import { ErrorBoundary } from "./ErrorBoundary";
import { pushHash, replaceHash, useHash } from "./location";
import { homeHash, navOf, parseHash, sectionHash, type Route } from "./routes";
import { SECTIONS } from "./sections";
import { ShortcutsDialog, ShortcutsOpener } from "./shortcuts";
import { NotBuilt, NotFound, ViewFailure } from "./views";

/** The chord's hint on the "Jump to" button: Cmd on an Apple keyboard, else Ctrl. */
const CHORD_HINT = /Mac|iPhone|iPad/.test(navigator.userAgent) ? "\u2318K" : "Ctrl K";

/** One key per view: per section of a project, per project's home; `#/` is "home". */
function viewKeyOf(route: Route): string {
  if (route.type === "section") {
    return `${route.project}/${route.section}`;
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
          <span>Projects could not be loaded:</span>
        </span>
        <span className="verbatim">{failure}</span>
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
            {retrying ? "Retrying" : "Retry"} <span className="sr-only">loading the projects</span>
          </span>
        </button>
      </div>
    );
  }

  return (
    <div className="project-switcher">
      <label htmlFor="project-switcher">Project</label>
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
            {projects === undefined ? "Loading projects" : current === "" ? "Choose a project" : current}
          </option>
        )}
        {known.map((project) => (
          <option key={project.slug} value={project.slug}>
            {project.name}
          </option>
        ))}
      </select>
    </div>
  );
}

/** Header, the home and the six sections, the current view inside its own error boundary, the jump palette. */
export function Shell({ scenario }: { scenario: string | null }) {
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
  const firstProject = projects.data?.[0]?.slug ?? null;
  const viewKey = viewKeyOf(route);

  // `#/` is the first project's home, without a history entry of its own.
  useEffect(() => {
    if (route.type === "home" && firstProject !== null) {
      replaceHash(homeHash(firstProject));
    }
  }, [route.type, firstProject]);

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
  // gone with focus on it; on first load focus stays at the top of the page.
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
    if (previous !== "home" || afterRetry) {
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

  const navProject = projectOf(route) ?? firstProject;
  const inboxHref = navProject === null ? "#/" : sectionHash(navProject, "inbox");

  let view;
  if (route.type === "not_found") {
    view = <NotFound reason={`Nothing lives at ${hash}.`} inboxHref={inboxHref} />;
  } else if (route.type === "home") {
    if (projects.data === undefined && projectsFailure !== null) {
      view = (
        <section className="view" aria-labelledby="view-title">
          <header className="view-head">
            <h1 id="view-title" tabIndex={-1}>
              Projects
            </h1>
          </header>
          <ErrorPanel
            title="The projects could not be loaded"
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
              No projects
            </h1>
          </header>
          <p>The daemon serves no project yet. Next: run spec init in a project and index it.</p>
        </section>
      );
    } else {
      view = <Skeleton label="Opening the first project" lines={3} />;
    }
  } else if (projects.data !== undefined && !projects.data.some((project) => project.slug === route.project)) {
    view = <NotFound reason={`There is no project ${route.project}.`} inboxHref={inboxHref} />;
  } else if (route.type === "project") {
    view = <HomeView key={route.project} project={route.project} />;
  } else if (route.section === "inbox") {
    view = <InboxView key={route.project} project={route.project} selectedId={route.id} />;
  } else if (route.section === "tasks") {
    view = <TasksView key={route.project} project={route.project} taskId={route.id} />;
  } else if (route.section === "tree") {
    view = <TreeView key={route.project} project={route.project} nodeRef={route.id} />;
  } else if (route.section === "graph") {
    view = <GraphView key={route.project} project={route.project} nodeRef={route.id} memory={graphMemory} />;
  } else {
    const section = SECTIONS.find((candidate) => candidate.id === route.section);
    view = section !== undefined && section.slice !== null ? <NotBuilt section={section} /> : null;
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
          Skip to content
        </a>
        <header className="app-header">
          <p className="brand">SpecEngine</p>
          <ProjectSwitcher
            route={route}
            projects={projects.data}
            failure={projectsFailure === null ? null : apiErrorOf(projectsFailure).message}
            retrying={projects.isFetching}
            onRetry={retryProjects}
          />
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
            <span>Jump to</span>
            <kbd className="jump-hint" aria-hidden="true">
              {CHORD_HINT}
            </kbd>
          </button>
          <button type="button" className="button button-quiet" onClick={openShortcuts}>
            <Icon name="keyboard" />
            <span>Keyboard shortcuts</span>
          </button>
        </header>
        <div className="app-body">
          <nav className="app-nav" aria-label="Sections">
            <ul>
              {navOf(navProject ?? "").map((entry) => {
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
          </nav>
          <main id="main" ref={main} tabIndex={-1} className="app-main">
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

import { useEffect, useRef, useState } from "react";
import { apiErrorOf } from "../api/client";
import { useDataSource } from "../api/provider";
import { useProjects } from "../api/queries";
import type { Project } from "../api/types";
import { InboxView } from "../inbox/InboxView";
import { TreeView } from "../tree/TreeView";
import { Announcer } from "../ui/announcer";
import { focusIsLost } from "../ui/focus";
import { Icon } from "../ui/Icon";
import { ErrorPanel, Skeleton } from "../ui/states";
import { useFocusLater } from "../ui/useFocusLater";
import { useRetainedFailure } from "../ui/useRetainedFailure";
import { DataSource } from "./DataSource";
import { ErrorBoundary } from "./ErrorBoundary";
import { pushHash, replaceHash, useHash } from "./location";
import { parseHash, sectionHash, type Route } from "./routes";
import { SECTIONS } from "./sections";
import { ShortcutsDialog, ShortcutsOpener } from "./shortcuts";
import { NotBuilt, NotFound, ViewFailure } from "./views";

function viewKeyOf(route: Route): string {
  return route.type === "section" ? `${route.project}/${route.section}` : route.type;
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
  const current = route.type === "section" ? route.project : "";
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
          const section = route.type === "section" ? route.section : "inbox";
          pushHash(sectionHash(event.target.value, section));
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

/** Header, the six sections, the current view inside its own error boundary. */
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
  const firstProject = projects.data?.[0]?.slug ?? null;
  const viewKey = viewKeyOf(route);

  useEffect(() => {
    if (route.type === "home" && firstProject !== null) {
      replaceHash(sectionHash(firstProject, "inbox"));
    }
  }, [route.type, firstProject]);

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

  const navProject = route.type === "section" ? route.project : firstProject;
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
  } else if (route.section === "inbox") {
    view = <InboxView key={route.project} project={route.project} selectedId={route.id} />;
  } else if (route.section === "tree") {
    view = <TreeView key={route.project} project={route.project} nodeRef={route.id} />;
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
          <button type="button" className="button button-quiet" onClick={openShortcuts}>
            <Icon name="keyboard" />
            <span>Keyboard shortcuts</span>
          </button>
        </header>
        <div className="app-body">
          <nav className="app-nav" aria-label="Sections">
            <ul>
              {SECTIONS.map((section) => {
                const active = route.type === "section" && route.section === section.id;
                return (
                  <li key={section.id}>
                    <a
                      href={navProject === null ? "#/" : sectionHash(navProject, section.id)}
                      aria-current={active ? "page" : undefined}
                    >
                      {section.label}
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
      </Announcer>
    </ShortcutsOpener>
  );
}

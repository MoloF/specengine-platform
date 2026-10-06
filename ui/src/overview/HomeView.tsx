import type { KeyboardEvent } from "react";
import { useOpenShortcuts } from "../app/shortcuts";
import { hasModifier, isTextField } from "../ui/keys";
import { InboxRegion } from "./InboxRegion";
import { TasksRegion } from "./TasksRegion";

/**
 * A project's home (`#/<project>`): what waits for the owner, from two reads the client has. The
 * Tasks region first (a plan to review, an approved spec that changed: the one control point,
 * ADR-0012), then the queue. Each region reads, fails and retries on its own; nothing here acts.
 */
export function HomeView({ project }: { project: string }) {
  const openShortcuts = useOpenShortcuts();

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || hasModifier(event) || isTextField(event.target)) {
      return;
    }
    if (event.key === "?") {
      event.preventDefault();
      openShortcuts();
    }
  }

  return (
    <section className="view home-view" aria-labelledby="home-title" onKeyDown={onKeyDown}>
      <header className="view-head">
        <h1 id="home-title" tabIndex={-1}>
          Overview
        </h1>
        <p className="view-about">
          What waits for you in <span className="mono">{project}</span>.
        </p>
      </header>
      <div className="home-regions">
        <TasksRegion project={project} />
        <InboxRegion project={project} />
      </div>
    </section>
  );
}

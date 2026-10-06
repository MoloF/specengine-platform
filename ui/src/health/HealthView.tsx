import { lazy, Suspense, type KeyboardEvent } from "react";
import { useCheck } from "../api/queries";
import { SECTIONS } from "../app/sections";
import { useOpenShortcuts } from "../app/shortcuts";
import { hasModifier, isTextField } from "../ui/keys";
import { Skeleton } from "../ui/states";

const HealthRegions = lazy(() => import("./HealthRegions").then((module) => ({ default: module.HealthRegions })));

const ABOUT = SECTIONS.find((section) => section.id === "health")?.about ?? "";

/**
 * Health (`#/<project>/health`, docs/features/ui-health.md): `spec check`'s answer and what is left
 * to decide, read only. One `getCheck` on entering, shared by Check, Findings and Debt and budgets;
 * one `getInbox` for What is left. Each region loads, fails and retries on its own; no clock is
 * read: the report carries no time, and "Check again" reads it once more. The heading stays put
 * while the regions' chunk loads, so focus on it is never lost.
 */
export function HealthView({ project }: { project: string }) {
  const openShortcuts = useOpenShortcuts();
  const check = useCheck(project);

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
    <section className="view health-view" aria-labelledby="health-title" onKeyDown={onKeyDown}>
      <header className="view-head">
        <h1 id="health-title" tabIndex={-1}>
          Health
        </h1>
        <p className="view-about">
          {ABOUT} The check of <span className="mono">{project}</span>, as <code>spec check</code> reports it.
        </p>
      </header>
      <Suspense fallback={<Skeleton label={`Loading the Health of ${project}`} lines={6} />}>
        <HealthRegions project={project} check={check} />
      </Suspense>
    </section>
  );
}

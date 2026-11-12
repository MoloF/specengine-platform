import { lazy, Suspense, type KeyboardEvent } from "react";
import type { Workspace, WorkspaceModule } from "../api/types";
import { useOpenShortcuts } from "../app/shortcuts";
import { useTranslate } from "../i18n";
import { fill, slot } from "../i18n/fill";
import { hasModifier, isTextField } from "../ui/keys";
import { Skeleton } from "../ui/states";

// The Overview's blocks, a chunk of their own, reached by a dynamic import alone, so first load
// never holds them (docs/features/department-overview.md "Acceptance criteria", DO7).
const DepartmentOverview = lazy(() => import("./DepartmentOverview").then((module) => ({ default: module.DepartmentOverview })));

/**
 * A project's home (`#/<project>`): the chosen department's own numbers, counted by status category
 * over its flows, what waits on it, its flow's gaps where its sections list Flow, and every
 * department side by side where its `flows` is null (docs/features/department-overview.md
 * "Acceptance criteria", DO12). `workspace` and `module` are the viewer's choice, `workspaces` the
 * served departments: null while their read is out, empty when it failed. Nothing here acts.
 */
export function HomeView({
  project,
  workspace,
  module,
  workspaces,
}: {
  project: string;
  workspace: Workspace | null;
  module: WorkspaceModule | null;
  workspaces: readonly Workspace[] | null;
}) {
  const t = useTranslate();
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

  const named = workspace !== null && workspaces !== null && workspaces.length >= 2;
  return (
    <section className="view home-view" aria-labelledby="home-title" onKeyDown={onKeyDown}>
      <header className="view-head">
        <h1 id="home-title" tabIndex={-1}>
          {t("home.title")}
        </h1>
        <p className="view-about">{fill(t("home.about", { project: slot("project") }), { project: <span className="mono">{project}</span> })}</p>
        {named && <p className="view-about">{fill(t("home.department.named", { department: slot("department") }), { department: <bdi>{workspace.title}</bdi> })}</p>}
      </header>
      <Suspense fallback={<Skeleton label={t("home.department.loading", { project })} lines={6} />}>
        <DepartmentOverview project={project} workspace={workspace} module={module} workspaces={workspaces} />
      </Suspense>
    </section>
  );
}

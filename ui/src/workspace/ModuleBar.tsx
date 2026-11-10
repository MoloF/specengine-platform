import { useId, type ReactNode } from "react";
import type { WorkspaceModule } from "../api/types";
import { useTranslate } from "../i18n";
import { ALL_MODULES, isDirection, isRooted } from "./choice";

/** One choice of the bar: a module's name, or ALL_MODULES. */
function ModuleOption({ pressed, onPress, children }: { pressed: boolean; onPress: () => void; children: ReactNode }) {
  return (
    <button type="button" className="module-option" aria-pressed={pressed} onClick={onPress}>
      {children}
    </button>
  );
}

/**
 * The module bar (`docs/canon/workspaces.md` "Screen"): on the Spec tree alone, and only when the
 * project serves a module; none is made up from the tree. Group Modules: All modules, then each
 * rooted module's title in served order; group Directions, each direction's title in served order,
 * only when one is served (docs/features/departments.md "Acceptance criteria", DP-11). One choice
 * across both, pressed. A choice roots the tree there, or lists a direction's kinds, and is Flow's
 * default too where Flow takes it; the bar stays whatever the read answers, so another module can be
 * chosen. Flow's own bar, of links, is in its places pane and lists rooted modules only.
 */
export function ModuleBar({
  modules,
  chosen,
  onChoose,
}: {
  modules: readonly WorkspaceModule[];
  /** The chosen module; null: All modules. */
  chosen: WorkspaceModule | null;
  /** A module's name, ALL_MODULES for the whole project. */
  onChoose: (name: string) => void;
}) {
  const t = useTranslate();
  const modulesId = useId();
  const directionsId = useId();
  if (modules.length === 0) {
    return null;
  }
  const rooted = modules.filter(isRooted);
  const directions = modules.filter(isDirection);
  const option = (module: WorkspaceModule) => (
    <ModuleOption
      key={module.name}
      pressed={chosen?.name === module.name}
      onPress={() => {
        onChoose(module.name);
      }}
    >
      <bdi>{module.title}</bdi>
    </ModuleOption>
  );
  return (
    <div className="module-bar">
      <div role="group" aria-labelledby={modulesId} className="module-bar-group">
        <span id={modulesId} className="module-bar-label">
          {t("workspace.modulesLabel")}
        </span>
        <ModuleOption
          pressed={chosen === null}
          onPress={() => {
            onChoose(ALL_MODULES);
          }}
        >
          {t("workspace.allModules")}
        </ModuleOption>
        {rooted.map(option)}
      </div>
      {directions.length > 0 && (
        <div role="group" aria-labelledby={directionsId} className="module-bar-group">
          <span id={directionsId} className="module-bar-label">
            {t("workspace.directionsLabel")}
          </span>
          {directions.map(option)}
        </div>
      )}
    </div>
  );
}

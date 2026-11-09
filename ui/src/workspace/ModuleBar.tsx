import { useId } from "react";
import type { WorkspaceModule } from "../api/types";
import { ALL_MODULES } from "./choice";
import { WORKSPACE_TEXT } from "./text";

/**
 * The module bar (docs/features/workspaces.md "The UI"): on the Spec tree and the Flow alone, and
 * only when the project serves a module; none is made up from the tree (D10). All modules, then each
 * module's title in served order, the chosen one pressed. A choice roots both views there; the bar
 * stays whatever the rooted read answers, so another module can be chosen.
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
  const labelId = useId();
  if (modules.length === 0) {
    return null;
  }
  return (
    <div role="group" aria-labelledby={labelId} className="module-bar">
      <span id={labelId} className="module-bar-label">
        {WORKSPACE_TEXT.modulesLabel}
      </span>
      <button
        type="button"
        className="module-option"
        aria-pressed={chosen === null}
        onClick={() => {
          onChoose(ALL_MODULES);
        }}
      >
        {WORKSPACE_TEXT.allModules}
      </button>
      {modules.map((module) => (
        <button
          key={module.name}
          type="button"
          className="module-option"
          aria-pressed={chosen?.name === module.name}
          onClick={() => {
            onChoose(module.name);
          }}
        >
          <bdi>{module.title}</bdi>
        </button>
      ))}
    </div>
  );
}

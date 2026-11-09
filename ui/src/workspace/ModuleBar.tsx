import { useId } from "react";
import type { WorkspaceModule } from "../api/types";
import { useTranslate } from "../i18n";
import { ALL_MODULES } from "./choice";

/**
 * The module bar (docs/features/workspaces.md "The UI"): on the Spec tree alone, and only when the
 * project serves a module; none is made up from the tree (D10). All modules, then each module's
 * title in served order, the chosen one pressed. A choice roots the tree there and is Flow's default
 * too; the bar stays whatever the rooted read answers, so another module can be chosen. Flow's own
 * bar, of links, is in its places pane (docs/features/writer-studio.md D4).
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
  const labelId = useId();
  if (modules.length === 0) {
    return null;
  }
  return (
    <div role="group" aria-labelledby={labelId} className="module-bar">
      <span id={labelId} className="module-bar-label">
        {t("workspace.modulesLabel")}
      </span>
      <button
        type="button"
        className="module-option"
        aria-pressed={chosen === null}
        onClick={() => {
          onChoose(ALL_MODULES);
        }}
      >
        {t("workspace.allModules")}
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

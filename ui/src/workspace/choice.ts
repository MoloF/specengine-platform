import { useCallback, useState } from "react";
import type { Workspace, WorkspaceDocument, WorkspaceModule } from "../api/types";
import { homeHash, sectionHash } from "../app/routes";
import { isSectionId } from "../app/sections";

// A viewer's choice of workspace and module (docs/features/workspaces.md "The UI", D8): per viewer
// and per project, in `localStorage`, as `specengine.board.as` is kept
// (`ui/README.md` "Screen rules", which lists the storage keys). Every name is the served document's: a stored one is
// looked up among the served names, never compared with a word of the UI's (W5). Every storage
// access may throw (a private window, a sandboxed frame, a policy): then the choice holds in the
// shell's state until a reload, as the tree's Show drift does (`src/tree/drift.ts`).

/** Where the chosen workspace's name is kept for a project. */
export function workspaceKey(project: string): string {
  return `specengine.workspace.${project}`;
}

/** Where the chosen module's name is kept for a project; `""` stands for All modules. */
export function moduleKey(project: string): string {
  return `specengine.module.${project}`;
}

/** The stored value of All modules. */
export const ALL_MODULES = "";

function recall(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Keeps `value` under `key`, null removing it; false when the browser refuses. */
function keep(key: string, value: string | null): boolean {
  try {
    if (value === null) {
      window.localStorage.removeItem(key);
    } else {
      window.localStorage.setItem(key, value);
    }
    return true;
  } catch {
    return false;
  }
}

/** The workspace a stored name gives: that one while the document serves it, else the first; null with none served. */
export function workspaceOf(document: WorkspaceDocument, stored: string | null): Workspace | null {
  return document.workspaces.find((workspace) => workspace.name === stored) ?? document.workspaces[0] ?? null;
}

/**
 * The module the stored value gives: none for All modules (`""`), a served name's module, else the
 * workspace's own `module` while served (no key, or a name no longer served); null: the whole project.
 */
export function moduleOf(document: WorkspaceDocument, workspace: Workspace | null, stored: string | null): WorkspaceModule | null {
  if (stored === ALL_MODULES) {
    return null;
  }
  const named = stored === null ? undefined : document.modules.find((module) => module.name === stored);
  return named ?? document.modules.find((module) => module.name === workspace?.module) ?? null;
}

/** A workspace's landing: its section, or the project's home for the Overview (and for a value the UI does not know). */
export function landingHash(project: string, workspace: Workspace | null): string {
  const landing = workspace?.landing ?? "";
  return isSectionId(landing) ? sectionHash(project, landing) : homeHash(project);
}

/** Choices the browser refused to store, per project: they hold for this shell's life. */
interface Held {
  workspaces: ReadonlyMap<string, string>;
  /** A module's stored value; null: the key removed. */
  modules: ReadonlyMap<string, string | null>;
}

const NOTHING_HELD: Held = { workspaces: new Map(), modules: new Map() };

/** What the shell knows of a project's lens: the chosen workspace and module, and how to choose. */
export interface WorkspaceChoice {
  /** The chosen workspace; null until the document is read, or when it was not. */
  workspace: Workspace | null;
  /** The chosen module; null: the whole project, or nothing read. */
  module: WorkspaceModule | null;
  /** Stores a workspace's name and removes the module key, so its own module applies. */
  chooseWorkspace: (name: string) => void;
  /** Stores a module's name, ALL_MODULES for the whole project. */
  chooseModule: (name: string) => void;
}

/**
 * The project's chosen workspace and module over its served document. Storage is the truth, read on
 * each render; a write it refuses is held in this hook's state, so it holds until a reload. Nothing
 * here routes: the switcher pushes the landing itself.
 */
export function useWorkspaceChoice(project: string | null, document: WorkspaceDocument | undefined): WorkspaceChoice {
  const [held, setHeld] = useState<Held>(NOTHING_HELD);

  const chooseWorkspace = useCallback(
    (name: string) => {
      if (project === null) {
        return;
      }
      const stored = keep(workspaceKey(project), name);
      const removed = keep(moduleKey(project), null);
      setHeld((last) => {
        const workspaces = new Map(last.workspaces);
        const modules = new Map(last.modules);
        if (stored) {
          workspaces.delete(project);
        } else {
          workspaces.set(project, name);
        }
        if (removed) {
          modules.delete(project);
        } else {
          modules.set(project, null);
        }
        return { workspaces, modules };
      });
    },
    [project],
  );

  const chooseModule = useCallback(
    (name: string) => {
      if (project === null) {
        return;
      }
      const stored = keep(moduleKey(project), name);
      setHeld((last) => {
        const modules = new Map(last.modules);
        if (stored) {
          modules.delete(project);
        } else {
          modules.set(project, name);
        }
        return { ...last, modules };
      });
    },
    [project],
  );

  if (project === null || document === undefined) {
    return { workspace: null, module: null, chooseWorkspace, chooseModule };
  }
  const storedWorkspace = held.workspaces.get(project) ?? recall(workspaceKey(project));
  const storedModule = held.modules.has(project) ? (held.modules.get(project) ?? null) : recall(moduleKey(project));
  const workspace = workspaceOf(document, storedWorkspace);
  return { workspace, module: moduleOf(document, workspace, storedModule), chooseWorkspace, chooseModule };
}

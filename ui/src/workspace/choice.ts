import { useCallback, useState } from "react";
import type { Workspace, WorkspaceDocument, WorkspaceModule } from "../api/types";
import { homeHash, sectionHash } from "../app/routes";
import { isSectionId } from "../app/sections";

// A viewer's choice of workspace and module (docs/features/workspaces.md "The UI", D8): per viewer
// and per project, in `localStorage`, as `specengine.board.as` is kept
// (`ui/README.md` "Screen rules", which lists the storage keys). Every name is the served document's: a stored one is
// looked up among the served names, never compared with a word of the UI's (W5). Every storage
// access may throw (a private window, a sandboxed frame, a policy): then the choice holds in the
// shell's state until a reload, as the tree's Show drift does (`src/tree/drift.ts`). On a hub the
// choice is the login's (docs/features/hub-department.md "Acceptance criteria", HD3): both keys end in
// it, so two logins in one browser keep their own, and the browser-wide keys of a local daemon stay
// unread there. With no stored choice the document's `default`, the asking login's department, is
// chosen; a stored one wins over it, so a default never reads as an assignment.

/**
 * Whose choice it is: a hub's login, null on a local daemon, as the identity read answered;
 * undefined when that read failed, and then no key is read or written: a choice holds in the shell's
 * state until a reload, as one the browser refuses to store.
 */
export type ChoiceLogin = string | null | undefined;

/** Where the chosen workspace's name is kept for a project: per login on a hub, browser-wide locally (`login` null). */
export function workspaceKey(project: string, login: string | null): string {
  return login === null ? `specengine.workspace.${project}` : `specengine.workspace.${project}.${login}`;
}

/** Where the chosen module's name is kept for a project, as the workspace's; `""` stands for All modules. */
export function moduleKey(project: string, login: string | null): string {
  return login === null ? `specengine.module.${project}` : `specengine.module.${project}.${login}`;
}

/** The stored value of All modules. */
export const ALL_MODULES = "";

/** A rooted module: a root document and its subtree, read from that root (the Tree's and Flow's). */
export function isRooted(module: WorkspaceModule): module is WorkspaceModule & { root: string } {
  return module.root !== null;
}

/**
 * A direction: the nodes of the kinds a module lists, with no root
 * (`docs/canon/workspaces.md` "The lens"). The Tree lists it flat; Flow, which draws from a root,
 * takes rooted modules only. The kinds are the project's words, passed to the read as served,
 * never compared with the UI's.
 */
export function isDirection(module: WorkspaceModule): module is WorkspaceModule & { root: null; kinds: string[] } {
  return module.root === null && module.kinds !== null;
}

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

/**
 * Whether the login can change the lens a document gives: more than one workspace to choose among, or a
 * module. With one workspace and no module every login sees the same lens, so nothing waits for the login.
 */
export function choosesByLogin(document: WorkspaceDocument): boolean {
  return document.workspaces.length > 1 || document.modules.length > 0;
}

/** A workspace the document serves by name; none for no name or one no longer served. */
function servedNamed(document: WorkspaceDocument, name: string | null): Workspace | undefined {
  return name === null ? undefined : document.workspaces.find((workspace) => workspace.name === name);
}

/**
 * The workspace a stored name gives (docs/features/hub-department.md "Acceptance criteria", HD3): that
 * one while the document serves it, else the document's `default` while served, else the first; null
 * with none served. Names are looked up among the served ones, never compared with a word of the UI's.
 */
export function workspaceOf(document: WorkspaceDocument, stored: string | null): Workspace | null {
  return servedNamed(document, stored) ?? servedNamed(document, document.default) ?? document.workspaces[0] ?? null;
}

/**
 * The workspace the viewer chose for `project`, as the switcher stored it for this login, over a
 * document read; null with none read. Outside the Shell (a dialog's default): a choice the browser
 * refused to store, or made with no login known, reads as the default, else the first, as after a reload.
 */
export function storedWorkspace(project: string, login: ChoiceLogin, document: WorkspaceDocument | undefined): Workspace | null {
  return document === undefined ? null : workspaceOf(document, login === undefined ? null : recall(workspaceKey(project, login)));
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

/** Choices the browser refused to store, or made with no login known, per project and login: they hold for this shell's life. */
interface Held {
  workspaces: ReadonlyMap<string, string>;
  /** A module's stored value; null: the key removed. */
  modules: ReadonlyMap<string, string | null>;
}

/** Whose held choice: a project's for one login, null locally, or with none known. */
function heldId(project: string, login: ChoiceLogin): string {
  return JSON.stringify([project, login === undefined ? 0 : login]);
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
 * The project's chosen workspace and module over its served document, for `login`. Storage is the
 * truth, read on each render; a write it refuses, or any choice while no login is known, is held in
 * this hook's state, so it holds until a reload. Nothing here routes: the switcher pushes the landing
 * itself. A caller with the login still out passes no document, so nothing is chosen before it is known.
 */
export function useWorkspaceChoice(project: string | null, login: ChoiceLogin, document: WorkspaceDocument | undefined): WorkspaceChoice {
  const [held, setHeld] = useState<Held>(NOTHING_HELD);

  const chooseWorkspace = useCallback(
    (name: string) => {
      if (project === null) {
        return;
      }
      const id = heldId(project, login);
      const stored = login !== undefined && keep(workspaceKey(project, login), name);
      const removed = login !== undefined && keep(moduleKey(project, login), null);
      setHeld((last) => {
        const workspaces = new Map(last.workspaces);
        const modules = new Map(last.modules);
        if (stored) {
          workspaces.delete(id);
        } else {
          workspaces.set(id, name);
        }
        if (removed) {
          modules.delete(id);
        } else {
          modules.set(id, null);
        }
        return { workspaces, modules };
      });
    },
    [project, login],
  );

  const chooseModule = useCallback(
    (name: string) => {
      if (project === null) {
        return;
      }
      const id = heldId(project, login);
      const stored = login !== undefined && keep(moduleKey(project, login), name);
      setHeld((last) => {
        const modules = new Map(last.modules);
        if (stored) {
          modules.delete(id);
        } else {
          modules.set(id, name);
        }
        return { ...last, modules };
      });
    },
    [project, login],
  );

  if (project === null || document === undefined) {
    return { workspace: null, module: null, chooseWorkspace, chooseModule };
  }
  const id = heldId(project, login);
  const storedWorkspace = held.workspaces.get(id) ?? (login === undefined ? null : recall(workspaceKey(project, login)));
  const storedModule = held.modules.has(id) ? (held.modules.get(id) ?? null) : login === undefined ? null : recall(moduleKey(project, login));
  const workspace = workspaceOf(document, storedWorkspace);
  return { workspace, module: moduleOf(document, workspace, storedModule), chooseWorkspace, chooseModule };
}

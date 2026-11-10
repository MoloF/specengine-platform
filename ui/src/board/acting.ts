import { useCallback, useSyncExternalStore } from "react";
import { CARD_USAGE } from "../api/client";
import { useIdentity, useTeam } from "../api/queries";
import type { ApiError, KnownTeamAct, Team } from "../api/types";
import { ACTING_FLAG } from "./labels";

// The acting name (`docs/features/board-ui.md` "The acting name, and acceptance"): a field in the
// Board's header, kept per viewer, sent as `as` on every write. Locally a convention: the name is
// typed and nothing verifies it (ADR-0034). On a hub the login takes its place once its read has
// resolved, and the stored name is neither sent nor written (`docs/canon/hub.md` "Screen"); what that
// login may do on a project is `allowed`, read here and never told apart by any word but the act's
// (`docs/features/hub-team.md` "Description and interactions").

/** Where the viewer's name is kept: per viewer, for every project. */
export const ACTING_KEY = "specengine.board.as";

const listeners = new Set<() => void>();

/** The name typed while the browser refuses to store it: it overrides the storage for this page. */
let unstored: string | null = null;

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The name as typed: empty when none is kept, and when the browser refuses its storage. */
export function recallActing(): string {
  if (unstored !== null) {
    return unstored;
  }
  try {
    return window.localStorage.getItem(ACTING_KEY) ?? "";
  } catch {
    return "";
  }
}

/** Keeps the name as typed, an empty one removing the key; a refused write keeps it for this page. */
export function rememberActing(name: string): void {
  try {
    if (name === "") {
      window.localStorage.removeItem(ACTING_KEY);
    } else {
      window.localStorage.setItem(ACTING_KEY, name);
    }
    unstored = null;
  } catch {
    unstored = name;
  }
  for (const listener of listeners) {
    listener();
  }
}

/**
 * The names the core keeps for no person, which can be no reviewer: `agent` and `local`, the one a
 * write with no `as` records (`docs/canon/card-flows.md` "Hand-off"). Words of the daemon, compared
 * so a form never offers what the daemon refuses.
 */
const NO_PERSON: readonly string[] = ["agent", "local"];

/**
 * A hand-off's reviewer as the form opens: the parent card's reviewer when it names one; else the
 * acting name, unless it names no person or is empty; else empty, for the person to fill in.
 */
export function handOffReviewer(parentReviewer: string | null, acting: string): string {
  if (parentReviewer !== null && parentReviewer.trim() !== "") {
    return parentReviewer;
  }
  const name = acting.trim();
  return name === "" || NO_PERSON.includes(name) ? "" : acting;
}

/** What a write sends as `as`: the name trimmed; none is null, which the daemon records `local`. */
export function actingName(typed: string): string | null {
  const name = typed.trim();
  return name === "" ? null : name;
}

/**
 * Whether a refused write's line is about the acting name, so it belongs next to its field: a 400
 * naming the flag. Any other 400 (a bad assignee, say) stays where the write was made.
 */
export function isActingLine({ status, message }: ApiError): boolean {
  return status === CARD_USAGE && message.includes(ACTING_FLAG);
}

/**
 * A hub's login, once its read has resolved. Null says only that no login is known: a local daemon,
 * the read still out, the read failed. Each of those keeps the stored name, never an empty one, which
 * a local write would record as `local` (`docs/canon/hub.md` "Screen").
 */
export function useHubLogin(): string | null {
  return useIdentity().data?.login ?? null;
}

/**
 * What `[team]` says of this login on `project`, once read. Null says only that nothing is known: no
 * login, which keeps a local daemon from being asked a route it does not have; the read still out;
 * the read failed, a last answer no longer holding.
 */
export function useTeamRow(project: string): Team | null {
  const login = useHubLogin();
  const read = useTeam(project, login !== null);
  return read.error === null ? (read.data ?? null) : null;
}

/**
 * Whether a control of an act stands on. Off only where an answer read says so: unknown, it stays on
 * and the route answers for itself, a 403 in the daemon's words being the truth either way. This
 * shapes the page and judges nobody.
 */
export function useAllowed(project: string): (act: KnownTeamAct) => boolean {
  const team = useTeamRow(project);
  return (act) => team === null || team.allowed.includes(act);
}

/** The word of `allowed` the Confirm control follows (`docs/features/hub-confirm.md`). */
const CONFIRM = "confirm";

/**
 * This login's row of `[team]` when an answer read lists `confirm`, else null. The other way round
 * from `useAllowed`: unknown is off, since a local daemon, a read still out and a failed read all
 * read as unknown, and a local daemon has no such control and no such route. A terminal confirms
 * whatever this says.
 */
export function useConfirmer(project: string): Team | null {
  const team = useTeamRow(project);
  return team?.allowed.includes(CONFIRM) === true ? team : null;
}

/** The setter a login comes with: a hub's page has nothing to keep. */
function keepNothing(): void {
  return undefined;
}

/** The acting name and its setter: every field showing it stays the same. A resolved login stands in for the name as typed. */
export function useActing(): readonly [string, (name: string) => void] {
  const typed = useSyncExternalStore(subscribe, recallActing);
  const login = useHubLogin();
  const set = useCallback((name: string) => {
    rememberActing(name);
  }, []);
  return login === null ? [typed, set] : [login, keepNothing];
}
